_NOTE: This is currently a proof of concept and is not published or ready for usage_

# Tactix

#### A Simple Actor Model Implementation inspired by Actix

[Actix](https://github.com/actix/actix) provides a great API for working with actors but it is missing some key features of the Actor Model that really need to be there. Tactix attempts to follow the Actix API whilst fixing some issues inherent within it.

Here we apply some techniques from [Alice Ryhl's Great Actor Model in Rust Article](https://ryhl.io/blog/actors-with-tokio/) and apply Actix's handler syntax to get the best of both worlds.

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
    addr.do_send(Increment);
    addr.do_send(Increment);
    addr.do_send(Increment);
    addr.do_send(Increment);
    addr.do_send(Decrement);
    let count = addr.send(GetCount).await.unwrap();

    assert_eq!(count, 3);
    Ok(())
}
```



- [x] Async Handlers
- [ ] Receivers
- [ ] started()
- [ ] ctx.spawn()
- [ ] Heirarchical Actor Supervision
