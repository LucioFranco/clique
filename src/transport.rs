use crate::protocol::Packet;
use futures::{Poll, Sink, StartSend, Stream};

#[allow(unused)]
type Error = Box<std::error::Error + Sync + Send + 'static>;
type Message = (u64, Packet);

pub trait Transport {
    type Error: Into<Error>;

    fn start_send(&mut self, message: Message) -> StartSend<Message, Self::Error>;

    fn poll_complete(&mut self) -> Poll<(), Self::Error>;

    fn poll(&mut self) -> Poll<Option<Message>, Self::Error>;
}

impl<T> Transport for T
where
    T: Stream<Item = Message> + Sink<SinkItem = Message, SinkError = <T as Stream>::Error>,
    T::Error: Into<Error>,
{
    type Error = T::Error;

    fn start_send(&mut self, message: Message) -> StartSend<Message, Self::Error> {
        self.start_send(message)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::Error> {
        self.poll_complete()
    }

    fn poll(&mut self) -> Poll<Option<Message>, Self::Error> {
        self.poll()
    }
}

#[cfg(test)]
pub mod mock {
    use crate::protocol::Packet;
    use futures::{sync::mpsc, Poll, Sink, StartSend, Stream};
    use std::fmt;

    type Message = (u64, Packet);

    pub struct Mock {
        tx: mpsc::Sender<Message>,
        rx: mpsc::Receiver<Message>,
    }

    pub enum MockError<T> {
        SendError(T),
        Stream,
    }

    impl Mock {
        pub fn new(limit: usize) -> (mpsc::Sender<Message>, mpsc::Receiver<Message>, Self) {
            let (tx1, rx1) = mpsc::channel(limit);
            let (tx2, rx2) = mpsc::channel(limit);

            let this = Self { tx: tx1, rx: rx2 };

            (tx2, rx1, this)
        }
    }

    impl Sink for Mock {
        type SinkItem = Message;
        type SinkError = MockError<Self::SinkItem>;

        fn start_send(
            &mut self,
            item: Self::SinkItem,
        ) -> StartSend<Self::SinkItem, Self::SinkError> {
            self.tx.start_send(item).map_err(Into::into)
        }

        fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
            self.tx.poll_complete().map_err(Into::into)
        }
    }

    impl Stream for Mock {
        type Item = Message;
        type Error = MockError<Self::Item>;

        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            self.rx.poll().map_err(|_| MockError::Stream)
        }
    }

    impl<T> fmt::Debug for MockError<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                MockError::SendError(_) => write!(f, "Mock SendError"),
                MockError::Stream => write!(f, "Mock StreamError"),
            }
        }
    }

    impl<T> fmt::Display for MockError<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                MockError::SendError(_) => write!(f, "Mock SendError"),
                MockError::Stream => write!(f, "Mock StreamError"),
            }
        }
    }

    impl<T> std::error::Error for MockError<T> {}

    impl<T> From<mpsc::SendError<T>> for MockError<T> {
        fn from(e: mpsc::SendError<T>) -> Self {
            MockError::SendError(e.into_inner())
        }
    }
}
