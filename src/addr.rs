use crate::{
    addr_sender::AddrSender,
    envelope::Envelope,
    recipient::Recipient,
    tactix::Sender,
    traits::{Actor, Handler, Message},
};
use tokio::sync::mpsc;

/// A struct to represent an actors address
pub struct Addr<A: Actor> {
    tx: AddrSender<A>,
}

/// Addr needs to be cloned
impl<A: Actor> Clone for Addr<A>
where
    A: Actor,
{
    fn clone(&self) -> Self {
        Addr::from(self.tx.clone())
    }
}

impl<A: Actor> Addr<A> {
    /// Create a new Addr from an Envelope sender
    pub fn new(tx: mpsc::Sender<Envelope<A>>) -> Self {
        Self {
            tx: AddrSender::new(tx),
        }
    }

    pub fn from(sender: AddrSender<A>) -> Self {
        Self { tx: sender }
    }

    /// Convert to a Recipient<M>
    pub fn recipient<M>(self) -> Recipient<M>
    where
        A: Actor + Handler<M>,
        M: Message,
    {
        self.into()
    }
}

#[async_trait::async_trait]
impl<A, M> Sender<M> for Addr<A>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn do_send(&self, msg: M) {
        self.tx.do_send(msg);
    }

    async fn send(&self, msg: M) -> Result<M::Response, String> {
        self.tx.send(msg).await
    }
}

impl<A, M> Into<Recipient<M>> for Addr<A>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn into(self) -> Recipient<M> {
        Recipient::new(Box::new(self.tx))
    }
}
