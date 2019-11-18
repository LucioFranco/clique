use crate::{
    builder::Builder,
    common::{Endpoint, NodeId, Scheduler, SchedulerEvents},
    error::{Error, Result},
    event::Event,
    handle::Handle,
    membership::{cut_detector::CutDetector, view::View, Membership},
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

const K: usize = 10;
const H: usize = 9;
const L: usize = 4;

pub struct Cluster<T, Target>
where
    T: Transport<Target>,
{
    handle: Handle,
    inner: Inner<T, Target>,
}

impl<T, Target> Cluster<T, Target>
where
    T: Transport<Target> + Send,
    Target: Into<Endpoint> + Send + Clone,
{
    pub(crate) fn new(handle: Handle, inner: Inner<T, Target>) -> Self {
        Cluster { handle, inner }
    }
    pub fn builder() -> Builder<T, Target> {
        Builder::new()
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    pub async fn start(&mut self) -> Result<()> {
        self.inner.start().await
    }

    pub async fn join(&mut self, seed_addr: Target) -> Result<()> {
        self.inner.join(seed_addr).await
    }
}

pub(crate) struct Inner<T, Target>
where
    T: Transport<Target>,
{
    membership: Option<Membership<ping_pong::PingPong>>,
    transport: T,
    listen_target: Target,
    event_tx: watch::Sender<Event>,
    endpoint: Endpoint,
    node_id: NodeId,
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
    pub(crate) async fn new(
        mut transport: T,
        listen_target: Target,
        event_tx: watch::Sender<Event>,
    ) -> Self {
        let server_stream = transport
            .listen_on(listen_target.clone())
            .await
            .unwrap_or_else(|e| panic!("Unable to start server: {:?}", e));

        let (client, client_stream) = Client::new(100);

        let endpoint = listen_target.clone().into();
        let node_id = NodeId::new();

        Self {
            // membership is instantiated either by a join attempt or on start
            membership: None,
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
        let node_id = NodeId::new();
        let listen_addr = self.listen_target.clone().into();

        let view = View::bootstrap(K as i32, vec![node_id], vec![listen_addr]);
        let cut_detector = CutDetector::new(K, H, L);
        let monitor = monitor::PingPong::new(Duration::new(10), Duration::new(10));

        let membership = membership::Membership::new(
            listen_addr,
            view,
            cut_detector,
            monitor,
            self.current_config_id,
            paxos,
            self.event_tx.clone(),
        );

        self.membership = Some(membership);

        self.run().await
    }

    pub async fn join(&mut self, seed_addr: Target) -> Result<()> {
        for _ in 0..10 {
            return self.join_attempt(seed_addr.into()).await?;
        }

        Err(Error::new_join_phase2())
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
                    self.handle_server(request).await;
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
                        _ => panic!("An unknown event type found. This cannot happen.")
                    }
                },
                request = self.client_stream.select_next_some() => {
                    let task = self.handle_client(request);
                    self.scheduler.push(task);
                },
                (subject, config_id) = edge_failure_notifications_rx.select_next_some() => {
                    if let Some(m) = self.membership {
                        m.edge_failure_notification(subject, config_id);
                    }
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
        request: (Request, oneshot::Sender<crate::Result<Response>>),
    ) {
        let (request, response_tx) = request;
        if let Some(m) = self.membership {
            self.membership
                .handle_message(request, response_tx, &mut self.scheduler)
                .await;
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
            ring_number: vec![],
            config_id: None,
        });

        let join_res = match self
            .transport
            .send(Request::new(seed_addr, req))
            .await
            .map_err(|e| Error::new_broken_pipe(Some(Box::new(e))))?
            .into_inner()
        {
            proto::ResponseKind::Join(res) => res,
            _ => Error::new_join_phase1(),
        };

        if join_res.status != proto::JoinStatus::SafeToJoin
            && join_res.status != proto::JoinStatus::HostnameAlreadyInRing
        {
            return Error::new_join_phase1();
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
                // TODO: add metadata to the cluster
                metadata: None,
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

    async fn cluster_from_join(&mut self, join_res: proto::JoinResponse) -> Result<()> {
        // Safe to proceed. Extract the list of endpoints and identifiers from the message,
        // assemble to MemberShipService object and start and RPCServer
        let endpoints = join_res.endpoints;
        let node_ids = join_res.identifiers;
        let metadata = join_res.cluster_metadata;

        debug_assert!(endpoints.len() > 0);
        debug_assert!(node_ids.len() > 0);

        let view = View::bootstrap(K as i32, node_ids, endpoints);
        let cut_detector = CutDetector::new(K, H, L);
        let monitor = monitor::PingPong::new(Duration::new(10), Duration::new(10));

        let membership = membership::Membership::new(
            self.listen_target.clone().into(),
            view,
            cut_detector,
            monitor,
            self.current_config_id,
            paxos,
            self.event_tx.clone(),
        );

        self.membership = Some(membership);

        Ok(())
    }
}
