use async_trait::async_trait;

use crate::traits::{Actor, EnvelopeApi};

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
    async fn handle(&mut self, act: &mut A) {
        self.0.handle(act).await
    }
}
