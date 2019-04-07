use crate::{
    peer::Peer,
    protocol::{Packet, PacketKind},
    transport::Transport,
};
use futures::{try_ready, Async, AsyncSink, Future, Poll, Stream};
use indexmap::IndexMap;
use std::{collections::VecDeque, net::SocketAddr, time::Duration};
use tokio_timer::Interval;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug)]
pub struct Node<T> {
    transport: T,
    send_queue: VecDeque<Packet>,
    gossip: Interval,
    peers: IndexMap<SocketAddr, Peer>,
}

impl<T: Transport> Node<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            send_queue: VecDeque::new(),
            gossip: Interval::new_interval(Duration::from_secs(1)),
            peers: IndexMap::new(),
        }
    }

    fn process_message(&mut self, message: Packet) {
        match message.kind() {
            PacketKind::Ping => {
                // 1. apply broadcasts
                // 2. collect broadcasts
                // 3. reply with ack(seq)
                let packet = Packet::ack(message.addr(), message.seq(), Vec::new());
                self.send_queue.push_back(packet);
            }

            PacketKind::Ack => {
                // 1. handle incoming ack with seqnum
                // 2. apply incoming ack
            }

            PacketKind::PingReq => unimplemented!(),
            PacketKind::NAck => unimplemented!(),
        }
    }

    // fn poll_server(&mut self) -> Poll<(), T::Error> {
    //     // let (id, message) = try_ready!(self.transport_stream.poll()).unwrap();

    //     // self.process_message(message);

    //     // try_ready!(self.gossip.poll())
    // }

    fn poll_incoming_messages(&mut self) -> Poll<(), Error> {
        match self.transport.poll() {
            Ok(Async::Ready(Some(msg))) => self.process_message(msg),
            Ok(Async::Ready(None)) => unreachable!(),
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Err(_) => panic!(),
        };

        Ok(().into())
    }

    fn poll_flush(&mut self) -> Poll<(), Error> {
        // lets keep trying to flush messages
        loop {
            if let Some(packet) = self.send_queue.pop_front() {
                match self.transport.start_send(packet).map_err(Into::into)? {
                    AsyncSink::Ready => continue,
                    AsyncSink::NotReady(packet) => {
                        self.send_queue.push_front(packet);
                        // TODO: return not ready?
                        self.transport.poll_complete().map_err(Into::into)?;
                    }
                }
            } else {
                return Ok(().into());
            }
        }
    }

    fn poll_gossip(&mut self) -> Poll<(), Error> {
        let _ = try_ready!(self.gossip.poll());

        let addr = "127.0.0.1:8888".parse().unwrap();
        self.send_queue.push_back(Packet::ping(addr, 1, Vec::new()));

        Ok(Async::NotReady)
    }

    fn poll_probe(&mut self) -> Poll<(), Error> {
        // let peers_to_probe = self.peers.peers_to_probe();

        // let fut = self.transport.probe_all(peers_to_probe);

        // set state to probing?
        // what to do with this?
        Ok(().into())
    }

    fn poll_rpc(&mut self) -> Poll<(), Error> {
        // let new_peer = try_ready!(self.transport.poll_rpc());

        // self.peers.add(new_peer);
        Ok(().into())
    }

    fn poll_node(&mut self) -> Poll<(), Error> {
        // self.poll_server()
        // .map_err(|e| error!("Server Error: {}", e.into()))

        // Poll for any incoming messages
        self.poll_incoming_messages()?;

        self.poll_probe()?;

        self.poll_rpc()?;

        self.poll_gossip()?;

        // Lets try to flush all the work we've attempted to do
        self.poll_flush()?;

        Ok(Async::NotReady)
    }
}

#[must_use]
impl<T: Transport> Future for Node<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.poll_node() {
            Ok(Async::Ready(_)) => unreachable!(),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => panic!("Error: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Node;
    use crate::protocol::Packet;
    use crate::transport::mock::Mock;
    use futures::{Future, Sink, Stream};
    use std::time::Duration;
    use tokio_test::{
        task::MockTask,
        timer::{advance, mocked},
    };

    #[test]
    fn node_ping_to_ack() {
        let mut task = MockTask::new();
        let (tx, rx, mock) = Mock::new(1000);

        mocked(|timer, time| {
            let mut node = Node::new(mock);
            let addr = "127.0.0.1:8888".parse().unwrap();

            let ping = Packet::ping(addr, 1, Vec::new());
            tx.send(ping).wait().unwrap();
            assert_not_ready!(task.enter(|| node.poll()));

            let msg = rx.wait().next().unwrap();
            let expected = Packet::ack(addr, 1, Vec::new());
            assert_eq!(msg, Ok(expected));
        });
    }

    #[test]
    fn node_gossip() {
        let mut task = MockTask::new();
        let (tx, mut rx, mock) = Mock::new(1000);

        mocked(|timer, time| {
            let mut node = Node::new(mock);

            assert_not_ready!(task.enter(|| node.poll()));

            assert_not_ready!(rx.poll());

            advance(timer, Duration::from_secs(1));
            assert!(task.is_notified());

            assert_not_ready!(task.enter(|| node.poll()));
            assert_ready!(rx.poll());
        });
    }
}
