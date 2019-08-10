mod cut_detector;
mod ring;
mod view;

use crate::{
    common::{ConfigId, Endpoint, Scheduler, SchedulerEvents},
    consensus::FastPaxos,
    error::Result,
    monitor::Monitor,
    transport::{
        proto::{self, JoinMessage, JoinResponse, JoinStatus, PreJoinMessage},
        Client, Request, Response,
    },
};
use futures::FutureExt;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};
use tokio_sync::{mpsc, oneshot};
use tracing::info;
use view::View;

type OutboundResponse = oneshot::Sender<crate::Result<Response>>;

#[derive(Debug)]
pub struct Membership<M> {
    host_addr: Endpoint,
    view: View,
    monitor: M,
    alerts: VecDeque<proto::Alert>,
    last_enqueued_alert: Instant,
    joiners_to_respond: HashMap<Endpoint, VecDeque<OutboundResponse>>,
    current_config_id: ConfigId,
    batch_window: Duration,
    paxos: FastPaxos,
}

impl<M: Monitor> Membership<M> {
    #[allow(dead_code)]
    pub fn new() -> Self {
        unimplemented!()
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
        _scheduler: &mut Scheduler,
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
            Consensus(msg) => {
                let res = self.paxos.handle_message(msg).await;
                response_tx.send(res).unwrap();
            }
            _ => unimplemented!(),
        };
    }

    pub async fn start_classic_round(&mut self) -> Result<()> {
        self.paxos.start_classic_round().await
    }

    pub async fn handle_pre_join(&mut self, msg: PreJoinMessage) -> Result<Response> {
        let PreJoinMessage { sender, node_id } = msg;

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

    #[allow(unreachable_code, unused_variables)]
    pub async fn handle_join(&mut self, msg: JoinMessage, response_tx: OutboundResponse) {
        let current_config_id = self.view.get_config().config_id();

        if msg.config_id == current_config_id {
            self.joiners_to_respond
                .entry(msg.sender.clone())
                .or_insert(VecDeque::new())
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
                // TODO: joining host is already present so return:
                // `SafeToJoin`, current endpoints, and ids.
                unimplemented!()
            } else {
                // TODO: Wrong config, return `CONFIG_CHANGE`
                unimplemented!()
            };
        }
    }

    pub fn create_failure_detectors(
        &mut self,
        scheduler: &mut Scheduler,
        client: &Client,
    ) -> Result<mpsc::Receiver<(Endpoint, ConfigId)>> {
        let (tx, rx) = mpsc::channel(1000);

        for subject in self.view.get_subjects(&self.host_addr)? {
            let fut = self.monitor.monitor(
                subject.clone(),
                client.clone(),
                self.current_config_id,
                tx.clone(),
            );
            scheduler.push(Box::pin(fut.map(|_| SchedulerEvents::None)));
        }

        Ok(rx)
    }

    pub fn edge_failure_notification(&mut self, _subject: Endpoint, _config_id: ConfigId) {
        // TODO: enqueue a new batch alert with this subject in the EdgeStatus::Down state≤
        unimplemented!()
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
}
