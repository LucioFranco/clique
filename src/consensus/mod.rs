use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use futures::{Future, FutureExt, Stream, StreamExt};
use tokio_sync::oneshot;
use tokio_timer::Delay;

mod paxos;

use paxos::Paxos;

use crate::{
    common::Endpoint,
    error::{Error, Result},
    transport::{proto, Broadcast, Client, Request, Response},
};

pub struct FastPaxos<'a, C, B> {
    broadcast: &'a mut B,
    my_addr: Endpoint,
    size: usize,
    paxos: Paxos<'a, C>,
    decided: AtomicBool,
    /// Channel used to communicate with upstream about decision
    decision_tx: oneshot::Sender<Vec<Endpoint>>,
    /// Channel the paxos instance will use to communicate with us about decision
    // paxos_rx: oneshot::Receiver<Vec<Endpoint>>,
    scheduled_paxos: Option<Box<Future<Output = Result<()>>>>,
}

impl<'a, C, B> FastPaxos<'a, C, B> {
    pub fn new(
        my_addr: Endpoint,
        size: usize,
        client: &'a mut C,
        broadcast: &'a mut B,
        decision_tx: oneshot::Sender<Vec<Endpoint>>,
        proposal_rx: oneshot::Receiver<Vec<Endpoint>>,
    ) -> FastPaxos<'a, C, B>
    where
        C: Client,
        B: Broadcast,
    {
        FastPaxos {
            my_addr: my_addr.clone(),
            broadcast,
            size,
            decision_tx,
            paxos: Paxos::new(client, size, my_addr),
            decided: AtomicBool::new(false),
        }
    }

    pub async fn propose(&self, proposal: Vec<Endpoint>) {
        let mut paxos_delay = 0;
        // TODO: register proposal with regular paxos instance
        unimplemented!();
    }

    pub async fn handle_message(&self, request: Request) -> Result<Response> {
        match request.kind() {
            proto::RequestKind::Consensus(_) => self.handle_fast_round(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_1a(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_1b(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_2a(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_2b(request).await,
            _ => Err(Error::new_unexpected_request(None)),
        }
    }

    async fn handle_fast_round(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }

    async fn on_decide(&mut self, hosts: Vec<Endpoint>) {
        // This is the only place where the value is set to `true`.
        self.decided.store(true, Ordering::SeqCst);

        if let Some(paxos_instance) = self.scheduled_paxos {
            drop(paxos_instance);
        }

        self.decision_tx.send(hosts).await;
    }

    fn get_random_delay(&self) -> Duration {}
}
