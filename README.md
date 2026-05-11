# Tactix

#### A Simple Actor Model Implementation built with Tokio

[Actix](https://github.com/actix/actix) provides a great API for working with actors but it is missing some key features of the Actor Model that really need to be there such as heirarchical supervision. Also Actix was created as a solution to asynchrony before tokio was available and as a result implemented it's own runtime. Overtime it switched to tokio as a default async runtime however much of the baggage from the original runtime remains in the code which has led to complexity. Tactix attempts to follow the Actix API whilst fixing some issues inherent within it utilizing as much of tokio as it can. 

Alice Ryhl a maintainer of tokio wrote a [great article on creating an actor model in tokio](https://ryhl.io/blog/actors-with-tokio) where it is outlined how to create an actor model system using tokio channels. This however leads to relatively verbose code as events must be discriminated. 

Tactix attempts to apply some techniques from [Alice Ryhl's article](https://ryhl.io/blog/actors-with-tokio/) and combine them with Actix's handler syntax whilst enabling safe async handlers in order to get the best of both worlds.

This is not a drop-in replacement for Actix but should be a relatively light lift and should improve developer erganomics for async handlers.   

For more informamtion on the inner workings of tactix you can read the blog post series on it:

1. [Rolling Your Own Rust Actor Model](https://vpunk.sh/blog/actor-model-in-rust/)
2. [Building self healing systems](https://vpunk.sh/blog/actor-model-in-rust-supervision/)

## Usage

```rust
use tactix::{Actor, Ctx, Handler, Message, Recipient, Sender};

#[derive(Debug, Message)]
pub struct Increment;

#[derive(Debug, Message)]
pub struct Decrement;

#[derive(Debug, Message)]
#[response(u64)]
pub struct GetCount;

pub struct Counter {
    count: u64,
}

impl Actor for Counter {}

impl Handler<Increment> for Counter {
    async fn handle(&mut self, _: Increment, _: &Ctx<Self>) {
        self.count += 1;
    }
}

impl Handler<Decrement> for Counter {
    async fn handle(&mut self, _: Decrement, _: &Ctx<Self>) {
        self.count -= 1;
    }
}

impl Handler<GetCount> for Counter {
    async fn handle(&mut self, _: GetCount, _: &Ctx<Self>) -> u64 {
        self.count
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let counter = Counter { count: 0 }.start();

    // Fire-and-forget with `tell`
    counter.tell(Increment);
    counter.tell(Increment);
    counter.tell(Increment);
    counter.tell(Decrement);

    // Type-erased recipient for dependency injection
    let decrementor: Recipient<Decrement> = counter.clone().recipient();
    decrementor.tell(Decrement);

    // Use `ask` to synchronise and get a response
    let _ = counter.ask(Increment).await;
    let count = counter.ask(GetCount).await;

    assert_eq!(count, 2);
    Ok(())
}
```




