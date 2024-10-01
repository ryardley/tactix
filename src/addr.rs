
use tokio::sync::{mpsc::{self}, oneshot};

use crate::{envelope::Envelope, envelope_inner::EnvelopeInner, traits::{Actor, Handler, Message, ToEnvelope}};

pub struct Addr<A: Actor> {
    tx: mpsc::Sender<Envelope<A>>,
}

impl<A: Actor> Addr<A> {
    pub fn new(tx: mpsc::Sender<Envelope<A>>) -> Self {
        Self { tx }
    }

    pub fn send_env(&self, env: Envelope<A>) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(env).await;
        });
    }
}

impl<A, M> ToEnvelope<A, M> for Addr<A>
where
    A: Actor + Handler<M>,
    M: Message + Send + 'static,
{
    fn pack(&self, msg: Option<M>, tx: Option<oneshot::Sender<M::Response>>) -> Envelope<A> {
        Envelope::new(Box::new(EnvelopeInner { msg, tx }))
    }
}

impl<A: Actor> Addr<A> {
    pub fn do_send<M>(&self, msg: M)
    where
        M: Message + Send + 'static,
        A: Actor + Handler<M>,
    {
        // Setup the envelope without a transmitter
        let env = self.pack(Some(msg), None);
        self.send_env(env);
    }

    pub async fn send<M>(&self, msg: M) -> Result<M::Response, String>
    where
        M: Message + Send + 'static,
        A: Actor + Handler<M>,
    {
        // make a oneshot
        let (tx, rx) = oneshot::channel::<M::Response>();

        // pass it to the envelope
        let env = self.pack(Some(msg), Some(tx));
        self.send_env(env);

        // receive on the oneshot
        Ok(rx.await.map_err(|_| "yikes".to_string())?)
    }
}
