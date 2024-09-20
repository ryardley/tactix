use tokio::sync::{mpsc, oneshot};

struct Envelope {
    msg: CounterMessage,
    tx: Option<oneshot::Sender<u32>>,
}

impl Envelope {
    pub fn new(msg: CounterMessage, tx: Option<oneshot::Sender<u32>>) -> Envelope {
        Envelope { msg, tx }
    }
}

#[derive(Clone)]
pub struct CounterHandle {
    sender: mpsc::Sender<Envelope>,
}

impl CounterHandle {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(8);
        let actor = Counter::new(receiver);
        tokio::spawn(run_my_actor(actor));

        Self { sender }
    }

    pub async fn do_send(&self, msg: CounterMessage) {
        let _ = self.sender.send(Envelope::new(msg, None)).await;
    }

    pub async fn send(&self, msg: CounterMessage) -> u32 {
        let (send, recv) = oneshot::channel();
        let env = Envelope::new(msg, Some(send));
        let _ = self.sender.send(env).await;
        recv.await.expect("Task was killed")
    }
}

impl Default for CounterHandle {
    fn default() -> Self {
        Self::new()
    }
}

struct Counter {
    receiver: mpsc::Receiver<Envelope>,
    count: u32,
}

pub enum CounterMessage {
    Increment,
    Decrement,
    GetCount,
}

impl Counter {
    fn new(receiver: mpsc::Receiver<Envelope>) -> Self {
        Counter { receiver, count: 0 }
    }
    fn handle_message(&mut self, env: Envelope) {
        let Envelope { msg, mut tx } = env;
        match msg {
            CounterMessage::Increment => {
                self.count += 1;
            }
            CounterMessage::Decrement => {
                self.count -= 1;
            }
            CounterMessage::GetCount => {
                if let Some(respond_to) = tx.take() {
                    let _ = respond_to.send(self.count);
                }
            }
        }
    }
}

async fn run_my_actor(mut actor: Counter) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg);
    }
}
