use crate::{
    error::{Error, Result},
    membership::Membership,
    transport::{Client, Server},
};
use futures::stream::{Stream, StreamExt};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio_sync::watch;

#[derive(Debug, Default, Clone)]
pub struct Event;

pub struct Cluster<S, C, T> {
    membership: Membership,
    server: S,
    client: C,
    listen_target: T,
    event_tx: watch::Sender<Event>,
    handle: Handle,
}

#[derive(Clone)]
pub struct Handle {
    event_rx: watch::Receiver<Event>,
}

impl<S, C, T> Cluster<S, C, T>
where
    S: Server<T, C>,
    C: Client + Clone,
    T: Clone,
{
    pub fn new(server: S, client: C, listen_target: T) -> Self {
        let (event_tx, event_rx) = watch::channel(Event::default());

        let handle = Handle { event_rx };

        Self {
            membership: Membership::new(),
            client,
            server,
            listen_target,
            event_tx,
            handle,
        }
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    pub async fn start(self) -> Result<()> {
        let Cluster {
            membership,
            server,
            client,
            listen_target,
            event_tx,
            ..
        } = self;

        let mut server = server.start(listen_target).await.unwrap().fuse();

        loop {
            futures::select! {
                request = server.next() => {
                    if let Some(request) = request {
                        unimplemented!()
                    } else {
                        return Err(Error::new_join(None))
                    }
                },
            };
        }
    }
}

impl Stream for Handle {
    type Item = Event;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.event_rx).poll_next(cx)
    }
}
