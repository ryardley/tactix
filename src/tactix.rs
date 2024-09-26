
use std::marker::PhantomData;

use async_trait::async_trait;

use tokio::sync::{
    mpsc::{self},
    oneshot::{self, Sender},
};

pub trait Message {
    type Response: Send;
}

/// This handles messages on Actors
pub trait Handler<M>
where
    M: Message + Send,
{
    fn handle(&mut self, msg: M) -> M::Response;
}

/// This allows us to run our handler via our Envelope
pub trait EnvelopeApi<A: Actor> {
    fn handle(&mut self, act: &mut A);
}

// 
pub struct Envelope<A>(pub Box<dyn EnvelopeApi<A> + Send + 'static>);
impl<A> Envelope<A> {
    pub fn new(inner: Box<dyn EnvelopeApi<A> + Send + 'static>) -> Self {
        Self(inner)
    }
}

impl<A> EnvelopeApi<A> for Envelope<A>
where
    A: Actor,
{
    fn handle(&mut self, act: &mut A) {
        self.0.handle(act)
    }
}

pub struct EnvelopeInner<M>
where
    M: Message + Send,
{
    msg: Option<M>,
    tx: Option<Sender<M::Response>>,
}

impl<A, M> EnvelopeApi<A> for EnvelopeInner<M>
where
    A: Actor + Handler<M>,
    M: Message + Send + 'static,
{
    fn handle(&mut self, act: &mut A) {
        if let Some(msg) = self.msg.take() {
            let res = <A as Handler<M>>::handle(act, msg);
            if let Some(tx) = self.tx.take() {
                let _ = tx.send(res);
            }
        }
    }
}

pub struct Addr<A: Actor + Sized> {
    tx: mpsc::Sender<Envelope<A>>,
}

impl<A: Actor> Addr<A> {
    fn new(tx: mpsc::Sender<Envelope<A>>) -> Self {
        Self { tx }
    }
}

#[async_trait]
pub trait AddrSend<A, M>
where
    A: Actor + Handler<M>,
    M: Message + Send,
{
    fn do_send(&self, msg: M);
    async fn send(&self, msg: M) -> Result<M::Response, String>;
}

#[async_trait]
impl<A, M> AddrSend<A, M> for Addr<A>
where
    A: Actor + Handler<M> + 'static,
    M: Message + Send + 'static,
{
    fn do_send(&self, msg: M) {
        // Setup the envelope without a transmitter
        let env: Envelope<A> = Envelope::new(Box::new(EnvelopeInner {
            msg: Some(msg),
            tx: None,
        }));

        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(env).await;
        });
    }

    async fn send(&self, msg: M) -> Result<M::Response, String> {
        // make a oneshot
        let (tx, rx) = oneshot::channel::<M::Response>();

        // pass it to the envelope
        let env: Envelope<A> = Envelope::new(Box::new(EnvelopeInner {
            msg: Some(msg),
            tx: Some(tx),
        }));
        
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(env).await;
        });

        // receive on the oneshot
        Ok(rx.await.map_err(|_| "yikes".to_string())?)
    }
}

pub trait Actor: Sized + Send + 'static {
    type Context;
    fn start(self) -> Addr<Self> {
        Context::new().run(self)
    }
}

trait ContextApi<A>
where
    A: Actor,
{
    fn new() -> Self;
    fn run(&self, act: A) -> Addr<A>;
}

pub struct Context<A> {
    _p: PhantomData<A>,
}

impl<A> ContextApi<A> for Context<A>
where
    A: Actor + Sized + Send + 'static,
{
    fn new() -> Self {
        Self { _p: PhantomData }
    }

    fn run(&self, mut act: A) -> Addr<A> {
        let (tx, mut rx) = mpsc::channel::<Envelope<A>>(100);
        let addr = Addr::new(tx);

        tokio::spawn(async move {
            while let Some(mut msg) = rx.recv().await {
                msg.handle(&mut act)
            }
        });
        addr
    }
}

