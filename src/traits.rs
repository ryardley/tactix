use std::sync::Arc;

use crate::{addr::Addr, context::Context, envelope::Envelope};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot, Mutex};

/// An Actor in the system. An object that manages it's memory purely by receiving messages
#[async_trait]
pub trait Actor: Sized + Send + Sync + 'static {
    type Context;
    fn start(self) -> Addr<Self> {
        Context::new().run(self)
    }

    async fn started(&self) {}
}


/// A Message can be sent to an Actor. It has a response which can be returned when using the
/// `send` async method.
pub trait Message: Send + 'static {
    type Response: Send;
}

/// This handles messages on Actors
/// Handlers can be added to actors like so:
///
/// ```
/// use tactix::*;
/// use async_trait::async_trait;
///
/// struct MyActor;
///
/// impl Actor for MyActor {
///   type Context = tactix::Context<Self>;
/// }
///
/// struct SomeMessage;
/// impl Message for SomeMessage {
///   type Response = u8;
/// }
///
/// #[async_trait]
/// impl Handler<SomeMessage> for MyActor {
///   async fn handle(&mut self, msg:SomeMessage) -> u8 {
///      // Implement your handler
///      1u8
///   }
/// }
/// ```
///
/// Note this trait requires the use of the `#[async_trait]` macro.
#[async_trait]
pub trait Handler<M>
where
    M: Message,
{
    async fn handle(&mut self, msg: M) -> M::Response;
}

/// This enables the host to pack messages to an envelope
pub trait ToEnvelope<A, M>
where
    A: Actor,
    M: Message,
{
    fn pack(&self, msg: Option<M>, tx: Option<oneshot::Sender<M::Response>>) -> Envelope<A>;
}

/// This allows us to run our handler via our actor bound Envelope
#[async_trait]
pub trait EnvelopeApi<A: Actor> {
    async fn handle(&mut self, act: Arc<Mutex<A>>);
}

/// Encapsulates the idea of a channel transmitter. Represents the ability to send messages
#[async_trait]
pub trait Sender<M: Message>: Sync {
    fn do_send(&self, msg: M);
    async fn send(&self, msg: M) -> Result<M::Response, String>;
}

/// Represent the ability to send messages wrapped in actor envelopes 
pub trait EnvelopeSender<A: Actor> {
    fn get_tx(&self) -> mpsc::Sender<Envelope<A>>;

    /// Send a given Envelope on the structs given envelope sender
    fn send_env(&self, env: Envelope<A>) {
        let tx = self.get_tx();
        tokio::spawn(async move {
            let _ = tx.send(env).await;
        });
    }
}
