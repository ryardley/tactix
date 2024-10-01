use async_trait::async_trait;
use tokio::sync::oneshot::Sender;

use crate::traits::{Actor, EnvelopeApi, Handler, Message};

pub struct EnvelopeInner<M>
where
    M: Message + Send,
{
    pub msg: Option<M>,
    pub tx: Option<Sender<M::Response>>,
}

#[async_trait]
impl<A, M> EnvelopeApi<A> for EnvelopeInner<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    async fn handle(&mut self, act: &mut A) {
        if let Some(msg) = self.msg.take() {
            let res = <A as Handler<M>>::handle(act, msg).await;
            if let Some(tx) = self.tx.take() {
                let _ = tx.send(res);
            }
        }
    }
}
