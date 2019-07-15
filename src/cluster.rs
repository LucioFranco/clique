use crate::{
    error::{Error, Result},
    membership::Membership,
    transport::{Client, Request, Response, Server},
};
use futures::{Stream, StreamExt};
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio_sync::{mpsc, oneshot, watch};
use tokio_timer::Interval;

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
    S: Server<T>,
    C: Client + Send + Clone,
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

    pub async fn start(&mut self) -> Result<()> {
        let mut server = self
            .server
            .start(self.listen_target.clone())
            .await
            .unwrap()
            .fuse();

        let mut edge_detector_ticker = Interval::new(Instant::now(), Duration::from_secs(1)).fuse();
        let mut monitor = crate::monitor::Monitor::new();
        // let mut batch_alert_

        let mut monitor_jobs = crate::common::Scheduler::new();

        let (client_tx, mut client_rx) = mpsc::channel(1000);
        let mut client_rx = client_rx.fuse();

        self.membership
            .create_failure_detectors(&mut monitor_jobs, client_tx.clone());

        loop {
            futures::select! {
                request = server.next() => {
                    if let Some(Ok(request)) = request {
                        self.membership.handle_message(request).await;
                    } else {
                        return Err(Error::new_join(None))
                    }
                },
                item = client_rx.next() => {
                    match item {
                        Some((req, tx)) => self.handle_client_request(req, tx).await,
                        None => eprintln!("Client stream closed")
                    }

                }
                res = monitor_jobs.next() => {
                },
                _ = edge_detector_ticker.next() => {
                    self.membership.tick().await;
                }
            };
        }
    }

    async fn handle_client_request(&mut self, request: Request, tx: oneshot::Sender<Response>) {
        let response = self.client.call(request).await.unwrap();
        tx.send(response).unwrap();
    }
}

impl Stream for Handle {
    type Item = Event;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.event_rx).poll_next(cx)
    }
}
