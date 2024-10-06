use crate::tactix::{Actor, Envelope, EnvelopeInner, EnvelopeSender, Handler, Message, Sender, ToEnvelope};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

pub struct AddrSender<A> {
    tx: mpsc::Sender<Envelope<A>>,
}

impl<A: Actor> AddrSender<A> {
    pub fn new(tx: mpsc::Sender<Envelope<A>>) -> Self {
        Self { tx }
    }
}

impl<A> EnvelopeSender<A> for AddrSender<A>
where
    A: Actor,
{
    fn get_tx(&self) -> mpsc::Sender<Envelope<A>> {
        self.tx.clone()
    }
}

impl<A, M> ToEnvelope<A, M> for AddrSender<A>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn pack(&self, msg: Option<M>, tx: Option<oneshot::Sender<M::Response>>) -> Envelope<A> {
        Envelope::new(Box::new(EnvelopeInner { msg, tx }))
    }
}

#[async_trait]
impl<A, M> Sender<M> for AddrSender<A>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn do_send(&self, msg: M) {
        self.send_env(self.pack(Some(msg), None))
    }

    async fn send(&self, msg: M) -> Result<M::Response, String> {
        // make a oneshot
        let (tx, rx) = oneshot::channel::<M::Response>();

        // pass it to the envelope
        self.send_env(self.pack(Some(msg), Some(tx)));

        // receive on the oneshot
        Ok(rx
            .await
            .map_err(|_| "Error receiving response".to_string())?)
    }
}
