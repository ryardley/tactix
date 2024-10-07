_NOTE: This is currently a proof of concept and is not published or ready for usage_

# Tactix

#### A Simple Actor Model Implementation inspired by Actix

[Actix](https://github.com/actix/actix) provides a great API for working with actors but it is missing some key features of the Actor Model that really need to be there such as heirarchical supervision. Also Actix was created as a solution to asynchrony before tokio was available and as a result implemented it's own runtime. Overtime it switched to tokio as a default async runtime however much of the baggage from the original runtime remains in the code which has led to complexity. Tactix attempts to follow the Actix API whilst fixing some issues inherent within it utilizing as much of tokio as it can. 

Alice Ryhl a maintainer of tokio wrote a [great article on creating an actor model in tokio](https://ryhl.io/blog/actors-with-tokio) where it is outlined how to create an actor model system using tokio channels. This however leads to relatively verbose code as events must be discriminated. 

Tactix attempts to apply some techniques from [Alice Ryhl's article](https://ryhl.io/blog/actors-with-tokio/) and combine them with Actix's handler syntax whilst enabling safe async handlers in order to get the best of both worlds.

This is not a drop-in replacement for Actix but should be a relatively light lift and should improve developer erganomics for async handlers.   

## Usage

```rust
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

#[tokio::main]
async fn main() -> Result<(), Box<String>> {
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
```



- [x] Async Handlers
- [x] Recipient
- [x] started()
- [x] Ensure handlers run in a non-blocking mutually exclusive way
- [ ] Heirarchical Actor Supervision
