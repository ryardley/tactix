use std::sync::Arc;

use crate::{addr::Addr, context::Context, envelope::Envelope};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot, Mutex};

#[async_trait]
pub trait Actor: Clone + Sized + Send + Sync + 'static {
    type Context;
    fn start(self) -> Addr<Self> {
        Context::new().run(self)
    }

    async fn started(&self) {}
}

pub trait Message: Send + 'static {
    type Response: Send;
}

/// This handles messages on Actors
#[async_trait]
pub trait Handler<M>
where
    M: Message,
{
    async fn handle(&mut self, msg: M) -> M::Response;
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
    async fn handle(&mut self, act: Arc<Mutex<A>>);
}

/// Represent the ability to send messages
#[async_trait]
pub trait Sender<M: Message>: Sync {
    fn do_send(&self, msg: M);
    async fn send(&self, msg: M) -> Result<M::Response, String>;
}

/// Represent the ability to send messages wrapped in actor envelopes 
pub trait EnvelopeSender<A: Actor> {
    fn get_tx(&self) -> mpsc::Sender<Envelope<A>>;
    fn send_env(&self, env: Envelope<A>) {
        let tx = self.get_tx();
        tokio::spawn(async move {
            let _ = tx.send(env).await;
        });
    }
}
