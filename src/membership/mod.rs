pub mod cut_detector;
pub mod ring;
pub mod view;

use crate::{
    common::{ConfigId, Endpoint, NodeId, Scheduler, SchedulerEvents},
    consensus::FastPaxos,
    error::Result,
    event::{Event, NodeStatusChange},
    monitor::Monitor,
    transport::{
        proto::{
            self, Alert, BatchedAlertMessage, EdgeStatus, JoinMessage, JoinResponse, JoinStatus,
            Metadata, PreJoinMessage, NodeStatus
        },
        Client, Request, Response,
    },
    metadata_manager::MetadataManager
};
use cut_detector::CutDetector;
use view::View;

use futures::FutureExt;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
    pin::Pin,
};
use tokio_sync::{mpsc, oneshot, watch};
use tracing::info;

type OutboundResponse = oneshot::Sender<crate::Result<Response>>;

#[derive(Debug)]
pub struct Membership<M> {
    host_addr: Endpoint,
    view: View,
    cut_detector: CutDetector,
    monitor: M,
    alerts: VecDeque<proto::Alert>,
    last_enqueued_alert: Instant,
    joiners_to_respond: HashMap<Endpoint, VecDeque<OutboundResponse>>,
    batch_window: Duration,
    paxos: FastPaxos,
    announced_proposal: bool,
    joiner_data: HashMap<Endpoint, (NodeId, Metadata)>,
    event_tx: mpsc::Sender<Event>,
    monitor_cancellers: Vec<oneshot::Sender<()>>,
}

impl<M: Monitor> Membership<M> {
    #[allow(dead_code)]
    pub fn new(
        host_addr: Endpoint,
        view: View,
        cut_detector: CutDetector,
        monitor: M,
        event_tx: mpsc::Sender<Event>,
        client: &Client,
    ) -> Self {
        // TODO: setup startup tasks

        let paxos = FastPaxos::new(
            host_addr,
            view.get_membership_size(),
            client.clone(),
            view.get_current_config_id(),
        );

        Self {
            host_addr,
            view,
            cut_detector,
            monitor,
            paxos,
            alerts: VecDeque::default(),
            last_enqueued_alert: Instant::now(),
            joiners_to_respond: HashMap::default(),
            batch_window: Duration::new(10, 0),
            announced_proposal: false,
            joiner_data: HashMap::default(),
            monitor_cancellers: vec![],
            event_tx,
        }
    }

    fn send_initial_notification(&self) {
        self.event_tx
            .send(Event::ViewChange(self.get_inititial_view_changes()));
    }

    fn get_inititial_view_changes(&self) -> Vec<NodeStatusChange> {
        let nodes = self.view.get_ring(0);

        nodes
            .iter()
            .map(|_| NodeStatusChange {
                endpoint: self.host_addr,
                status: EdgeStatus::Up,
                metadata: Metadata::default(),
            })
            .collect()
    }

    pub fn view(&self) -> Vec<&Endpoint> {
        self.view
            .get_ring(0)
            .expect("There is always a ring!")
            .iter()
            .collect()
    }

    pub async fn handle_message(
        &mut self,
        request: Request,
        response_tx: OutboundResponse,
        scheduler: &mut Scheduler,
    ) {
        use proto::RequestKind::*;
        let (_target, kind) = request.into_parts();

        match kind {
            PreJoin(msg) => {
                let res = self.handle_pre_join(msg).await;
                response_tx.send(res).unwrap();
            }
            Join(msg) => {
                self.handle_join(msg, response_tx).await;
            }
            BatchedAlert(msg) => {
                let res = self.handle_batched_alert_message(msg, scheduler).await;
                if let Err(_e) = res {
                    unimplemented!()
                }
            }
            Probe => {
                response_tx.send(Ok(self.handle_probe_message())).unwrap();
            }
            Consensus(msg) => {
                let res = self.paxos.handle_message(msg, scheduler).await;
                if let Err(_e) = res {
                    unimplemented!()
                }
            }
        };
    }

    pub async fn start_classic_round(&mut self) -> Result<()> {
        self.paxos.start_classic_round().await
    }

    pub async fn handle_pre_join(&mut self, msg: PreJoinMessage) -> Result<Response> {
        let PreJoinMessage {
            sender, node_id, ..
        } = msg;

        let status = self.view.is_safe_to_join(&sender, &node_id);
        let config_id = self.view.get_config().config_id();

        let endpoints =
            if status == JoinStatus::SafeToJoin || status == JoinStatus::HostnameAlreadyInRing {
                self.view.get_expected_observers(&sender)
            } else {
                Vec::new()
            };

        let join_res = JoinResponse {
            sender,
            status,
            config_id,
            endpoints,
            identifiers: Vec::new(),
            cluster_metadata: HashMap::new(),
        };

        info!(
            message = "Join at seed.",
            seed = %self.host_addr,
            sender = %join_res.sender,
            config = %join_res.config_id,
            size = %self.view.get_membership_size()
        );

        Ok(Response::new_join(join_res))
    }

    pub async fn handle_join(&mut self, msg: JoinMessage, response_tx: OutboundResponse) {
        let config = self.view.get_config();
        let current_config_id = config.config_id();

        if msg.config_id == current_config_id {
            self.joiners_to_respond
                .entry(msg.sender.clone())
                .or_insert_with(VecDeque::new)
                .push_back(response_tx);

            let alert = proto::Alert {
                src: self.host_addr.clone(),
                dst: msg.sender.clone(),
                edge_status: proto::EdgeStatus::Up,
                config_id: current_config_id,
                node_id: Some(msg.node_id.clone()),
                ring_number: msg.ring_number,
                metadata: None,
            };

            self.enqueue_alert(alert);
        } else {
            // This is the case where the config changed between phase 1
            // and phase 2 of the join process.
            let response = if self.view.is_host_present(&msg.sender)
                && self.view.is_node_id_present(&msg.node_id)
            {
                // Race condition where a observer already crossed H messages for the joiner and
                // changed the configuration, but the JoinPhase2 message shows up at the observer
                // after it has already added the joiner. In this case, simply tell the joiner it's
                // safe to join
                proto::JoinResponse {
                    sender: self.host_addr,
                    status: JoinStatus::SafeToJoin,
                    config_id: config.config_id(),
                    endpoints: config.endpoints.clone(),
                    identifiers: config.node_ids.clone(),
                    cluster_metadata: HashMap::new(),
                }
            } else {
                proto::JoinResponse {
                    sender: self.host_addr,
                    status: JoinStatus::ConfigChanged,
                    config_id: config.config_id(),
                    endpoints: vec![],
                    identifiers: vec![],
                    cluster_metadata: HashMap::new(),
                }
            };

            response_tx.send(Ok(Response::new(proto::ResponseKind::Join(response))));
        }
    }

    // Invoked by observers of a node for failure detection
    fn handle_probe_message(&self) -> Response {
        Response::new_probe(NodeStatus::Up) // TODO: FIXME THIS IS WRONG
    }

    // Receives edge update events and delivers them to the cut detector to check if it will
    // return a valid proposal.
    //
    // Edge update messages that do not affect the ongoing proposal need to be dropped.
    async fn handle_batched_alert_message(
        &mut self,
        msg_batch: BatchedAlertMessage,
        scheduler: &mut Scheduler,
    ) -> Result<()> {
        let current_config_id = self.view.get_current_config_id();
        let size = self.view.get_membership_size();
        let mut proposal: Vec<Endpoint> = msg_batch
            .alerts
            .iter()
            // filter out messages which violate membership invariants
            // And then run the cut detector to see if there is a new proposal
            .filter_map(|message| {
                if !self.filter_alert_messages(&msg_batch, message, size, &current_config_id) {
                    return None;
                }

                Some(self.cut_detector.aggregate(message))
            })
            .flatten()
            .collect();

        proposal.extend(self.cut_detector.invalidate_failing_edges(&mut self.view));

        if !proposal.is_empty() {
            self.announced_proposal = true;

            self.event_tx.send(Event::ViewChangeProposal(
                self.create_node_status_change_list(proposal),
            ));

            self.paxos.propose(proposal, scheduler).await?
        }

        Ok(())
    }

    fn create_node_status_change_list(&self, proposal: Vec<Endpoint>) -> Vec<NodeStatusChange> {
        proposal
            .iter()
            .map(|node| NodeStatusChange {
                endpoint: node.to_string(),
                status: if self.view.is_host_present(node) {
                    EdgeStatus::Down
                } else {
                    EdgeStatus::Up
                },
                metadata: Metadata::default(),
            })
            .collect()
    }

    // Filter for removing invalid edge update messages. These include messages
    // that were for a configuration that the current node is not a part of, and messages
    // that violate teh semantics of being a part of a configuration
    fn filter_alert_messages(
        &mut self,
        _message_batch: &BatchedAlertMessage, // Might require this later for loggign
        message: &Alert,
        _size: usize,
        config_id: &ConfigId,
    ) -> bool {
        let dst = &message.dst;

        if *config_id != message.config_id {
            return false;
        }

        // An invariant to maintain is that a node can only go into the membership set once
        // and leave it once
        if message.edge_status == EdgeStatus::Down && !self.view.is_host_present(&dst) {
            return false;
        }

        if message.edge_status == EdgeStatus::Up {
            // Add joiner data after the node is done being added to the set. Store in a
            // temp location for now.
            self.joiner_data.insert(
                dst.clone(),
                (
                    message.node_id.clone().take().unwrap(),
                    message.metadata.clone().take().unwrap(),
                ),
            );
        }

        true
    }

    pub fn create_failure_detectors(
        &mut self,
        scheduler: &mut Scheduler,
        client: &Client,
    ) -> Result<mpsc::Receiver<(Endpoint, ConfigId)>> {
        let (tx, rx) = mpsc::channel(1000);

        for subject in self.view.get_subjects(&self.host_addr)? {
            let (mon_tx, mon_rx) = oneshot::channel();

            let fut = self.monitor.monitor(
                subject.clone(),
                client.clone(),
                self.view.get_current_config_id(),
                tx.clone(),
                mon_rx,
            );
            scheduler.push(Pin::new(Box::new(fut.map(|_| SchedulerEvents::None))));

            self.monitor_cancellers.push(mon_tx);
        }

        Ok(rx)
    }

    pub fn edge_failure_notification(&mut self, subject: Endpoint, config_id: ConfigId) {
        if config_id != self.view.get_current_config_id() {
            // TODO: Figure out why &String does not impl Value
            // info!(
            //     target: "Failure notification from old config.",
            //     subject = subject,
            //     config = self.view.get_current_config_id(),
            //     old_config = config_id
            // );
            return;
        }

        let alert = proto::Alert {
            src: self.host_addr.clone(),
            dst: subject,
            edge_status: proto::EdgeStatus::Down,
            config_id,
            node_id: None,
            ring_number: self.view.get_ring_numbers(&self.host_addr, &subject),
            metadata: None,
        };

        self.enqueue_alert(alert);
    }

    pub fn get_batch_alerts(&mut self) -> Option<proto::BatchedAlertMessage> {
        if !self.alerts.is_empty()
            && (Instant::now() - self.last_enqueued_alert) > self.batch_window
        {
            let alerts = self.alerts.drain(..).collect();

            Some(proto::BatchedAlertMessage {
                sender: self.host_addr.clone(),
                alerts,
            })
        } else {
            None
        }
    }

    pub fn enqueue_alert(&mut self, alert: proto::Alert) {
        self.last_enqueued_alert = Instant::now();
        self.alerts.push_back(alert);
    }

    /// This is invoked when the consensus module decides on a proposal
    ///
    /// Any node that is not in the membership list will be added to the cluster,
    /// and any node that is currently in the membership list, but not in the proposal
    /// will be removed.
    pub async fn on_decide(&mut self, proposal: Vec<Endpoint>) -> Result<()> {
        // TODO: Handle metadata updates
        // TODO: Handle subscriptions

        self.cancel_failure_detectors();

        for node in &proposal {
            if self.view.is_host_present(&node) {
                self.view.ring_delete(&node)?;
            } else {
                if let Some((node_id, _metadata)) = self.joiner_data.remove(node) {
                    self.view.ring_add(node.clone(), node_id)?;
                } else {
                    panic!("Node not present in pre-join metadata")
                }
            }
        }

        let _current_config_id = self.view.get_current_config_id();

        // clear data structures
        self.cut_detector.clear();

        self.announced_proposal = false;

        if self.view.is_host_present(&self.host_addr) {
            // TODO: inform edge failure detector about config change
        } else {
            // We need to gracefully exit by calling a user handler and invalidating the current
            // session
            unimplemented!()
        }

        // TODO: Instantiate new consensus instance
        // self.paxos = FastPaxos::new(self.host_addr, self.view.get_membership_size(), )

        self.respond_to_joiners(proposal);

        Ok(())
    }

    fn cancel_failure_detectors(&mut self) {
        self.monitor_cancellers.iter().for_each(|tx| tx.send(()).unwrap());
    }

    fn respond_to_joiners(&mut self, proposal: Vec<Endpoint>) {
        let configuration = self.view.get_config();

        let join_res = JoinResponse {
            sender: self.host_addr.clone(),
            status: JoinStatus::SafeToJoin,
            config_id: configuration.config_id(),
            endpoints: configuration.endpoints.clone(),
            identifiers: configuration.node_ids.clone(),
            cluster_metadata: HashMap::new(), // TODO: metadata manager
        };

        for node in proposal {
            self.joiners_to_respond.remove(&node).and_then(|joiners| {
                joiners.into_iter().for_each(|joiner| {
                    joiner
                        .send(Ok(Response::new_join(join_res.clone())))
                        .expect("Unable to send response");
                });

                // This is so the compiler can infer the type of the closure to be Option<()>
                Some(())
            });
        }
    }
}
