use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use futures::FutureExt;
use rand::Rng;
use tokio_sync::oneshot;
use tokio_timer::Delay;

mod paxos;

use paxos::Paxos;

use crate::{
    common::{ConfigId, Endpoint},
    error::{Error, Result},
    transport::{
        proto::{self},
        Broadcast, Client, Request,
    },
};

const BASE_DELAY: u64 = 1000;

/// Represents an instance of the Fast Paxos consensus protocol.
///
/// This protocol has a fast path as compared to paxos, when a majority set agrees on the same
/// membershipset, each node individually accepts the value without the involvement of a leadero
/// r/coordinator.
///
/// If a qorum is not forumed, then it falls back to an instance of regular Paxos, which is
/// scheduled to be run after a random interval of time. The randomness is introduced so that
/// multiple nodes do not start their own instances of paxos as coordinators.
pub struct FastPaxos<'a, C, B> {
    broadcast: &'a mut B,
    my_addr: Endpoint,
    size: usize,
    decided: AtomicBool,
    paxos: Option<Paxos<'a, C>>,
    config_id: ConfigId,
    votes_received: HashSet<Endpoint>, // should be a bitset?
    votes_per_proposal: HashMap<Vec<Endpoint>, usize>,
}

impl<'a, C, B> FastPaxos<'a, C, B>
where
    C: Client,
    B: Broadcast,
{
    pub fn new(
        my_addr: Endpoint,
        size: usize,
        client: &'a mut C,
        broadcast: &'a mut B,
        config_id: ConfigId,
    ) -> FastPaxos<'a, C, B> {
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

        // TODO: Handle Vec<Result<Response>>
        self.broadcast.broadcast(request).await;
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
    pub async fn handle_message(&mut self, request: Request) -> Result<Vec<Endpoint>> {
        match request.kind() {
            proto::RequestKind::Consensus(req_type) => {
                if self.paxos.is_some() {
                    match req_type {
                        proto::Consensus::FastRoundPhase2bMessage(msg) => {
                            self.handle_fast_round(msg).await
                        }
                        // TODO: need to figure out communication b/w paxos and fast paxos
                        _ => unimplemented!(),
                    }
                } else {
                    Err(Error::new_unexpected_request(None))
                }
            }
            _ => Err(Error::new_unexpected_request(None)),
        }
    }

    async fn handle_fast_round<'b>(
        &'b mut self,
        request: &'b proto::FastRoundPhase2bMessage,
    ) -> crate::Result<Vec<Endpoint>> {
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

        let F = ((self.size - 1) as f64 / 4f64).floor();

        if self.votes_received.len() >= (self.size as f64 - F) as usize {
            if *count >= (self.size as f64 - F) as usize {
                return self.on_decide(request.endpoints.clone());
            }
        }

        Err(Error::fast_round_failure())
    }

    fn on_decide(&mut self, hosts: Vec<Endpoint>) -> Result<Vec<Endpoint>> {
        // This is the only place where the value is set to `true`.
        self.decided.store(true, Ordering::SeqCst);

        let paxos_instance = self.paxos.take();

        if let Some(instance) = paxos_instance {
            drop(instance);
        }

        Ok(hosts)
    }

    async fn start_classic_round(&mut self) -> Result<()> {
        if !self.decided.load(Ordering::SeqCst) {
            if let Some(paxos) = &mut self.paxos {
                paxos.start_round().await?;
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
