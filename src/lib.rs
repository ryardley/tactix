mod my_actor;
mod counter;

pub use counter::*;
pub use my_actor::*;

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
