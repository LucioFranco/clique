mod paxos;

use crate::{
    common::{ConfigId, Endpoint, Scheduler, SchedulerEvents},
    error::{Error, Result},
    transport::{
        proto::{self, Consensus, Consensus::*},
        Message,
    },
};
use futures::FutureExt;
use rand::Rng;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::{sync::oneshot, time::delay_for};
use tracing::debug;

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
    // client: Client,
    my_addr: Endpoint,
    size: usize,
    decided: AtomicBool,
    paxos: Paxos,
    config_id: ConfigId,
    votes_received: HashSet<Endpoint>, // should be a bitset?
    votes_per_proposal: HashMap<Vec<Endpoint>, usize>,
    cancel_tx: Option<oneshot::Sender<()>>,

    messages: VecDeque<(Option<Endpoint>, Message)>,
    proposal: Option<Vec<Endpoint>>,
}

impl FastPaxos {
    pub fn new(my_addr: Endpoint, size: usize, config_id: ConfigId) -> FastPaxos {
        FastPaxos {
            config_id,
            size,
            my_addr: my_addr.clone(),
            decided: AtomicBool::new(false),
            paxos: Paxos::new(size, my_addr, config_id),
            votes_received: HashSet::default(),
            votes_per_proposal: HashMap::default(),
            cancel_tx: None,

            messages: VecDeque::new(),
            proposal: None,
        }
    }

    pub fn has_decided(&self) -> bool {
        self.proposal.is_some()
    }

    pub fn proposal(&mut self) -> Option<Vec<Endpoint>> {
        self.proposal.as_ref().map(|v| v.clone())
    }

    /// Propose a new membership set to the cluster
    ///
    /// # Errors
    ///
    /// Returns `NewBrokenPipe` if the broadcast was not sucessful
    #[allow(dead_code)]
    pub fn propose(&mut self, proposal: Vec<Endpoint>, scheduler: &mut Scheduler) {
        self.paxos.register_fast_round(proposal.clone());

        let mut paxos_delay = delay_for(self.get_random_delay()).fuse();

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

        let kind = proto::RequestKind::Consensus(FastRoundPhase2bMessage(
            proto::FastRoundPhase2bMessage {
                sender: self.my_addr.clone(),
                config_id: self.config_id,
                endpoints: proposal,
            },
        ));

        self.messages.push_back((None, kind.into()));
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
    // pub async fn handle_message(
    //     &mut self,
    //     msg: Consensus,
    //     scheduler: &mut Scheduler,
    // ) -> Result<()> {
    //     match msg {
    //         FastRoundPhase2bMessage(req) => self.handle_fast_round(&req, scheduler).await?,
    //         Phase1aMessage(req) => self.paxos.handle_phase_1a(req).await?,
    //         Phase1bMessage(req) => self.paxos.handle_phase_1b(req).await?,
    //         Phase2aMessage(req) => self.paxos.handle_phase_2a(req).await?,
    //         Phase2bMessage(req) => {
    //             let proposal = self.paxos.handle_phase_2b(req).await?;
    //             self.on_decide(proposal, scheduler);
    //         }
    //     };

    //     Ok(())
    // }

    pub fn step(&mut self, msg: Consensus, view: Vec<&Endpoint>) -> Vec<(Endpoint, Message)> {
        match msg {
            FastRoundPhase2bMessage(req) => {
                if let Some(proposal) = self.handle_fast_round(req) {
                    self.proposal = Some(proposal);
                }
            }
            Phase1aMessage(req) => self.paxos.handle_phase_1a(req),
            Phase1bMessage(req) => self.paxos.handle_phase_1b(req),
            Phase2aMessage(req) => self.paxos.handle_phase_2a(req),
            Phase2bMessage(req) => {
                if let Some(proposal) = self.paxos.handle_phase_2b(req) {
                    self.proposal = Some(proposal);
                }
            }
        };

        let mut msgs = Vec::new();

        for msg in self.messages.drain(..) {
            match msg {
                // Unary message
                (Some(to), msg) => msgs.push((to, msg)),
                // Broadcast message.. need a better abstraction
                (None, msg) => {
                    for to in &view {
                        let to = to.clone();
                        msgs.push((to.clone(), msg.clone()));
                    }
                }
            }
        }

        msgs
    }

    /// Handles fast round message in fast paxos, which is really a classic round Phase2b message
    /// but the state of the cluster is such that, you have an almost everywhere agreement on the
    /// proposal as all nodes are processing the same alerts for the nodes in the system.
    ///
    /// If 3/4th of the nodes in the system vote for the same proposal, it is automatically
    /// accepted.
    ///
    /// If not, the cluster falls back to classic paxos for this round.
    fn handle_fast_round(
        &mut self,
        request: proto::FastRoundPhase2bMessage,
    ) -> Option<Vec<Endpoint>> {
        let proto::FastRoundPhase2bMessage {
            config_id,
            sender,
            endpoints,
            ..
        } = request;

        // The proposer is on the wrong configuration
        if config_id != self.config_id {
            debug!(message = "Incorrect config_id");
            return None;
        }

        // We have already seen this proposal
        if self.votes_received.contains(&sender) {
            debug!(message = "We have already received this vote", ?sender);
            return None;
        }

        // We have already make a decision
        if self.decided.load(Ordering::SeqCst) {
            debug!(message = "We have already made a decision");
            return None;
        }

        self.votes_received.insert(sender.clone());

        self.votes_per_proposal
            .entry(endpoints.clone())
            .or_insert(0);

        self.votes_per_proposal
            .entry(endpoints.clone())
            .and_modify(|c| *c += 1);

        let count = self
            .votes_per_proposal
            .get(&endpoints.clone())
            .unwrap_or_else(|| {
                unreachable!("Shouldn't happen as we ensure that the value is initialized")
            });

        // Fast paxos resiliency
        // https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/tr-2005-112.pdf
        // secion 3.4.1
        let f = ((self.size - 1) as f64 / 4f64).floor() as usize;

        debug!(
            votes_received = self.votes_received.len(),
            count, self.size, f
        );

        if self.votes_received.len() >= self.size - f && *count >= self.size - f {
            // self.on_decide(request.endpoints.clone());
            debug!(message = "Consensus reached", proposal = ?endpoints.clone());
            Some(endpoints)
        } else {
            None
        }
    }

    // fn on_decide(&mut self, proposal: Vec<Endpoint>, scheduler: &mut Scheduler) {
    //     // This is the only place where the value is set to `true`.
    //     self.decided.store(true, Ordering::SeqCst);

    //     // Cancel the schduled paxos round 1a
    //     if let Some(cancel_tx) = self.cancel_tx.take() {
    //         cancel_tx.send(()).unwrap();
    //     }

    //     let task = async move { SchedulerEvents::Decision(proposal) };

    //     scheduler.push(Box::pin(task));
    // }

    pub fn start_classic_round(&mut self) {
        if !self.decided.load(Ordering::SeqCst) {
            // The java impl does this..beacause the fast round registers itself with the paxos
            // round which sets classic paxos round to 1.
            self.paxos.start_phase_1a(2);
        }
    }

    fn get_random_delay(&self) -> Duration {
        let jitter_rate: f64 = 1f64 / self.size as f64;
        let mut rng = rand::thread_rng();

        let jitter = ((-1000f64 * (1.0f64 - rng.gen::<f64>()).ln()) / jitter_rate) as u64;

        Duration::from_millis(jitter + BASE_DELAY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: i32 = 5;
    const NODES: [&str; 5] = ["chicago", "new-york", "boston", "seattle", "la"];
    const CONFIG_ID: i64 = 0;

    #[test]
    fn test_handle_fast_round() {
        let PROPOSAL: Vec<Endpoint> = vec![
            "chicago".to_string(),
            "new-york".to_string(),
            "boston".to_string(),
            "seattle".to_string(),
        ];

        let mut fp = FastPaxos::new(NODES[0].to_string(), 5, CONFIG_ID);

        let req1 = FastRoundPhase2bMessage(proto::FastRoundPhase2bMessage {
            sender: NODES[0].to_string(),
            config_id: CONFIG_ID,
            endpoints: PROPOSAL.clone(),
        });

        // Fast round 2b messages don't send any broadcasts
        let result = fp.step(req1, vec![]);
        assert!(!fp.has_decided());

        let req2 = FastRoundPhase2bMessage(proto::FastRoundPhase2bMessage {
            sender: NODES[1].to_string(),
            config_id: CONFIG_ID,
            endpoints: PROPOSAL.clone(),
        });

        let result = fp.step(req2, vec![]);
        assert!(!fp.has_decided());

        let req3 = FastRoundPhase2bMessage(proto::FastRoundPhase2bMessage {
            sender: NODES[2].to_string(),
            config_id: CONFIG_ID,
            endpoints: PROPOSAL.clone(),
        });

        let result = fp.step(req3, vec![]);
        assert!(!fp.has_decided());

        let req4 = FastRoundPhase2bMessage(proto::FastRoundPhase2bMessage {
            sender: NODES[3].to_string(),
            config_id: CONFIG_ID,
            endpoints: PROPOSAL.clone(),
        });

        let result = fp.step(req4, vec![]);
        assert!(fp.has_decided());
        assert_eq!(fp.proposal(), Some(PROPOSAL.clone()));

        let wrong_proposal = vec![
            "chicago".to_string(),
            "new-york".to_string(),
            "boston".to_string(),
            "seattle".to_string(),
            "la".to_string(),
        ];

        let req5 = FastRoundPhase2bMessage(proto::FastRoundPhase2bMessage {
            sender: NODES[4].to_string(),
            config_id: CONFIG_ID,
            endpoints: wrong_proposal,
        });

        let result = fp.step(req5, vec![]);
        assert!(fp.has_decided());
        assert_eq!(fp.proposal(), Some(PROPOSAL));
    }
}
