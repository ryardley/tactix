//! Actor model framework for Rust built on Tokio.
//!
//! Tactix provides an ergonomic actor model with hierarchical supervision, async
//! handlers, automatic restart on panic, and support for both fire-and-forget
//! (`tell`) and request-response (`ask`) message patterns.
//!
//! # Installation
//!
//! ```bash
//! cargo add tactix
//! ```
//!
//! # Getting started
//!
//! ```rust
//! use async_trait::async_trait;
//! use tactix::{Actor, Ctx, Handler, Message, Recipient, Sender};
//!
//! // --- Actor ---
//! struct Counter {
//!     count: u64,
//! }
//!
//! impl Actor for Counter {}
//!
//! // --- Messages ---
//!
//! #[derive(Message)]
//! struct Increment;
//!
//! #[derive(Message)]
//! struct Decrement;
//!
//! #[derive(Message)]
//! #[response(u64)]
//! struct GetCount;
//!
//! // --- Handlers ---
//!
//! #[async_trait]
//! impl Handler<Increment> for Counter {
//!     async fn handle(&mut self, _: Increment, _: &Ctx<Self>) {
//!         self.count += 1;
//!     }
//! }
//!
//! #[async_trait]
//! impl Handler<Decrement> for Counter {
//!     async fn handle(&mut self, _: Decrement, _: &Ctx<Self>) {
//!         self.count -= 1;
//!     }
//! }
//!
//! #[async_trait]
//! impl Handler<GetCount> for Counter {
//!     async fn handle(&mut self, _: GetCount, _: &Ctx<Self>) -> u64 {
//!         self.count
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), String> {
//!     let counter = Counter { count: 0 }.start();
//!
//!     // Fire-and-forget with `tell`
//!     counter.tell(Increment);
//!     counter.tell(Increment);
//!     counter.tell(Decrement);
//!
//!     // Type-erased recipient for dependency injection
//!     let r: Recipient<Decrement> = counter.clone().recipient();
//!     r.tell(Decrement);
//!
//!     // Use `ask` with a no-response message to synchronise — the response
//!     // is sent only after all prior messages have been handled.
//!     let _ = counter.ask(Increment).await;
//!
//!     // Request-response with `ask`
//!     assert_eq!(counter.ask(GetCount).await, 1);
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use futures::FutureExt;
use std::sync::OnceLock;
use std::{
    panic::AssertUnwindSafe,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

pub use macros::Message;

/// The global root actor system.
///
/// `ActorSystem` is a singleton sentinel actor that serves as the root of the
/// actor hierarchy. Actors spawned via [`Actor::start()`] are registered as
/// children of the system; when the system stops, all children are stopped
/// recursively.
///
/// # Access
///
/// - [`ActorSystem::global()`] returns the system's [`Ctx`], giving you access
///   to [`spawn`](Ctx::spawn) for manual child creation.
/// - [`ActorSystem::addr()`] returns the system's [`Addr`], so you can send it
///   messages — most importantly [`Shutdown`].
///
/// # Graceful shutdown
///
/// Calling [`ActorSystem::shutdown().await`](ActorSystem::shutdown) performs a
/// two-phase graceful shutdown:
///
/// 1. Sends a [`Shutdown`] message to the system, which is queued behind any
///    messages already in the mailbox.
/// 2. Waits for the system to drain its mailbox, stop all children (via
///    [`Stoppable::stop`] + [`Stoppable::wait_until_stopped`]), invoke the
///    [`Actor::stopped`] lifecycle hook, and finally exit the actor task.
///
/// Because `Shutdown` is delivered through the normal message queue, any
/// messages sent *before* it are processed first. This ensures an orderly
/// wind-down.
///
/// # Panics
///
/// The global system is initialised lazily on first access and cannot be
/// re-initialised afterwards. Sending messages to the system after it has
/// shut down is a no-op (the message is dropped).
pub struct ActorSystem;

/// Message to gracefully shut down the global actor system.
///
/// Sending this to [`ActorSystem::addr()`] triggers a graceful shutdown:
/// the system finishes processing the current message, stops all children,
/// calls [`Actor::stopped`], and then exits.
///
/// You typically don't need to construct this manually — use
/// [`ActorSystem::shutdown()`] instead.
#[derive(Message)]
pub struct Shutdown;

#[async_trait]
impl Actor for ActorSystem {}

#[async_trait]
impl Handler<Shutdown> for ActorSystem {
    async fn handle(&mut self, _: Shutdown, ctx: &Ctx<Self>) {
        ctx.stop();
    }
}

static ACTOR_SYSTEM: OnceLock<Ctx<ActorSystem>> = OnceLock::new();

impl ActorSystem {
    /// Returns a reference to the global `ActorSystem` context, initialising it
    /// on first call.
    pub fn global() -> &'static Ctx<ActorSystem> {
        ACTOR_SYSTEM.get_or_init(|| {
            let mut once = Some(ActorSystem);
            start_actor(
                move || {
                    once.take()
                        .expect("ActorSystem factory called more than once")
                },
                CancellationToken::new(),
                mpsc::unbounded_channel().0,
                SupervisionStrategy::default(),
            )
        })
    }

    /// Returns the address of the global `ActorSystem`.
    ///
    /// Use this to send messages (e.g. [`Shutdown`]) to the system from
    /// anywhere in your application.
    pub fn addr() -> Addr<ActorSystem> {
        Self::global().address()
    }

    /// Gracefully shut down the entire actor system.
    ///
    /// Sends [`Shutdown`] to the system and waits for it to finish processing
    /// pending messages, stop all children, and exit.
    pub async fn shutdown() {
        let addr = Self::addr();
        addr.tell(Shutdown);
        addr.wait_until_stopped().await;
    }
}

type PointerToActorMessage<A> = Box<dyn ActorMessage<A>>;

/// Trait implemented by all actors.
///
/// Actors are long-lived, stateful objects that communicate via message passing.
/// Each actor runs in its own Tokio task and processes messages sequentially.
///
/// # Lifecycle
///
/// 1. [`started`](Actor::started) is called once before the first message.
/// 2. Messages are handled one-by-one via [`Handler`] implementations.
/// 3. If a handler panics, the actor restarts according to [`SupervisionStrategy`];
///    [`restarted`](Actor::restarted) is called after each restart.
/// 4. When stopped (via [`Ctx::stop`] or `SupervisionStrategy::NoRestart`),
///    [`stopped`](Actor::stopped) is called and the task exits.
#[async_trait]
pub trait Actor: Send + Sync + Sized + 'static {
    /// Spawn this actor on the global system with default supervision
    /// ([`SupervisionStrategy::NoRestart`]) and return its address.
    ///
    /// To configure supervision or spawn as a child of another actor, use
    /// [`Ctx::spawn`] or [`Ctx::spawn_with_config`] instead.
    fn start(self) -> Addr<Self> {
        let mut once = Some(self);
        ActorSystem::global().spawn_with_config(
            move || once.take().expect("Factory can only be accessed once!"),
            SupervisionStrategy::NoRestart,
        )
    }
    /// Called after the actor task starts, before any messages are processed.
    async fn started(&self, _ctx: &Ctx<Self>) {}
    /// Called after the actor has stopped (all messages drained, children
    /// stopped) and the task is about to exit.
    async fn stopped(&self, _ctx: &Ctx<Self>) {}
    /// Called after a restart before the first message is processed.
    ///
    /// `restarts` is the total number of restarts that have occurred.
    async fn restarted(&self, _restarts: u64, _ctx: &Ctx<Self>) {}
    /// Called when a child actor has escalated after exhausting its restart
    /// budget.
    ///
    /// Return `Some(Interrupt)` to influence the parent's behaviour (default:
    /// [`Interrupt::RestartToEscalate`]), or `None` to ignore the escalation.
    async fn child_escalated(&self, _ctx: &Ctx<Self>) -> Option<Interrupt> {
        Some(Interrupt::RestartToEscalate)
    }
}

/// Outcome of an actor's event loop iteration that drives lifecycle decisions.
pub enum Interrupt {
    /// Gracefully stop the actor.
    Stop,
    /// Restart the actor and, if the restart budget is exhausted, escalate
    /// to the parent.
    RestartToEscalate,
}

/// Supervision strategy that controls how panics in an actor are handled.
pub enum SupervisionStrategy {
    /// Do not restart on panic. The actor task exits immediately with no
    /// escalation to the parent.
    NoRestart,
    /// Restart up to `max_restarts` times within a sliding `window` of seconds.
    ///
    /// Once the budget is exhausted the actor stops and an escalation signal
    /// is sent to the parent actor's [`child_escalated`](Actor::child_escalated)
    /// hook.
    Restart { window: u64, max_restarts: u64 },
}

impl Default for SupervisionStrategy {
    fn default() -> Self {
        Self::Restart {
            window: 5,
            max_restarts: 3,
        }
    }
}

fn start_actor<A, F>(
    mut factory: F,
    cancel: CancellationToken,
    escalate_to_parent: mpsc::UnboundedSender<()>,
    restart_config: SupervisionStrategy,
) -> Ctx<A>
where
    A: Actor + Sync,
    F: FnMut() -> A + Send + 'static,
{
    let (tx, mut rx) = mpsc::unbounded_channel::<PointerToActorMessage<A>>();
    let (child_escalations, mut child_escalations_rx) = mpsc::unbounded_channel();
    let stopped = CancellationToken::new();
    let ctx = Ctx::<A> {
        addr: Addr {
            tx,
            stopped: stopped.clone(),
        },
        children: Arc::new(Mutex::new(Vec::new())),
        cancel,
        stopped: stopped.clone(),
        child_escalations,
    };
    let ctx_loop = ctx.clone();
    tokio::spawn(async move {
        let mut is_restart = false;
        let ctx = ctx_loop;
        let _stopped_guard = stopped.drop_guard();

        let mut restarts = 0u64;
        let mut first_restart = Instant::now();
        loop {
            let mut actor = factory();
            actor.started(&ctx).await;
            if is_restart {
                actor.restarted(restarts, &ctx).await;
            }
            let code = loop {
                tokio::select! {
                    biased; // Important makes sure we check in order

                    // Check the cancel signal
                    _ = ctx.cancel.cancelled() => {
                        break Interrupt::Stop;
                    }

                    // Check if our children have escalated
                    Some(_) = child_escalations_rx.recv() => {
                        if let Some(interrupt) = actor.child_escalated(&ctx).await {
                            break interrupt;
                        }
                    }

                    // Receive a message
                    msg = rx.recv() => {
                        let Some(mut msg) = msg else {
                            break Interrupt::Stop;
                        };
                        if let Err(panic) = AssertUnwindSafe(msg.process(&mut actor, &ctx))
                            .catch_unwind()
                            .await
                        {
                            let msg = panic.downcast_ref::<&str>().copied()
                                .or_else(|| panic.downcast_ref::<String>().map(|s| s.as_str()))
                                .unwrap_or("<non-string panic>");

                            eprintln!("ACTOR PANIC!\n actor:{}\n reason: {}\n restarting...", std::any::type_name::<A>(), msg);
                            break Interrupt::RestartToEscalate;
                        }
                    }
                }
            };

            // Stop and wait for all children regardless of why we exited.
            ctx.stop_all_children().await;

            match code {
                Interrupt::Stop => {
                    actor.stopped(&ctx).await;
                    break;
                }
                Interrupt::RestartToEscalate => {
                    match &restart_config {
                        SupervisionStrategy::NoRestart => {
                            // Unsupervised: just exit, do not escalate.
                            break;
                        }
                        SupervisionStrategy::Restart {
                            window,
                            max_restarts,
                        } => {
                            is_restart = true;
                            if first_restart.elapsed().as_secs() > *window {
                                restarts = 0;
                                first_restart = Instant::now();
                            }
                            restarts += 1;
                            if restarts >= *max_restarts {
                                eprintln!(
                                    "Actor restarted {} times in {}s, escalating.",
                                    max_restarts, window
                                );
                                let _ = escalate_to_parent.send(());
                                break;
                            }
                        }
                    }
                }
            }
        }
        // _stopped_guard drops here and signals `stopped`.
    });
    ctx
}

/// Context handle for an actor.
///
/// Provides access to the actor's address and the ability to spawn child actors.
/// Every [`Handler`] receives a reference to `Ctx` so actors can supervise their
/// children.
///
/// Cloning `Ctx` is cheap (it uses `Arc` internally).
pub struct Ctx<A: Actor> {
    addr: Addr<A>,
    children: Arc<Mutex<Vec<Arc<dyn Stoppable + Send + Sync>>>>,
    cancel: CancellationToken,
    stopped: CancellationToken,
    child_escalations: mpsc::UnboundedSender<()>,
}

impl<A: Actor> Clone for Ctx<A> {
    fn clone(&self) -> Self {
        Self {
            addr: self.addr.clone(),
            children: self.children.clone(),
            cancel: self.cancel.clone(),
            stopped: self.stopped.clone(),
            child_escalations: self.child_escalations.clone(),
        }
    }
}

impl<A: Actor> Ctx<A> {
    /// Returns the address of this actor.
    #[must_use]
    pub fn address(&self) -> Addr<A> {
        self.addr.clone()
    }

    /// Spawn a child actor with the default supervision
    /// ([`SupervisionStrategy::default`]).
    ///
    /// The child is linked to this actor's cancellation scope: if the parent
    /// stops the child is also cancelled. When the child exhausts its restart
    /// budget the parent's [`child_escalated`](Actor::child_escalated) hook is
    /// invoked.
    pub fn spawn<B, F>(&self, factory: F) -> Addr<B>
    where
        F: FnMut() -> B + Send + 'static,
        B: Actor,
    {
        self.spawn_with_config(factory, SupervisionStrategy::default())
    }

    /// Spawn a child actor with a custom supervision strategy.
    ///
    /// See [`spawn`](Ctx::spawn) for details. Use this method when you need
    /// to override the default [`SupervisionStrategy`].
    pub fn spawn_with_config<B, F>(&self, factory: F, config: SupervisionStrategy) -> Addr<B>
    where
        F: FnMut() -> B + Send + 'static,
        B: Actor,
    {
        let child = start_actor(
            factory,
            self.cancel.child_token(),
            self.child_escalations.clone(),
            config,
        );
        self.children.lock().unwrap().push(Arc::new(child.clone()));
        child.address()
    }
}

/// Interface for objects that can be stopped and awaited for shutdown.
#[async_trait]
pub trait Stoppable {
    /// Signal the object to stop. This is non-blocking.
    fn stop(&self);
    /// Wait until the object has fully stopped.
    async fn wait_until_stopped(&self);
    /// Signal all children to stop and wait for them to finish.
    async fn stop_all_children(&self);
}

/// Trait implemented by all message types.
///
/// `Message` defines the response type associated with a message. Messages
/// must be [`Send`] and `'static`.
///
/// # Derive macro
///
/// The `macros` crate provides a convenience derive macro:
///
/// ```rust,ignore
/// use tactix::Message;
///
/// #[derive(Message)]
/// struct Ping;                       // Response = ()
///
/// #[derive(Message)]
/// #[response(String)]
/// struct GetName;                    // Response = String
/// ```
pub trait Message: Send + 'static {
    /// The type returned by the handler for this message (use `()` for
    /// fire-and-forget messages).
    type Response: Send;
}

/// Trait implemented on an [`Actor`] to process a specific [`Message`] type.
///
/// Each actor can implement `Handler` for any number of message types, each
/// with its own response type.
///
/// # Async
///
/// Handlers are async and receive a borrow to the actor's [`Ctx`], allowing
/// them to spawn children or send messages to other actors while handling.
#[async_trait]
pub trait Handler<M>
where
    Self: Actor,
    M: Message,
{
    /// Process an incoming message and return a response.
    async fn handle(&mut self, msg: M, ctx: &Ctx<Self>) -> M::Response;
}

/// Internal trait for type-erased message dispatch inside the actor task.
///
/// You should not need to implement this trait directly; it is automatically
/// implemented via [`Envelope`] for any pair that satisfies
/// `A: Actor + Handler<M>`.
#[async_trait]
pub trait ActorMessage<A: Actor>: Send {
    async fn process(&mut self, actor: &mut A, ctx: &Ctx<A>);
}

/// Wraps a message payload with an optional oneshot channel for the response.
///
/// When a response channel is provided the handler's return value is sent back
/// over it. When `tx` is `None` the message is fire-and-forget.
pub struct Envelope<M>
where
    M: Message,
{
    pub msg: Option<M>,
    pub tx: Option<oneshot::Sender<M::Response>>,
}

impl<M> Envelope<M>
where
    M: Message,
{
    /// Creates a new `Envelope`, boxed, ready to send over the actor's channel.
    pub fn new(msg: Option<M>, tx: Option<oneshot::Sender<M::Response>>) -> Box<Self> {
        Box::new(Self { msg, tx })
    }
}

#[async_trait]
impl<A, M> ActorMessage<A> for Envelope<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    async fn process(&mut self, act: &mut A, ctx: &Ctx<A>) {
        if let Some(msg) = self.msg.take() {
            let res = act.handle(msg, ctx).await;
            if let Some(tx) = self.tx.take() {
                let _ = tx.send(res);
            }
        }
    }
}

/// Address of an actor. Messages are sent through this handle.
///
/// `Addr` is cheaply cloneable and can be used to:
/// - Fire-and-forget a message via [`tell`](Sender::tell).
/// - Await a response via [`ask`](Sender::ask).
/// - Create a type-erased [`Recipient`] via [`recipient`](Sender::recipient).
pub struct Addr<A>
where
    A: Actor,
{
    tx: mpsc::UnboundedSender<PointerToActorMessage<A>>,
    stopped: CancellationToken,
}

impl<A: Actor> Addr<A> {
    /// Wait until the actor task has fully stopped.
    pub async fn wait_until_stopped(&self) {
        self.stopped.cancelled().await;
    }
}

impl<A> Clone for Addr<A>
where
    A: Actor,
{
    fn clone(&self) -> Self {
        Addr {
            tx: self.tx.clone(),
            stopped: self.stopped.clone(),
        }
    }
}

/// Capability to send messages of type `M` to an actor.
///
/// Implemented by both [`Addr`] and [`Recipient`].
#[async_trait]
pub trait Sender<M>
where
    M: Message,
{
    /// Send a message and await the response.
    ///
    /// # Panics
    ///
    /// Panics if the actor has stopped before sending a response.
    async fn ask(&self, msg: M) -> M::Response;
    /// Send a message without waiting for a response (fire-and-forget).
    fn tell(&self, msg: M);
    /// Convert this sender into a type-erased [`Recipient`].
    ///
    /// This is useful for dependency injection: a `Recipient<M>` does not
    /// expose the actor type.
    fn recipient(self) -> Recipient<M>
    where
        Self: Sized + Send + Sync + 'static,
    {
        Recipient { tx: Box::new(self) }
    }
}

#[async_trait]
impl<M, A> Sender<M> for Addr<A>
where
    M: Message,
    A: Actor + Handler<M>,
{
    async fn ask(&self, msg: M) -> M::Response {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(Envelope::new(Some(msg), Some(tx)));
        rx.await.expect("actor dropped before responding")
    }
    fn tell(&self, msg: M) {
        let _ = self.tx.send(Envelope::new(Some(msg), None));
    }
}

/// Type-erased sender for a specific message type.
///
/// Wraps any [`Sender<M>`] behind a trait object so the concrete actor type
/// is hidden. Useful for dependency injection where you want to expose only
/// the ability to send a particular message.
pub struct Recipient<M: Message> {
    tx: Box<dyn Sender<M> + Send + Sync + 'static>,
}

impl<M> Recipient<M>
where
    M: Message,
{
    /// Create a new `Recipient` from a boxed [`Sender`].
    pub fn new(tx: Box<dyn Sender<M> + Send + Sync + 'static>) -> Self {
        Recipient { tx }
    }
}

#[async_trait]
impl<M> Sender<M> for Recipient<M>
where
    M: Message,
{
    async fn ask(&self, msg: M) -> M::Response {
        self.tx.ask(msg).await
    }

    fn tell(&self, msg: M) {
        self.tx.tell(msg);
    }
}

#[async_trait]
impl<A> Stoppable for Ctx<A>
where
    A: Actor,
{
    fn stop(&self) {
        self.cancel.cancel();
    }

    async fn wait_until_stopped(&self) {
        self.stopped.cancelled().await;
    }

    async fn stop_all_children(&self) {
        let children: Vec<_> = self.children.lock().unwrap().drain(..).collect();
        for child in children {
            child.stop();
            child.wait_until_stopped().await;
        }
    }
}

#[cfg(test)]
mod simple_tests {
    use crate::{Actor, Addr, Ctx, Handler, Message, Sender, Stoppable};
    use async_trait::async_trait;
    use std::time::Instant;

    #[tokio::test(flavor = "multi_thread")]
    async fn simple_counter() -> anyhow::Result<()> {
        // Simple counter
        struct Counter {
            count: i64,
        }

        impl Actor for Counter {}

        #[derive(Message)]
        struct Increment;

        #[derive(Message)]
        struct Decrement;

        #[derive(Message)]
        #[response(i64)]
        struct GetCount;

        #[async_trait]
        impl Handler<Increment> for Counter {
            async fn handle(&mut self, _: Increment, _: &Ctx<Self>) {
                self.count += 1;
            }
        }

        #[async_trait]
        impl Handler<Decrement> for Counter {
            async fn handle(&mut self, _: Decrement, _: &Ctx<Self>) {
                self.count -= 1;
            }
        }

        #[async_trait]
        impl Handler<GetCount> for Counter {
            async fn handle(&mut self, _: GetCount, _: &Ctx<Self>) -> i64 {
                self.count
            }
        }

        let counter = Counter { count: 0 }.start();

        let mut handles = vec![];

        let t1 = counter.clone();
        let t2 = counter.clone();

        let total = 10_000_000;
        let start = Instant::now();
        handles.push(tokio::task::spawn(async move {
            for _ in 0..(total / 2) {
                t1.tell(Increment);
            }
            Ok::<_, anyhow::Error>(())
        }));
        handles.push(tokio::task::spawn(async move {
            for _ in 0..(total / 2) {
                t2.tell(Decrement);
            }
            Ok::<_, anyhow::Error>(())
        }));
        for handle in handles {
            handle.await??;
        }
        let count = counter.ask(GetCount).await;
        assert_eq!(count, 0);
        let finished = start.elapsed();
        let msg_per_sec = total as f64 / finished.as_secs_f64();
        println!("{:.1} million msg/sec", msg_per_sec / 1_000_000.0);
        Ok(())
    }

    #[tokio::test]
    async fn restart_counter() -> anyhow::Result<()> {
        struct Db {
            value: i64,
        }

        #[async_trait]
        impl Actor for Db {
            async fn stopped(&self, _: &Ctx<Self>) {
                println!("Db stopped");
            }
        }

        #[derive(Message)]
        #[response(i64)]
        struct DbGet;

        #[derive(Message)]
        struct DbSet(i64);

        #[async_trait]
        impl Handler<DbGet> for Db {
            async fn handle(&mut self, _: DbGet, _: &Ctx<Self>) -> i64 {
                self.value
            }
        }

        #[async_trait]
        impl Handler<DbSet> for Db {
            async fn handle(&mut self, msg: DbSet, _: &Ctx<Self>) {
                self.value = msg.0;
            }
        }

        struct Counter {
            db: Addr<Db>,
        }

        #[async_trait]
        impl Actor for Counter {
            async fn stopped(&self, _: &Ctx<Self>) {
                println!("Counter stopped");
            }
        }

        #[derive(Message)]
        #[response(i64)]
        struct Increment;

        #[derive(Message)]
        #[response(i64)]
        struct GetCount;

        #[derive(Message)]
        struct Poison;

        #[derive(Message)]
        struct Stop;

        #[async_trait]
        impl Handler<Increment> for Counter {
            async fn handle(&mut self, _: Increment, _: &Ctx<Self>) -> i64 {
                let count = self.db.ask(DbGet).await + 1;
                self.db.tell(DbSet(count));
                count
            }
        }

        #[async_trait]
        impl Handler<GetCount> for Counter {
            async fn handle(&mut self, _: GetCount, _: &Ctx<Self>) -> i64 {
                self.db.ask(DbGet).await
            }
        }

        #[async_trait]
        impl Handler<Poison> for Counter {
            async fn handle(&mut self, _: Poison, _: &Ctx<Self>) {
                panic!("poisoned!");
            }
        }

        struct Root {}

        #[async_trait]
        impl Actor for Root {
            async fn stopped(&self, _: &Ctx<Self>) {
                println!("Root stopped");
            }
        }

        #[async_trait]
        impl Handler<Stop> for Root {
            async fn handle(&mut self, _: Stop, ctx: &Ctx<Self>) {
                println!("in Stop handler");
                ctx.stop();
            }
        }

        #[derive(Message)]
        #[response(Addr<Counter>)]
        struct GetCounter;

        #[async_trait]
        impl Handler<GetCounter> for Root {
            async fn handle(&mut self, _: GetCounter, ctx: &Ctx<Self>) -> Addr<Counter> {
                let db = ctx.spawn(|| Db { value: 0 });
                let counter = ctx.spawn(move || Counter { db: db.clone() });
                counter
            }
        }

        let root = Root {}.start();

        let counter = root.ask(GetCounter).await;
        for _ in 0..5 {
            counter.tell(Increment);
        }
        assert_eq!(counter.ask(GetCount).await, 5);

        counter.tell(Poison);
        counter.ask(Increment).await;

        let count = counter.ask(GetCount).await;
        assert_eq!(count, 6, "state survives because it lives in the db actor");
        root.tell(Stop);
        println!("just called stop!");
        root.wait_until_stopped().await;
        println!("goodbye");
        Ok(())
    }
}

//////////////////////////

#[cfg(test)]
mod bank_tests {
    use std::time::Duration;

    use crate::{Actor, Ctx, Handler, Message, Sender};
    use async_trait::async_trait;
    use tokio::time::sleep;

    struct Deposit(u64);
    impl Message for Deposit {
        type Response = ();
    }

    struct Withdraw(u64);
    impl Message for Withdraw {
        type Response = Result<(), String>;
    }

    struct GetBalance;
    impl Message for GetBalance {
        type Response = u64;
    }

    struct GetAccountInfo;
    impl Message for GetAccountInfo {
        type Response = (u64, u64, u64);
    }

    #[derive(Clone)]
    struct BankAccount {
        balance: u64,
        total_deposits: u64,
        total_withdrawals: u64,
    }

    impl BankAccount {
        fn new(initial_balance: u64) -> Self {
            Self {
                balance: initial_balance,
                total_deposits: 0,
                total_withdrawals: 0,
            }
        }
    }

    impl Actor for BankAccount {}

    #[async_trait]
    impl Handler<Deposit> for BankAccount {
        async fn handle(&mut self, msg: Deposit, _: &Ctx<Self>) {
            tokio::time::sleep(Duration::from_millis(6)).await;
            self.balance += msg.0;
            self.total_deposits += msg.0;
            println!("Deposit: {}. New balance: {}", msg.0, self.balance);
        }
    }
    #[async_trait]
    impl Handler<Withdraw> for BankAccount {
        async fn handle(&mut self, msg: Withdraw, _: &Ctx<Self>) -> Result<(), String> {
            if self.balance >= msg.0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
                self.balance -= msg.0;
                self.total_withdrawals += msg.0;
                println!("Withdrawal: {}. New balance: {}", msg.0, self.balance);
                Ok(())
            } else {
                Err(format!(
                    "Insufficient funds. Current balance: {}",
                    self.balance
                ))
            }
        }
    }

    #[async_trait]
    impl Handler<GetAccountInfo> for BankAccount {
        async fn handle(&mut self, _msg: GetAccountInfo, _: &Ctx<Self>) -> (u64, u64, u64) {
            println!("GetAccountInfo!");
            (self.balance, self.total_deposits, self.total_withdrawals)
        }
    }
    #[async_trait]
    impl Handler<GetBalance> for BankAccount {
        async fn handle(&mut self, _msg: GetBalance, _: &Ctx<Self>) -> u64 {
            self.balance
        }
    }

    #[tokio::test]
    async fn test_bank_account_race_condition() {
        let initial_balance = 1000;
        let account = BankAccount::new(initial_balance).start();

        let deposit_amount = 100;
        let withdraw_amount = 200;
        let num_operations = 5;

        // Spawn multiple tasks to deposit and withdraw concurrently
        let deposit_task = tokio::spawn({
            let account = account.clone();
            async move {
                for _ in 0..num_operations {
                    account.tell(Deposit(deposit_amount));
                    sleep(Duration::from_millis(3)).await;
                }
            }
        });

        let withdraw_task = tokio::spawn({
            let account = account.clone();
            async move {
                for _ in 0..num_operations {
                    account.tell(Withdraw(withdraw_amount));
                    sleep(Duration::from_millis(9)).await;
                }
            }
        });

        // Wait for both tasks to complete
        let _ = tokio::join!(deposit_task, withdraw_task);

        // Get the final account info
        let (final_balance, total_deposits, total_withdrawals) = account.ask(GetAccountInfo).await;

        // Assertions to check for race conditions
        let expected_deposits = deposit_amount * num_operations;
        let expected_withdrawals = withdraw_amount * num_operations;
        let expected_balance = initial_balance + expected_deposits - expected_withdrawals;

        assert_eq!(
            total_deposits, expected_deposits,
            "Total deposits don't match expected value"
        );
        assert_eq!(
            total_withdrawals, expected_withdrawals,
            "Total withdrawals don't match expected value"
        );

        // This assertion might fail due to race condition
        assert_eq!(
            final_balance, expected_balance,
            "Final balance doesn't match expected value"
        );

        // This assertion checks if the balance is consistent with deposits and withdrawals
        assert_eq!(
            final_balance,
            initial_balance + total_deposits - total_withdrawals,
            "Balance is inconsistent with recorded deposits and withdrawals"
        );
    }
}
