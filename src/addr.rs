use crate::{
    addr_sender::AddrSender,
    envelope::Envelope,
    tactix::Sender,
    traits::{Actor, Handler, Message},
};
use tokio::sync::mpsc;

pub struct Addr<A: Actor> {
    tx: AddrSender<A>,
}

impl<A: Actor> Addr<A> {
    pub fn new(tx: mpsc::Sender<Envelope<A>>) -> Self {
        Self {
            tx: AddrSender::new(tx),
        }
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
