use std::marker::PhantomData;

use tokio::sync::mpsc;

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

    pub fn run(&self, mut act: A) -> Addr<A> {
        let (tx, mut rx) = mpsc::channel::<Envelope<A>>(100);
        let addr = Addr::new(tx);

        tokio::spawn(async move {
            while let Some(mut msg) = rx.recv().await {
                msg.handle(&mut act).await
            }
        });
        addr
    }
}
