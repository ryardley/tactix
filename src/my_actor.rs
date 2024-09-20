use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct MyActorHandle {
    sender: mpsc::Sender<ActorMessage>,
}

impl MyActorHandle {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(8);
        let actor = MyActor::new(receiver);
        tokio::spawn(run_my_actor(actor));

        Self { sender }
    }

    pub async fn greet(&self, name: &str) -> String {
        let (send, recv) = oneshot::channel();
        let msg = ActorMessage::Greet {
            respond_to: send,
            name: name.to_owned(),
        };
        let _ = self.sender.send(msg).await;
        recv.await.expect("Task was killed")
    }
}

impl Default for MyActorHandle {
    fn default() -> Self {
        Self::new()
    }
}

struct MyActor {
    receiver: mpsc::Receiver<ActorMessage>,
}

pub enum ActorMessage {
    Greet {
        name: String,
        respond_to: oneshot::Sender<String>,
    },
}

impl MyActor {
    fn new(receiver: mpsc::Receiver<ActorMessage>) -> Self {
        MyActor { receiver }
    }
    fn handle_message(&mut self, msg: ActorMessage) {
        match msg {
            ActorMessage::Greet { respond_to, name } => {
                let _ = respond_to.send(format!("Hello {}!", name));
            }
        }
    }
}

async fn run_my_actor(mut actor: MyActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg);
    }
}
