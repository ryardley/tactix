use crate::{
    tactix::{Message, Sender},
};
use async_trait::async_trait;

pub struct Recipient<M: Message> {
    tx: Box<dyn Sender<M>>,
}

impl<M> Recipient<M>
where
    M: Message,
{
    pub fn new(tx: Box<dyn Sender<M>>) -> Self {
        Recipient { tx }
    }
}
#[async_trait]
impl<M> Sender<M> for Recipient<M>
where
    M: Message,
{
    fn do_send(&self, msg: M) {
        self.tx.do_send(msg)
    }

    async fn send(&self, msg: M) -> Result<M::Response, String> {
        self.tx.send(msg).await
    }
}
