use clique::transport::{Request, Response, Transport};
use futures::stream::FuturesUnordered;

use std::{cmp::Ordering, collections::BinaryHeap, time::Instant};

struct SimulatedTransport {
    priority: BinaryHeap<Event>,
}

impl SimulatedTransport {}

struct Event {
    message: Message,
    time: Instant,
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(other.time)
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

struct Simulator {
    transport: SimulatedTransport,
    runner: FuturesUnordered,
}

enum Message {
    Req(Request),
    Res(Response),
}

fn main() {
    println!("Hello, world!");
}
