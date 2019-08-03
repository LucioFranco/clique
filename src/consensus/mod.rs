use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use futures::FutureExt;
use rand::Rng;
use tokio_sync::{mpsc, oneshot};
use tokio_timer::Delay;

mod paxos;

use paxos::Paxos;

use crate::{
    common::{ConfigId, Endpoint},
    error::{Error, Result},
    transport::{
        proto::{self, Consensus, Consensus::*, RequestKind::*},
        Broadcast, Client, Request, Response,
    },
};

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
    broadcast: mpsc::Sender<(Request, oneshot::Sender<Response>)>,
    my_addr: Endpoint,
    size: usize,
    decided: AtomicBool,
    paxos: Option<Paxos>,
    config_id: ConfigId,
    votes_received: HashSet<Endpoint>, // should be a bitset?
    votes_per_proposal: HashMap<Vec<Endpoint>, usize>,
}

impl FastPaxos {
    pub fn new(
        my_addr: Endpoint,
        size: usize,
        client: mpsc::Sender<(Request, oneshot::Sender<Response>)>,
        broadcast: mpsc::Sender<(Request, oneshot::Sender<Response>)>,
        config_id: ConfigId,
    ) -> FastPaxos {
        FastPaxos {
            broadcast,
            config_id,
            size: size,
            my_addr: my_addr.clone(),
            decided: AtomicBool::new(false),
            paxos: Some(Paxos::new(client, size, my_addr, config_id)),
            votes_received: HashSet::default(),
            votes_per_proposal: HashMap::default(),
        }
    }

    /// Propose a new membership set to the cluster
    ///
    /// # Errors
    ///
    /// Returns `NewBrokenPipe` if the broadcast was not sucessful
    pub async fn propose(&mut self, proposal: Vec<Endpoint>) -> Result<()> {
        let paxos_delay = Delay::new(Instant::now() + self.get_random_delay()).fuse();

        async {
            paxos_delay.await;
            self.start_classic_round().await;
        }
            .await;

        let (tx, rx) = oneshot::channel();

        let request = Request::new_fast_round(tx, self.my_addr.clone(), self.config_id, proposal);

        // self.broadcast.send((request, tx));
        rx.await.map_err(|_| Error::new_broken_pipe(None))?;

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
    pub async fn handle_message(
        &mut self,
        msg: Consensus,
        res_tx: oneshot::Sender<Result<Response>>,
    ) -> Result<()> {
        match msg {
            FastRoundPhase2bMessage(req) => {
                if self.paxos.is_some() {
                    return self.handle_fast_round(&req).await;
                }
            }
            _ => unimplemented!(),
        };

        Ok(())
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

        if self.votes_received.len() >= (self.size as f64 - f) as usize {
            if *count >= (self.size as f64 - f) as usize {
                return self.on_decide(request.endpoints.clone());
            }
        }

        Err(Error::fast_round_failure())
    }

    fn on_decide(&mut self, hosts: Vec<Endpoint>) -> Result<()> {
        // This is the only place where the value is set to `true`.
        self.decided.store(true, Ordering::SeqCst);

        let paxos_instance = self.paxos.take();

        if let Some(instance) = paxos_instance {
            drop(instance);
        }

        // TODO: surface decision to membership
        Ok(())
    }

    async fn start_classic_round(&mut self) -> Result<()> {
        if !self.decided.load(Ordering::SeqCst) {
            if let Some(paxos) = &mut self.paxos {
                // The java impl does this..
                paxos.start_phase_1a(2).await?;
            }
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
