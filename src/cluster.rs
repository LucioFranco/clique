use crate::{
    common::{Endpoint, NodeId, Scheduler, SchedulerEvents},
    error::{Error, Result},
    event::Event,
    membership::{cut_detector::CutDetector, view::View, Membership},
    monitor::ping_pong,
    transport::{proto, Message, Request, Response, Transport2},
};
use futures::{
    future::{self, BoxFuture, FutureExt},
    stream::{Fuse, StreamExt},
};
use std::time::Duration;
use tokio::{
    sync::{broadcast, mpsc, oneshot},
    time::interval,
};

const K: usize = 10;
const H: usize = 9;
const L: usize = 4;

type Handle = broadcast::Receiver<Event>;

// pub struct Cluster<T, Target>
// where
//     T: Transport<Target>,
// {
//     handle: broadcast::Sender<Event>,
//     inner: Inner<T, Target>,
// }

// impl<T, Target> Cluster<T, Target>
// where
//     T: Transport<Target> + Send,
//     Target: Into<Endpoint> + Send + Clone,
// {
//     pub(crate) fn new(handle: broadcast::Sender<Event>, inner: Inner<T, Target>) -> Self {
//         Cluster { handle, inner }
//     }

//     pub fn builder() -> Builder<T, Target> {
//         Builder::new()
//     }

//     pub fn handle(&self) -> Handle {
//         self.handle.subscribe()
//     }

//     pub async fn start(&mut self) -> Result<()> {
//         // self.inner.start().await
//         todo!()
//     }

//     pub async fn join(&mut self, seed_addr: Target) -> Result<()> {
//         // self.inner.join(seed_addr).await
//         todo!()
//     }
// }

pub struct Cluster<T> {
    membership: Membership<ping_pong::PingPong>,
    transport: T,
    listen_target: Endpoint,
    event_tx: broadcast::Sender<Event>,
    endpoint: Endpoint,
    node_id: NodeId,
    scheduler: Scheduler,
    // client: Client,
    // client_stream: client::RequestStream,
    // server_stream: Fuse<T::ServerStream>,
}

impl<T> Cluster<T>
where
    T: Transport2 + Send,
{
    #![allow(dead_code)]
    pub(crate) async fn new(
        mut transport: T,
        listen_target: Endpoint,
        event_tx: broadcast::Sender<Event>,
    ) -> Self {
        // let server_stream = transport
        //     .listen_on(listen_target.clone())
        //     .await
        //     .unwrap_or_else(|e| panic!("Unable to start server: {:?}", e));

        // let (client, client_stream) = Client::new(100);

        // let endpoint = listen_target.clone().into();
        // let node_id = NodeId::new();

        // Self {
        //     // membership is instantiated either by a join attempt or on start
        //     membership: None,
        //     transport,
        //     listen_target,
        //     event_tx,
        //     endpoint,
        //     node_id,
        //     client,
        //     client_stream,
        //     scheduler: Scheduler::new(),
        //     server_stream: server_stream.fuse(),
        // }
        todo!()
    }

    pub async fn start(transport: T, listen_target: Endpoint) {
        let node_id = NodeId::default();
        let node_ids = vec![node_id.clone()];
        let endpoints = vec![listen_target.clone()];

        let mut c = Cluster::new2(node_id, node_ids, endpoints, transport, listen_target);
        c.run().await.unwrap()
    }

    pub(crate) fn new2(
        node_id: NodeId,
        node_ids: Vec<NodeId>,
        endpoints: Vec<Endpoint>,
        transport: T,
        listen_target: Endpoint,
    ) -> Self {
        let listen_addr = listen_target.clone();
        let (event_tx, event_rx) = broadcast::channel(1024);

        let view = View::bootstrap(K as i32, vec![node_id.clone()], vec![listen_addr.clone()]);
        let cut_detector = CutDetector::new(K, H, L);
        let monitor = ping_pong::PingPong::new(Duration::from_secs(10), Duration::from_secs(10));

        let membership =
            Membership::new(listen_addr, view, cut_detector, monitor, event_tx.clone());

        let endpoint = listen_target.clone().into();

        Self {
            membership,
            transport,
            listen_target,
            event_tx,
            endpoint,
            node_id,
            scheduler: Scheduler::new(),
        }
    }

    pub(crate) fn from_join(
        listen_target: Endpoint,
        join_res: proto::JoinResponse,
    ) -> Result<Self> {
        // Safe to proceed. Extract the list of endpoints and identifiers from the message,
        // assemble to MemberShipService object and start and RPCServer
        let endpoints = join_res.endpoints;
        let node_ids = join_res.identifiers;
        let _metadata = join_res.cluster_metadata;

        debug_assert!(!endpoints.is_empty());
        debug_assert!(!node_ids.is_empty());

        //    Self::new(join_res)
        todo!()
    }

    // pub async fn start(&mut self) -> Result<()> {
    //     let node_id = NodeId::new();

    //     // self.membership = Some(membership);

    //     self.run().await
    // }

    // pub async fn join(&mut self, seed_addr: Target) -> Result<()> {
    //     for i in 0usize..10usize {
    //         tracing::debug!(message = "joining.", attempt = i);
    //         if let Ok(()) = self.join_attempt(seed_addr.clone().into()).await {
    //             return Ok(());
    //         }
    //     }

    //     Err(Error::new_join_phase2())
    // }

    async fn run(&mut self) -> Result<()> {
        tracing::debug!("running.");

        // TODO: re-enable this
        // let mut _edge_failure_notifications_rx = self
        //     .membership
        //     .create_failure_detectors(&mut self.scheduler, &self.client)?;

        let mut alert_batcher_interval = interval(Duration::from_millis(100)).fuse();

        // let mut server_stream = self.transport.listen_

        loop {
            futures::select! {
                res = self.transport.recv().fuse() => {
                    match res {
                        Ok((from, Message::Request(rk))) => {
                            self.membership.step(from, rk);
                        }
                        _ => todo!("ooops")
                    }

                    self.send_messages();
                },
                event = self.scheduler.select_next_some() => {
                    match event {
                        SchedulerEvents::StartClassicRound => {
                            self.membership.start_classic_round()?;
                            continue;
                        },
                        SchedulerEvents::Decision(proposal) => {
                            self.membership.on_decide(proposal);
                            continue;
                        }
                        SchedulerEvents::None => continue,
                        _ => panic!("An unknown event type found. This cannot happen.")
                    }
                },
                // request = self.client_stream.select_next_some() => {
                //     let task = self.handle_client(request);
                //     self.scheduler.push(task);
                // },
                // res = edge_failure_notifications_rx.recv().fuse() => {
                //     let (subject, config_id) = res.unwrap();
                //     mem.edge_failure_notification(subject, config_id);
                // },
                // _ = alert_batcher_interval.select_next_some() => {
                //     if let Some(msg) = self.membership.get_batch_alerts() {
                //         let req = proto::RequestKind::BatchedAlert(msg);
                //         self.client.broadcast(req).await?;
                //     }
                // },
            };
        }
    }

    fn send_messages(&mut self) {
        for (to, msg) in self.membership.drain_messages() {
            self.transport.send_to(to, msg);
        }
    }

    // Need this to be a boxed futures since we want to send these tasks into
    // the FuturesUnordered and we require that the lifetime of the future
    // is static due to the box. It seems that the compiler can't infer the
    // lifetime from the borrow that is calling handle_client if it were an
    // async fn.
    // fn handle_client(
    //     &mut self,
    //     request: client::RequestType,
    // ) -> BoxFuture<'static, SchedulerEvents> {
    //     use client::RequestType::*;

    //     match request {
    //         Unary(request, tx) => {
    //             let task = self
    //                 .transport
    //                 .send(request)
    //                 .map(|res| tx.send(res.map_err(|_| Error::new_broken_pipe(None))))
    //                 .map(|_| SchedulerEvents::None);

    //             Box::pin(task)
    //         }
    //         Broadcast(request) => {
    //             // get all the members in the current config
    //             let view = self.membership.view();

    //             let mut tasks = Vec::new();
    //             for endpoint in view {
    //                 let task = self
    //                     .transport
    //                     .send(Request::new(endpoint.clone(), request.clone()));

    //                 tasks.push(task);
    //             }

    //             Box::pin(future::join_all(tasks).map(|_| SchedulerEvents::None))
    //         }
    //     }
    // }
}
