use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use futures::{stream::FuturesUnordered, Future, FutureExt, Stream, StreamExt};
use rand::Rng;
use tokio_sync::oneshot;
use tokio_timer::Delay;

mod paxos;

use paxos::Paxos;

use crate::{
    common::Endpoint,
    error::{Error, Result},
    transport::{
        proto::{self, ConfigId, Endpoint, Phase2bMessage},
        Broadcast, Client, Request, Response,
    },
};

const BASE_DELAY: u64 = 1000;

#[derive(Debug)]
enum CancelPaxos {}

pub struct FastPaxos<'a, C, B> {
    broadcast: &'a mut B,
    my_addr: Endpoint,
    size: usize,
    decided: AtomicBool,
    /// Channel used to communicate with upstream about decision
    decision_tx: oneshot::Sender<Vec<Endpoint>>,
    /// Channel the paxos instance will use to communicate with us about decision
    // paxos_rx: oneshot::Receiver<Vec<Endpoint>>,
    paxos: Option<Paxos<'a, C>>,
    /// Latest configuration we accepted.
    config_id: ConfigId,
    votes_received: HashSet<Endpoint>, // should be a bitset?
    votes_per_proposal: HashMap<Vec<Endpoint>, AtomicUsize>,
}

impl<'a, C, B> FastPaxos<'a, C, B> {
    pub fn new(
        my_addr: Endpoint,
        size: usize,
        client: &'a mut C,
        broadcast: &'a mut B,
        decision_tx: oneshot::Sender<Vec<Endpoint>>,
        proposal_rx: oneshot::Receiver<Vec<Endpoint>>,
        config_id: ConfigId,
    ) -> FastPaxos<'a, C, B>
    where
        C: Client,
        B: Broadcast,
    {
        FastPaxos {
            broadcast,
            decision_tx,
            config_id,
            size: size,
            my_addr: my_addr.clone(),
            decided: AtomicBool::new(false),
            paxos: Some(Paxos::new(client, my_addr, config_id)),
            votes_received: HashSet::default(),
            votes_per_proposal: HashMap::default(),
        }
    }

    pub async fn propose(&self, proposal: Vec<Endpoint>) -> Result<()> {
        let mut paxos_delay = Delay::new(Instant::now() + self.get_random_delay()).fuse();

        // a hacky way to schedule a task after the delay
        async {
            paxos_delay.await;
            self.start_classic_round().await;
        }
            .await;

        let (tx, rx) = oneshot::channel();

        let kind = proto::RequestKind::Consensus(proto::Consensus::FastRoundPhase2bMessage(
            proto::FastRoundPhase2bMessage {
                sender: self.my_addr,
                config_id: self.config_id,
                endpoints: proposal,
            },
        ));

        let request = Request::new(tx, kind);

        self.broadcast.broadcast(request).await;
        rx.await;

        // TODO: better error type
        Ok(())
    }

    pub async fn handle_message(&self, request: Request) -> Result<()> {
        match request.kind() {
            proto::RequestKind::Consensus(req_type) => {
                if let Some(paxos) = self.paxos {
                    match req_type {
                        proto::Consensus::FastRoundPhase2bMessage(msg) => {
                            self.handle_fast_round(msg).await
                        }
                        // TODO: need to figure out communication b/w paxos and fast paxos
                        // proto::Consensus::Phase1aMessage(_) => paxos.handle_phase_1a(request).await,
                        // proto::Consensus::Phase1bMessage(_) => paxos.handle_phase_1b(request).await,
                        // proto::Consensus::Phase2aMessage(_) => paxos.handle_phase_2a(request).await,
                        // proto::Consensus::Phase2bMessage(_) => paxos.handle_phase_2b(request).await,
                    }
                } else {
                    Err(Error::new_unexpected_request(None))
                }
            }
            _ => Err(Error::new_unexpected_request(None)),
        }
    }

    async fn handle_fast_round(
        &self,
        request: &proto::FastRoundPhase2bMessage,
    ) -> crate::Result<()> {
        if request.config_id != self.config_id {
            return Ok(());
        }

        if self.votes_received.contains(&request.sender) {
            return Ok(());
        }

        if (self.decided.load(Ordering::SeqCst)) {
            return Ok(());
        }

        self.votes_received.insert(request.sender.clone());

        let count = self
            .votes_per_proposal
            .entry(request.endpoints)
            .and_modify(|votes| {
                votes.fetch_add(1, Ordering::SeqCst);
            })
            .or_insert(AtomicUsize::new(0));

        let F = ((self.size - 1) as f64 / 4f64).floor();

        if (self.votes_received.len() >= (self.size as f64 - F) as usize) {
            if (AtomicUsize(count) >= (self.size as f64 - F) as usize) {
                self.on_decide(request.endpoints);
            }
        }
        Ok(())
    }

    async fn on_decide(self, hosts: Vec<Endpoint>) -> Result<()> {
        // This is the only place where the value is set to `true`.
        self.decided.store(true, Ordering::SeqCst);

        let paxos_instance = self.paxos.take();

        if let Some(instance) = paxos_instance {
            drop(instance);
        }

        self.decision_tx
            .send(hosts)
            .map_err(|_| Error::new_broken_pipe(None))
    }
    async fn start_classic_round(&mut self) -> Result<()> {
        if !self.decided.load(Ordering::SeqCst) {
            if let Some(paxos) = self.paxos {
                paxos.start_round().await
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn get_random_delay(&self) -> Duration {
        let jitter_rate: f64 = 1f64 / self.size as f64;
        let mut rng = rand::thread_rng();

        let jitter = ((-1000f64 * (1.0f64 - rng.gen::<f64>()).ln()) / jitter_rate) as u64;

        Duration::from_millis(jitter + BASE_DELAY)
    }
}
