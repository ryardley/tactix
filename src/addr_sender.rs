use crate::tactix::{Actor, Envelope, EnvelopeInner, EnvelopeSender, Handler, Message, Sender, ToEnvelope};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

/// AddrSender is a Sender that is bound to the Given Actor and can only send messages for that
/// specific Actor on it's Envelope sender.
pub struct AddrSender<A:Actor> {
    tx: mpsc::Sender<Envelope<A>>,
}

impl<A:Actor> Clone for AddrSender<A> {
    fn clone(&self) -> Self {
        AddrSender::new(self.tx.clone())
    }
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
    /// Pack and send the given message without a return channel via AddrSender<A>'s internal Envelope<A> sender.
    fn do_send(&self, msg: M) {
        self.send_env(self.pack(Some(msg), None))
    }

    /// Pack and send the given message with a return channel via AddrSender<A>'s internal Envelope<A> sender.
    async fn send(&self, msg: M) -> Result<M::Response, String> {
        println!("SEND");
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
