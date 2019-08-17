use crate::{
    builder::Builder,
    common::{Endpoint, NodeId, Scheduler, SchedulerEvents},
    error::{Error, Result},
    event::Event,
    handle::Handle,
    membership::Membership,
    monitor::ping_pong,
    transport::{client, proto, Client, Request, Response, Transport},
};
use futures::{
    future::{self, BoxFuture},
    stream::Fuse,
    FutureExt, StreamExt,
};
use std::{collections::HashMap, time::Duration};
use tokio_sync::{oneshot, watch};
use tokio_timer::Interval;

pub struct Cluster<T, Target>
where
    T: Transport<Target>,
{
    handle: Handle,
    inner: State<T, Target>,
}

#[allow(dead_code)]
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
    Target: Into<Endpoint> + Send + Clone,
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
    #[allow(dead_code)]
    listen_target: Target,
    #[allow(dead_code)]
    event_tx: watch::Sender<Event>,

    endpoint: Endpoint,
    node_id: NodeId,

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
    Target: Into<Endpoint> + Send + Clone,
{
    #![allow(dead_code)]
    async fn new(mut transport: T, listen_target: Target, event_tx: watch::Sender<Event>) -> Self {
        let server_stream = transport.listen_on(listen_target.clone()).await.unwrap();

        let (client, client_stream) = Client::new(100);

        let endpoint = listen_target.clone().into();
        let node_id = NodeId::new();

        Self {
            membership: Membership::new(),
            transport,
            listen_target,
            event_tx,
            endpoint,
            node_id,
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
        for _ in 0..10 {
            match self.join_attempt(seed_addr.into()).await {
                _ => panic!(),
            }
        }
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        let mut edge_failure_notifications_rx = self
            .membership
            .create_failure_detectors(&mut self.scheduler, &self.client)?
            .fuse();

        let mut alert_batcher_interval = Interval::new_interval(Duration::from_millis(100)).fuse();

        loop {
            futures::select! {
                request = self.server_stream.select_next_some() => {
                    self.handle_server(request).await?;
                },
                event = self.scheduler.select_next_some() => {
                    match event {
                        SchedulerEvents::StartClassicRound => {
                            self.membership.start_classic_round().await?;
                            continue;
                        },
                        SchedulerEvents::Decision(proposal) => {
                            self.membership.on_decide(proposal).await?;
                            continue;
                        }
                        SchedulerEvents::None => continue,
                        _ => unimplemented!()
                    }
                },
                request = self.client_stream.select_next_some() => {
                    let task = self.handle_client(request);
                    self.scheduler.push(task);
                },
                (subject, config_id) = edge_failure_notifications_rx.select_next_some() => {
                    self.membership.edge_failure_notification(subject, config_id);
                },
                _ = alert_batcher_interval.select_next_some() => {
                    if let Some(msg) = self.membership.get_batch_alerts() {
                        let req = proto::RequestKind::BatchedAlert(msg);
                        self.client.broadcast(req).await?;
                    }
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
                self.membership
                    .handle_message(request, response_tx, &mut self.scheduler)
                    .await;

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

    async fn join_attempt(&mut self, seed_addr: Endpoint) -> Result<()> {
        let req = proto::RequestKind::PreJoin(proto::PreJoinMessage {
            sender: self.endpoint.clone(),
            node_id: self.node_id.clone(),
        });

        let join_res = match self
            .transport
            .send(Request::new(seed_addr, req))
            .await
            .map_err(|e| Error::new_broken_pipe(Some(Box::new(e))))?
            .into_inner()
        {
            proto::ResponseKind::Join(res) => res,
            _ => unimplemented!("wrong request type"),
        };

        if join_res.status != proto::JoinStatus::SafeToJoin
            && join_res.status != proto::JoinStatus::HostnameAlreadyInRing
        {
            unimplemented!("JoinPhaseOneException");
        }

        let config_to_join = if join_res.status == proto::JoinStatus::HostnameAlreadyInRing {
            -1
        } else {
            join_res.config_id
        };

        let res = self
            .send_join_phase2(join_res)
            // TODO: probably want to make this a stream
            .await?
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(|r| {
                if let proto::ResponseKind::Join(res) = r.into_inner() {
                    Some(res)
                } else {
                    None
                }
            })
            .filter(|r| r.status == proto::JoinStatus::SafeToJoin)
            .filter(|r| r.config_id != config_to_join)
            .take(1)
            .next();

        if let Some(join) = res {
            self.cluster_from_join(join).await
        } else {
            Err(Error::new_join_phase2())
        }
    }

    async fn send_join_phase2(
        &mut self,
        join_res: proto::JoinResponse,
    ) -> Result<Vec<Result<Response>>> {
        let mut ring_num_per_obs = HashMap::new();

        let mut ring_num = 0;
        for obs in &join_res.endpoints {
            ring_num_per_obs
                .entry(obs)
                .or_insert_with(Vec::new)
                .push(ring_num);
            ring_num += 1;
        }

        let mut in_flight_futs = Vec::new();

        for (endpoint, ring_nums) in ring_num_per_obs {
            let join = proto::RequestKind::Join(proto::JoinMessage {
                sender: self.endpoint.clone(),
                node_id: self.node_id.clone(),
                ring_number: ring_nums,
                config_id: join_res.config_id,
            });

            let fut = self.transport.send(Request::new(endpoint.clone(), join));
            in_flight_futs.push(fut);
        }

        let responses = future::join_all(in_flight_futs)
            .await
            .into_iter()
            .map(|r| match r {
                Ok(r) => Ok(r),
                Err(e) => Err(Error::new_broken_pipe(Some(Box::new(e)))),
            })
            .collect();

        Ok(responses)
    }

    async fn cluster_from_join(&mut self, _join_res: proto::JoinResponse) -> Result<()> {
        unimplemented!()
    }
}
