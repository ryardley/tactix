use crate::tactix::{EnvelopeInner, Message, Sender};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

pub struct Recipient<M: Message> {
    tx: mpsc::Sender<EnvelopeInner<M>>,
}

impl<M: Message> Recipient<M> {
    fn get_tx(&self) -> mpsc::Sender<EnvelopeInner<M>> {
        self.tx.clone()
    }
    fn send_env(&self, env: EnvelopeInner<M>) {
        let tx = self.get_tx();
        tokio::spawn(async move {
            let _ = tx.send(env).await;
        });
    }

    fn pack(&self, msg: Option<M>, tx: Option<oneshot::Sender<M::Response>>) -> EnvelopeInner<M> {
        EnvelopeInner { msg, tx }
    }
}

#[async_trait]
impl<M> Sender<M> for Recipient<M>
where
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
