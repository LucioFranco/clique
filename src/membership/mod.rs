mod ring;
mod view;

use crate::{
    common::{Endpoint, Scheduler, SchedulerEvents},
    consensus::FastPaxos,
    error::Result,
    monitor::Monitor,
    transport::{
        proto::{self, JoinMessage, JoinResponse, JoinStatus, PreJoinMessage},
        Client, Request, Response,
    },
};
use futures::FutureExt;
use std::collections::{HashMap, VecDeque};
use tracing::info;
use view::View;

#[derive(Debug)]
pub struct Membership<M> {
    host_addr: Endpoint,
    view: View,
    monitor: M,
    alerts: VecDeque<()>,
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
        _scheduler: &mut Scheduler,
    ) -> Result<Response> {
        use proto::RequestKind::*;
        let (_target, kind) = request.into_parts();

        let response = match kind {
            PreJoin(msg) => self.handle_pre_join(msg).await?,
            Join(msg) => self.handle_join(msg).await?,
            Consensus(msg) => self.paxos.handle_message(msg).await?,
            _ => unimplemented!(),
        };

        Ok(response)
    }

    pub async fn start_classic_round(&mut self) -> Result<()> {
        self.paxos.start_classic_round().await
    }

    pub async fn handle_pre_join(&mut self, msg: PreJoinMessage) -> Result<Response> {
        let PreJoinMessage {
            sender,
            node_id,
            // ring_number: _,
            // config_id: _,
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

    #[allow(unreachable_code, unused_variables)]
    pub async fn handle_join(&mut self, msg: JoinMessage) -> Result<Response> {
        let current_config_id = self.view.get_config().config_id();

        if msg.config_id == current_config_id {
            // TODO: This is the case where we got a join message and are int he same config
            // as the PreJoin response was created in. This means we can attempt to propose this
            // node.

            // TODO: setup alertmessage and enqueue it. The edge is up!

            unimplemented!()
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

            Ok(Response::new_join(response))
        }
    }

    pub fn create_failure_detectors(
        &mut self,
        scheduler: &mut Scheduler,
        _client: Client,
    ) -> Result<()> {
        for subject in self.view.get_subjects(&self.host_addr)? {
            let (tx, _rx) = tokio_sync::mpsc::channel(100);
            let fut = self.monitor.monitor(subject.clone(), tx);
            scheduler.push(Box::pin(fut.map(|_| SchedulerEvents::None)));
        }

        Ok(())
    }

    pub fn drain_alerts(&mut self) -> Vec<()> {
        self.alerts.drain(..).take(5).collect()
    }
}
