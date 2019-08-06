use crate::{
    common::{Scheduler, SchedulerEvents},
    error::{Error, Result},
    membership::Membership,
    monitor::{ping_pong, Monitor},
    transport::{Client, Request, Response, Transport},
};
use futures::{FutureExt, Stream, StreamExt};
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio_sync::{mpsc, oneshot, watch};
use tokio_timer::Interval;

#[derive(Debug, Default, Clone)]
pub struct Event;

pub struct Cluster<T, Target> {
    membership: Membership<ping_pong::PingPong>,
    transport: T,
    listen_target: Target,
    event_tx: watch::Sender<Event>,
    handle: Handle,
    tasks: Scheduler,
}

#[derive(Clone)]
pub struct Handle {
    event_rx: watch::Receiver<Event>,
}

impl<T, Target> Cluster<T, Target>
where
    T: Transport<Target> + Send,
    Target: Send + Clone,
{
    pub fn new(transport: T, listen_target: Target) -> Self {
        let (event_tx, event_rx) = watch::channel(Event::default());

        let handle = Handle { event_rx };

        Self {
            membership: Membership::new(),
            transport,
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
            .transport
            .listen_on(self.listen_target.clone())
            .await
            .unwrap()
            .fuse();

        let mut scheduler = Scheduler::new();

        let (client_tx, mut client_rx) = mpsc::channel(1000);
        let mut client_rx = client_rx.fuse();
        let (broadcast_tx, broadcast_rx) = mpsc::channel(1000);
        let mut broadcast_rx = broadcast_rx.fuse();
        let client = Client::new(client_tx, broadcast_tx);

        self.membership
            .create_failure_detectors(&mut scheduler, client.clone());

        let mut alert_batcher_interval = Interval::new_interval(Duration::from_millis(100)).fuse();

        loop {
            futures::select! {
                request = server.select_next_some() => {
                    if let Ok((request, response_tx)) = request {
                        let response = self.membership.handle_message(request, &mut scheduler).await;
                        response_tx.send(response);
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
                (request, tx) = client_rx.select_next_some() => {
                    let task = self
                        .transport
                        .send(request)
                        .map(|res| tx.send(res.map_err(|_| Error::new_broken_pipe(None))))
                        .map(drop);
                    scheduler.push(Box::pin(task.map(|_| SchedulerEvents::None)));
                },
                request = broadcast_rx.select_next_some() => {
                    let view = self.membership.view();

                    for endpoint in view {
                        let task = self.transport.send(Request::new(endpoint.clone(), request.clone()));
                        scheduler.push(Box::pin(task.map(|_| SchedulerEvents::None)));
                    }
                },
                _ = alert_batcher_interval.select_next_some() => {
                    self.membership.drain_alerts();
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
