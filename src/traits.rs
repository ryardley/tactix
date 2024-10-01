use crate::{addr::Addr, context::Context, envelope::Envelope};
use async_trait::async_trait;
use tokio::sync::oneshot::Sender;

pub trait Actor: Sized + Send + 'static {
    type Context;
    fn start(self) -> Addr<Self> {
        Context::new().run(self)
    }
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

pub trait ToEnvelope<A, M>
where
    A: Actor,
    M: Message,
{
    fn pack(&self, msg: Option<M>, tx: Option<Sender<M::Response>>) -> Envelope<A>;
}

/// This allows us to run our handler via our Envelope
#[async_trait]
pub trait EnvelopeApi<A: Actor> {
    async fn handle(&mut self, act: &mut A);
}
