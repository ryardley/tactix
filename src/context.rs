use std::{marker::PhantomData, sync::Arc};
use tokio::sync::{broadcast, mpsc, Mutex};

use crate::{
    addr::Addr,
    envelope::Envelope,
    traits::{Actor, EnvelopeApi, Sender},
    Handler, Message,
};

/// Methods concerned with the context of the given Actor
pub struct Context<A: Actor>
where
    A: Actor<Context = Context<A>>,
{
    _p: PhantomData<A>,
    stop_tx: broadcast::Sender<()>,
    addr: Option<Addr<A>>,
}

impl<A> Clone for Context<A>
where
    A: Actor<Context = Context<A>>,
{
    fn clone(&self) -> Self {
        Context {
            stop_tx: self.stop_tx.clone(),
            _p: PhantomData,
            addr: None,
        }
    }
}

impl<A> Context<A>
where
    A: Actor<Context = Context<A>>,
{
    /// Construct a new Context
    pub fn new() -> Self {
        let (stop_tx, _) = broadcast::channel(1);
        Self {
            _p: PhantomData,
            stop_tx,
            addr: None,
        }
    }

    /// Stop the actor and any running actor processes
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
    }

    /// Setup a Mailbox for this Actor. Pull messages of the Mailbox and process them as the come.
    /// In a separate thread run the started function.
    pub fn run(mut self, act: A) -> Addr<A>
    where
        A: Actor<Context = Self>,
    {
        let (tx, mut rx) = mpsc::channel::<Envelope<A>>(100);
        let addr = Addr::new(tx);
        let act_ref = Arc::new(Mutex::new(act));
        self.addr = Some(addr.clone());

        // Listen for events sent to the Actor and handle them.
        // If a stop signal is received then stop listening.
        tokio::spawn({
            let a = act_ref.clone();
            let mut stop_rx = self.clone().stop_tx.subscribe();
            let ctx = self.clone();
            async move {
                loop {
                    tokio::select! {
                        Some(mut msg) = rx.recv() => {
                            msg.handle(a.clone(), ctx.clone()).await;
                        },
                        Ok(_) = stop_rx.recv() => {
                            break;
                        }
                    }
                }
                a.lock().await.stopped(ctx.clone()).await;
            }
        });

        // Run the started() method on the actor. If the stop signal is received then stop.
        tokio::spawn({
            let a = act_ref.clone();
            let mut stop_rx = self.clone().stop_tx.subscribe();
            let ctx = self;
            async move {
                let mut mutex = a.lock().await;
                let fut = mutex.started(ctx.clone());
                tokio::select! {
                    _ = fut => {},
                    Ok(_) = stop_rx.recv() => {}
                }
            }
        });
        addr
    }

    fn notify<M>(&mut self, msg: M)
    where
        M: Message,
        A: Actor + Handler<M>,
    {
        if let Some(addr) = &self.addr {
            addr.do_send(msg);
        }
    }
}
