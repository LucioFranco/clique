use crate::event::Event;
use futures::Stream;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct Handle {
    event_rx: broadcast::Receiver<Event>,
}

impl Handle {
    pub fn new(event_rx: broadcast::Receiver<Event>) -> Self {
        Handle { event_rx }
    }
}

impl Stream for Handle {
    type Item = Event;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.event_rx).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use tokio_sync::broadcast;

    #[test]
    fn unpin() {
        fn assert_unpin<T: Unpin>(_: T) {}

        let (_tx, rx) = broadcast::channel(10);

        assert_unpin(Handle { event_rx: rx });
    }
}
