//! Performance baseline tests for tactix.
//!
//! These exist to measure the cost of the actor runtime before and after
//! changing the default execution model (work-stealing runtime -> single
//! threaded arbiters).
//!
//! Run the full set in release mode:
//!
//! ```bash
//! cargo test --release --test perf -- --ignored --nocapture
//! ```
//!
//! Scale the workload with environment variables:
//!
//! ```bash
//! PERF_N=5000000 PERF_ACTORS=128 \
//!   cargo test --release --test perf -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use tactix::{Actor, Addr, Ctx, Handler, Message, Sender};

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn report(name: &str, ops: usize, elapsed: Duration) {
    let per_sec = ops as f64 / elapsed.as_secs_f64();
    println!(
        "[perf] {name:<24} {per_sec:>14.2} ops/s  (ops={ops}, elapsed={elapsed:?})"
    );
}

struct Counter {
    count: u64,
}

impl Actor for Counter {}

#[derive(Message)]
struct Inc;

#[derive(Message)]
#[response(u64)]
struct Bump;

#[derive(Message)]
#[response(u64)]
struct Get;

impl Handler<Inc> for Counter {
    async fn handle(&mut self, _: Inc, _: &Ctx<Self>) {
        self.count += 1;
    }
}

impl Handler<Bump> for Counter {
    async fn handle(&mut self, _: Bump, _: &Ctx<Self>) -> u64 {
        self.count += 1;
        self.count
    }
}

impl Handler<Get> for Counter {
    async fn handle(&mut self, _: Get, _: &Ctx<Self>) -> u64 {
        self.count
    }
}

struct Front {
    backend: Addr<Counter>,
}

impl Actor for Front {}

impl Handler<Bump> for Front {
    async fn handle(&mut self, _: Bump, _: &Ctx<Self>) -> u64 {
        self.backend.ask(Bump).await
    }
}

/// Cheap always-on smoke test so the harness itself is exercised by `cargo test`.
#[tokio::test(flavor = "multi_thread")]
async fn perf_smoke() {
    let total = 10_000;
    let counter = Counter { count: 0 }.start();
    let start = Instant::now();
    for _ in 0..total {
        counter.tell(Inc);
    }
    let count = counter.ask(Get).await;
    let elapsed = start.elapsed();
    report("smoke_single_actor_tell", total, elapsed);
    assert_eq!(count, total as u64);
    assert!(
        total as f64 / elapsed.as_secs_f64() > 10_000.0,
        "throughput catastrophically low: {elapsed:?} for {total} messages"
    );
}

/// Throughput of a single actor draining its mailbox via `tell`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "performance baseline; run with --release --ignored --nocapture"]
async fn perf_single_actor_tell_throughput() {
    let total = env_usize("PERF_N", 1_000_000);
    let counter = Counter { count: 0 }.start();

    let start = Instant::now();
    for _ in 0..total {
        counter.tell(Inc);
    }
    let count = counter.ask(Get).await;
    let elapsed = start.elapsed();

    report("single_actor_tell", total, elapsed);
    assert_eq!(count, total as u64);
}

/// Sequential `ask` round-trip latency for a single actor.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "performance baseline; run with --release --ignored --nocapture"]
async fn perf_ask_roundtrip_latency() {
    let total = env_usize("PERF_ASKS", 10_000);
    let counter = Counter { count: 0 }.start();

    let start = Instant::now();
    for _ in 0..total {
        let _ = counter.ask(Get).await;
    }
    let elapsed = start.elapsed();

    let per_op = elapsed / total as u32;
    println!(
        "[perf] {:<24} {per_op:>14?}/op  (ops={total}, elapsed={elapsed:?})",
        "ask_roundtrip"
    );
}

/// Fan-out: many actors each drained by a concurrent producer task.
///
/// This is the workload most sensitive to the execution model: with a
/// work-stealing pool all actors share the worker threads, with arbiters they
/// are pinned to a fixed set of single-threaded runtimes.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "performance baseline; run with --release --ignored --nocapture"]
async fn perf_multi_actor_fanout() {
    let actors = env_usize("PERF_ACTORS", 64);
    let total = env_usize("PERF_N", 1_000_000);
    let per_actor = (total / actors).max(1);

    let addrs: Vec<Addr<Counter>> = (0..actors).map(|_| Counter { count: 0 }.start()).collect();

    let start = Instant::now();
    let mut handles = Vec::with_capacity(actors);
    for addr in &addrs {
        let addr = addr.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..per_actor {
                addr.tell(Inc);
            }
        }));
    }
    for handle in handles {
        handle.await.unwrap();
    }
    for addr in &addrs {
        let _ = addr.ask(Get).await;
    }
    let elapsed = start.elapsed();

    report("multi_actor_fanout", actors * per_actor, elapsed);
}

/// Cost of starting many actors (relevant because arbiters add thread/runtime
/// bookkeeping on the spawn path).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "performance baseline; run with --release --ignored --nocapture"]
async fn perf_actor_spawn() {
    let total = env_usize("PERF_SPAWNS", 10_000);

    let start = Instant::now();
    let mut addrs = Vec::with_capacity(total);
    for _ in 0..total {
        addrs.push(Counter { count: 0 }.start());
    }
    let elapsed = start.elapsed();

    report("actor_spawn", total, elapsed);

    for addr in &addrs {
        let _ = addr.ask(Get).await;
    }
}

/// Cross-actor `ask` (front actor awaits a backend actor) round-trip latency.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "performance baseline; run with --release --ignored --nocapture"]
async fn perf_cross_actor_ask() {
    let total = env_usize("PERF_ASKS", 10_000);
    let backend = Counter { count: 0 }.start();
    let front = Front {
        backend: backend.clone(),
    }
    .start();

    let start = Instant::now();
    for _ in 0..total {
        let _ = front.ask(Bump).await;
    }
    let elapsed = start.elapsed();

    let per_op = elapsed / total as u32;
    println!(
        "[perf] {:<24} {per_op:>14?}/op  (ops={total}, elapsed={elapsed:?})",
        "cross_actor_ask"
    );
}
