use crate::{
    addr_sender::AddrSender,
    envelope::Envelope,
    recipient::Recipient,
    tactix::Sender,
    traits::{Actor, Handler, Message},
};
use tokio::sync::mpsc;


#[derive(Clone)]
pub struct Addr<A: Actor> {
    tx: AddrSender<A>,
}

impl<A: Actor> Addr<A> {
    pub fn new(tx: mpsc::Sender<Envelope<A>>) -> Self {
        Self {
            tx: AddrSender::new(tx),
        }
    }

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
