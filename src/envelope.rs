use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::traits::{Actor, EnvelopeApi};

/// An envelope is bound to an Actor and represents a Message sent through a specific actor
/// Envelopes hold an envelope implementation.
pub struct Envelope<A>(pub Box<dyn EnvelopeApi<A> + Send + 'static>);

impl<A> Envelope<A> {
    pub fn new(inner: Box<dyn EnvelopeApi<A> + Send + 'static>) -> Self {
        Self(inner)
    }
}

#[async_trait]
impl<A> EnvelopeApi<A> for Envelope<A>
where
    A: Actor,
{
    async fn handle(&mut self, act: Arc<Mutex<A>>, ctx: A::Context) {
        self.0.handle(act, ctx).await
    }
}
