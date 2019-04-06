use crate::protocol::Message;
use futures::{Poll, StartSend};

pub trait Transport {
    type Error;

    fn start_send(&mut self, message: Message) -> StartSend<(u8, Message), Self::Error>;

    fn poll_complete(&mut self) -> Poll<(), Self::Error>;

    fn poll_recv(&mut self) -> Poll<(u64, Message), Self::Error>;
}

#[cfg(test)]
pub mod mock {
    use crate::protocol::udp::Message;
    use futures::{sync::mpsc, Poll, Sink, StartSend, Stream};

    pub struct Mock {
        tx: mpsc::Sender<(u64, Message)>,
        rx: mpsc::Receiver<(u64, Message)>,
    }

    impl Mock {
        pub fn new(
            limit: usize,
        ) -> (
            mpsc::Sender<(u64, Message)>,
            mpsc::Receiver<(u64, Message)>,
            Self,
        ) {
            let (tx1, rx1) = mpsc::channel(limit);
            let (tx2, rx2) = mpsc::channel(limit);

            let this = Self { tx: tx1, rx: rx2 };

            (tx2, rx1, this)
        }
    }

    impl Sink for Mock {
        type SinkItem = (u64, Message);
        type SinkError = mpsc::SendError<Self::SinkItem>;

        fn start_send(
            &mut self,
            item: Self::SinkItem,
        ) -> StartSend<Self::SinkItem, Self::SinkError> {
            self.tx.start_send(item)
        }

        fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
            self.tx.poll_complete()
        }
    }

    impl Stream for Mock {
        type Item = (u64, Message);
        type Error = ();

        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            self.rx.poll()
        }
    }
}
