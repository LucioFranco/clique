use crate::{broadcast::Broadcast, gossip::Gossip, protocol::Message, transport::Transport};
use futures::{try_ready, Async, AsyncSink, Future, Poll};
use futures::{Sink, Stream};
use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug)]
pub struct Node<T> {
    transport: T,
    send_queue: VecDeque<(u64, Message)>,
    gossip: Gossip,
}

impl<T> Node<T>
where
    T: Stream<Item = (u64, Message)> + Sink<SinkItem = (u64, Message)>,
    T::Error: fmt::Debug,
    T::SinkError: fmt::Debug,
{
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            send_queue: VecDeque::new(),
            gossip: Gossip::new(Duration::from_secs(1)),
        }
    }

    fn process_message(&mut self, message: Message) {
        match message {
            Message::Ping(seq, broadcasts) => {
                // 1. apply broadcasts
                // 2. collect broadcasts
                // 3. reply with ack(seq)
                self.send_queue
                    .push_back((1, Message::Ack(seq, Vec::new())));
            }

            Message::Ack(seq, broadcasts) => {
                // 1. handle incoming ack with seqnum
                // 2. apply incoming ack
            }

            Message::PingReq(seq, broadcasts) => unimplemented!(),
            Message::NAck(seq, broadcasts) => unimplemented!(),
        }
    }

    // fn poll_server(&mut self) -> Poll<(), T::Error> {
    //     // let (id, message) = try_ready!(self.transport_stream.poll()).unwrap();

    //     // self.process_message(message);

    //     // try_ready!(self.gossip.poll())
    // }

    fn poll_incoming_messages(&mut self) -> Poll<(), ()> {
        match self.transport.poll() {
            Ok(Async::Ready(Some((_id, msg)))) => self.process_message(msg),
            Ok(Async::Ready(None)) => unreachable!(),
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Err(_) => panic!(),
        };

        Ok(().into())
    }

    fn poll_flush(&mut self) -> Poll<(), ()> {
        // lets keep trying to flush messages
        loop {
            if let Some(message) = self.send_queue.pop_front() {
                match self.transport.start_send(message) {
                    Ok(AsyncSink::Ready) => continue,
                    Ok(AsyncSink::NotReady(message)) => {
                        self.send_queue.push_front(message);
                        // TODO: return not ready?
                        self.transport.poll_complete().unwrap();
                    }
                    Err(e) => panic!("error"),
                }
            } else {
                return Ok(().into());
            }
        }
    }

    fn poll_gossip(&mut self) -> Poll<(), ()> {
        // try_ready!(self.gossip.delay());

        // let peers = self.peers.sample();
        // let fut = self.transport.ping_all(peers);

        // set state to pinging?
        // what to do with this fut?
        Ok(().into())
    }

    fn poll_probe(&mut self) -> Poll<(), ()> {
        // let peers_to_probe = self.peers.peers_to_probe();

        // let fut = self.transport.probe_all(peers_to_probe);

        // set state to probing?
        // what to do with this?
        Ok(().into())
    }

    fn poll_rpc(&mut self) -> Poll<(), ()> {
        // let new_peer = try_ready!(self.transport.poll_rpc());

        // self.peers.add(new_peer);
        Ok(().into())
    }
}

#[must_use]
impl<T> Future for Node<T>
where
    T: Stream<Item = (u64, Message)> + Sink<SinkItem = (u64, Message)>,
    T::Error: fmt::Debug,
    T::SinkError: fmt::Debug,
{
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // self.poll_server()
        // .map_err(|e| error!("Server Error: {}", e.into()))

        // Poll for any incoming messages
        self.poll_incoming_messages();
        self.poll_flush();

        self.poll_probe();

        self.poll_rpc();

        self.poll_gossip();

        Ok(Async::NotReady)
    }
}

#[cfg(test)]
mod tests {
    use super::Node;
    use crate::protocol::udp::Message;
    use crate::transport::mock::Mock;
    use futures::{Future, Sink, Stream};
    use tokio_test::task::MockTask;

    #[test]
    fn node_ping_to_ack() {
        let mut task = MockTask::new();
        let (tx, rx, mock) = Mock::new(1000);
        let mut node = Node::new(mock);

        tx.send((0, Message::Ping(1, Vec::new()))).wait();
        assert_not_ready!(task.enter(|| node.poll()));

        let msg = rx.wait().next().unwrap();
        assert_eq!(msg, Ok((1, Message::Ack(1, Vec::new()))));
    }
}
