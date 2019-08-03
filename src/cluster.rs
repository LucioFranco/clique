use crate::{
    common::{Scheduler, SchedulerEvents},
    error::{Error, Result},
    membership::Membership,
    monitor::{ping_pong, Monitor},
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
    membership: Membership<ping_pong::PingPong>,
    server: S,
    client: C,
    listen_target: T,
    event_tx: watch::Sender<Event>,
    handle: Handle,
    tasks: Scheduler,
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
            tasks: Scheduler::new(),
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

        let (client_tx, mut client_rx) = mpsc::channel(1000);
        let mut client_rx = client_rx.fuse();

        self.membership
            .create_failure_detectors(&mut self.tasks, client_tx.clone());

        let mut alert_batcher_interval = Interval::new_interval(Duration::from_millis(100)).fuse();
        let mut scheduler = Scheduler::new();

        loop {
            futures::select! {
                request = server.next() => {
                    if let Some(Ok(request)) = request {
                        self.membership.handle_message(request, &mut scheduler).await;
                    } else {
                        return Err(Error::new_join(None))
                    }
                },
                event = scheduler.select_next_some() => {
                    match event {
                        SchedulerEvents::StartClassicRound => {
                            self.membership.start_classic_round().await;
                            continue;
                        },
                        SchedulerEvents::None => continue,
                        _ => unimplemented!()
                    }
                },
                item = client_rx.next() => {
                    match item {
                        Some((req, tx)) => self.handle_client_request(req, tx).await,
                        None => eprintln!("Client stream closed")
                    }

                },
                _ = alert_batcher_interval.next() => {
                    self.membership.drain_alerts();
                },
                res = self.tasks.next() => {
                },
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
