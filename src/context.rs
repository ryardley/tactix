use std::{marker::PhantomData, sync::Arc};

use tokio::sync::{mpsc, Mutex};

use crate::{
    addr::Addr,
    envelope::Envelope,
    traits::{Actor, EnvelopeApi},
};

pub struct Context<A> {
    _p: PhantomData<A>,
}

impl<A: Actor> Context<A> {
    pub fn new() -> Self {
        Self { _p: PhantomData }
    }

    pub fn run(&self, act: A) -> Addr<A> {
        let (tx, mut rx) = mpsc::channel::<Envelope<A>>(100);
        let addr = Addr::new(tx);
        let act_ref = Arc::new(Mutex::new(act));

        tokio::spawn({
            let a = act_ref.clone();
            async move {

                while let Some(mut msg) = rx.recv().await {
                    msg.handle(a.clone()).await
                }
            }
        });

        tokio::spawn({
            let a = act_ref.clone();
            async move { a.lock().await.started().await; }
        });
        addr
    }
}
