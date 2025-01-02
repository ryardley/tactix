use std::{marker::PhantomData, sync::Arc};

use tokio::sync::{broadcast, mpsc, RwLock};

use crate::{
    addr::Addr,
    envelope::Envelope,
    traits::{Actor, EnvelopeApi},
};

/// Methods concerned with the context of the given Actor
pub struct Context<A: Actor>
where
    A: Actor<Context = Context<A>>,
{
    _p: PhantomData<A>,
    stop_tx: broadcast::Sender<()>,
}

impl<A> Clone for Context<A>
where
    A: Actor<Context = Context<A>>,
{
    fn clone(&self) -> Self {
        Context {
            stop_tx: self.stop_tx.clone(),
            _p: PhantomData,
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
        }
    }

    /// Stop the actor and any running actor processes
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
    }

    /// Setup a Mailbox for this Actor. Pull messages of the Mailbox and process them as the come.
    /// In a separate thread run the started function.
    pub fn run(self, act: A) -> Addr<A>
    where
        A: Actor<Context = Self>,
    {
        let (tx, mut rx) = mpsc::channel::<Envelope<A>>(100);
        let addr = Addr::new(tx);
        let act_ref = Arc::new(RwLock::new(act));

        // Listen for events sent to the Actor and handle them.
        // If a stop signal is received then stop listening.
        tokio::spawn({
            let a = act_ref.clone();
            let mut stop_rx = self.stop_tx.subscribe();
            let c = self.clone();
            async move {
                loop {
                    tokio::select! {
                        Some(mut msg) = rx.recv() => {
                            msg.handle(a.clone(), c.clone()).await;
                        },
                        Ok(_) = stop_rx.recv() => {
                            break;
                        }
                    }
                }
                a.read().await.stopped().await;
            }
        });

        // Run the started() method on the actor. If the stop signal is received then stop.
        tokio::spawn({
            let a = act_ref.clone();
            let mut stop_rx = self.stop_tx.subscribe();

            async move {
                let act_readonly = a.read().await;
                let fut = act_readonly.started();
                tokio::select! {
                    _ = fut => {},
                    Ok(_) = stop_rx.recv() => {}
                }
            }
        });
        addr
    }
}
