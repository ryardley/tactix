use std::sync::Arc;

use crate::{addr::Addr, context::Context, envelope::Envelope};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot, Mutex};

/// Defines an Actor. An Actor is an independent unit that processes messages, makes decisions, and communicates with other Actors without shared state.
#[async_trait]
pub trait Actor: Sized + Send + Sync + 'static {
    type Context: Send + Sync + 'static;

    /// Start the actor and return an address
    fn start(self) -> Addr<Self>
    where
        Self: Actor<Context = Context<Self>>,
    {
        Context::new().run(self)
    }

    /// Async handler that is run after the actor is started
    async fn started(&self) {}

    /// Async handler that is run after the actor has been stopped
    async fn stopped(&self) {}
}

/// Defines a message type for actor communication, specifying an associated response type.
pub trait Message: Send + 'static {
    type Response: Send;
}

/// Defines an asynchronous message handler for processing actor messages and returning responses.
///
/// Handlers can be added to actors like so:
///
/// ```
/// use tactix::*;
/// use async_trait::async_trait;
///
/// struct MyActor;
///
/// impl Actor for MyActor {
///   type Context = tactix::Context<Self>;
/// }
///
/// struct SomeMessage;
/// impl Message for SomeMessage {
///   type Response = u8;
/// }
///
/// #[async_trait]
/// impl Handler<SomeMessage> for MyActor {
///   async fn handle(&mut self, msg:SomeMessage, _:Self::Context) -> u8 {
///      // Implement your handler
///      1u8
///   }
/// }
/// ```
///
/// Note this trait requires the use of the `#[async_trait]` macro.
#[async_trait]
pub trait Handler<M>
where
    Self: Actor,
    M: Message,
{
    async fn handle(&mut self, msg: M, ctx: Self::Context) -> M::Response;
}

/// This enables the host to pack messages to an envelope
pub trait ToEnvelope<A, M>
where
    A: Actor,
    M: Message,
{
    fn pack(&self, msg: Option<M>, tx: Option<oneshot::Sender<M::Response>>) -> Envelope<A>;
}

/// This allows us to run our handler via our actor bound Envelope
#[async_trait]
pub trait EnvelopeApi<A: Actor> {
    async fn handle(&mut self, act: Arc<Mutex<A>>, ctx: A::Context);
}

/// Encapsulates the idea of a channel transmitter. Represents the ability to send messages
#[async_trait]
pub trait Sender<M: Message>: Sync {
    fn do_send(&self, msg: M);
    async fn send(&self, msg: M) -> Result<M::Response, String>;
}

/// Represent the ability to send messages wrapped in actor envelopes
pub trait EnvelopeSender<A: Actor> {
    fn get_tx(&self) -> mpsc::Sender<Envelope<A>>;

    /// Send a given Envelope on the structs given envelope sender
    fn send_env(&self, env: Envelope<A>) {
        let tx = self.get_tx();
        tokio::spawn(async move {
            let _ = tx.send(env).await;
        });
    }
}
