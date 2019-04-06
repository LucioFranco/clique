use crate::protocol::Message;
use futures::{
    sink::SendAll,
    stream::{iter_ok, IterOk},
    try_ready, Async, Future, Poll, Sink, Stream,
};
use std::time::{Duration, Instant};
use std::vec::IntoIter;
use tokio::timer::Delay;

#[derive(Debug)]
pub struct Gossip {
    interval: Duration,
    delay: Option<Delay>,
}

impl Gossip {
    pub fn new(interval: Duration) -> Self {
        let deadline = Instant::now() + interval;
        let delay = Some(Delay::new(deadline));

        Gossip { interval, delay }
    }

    pub fn reset(&mut self) {}
}

// impl<T> Future for Gossip<T>
// where
//     T: Sink<SinkItem = (u64, Message)> + Clone,
// {
//     type Item = ();
//     type Error = ();

//     fn poll(&mut self) -> Poll<(), ()> {
//         match self.state {
//             State::Gossip => {
//                 let transport = self.transport.clone();
//                 let message = Message::Ping(1, Vec::new());
//                 let messages = vec![(1, message)];
//                 let fut = transport.send_all(iter_ok(messages));

//                 self.state = State::Sending(fut);
//             }

//             State::Sending(ref mut fut) => try_ready!(fut
//                 .poll()
//                 .map_err(|e| error!("Gossip Error sending all request: {}", e))),

//             State::Wait(ref mut fut) => {
//                 return fut.poll().map_err(|e| error!("Gossip Timer Error: {}", e));
//             }
//         }
//     }
// }
