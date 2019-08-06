use crate::{
    builder::Builder,
    common::{Scheduler, SchedulerEvents},
    error::{Error, Result},
    event::Event,
    handle::Handle,
    membership::Membership,
    monitor::{ping_pong, Monitor},
    transport::{client, Client, Request, Response, Transport},
};
use futures::{
    future::{self, BoxFuture},
    stream::Fuse,
    FutureExt, Stream, StreamExt,
};
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio_sync::{mpsc, oneshot, watch};
use tokio_timer::Interval;

pub struct Cluster<T, Target>
where
    T: Transport<Target>,
{
    handle: Handle,
    inner: State<T, Target>,
}

enum State<T, Target>
where
    T: Transport<Target>,
{
    Idle(Builder<T, Target>),
    Running(Inner<T, Target>),
}

impl<T, Target> Cluster<T, Target>
where
    T: Transport<Target> + Send,
    Target: Send + Clone,
{
    pub fn builder() -> Builder<T, Target> {
        unimplemented!()
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    pub async fn start(&mut self) -> Result<()> {
        match &mut self.inner {
            State::Running(inner) => inner.start().await,
            _ => unreachable!(),
        }
    }

    pub async fn join(&mut self, seed_addr: Target) -> Result<()> {
        match &mut self.inner {
            State::Running(inner) => inner.join(seed_addr).await,
            _ => unreachable!(),
        }
    }
}

struct Inner<T, Target>
where
    T: Transport<Target>,
{
    membership: Membership<ping_pong::PingPong>,
    transport: T,
    listen_target: Target,
    event_tx: watch::Sender<Event>,

    // TODO: might make sense to move this to the fn run since that seems to be the
    // only place we actually use it.
    scheduler: Scheduler,

    client: Client,
    client_stream: Fuse<client::RequestStream>,
    server_stream: Fuse<T::ServerStream>,
}

impl<T, Target> Inner<T, Target>
where
    T: Transport<Target> + Send,
    Target: Send + Clone,
{
    async fn new(mut transport: T, listen_target: Target, event_tx: watch::Sender<Event>) -> Self {
        let server_stream = transport.listen_on(listen_target.clone()).await.unwrap();

        let (client, mut client_stream) = Client::new(100);

        Self {
            membership: Membership::new(),
            transport,
            listen_target,
            event_tx,
            scheduler: Scheduler::new(),
            client,
            client_stream: client_stream.fuse(),
            server_stream: server_stream.fuse(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        self.run().await
    }

    pub async fn join(&mut self, seed_addr: Target) -> Result<()> {
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        self.membership
            .create_failure_detectors(&mut self.scheduler, self.client.clone());

        let mut alert_batcher_interval = Interval::new_interval(Duration::from_millis(100)).fuse();

        loop {
            futures::select! {
                request = self.server_stream.select_next_some() => {
                    self.handle_server(request).await?;
                },
                event = self.scheduler.select_next_some() => {
                    match event {
                        SchedulerEvents::StartClassicRound => {
                            self.membership.start_classic_round().await;
                            continue;
                        },
                        SchedulerEvents::None => continue,
                        _ => unimplemented!()
                    }
                },
                request = self.client_stream.select_next_some() => {
                    let task = self.handle_client(request);
                    self.scheduler.push(task);
                }
                _ = alert_batcher_interval.select_next_some() => {
                    self.membership.drain_alerts();
                },
            };
        }
    }

    async fn handle_server(
        &mut self,
        request: std::result::Result<(Request, oneshot::Sender<crate::Result<Response>>), T::Error>,
    ) -> Result<()> {
        match request {
            Ok((request, response_tx)) => {
                let response = self
                    .membership
                    .handle_message(request, &mut self.scheduler)
                    .await;
                response_tx.send(response);

                Ok(())
            }
            Err(e) => Err(Error::new_join(Some(Box::new(e)))),
        }
    }

    // Need this to be a boxed futures since we want to send these tasks into
    // the FuturesUnordered and we require that the lifetime of the future
    // is static due to the box. It seems that the compiler can't infer the
    // lifetime from the borrow that is calling handle_client if it were an
    // async fn.
    fn handle_client(
        &mut self,
        request: client::RequestType,
    ) -> BoxFuture<'static, SchedulerEvents> {
        use client::RequestType::*;

        match request {
            Unary(request, tx) => {
                let task = self
                    .transport
                    .send(request)
                    .map(|res| tx.send(res.map_err(|_| Error::new_broken_pipe(None))))
                    .map(|_| SchedulerEvents::None);

                Box::pin(task)
            }
            Broadcast(request) => {
                let view = self.membership.view();

                let mut tasks = Vec::new();
                for endpoint in view {
                    let task = self
                        .transport
                        .send(Request::new(endpoint.clone(), request.clone()));

                    tasks.push(task);
                }

                Box::pin(future::join_all(tasks).map(|_| SchedulerEvents::None))
            }
        }
    }

    async fn join_attempt(&mut self, seed_addr: Target) -> Result<Response> {
        unimplemented!()
    }
}
