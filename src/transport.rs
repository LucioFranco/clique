use crate::protocol::Packet;
use futures::{Poll, Sink, StartSend, Stream};
use std::net::SocketAddr;

#[allow(unused)]
type Error = Box<std::error::Error + Sync + Send + 'static>;

pub trait Transport {
    type Error: Into<Error>;

    fn start_send(&mut self, item: Packet) -> StartSend<Packet, Self::Error>;

    fn poll_complete(&mut self) -> Poll<(), Self::Error>;

    fn poll(&mut self) -> Poll<Option<Packet>, Self::Error>;
}

impl<T> Transport for T
where
    T: Stream<Item = Packet> + Sink<SinkItem = Packet, SinkError = <T as Stream>::Error>,
    T::Error: Into<Error>,
{
    type Error = T::Error;

    fn start_send(&mut self, item: Packet) -> StartSend<Packet, Self::Error> {
        self.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::Error> {
        self.poll_complete()
    }

    fn poll(&mut self) -> Poll<Option<Packet>, Self::Error> {
        self.poll()
    }
}

#[cfg(test)]
pub mod mock {
    use crate::protocol::Packet;
    use futures::{future::poll_fn, sync::mpsc, Future, Poll, Sink, StartSend, Stream};
    use std::{
        fmt,
        sync::{Arc, Mutex},
    };

    #[derive(Debug)]
    pub struct Mock {
        tx: mpsc::Sender<Packet>,
        rx: mpsc::Receiver<Packet>,
    }

    #[derive(Clone, Debug)]
    pub struct Handle {
        tx: mpsc::Sender<Packet>,
        rx: Arc<Mutex<mpsc::Receiver<Packet>>>,
    }

    pub enum MockError<T> {
        SendError(T),
        Stream,
    }

    impl Mock {
        pub fn new(limit: usize) -> (Handle, Mock) {
            let (tx1, rx1) = mpsc::channel(limit);
            let (tx2, rx2) = mpsc::channel(limit);

            let mock = Mock { tx: tx1, rx: rx2 };
            let handle = Handle {
                tx: tx2,
                rx: Arc::new(Mutex::new(rx1)),
            };

            (handle, mock)
        }
    }

    impl Handle {
        pub fn send(&mut self, item: Packet) -> Result<(), mpsc::SendError<Packet>> {
            let mut item = Some(item);

            while let Some(i) = item.take() {
                use futures::AsyncSink::*;
                if let NotReady(i) = self.tx.start_send(i)? {
                    item = Some(i)
                }

                poll_fn(|| self.tx.poll_complete()).wait()?;
            }

            poll_fn(|| self.tx.poll_complete()).wait()
        }

        pub fn poll(&mut self) -> Poll<Option<Packet>, ()> {
            self.rx.lock().unwrap().poll()
        }

        pub fn get(&mut self) -> Option<Packet> {
            poll_fn(|| self.rx.lock().unwrap().poll()).wait().unwrap()
        }
    }

    impl Sink for Mock {
        type SinkItem = Packet;
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
        type Item = Packet;
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
