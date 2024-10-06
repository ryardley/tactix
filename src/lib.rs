mod addr;
mod addr_sender;
mod context;
mod envelope;
mod envelope_inner;
mod recipient;
mod tactix;
mod traits;

#[cfg(test)]
mod tests {
    use crate::{
        recipient::Recipient,
        tactix::{Actor, Context, Handler, Message, Sender},
    };
    use async_trait::async_trait;

    #[derive(Debug)]
    pub struct Increment;
    impl Message for Increment {
        type Response = ();
    }

    #[derive(Debug)]
    pub struct Decrement;
    impl Message for Decrement {
        type Response = ();
    }

    #[derive(Debug)]
    pub struct GetCount;
    impl Message for GetCount {
        type Response = u64;
    }
    #[derive(Clone)]
    pub struct Counter {
        count: u64,
    }

    impl Counter {
        pub fn new() -> Self {
            Self { count: 0 }
        }
    }

    impl Actor for Counter {
        type Context = Context<Self>;
    }

    #[async_trait]
    impl Handler<Increment> for Counter {
        async fn handle(&mut self, _msg: Increment) {
            println!("INC");
            self.count += 1;
        }
    }

    #[async_trait]
    impl Handler<Decrement> for Counter {
        async fn handle(&mut self, _msg: Decrement) {
            println!("DEC");
            self.count -= 1;
        }
    }

    #[async_trait]
    impl Handler<GetCount> for Counter {
        async fn handle(&mut self, _: GetCount) -> u64 {
            println!("GET");
            let s = self.count;
            println!("SENDING: {}", s);
            s
        }
    }

    #[tokio::test]
    async fn test_counter() -> Result<(), Box<String>> {
        let addr = Counter::new().start();
        let incrementor: Recipient<Increment> = addr.clone().recipient();
        let decrementor: Recipient<Decrement> = addr.clone().recipient();
        addr.do_send(Increment);
        incrementor.do_send(Increment);
        addr.do_send(Increment);
        addr.do_send(Increment);
        addr.do_send(Decrement);
        decrementor.do_send(Decrement);
        let count = addr.send(GetCount).await.unwrap();

        assert_eq!(count, 2);
        Ok(())
    }
}
