mod counter;
mod my_actor;


pub use counter::*;
pub use my_actor::*;
use tokio::sync::{mpsc, oneshot};

trait Message {
    type Response;
}

#[derive(Debug)]
struct Increment;
impl Message for Increment {
    type Response = ();
}

#[derive(Debug)]
struct Decrement;
impl Message for Decrement {
    type Response = ();
}

trait Handler<M>
where
    M: Message + Send,
{
    fn handle(&mut self, msg: M);
}

trait EnvelopeApi<A: Actor> {
    fn handle(&mut self, act: &mut A);
}

struct Envelope<A>(pub Box<dyn EnvelopeApi<A> + Send + 'static>);
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

struct EnvelopeInner<M>
where
    M: Message + Send,
{
    msg: Option<M>,
}

impl<A, M> EnvelopeApi<A> for EnvelopeInner<M>
where
    A: Actor + Handler<M>,
    M: Message + Send + 'static,
{
    fn handle(&mut self, act: &mut A) {
        if let Some(msg) = self.msg.take() {
            <A as Handler<M>>::handle(act, msg);
        }
    }
}

struct Addr<A: Actor> {
    tx: mpsc::Sender<Envelope<A>>,
}

impl<A: Actor> Addr<A> {
    fn new(tx: mpsc::Sender<Envelope<A>>) -> Self {
        Self { tx }
    }
}

trait AddrSend<A, M>
where
    A: Actor + Handler<M>,
    M: Message + Send,
{
    fn do_send(&mut self, msg: M);
}

impl<A, M> AddrSend<A, M> for Addr<A>
where
    A: Actor + Handler<M> + 'static,
    M: Message + Send + 'static,
{
    fn do_send(&mut self, msg: M) {
        let env: Envelope<A> = Envelope::new(Box::new(EnvelopeInner { msg: Some(msg) }));

        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(env).await;
        });
    }
}

trait Actor {
    fn start() {
        
    }
}

#[derive(Default)]
pub struct Counter {
    count: u64,
}

impl Handler<Increment> for Counter {
    fn handle(&mut self, _msg: Increment) {
        self.count += 1;
    }
}

impl Handler<Decrement> for Counter {
    fn handle(&mut self, _msg: Decrement) {
        self.count -= 1;
    }
}

#[cfg(test)]
mod tests {
    use crate::{CounterHandle, CounterMessage, MyActorHandle};

    #[tokio::test]
    async fn test_actor() {
        let addr = MyActorHandle::new();
        let message = addr.greet("Rudi").await;
        assert_eq!(message, "Hello Rudi!");
    }

    #[tokio::test]
    async fn test_counter() {
        let addr = CounterHandle::new();
        addr.do_send(CounterMessage::Increment).await;
        addr.do_send(CounterMessage::Increment).await;
        addr.do_send(CounterMessage::Increment).await;
        addr.do_send(CounterMessage::Increment).await;
        addr.do_send(CounterMessage::Decrement).await;
        let count = addr.send(CounterMessage::GetCount).await;
        assert_eq!(count, 3);
    }
}
