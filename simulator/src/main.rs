use clique::transport::{Request, Response};

use std::{cmp::Ordering, collections::BinaryHeap, time::Instant};

#[allow(dead_code)]
struct SimulatedTransport {
    priority: BinaryHeap<Event>,
}

impl SimulatedTransport {}

struct Event {
    #[allow(dead_code)]
    message: Message,
    time: Instant,
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
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

impl Eq for Event {}

enum Message {
    #[allow(dead_code)]
    Req(Request),
    #[allow(dead_code)]
    Res(Response),
}

fn main() {
    println!("Hello, world!");
}
