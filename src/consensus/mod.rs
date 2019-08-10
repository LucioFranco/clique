mod paxos;

use crate::{
    common::{ConfigId, Endpoint, Scheduler, SchedulerEvents},
    error::{Error, Result},
    transport::{
        proto::{self, Consensus, Consensus::*},
        Client, Response,
    },
};
use futures::FutureExt;
use rand::Rng;
use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tokio_sync::oneshot;
use tokio_timer::Delay;

use paxos::Paxos;

const BASE_DELAY: u64 = 1000;

/// Represents an instance of the Fast Paxos consensus protocol.
///
/// This protocol has a fast path as compared to paxos, when a majority set agrees on the same
/// membershipset, each node individually accepts the value without the involvement of a
/// leader/coordinator.
///
/// If a quorum is not formed, then it falls back to an instance of regular Paxos, which is
/// scheduled to be run after a random interval of time. The randomness is introduced so that
/// multiple nodes do not start their own instances of paxos as coordinators.
#[derive(Debug)]
pub struct FastPaxos {
    client: Client,
    my_addr: Endpoint,
    size: usize,
    decided: AtomicBool,
    paxos: Paxos,
    config_id: ConfigId,
    votes_received: HashSet<Endpoint>, // should be a bitset?
    votes_per_proposal: HashMap<Vec<Endpoint>, usize>,
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl FastPaxos {
    #[allow(dead_code)]
    pub fn new(my_addr: Endpoint, size: usize, client: Client, config_id: ConfigId) -> FastPaxos {
        FastPaxos {
            client: client.clone(),
            config_id,
            size,
            my_addr: my_addr.clone(),
            decided: AtomicBool::new(false),
            paxos: Paxos::new(client, size, my_addr, config_id),
            votes_received: HashSet::default(),
            votes_per_proposal: HashMap::default(),
            cancel_tx: None,
        }
    }

    /// Propose a new membership set to the cluster
    ///
    /// # Errors
    ///
    /// Returns `NewBrokenPipe` if the broadcast was not sucessful
    #[allow(dead_code)]
    pub async fn propose(
        &mut self,
        proposal: Vec<Endpoint>,
        scheduler: &mut Scheduler,
    ) -> Result<()> {
        let mut paxos_delay = Delay::new(Instant::now() + self.get_random_delay()).fuse();

        let (tx, cancel_rx) = oneshot::channel();
        let mut cancel_rx = cancel_rx.fuse();

        let task = async move {
            futures::select! {
                _ = cancel_rx => SchedulerEvents::None,
                _ = paxos_delay => SchedulerEvents::StartClassicRound,
            }
        };

        scheduler.push(Box::pin(task));

        // Make sure to cancel the previous task if it's present. There is always only one instance
        // of a classic paxos round
        if let Some(cancel) = self.cancel_tx.replace(tx) {
            cancel.send(()).unwrap();
        }

        let kind = proto::RequestKind::Consensus(proto::Consensus::FastRoundPhase2bMessage(
            proto::FastRoundPhase2bMessage {
                sender: self.my_addr.clone(),
                config_id: self.config_id,
                endpoints: proposal,
            },
        ));

        self.client.broadcast(kind).await?;

        Ok(())
    }

    /// Handles a consensus message which the membership service reveives.
    ///
    /// # Errors
    ///
    /// * `NewUnexpectedRequest`: if any other request kind other tha a Consensus message is
    /// passed.
    /// * `VoteAlredyReceived`: If the request is from a sender who has already cast a vote which
    /// this node as received.
    /// * `AlreadyReachedConsensus`: If this is called after the cluster has already reached
    /// consensus.
    /// * `FastRoundFailure`: If fast paxos is unable to reach consensus
    pub async fn handle_message(&mut self, msg: Consensus) -> Result<Response> {
        match msg {
            FastRoundPhase2bMessage(req) => self.handle_fast_round(&req).await?,
            _ => unimplemented!(),
        };

        Ok(Response::consensus())
    }

    async fn handle_fast_round(
        &mut self,
        request: &proto::FastRoundPhase2bMessage,
    ) -> crate::Result<()> {
        if request.config_id != self.config_id {
            return Err(Error::new_unexpected_request(None));
        }

        if self.votes_received.contains(&request.sender) {
            return Err(Error::vote_already_received());
        }

        if self.decided.load(Ordering::SeqCst) {
            return Err(Error::already_reached_consensus());
        }

        self.votes_received.insert(request.sender.clone());

        let count = self
            .votes_per_proposal
            .entry(request.endpoints.clone())
            .and_modify(|votes| *votes += 1)
            .or_insert(0);

        let f = ((self.size - 1) as f64 / 4f64).floor();

        if self.votes_received.len() >= (self.size as f64 - f) as usize 
            && *count >= (self.size as f64 - f) as usize{
                return self.on_decide(request.endpoints.clone());
        }

        Err(Error::fast_round_failure())
    }

    fn on_decide(&mut self, _hosts: Vec<Endpoint>) -> Result<()> {
        // This is the only place where the value is set to `true`.
        self.decided.store(true, Ordering::SeqCst);

        if let Some(cancel_tx) = self.cancel_tx.take() {
            cancel_tx.send(()).unwrap();
        }

        // TODO: surface decision to membership
        Ok(())
    }

    pub async fn start_classic_round(&mut self) -> Result<()> {
        if !self.decided.load(Ordering::SeqCst) {
            // The java impl does this..
            self.paxos.start_phase_1a(2).await?;
        }
        Ok(())
    }

    fn get_random_delay(&self) -> Duration {
        let jitter_rate: f64 = 1f64 / self.size as f64;
        let mut rng = rand::thread_rng();

        let jitter = ((-1000f64 * (1.0f64 - rng.gen::<f64>()).ln()) / jitter_rate) as u64;

        Duration::from_millis(jitter + BASE_DELAY)
    }
}
