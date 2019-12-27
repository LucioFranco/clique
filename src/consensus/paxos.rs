use crate::{
    common::{ConfigId, Endpoint},
    error::{Error, Result},
    transport::{
        proto::{
            Consensus, Phase1aMessage, Phase1bMessage, Phase2aMessage, Phase2bMessage, Rank,
            RequestKind,
        },
        Message,
    },
};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet, VecDeque},
    convert::TryInto,
    hash::Hasher,
};

use tracing::{debug, info};
use twox_hash::XxHash32;

#[derive(Debug)]
pub struct Paxos {
    // TODO: come up with a design for onDecide
    // client: Client,
    size: usize,
    my_addr: Endpoint,
    /// Highest-numbered round we have participated in
    rnd: Rank,
    /// Highest-numbred round we have cast a vote in
    vrnd: Rank,
    /// the values we voted to accept in a given round `i`
    vval: Vec<Endpoint>,
    /// The higheset-numbered round we have begun
    crnd: Rank,
    /// The value we have picked for a given round `i`
    cval: Vec<Endpoint>,
    // The config ID we are working with.
    config_id: ConfigId,
    phase_1b_messages: Vec<Phase1bMessage>,
    phase_2a_messages: Vec<Phase2aMessage>,
    accept_responses: HashMap<Rank, HashMap<Endpoint, Phase2bMessage>>,
    decided: bool,

    messages: VecDeque<(Option<Endpoint>, Message)>,
}

impl Paxos {
    pub fn new(size: usize, my_addr: Endpoint, config_id: ConfigId) -> Paxos {
        Paxos {
            size,
            my_addr,
            config_id,
            crnd: Rank {
                round: 0,
                node_index: 0,
            },
            rnd: Rank {
                round: 0,
                node_index: 0,
            },
            vrnd: Rank {
                round: 0,
                node_index: 0,
            },
            cval: Vec::new(),
            vval: Vec::new(),
            phase_1b_messages: Vec::new(),
            phase_2a_messages: Vec::new(),
            accept_responses: HashMap::new(),
            decided: false,

            messages: VecDeque::new(),
        }
    }

    /// Starts a classic paxos round by senidng out a Phase 1a message as the coordinator
    ///
    /// Using ranks as round numbers ensure uniquenes even with multiple rounds happening at the
    /// same time.
    pub fn start_phase_1a(&mut self, round: u32) {
        debug!(
            message = "Start phase 1a",
            round = round,
            crnd = self.crnd.round
        );
        if self.crnd.round > round {
            debug!(
                message = "Rejecting request as round is < current round",
                crnd = self.crnd.round
            );
            return;
        }

        let mut hasher = XxHash32::with_seed(0);
        hasher.write(self.my_addr.as_bytes());

        let hash = hash_str(&self.my_addr);

        self.crnd = Rank {
            round,
            node_index: hash,
        };

        debug!(
            message = "Sending Phase 1a message",
            config_id = self.config_id,
            crnd = ?self.crnd,
            rnd = ?self.rnd,
            vrnd = ?self.vrnd
        );

        let kind = RequestKind::Consensus(Consensus::Phase1aMessage(Phase1aMessage {
            config_id: self.config_id,
            sender: self.my_addr.clone(),
            rank: self.crnd,
        }));

        self.messages.push_back((None, kind.into()));
    }

    /// At acceptor, handle a Phase 1a message from a coordinator.
    ///
    /// If `crnd` > then we don't respond back.
    #[allow(dead_code)]
    pub(crate) fn handle_phase_1a(&mut self, request: Phase1aMessage) {
        let Phase1aMessage {
            sender,
            config_id,
            rank,
        } = request;

        if config_id != self.config_id {
            debug!(
                message = "Rejecting as config_id is not current",
                config_id = self.config_id
            );
            return;
        }

        if self.crnd < rank {
            self.crnd = rank;
        } else {
            debug!(
                message = "Rejecting request as round is < current round",
                crnd = self.crnd.round
            );
            return;
        }

        debug!(
            message = "Sending Phase 1a message",
            config_id = self.config_id,
            crnd = ?self.crnd,
            rnd = ?self.rnd,
            vrnd = ?self.vrnd
        );

        let kind = RequestKind::Consensus(Consensus::Phase1bMessage(Phase1bMessage {
            config_id: self.config_id,
            rnd: self.rnd,
            sender: self.my_addr.clone(),
            vrnd: self.vrnd,
            vval: self.vval.clone(),
        }));

        self.messages.push_back((Some(sender), kind.into()));
    }

    /// At coordinator, collect phase 1b messages from acceptors and check if they have already
    /// voted and if a value might have already been chosen
    #[allow(dead_code)]
    pub(crate) fn handle_phase_1b(&mut self, request: Phase1bMessage) {
        let message = request.clone();

        let Phase1bMessage { config_id, rnd, .. } = request;

        if config_id != self.config_id {
            return;
        }

        // Only handle responses where crnd == i
        if rnd != self.crnd {
            return;
        }

        self.phase_1b_messages.push(message.clone());

        if self.phase_1b_messages.len() > (self.size / 2) {
            let chosen_proposal = select_proposal(&self.phase_1b_messages, self.size);
            if self.crnd == rnd && self.cval.is_empty() && !chosen_proposal.is_empty() {
                self.cval = chosen_proposal.clone();
                let kind = RequestKind::Consensus(Consensus::Phase2aMessage(Phase2aMessage {
                    sender: self.my_addr.clone(),
                    config_id: self.config_id,
                    rnd: self.crnd,
                    vval: chosen_proposal,
                }));

                self.messages.push_back((None, kind.into()));
            }
        }
    }

    /// At acceptor, accept a phase 2a message.
    #[allow(dead_code)]
    pub(crate) fn handle_phase_2a(&mut self, request: Phase2aMessage) {
        let Phase2aMessage {
            config_id,
            rnd,
            vval,
            sender,
        } = request;

        if config_id != self.config_id {
            debug!(
                message = "Rejecting as config_id is not current",
                config_id = self.config_id
            );
            return;
        }

        if self.rnd <= rnd && self.vrnd != rnd {
            debug!(
                message = "Sending Phase 2b message",
                config_id = self.config_id,
                crnd = ?self.crnd,
                rnd = ?self.rnd,
                vrnd = ?self.vrnd
            );
            self.rnd = rnd;
            self.vrnd = rnd;
            self.vval = vval.clone();

            let kind = RequestKind::Consensus(Consensus::Phase2bMessage(Phase2bMessage {
                config_id,
                rnd,
                sender: self.my_addr.clone(),
                endpoints: vval,
            }));

            self.messages.push_back((sender.into(), kind.into()));
        }
    }

    #[allow(dead_code)]
    pub(crate) fn handle_phase_2b(&mut self, request: Phase2bMessage) -> Option<Vec<Endpoint>> {
        let Phase2bMessage {
            config_id,
            rnd,
            endpoints,
            ..
        } = request;

        if config_id != self.config_id {
            debug!(
                message = "Rejecting as config_id is not current",
                config_id = self.config_id
            );
            return None;
        }

        let phase_2b_messages_in_rnd = self
            .accept_responses
            .entry(rnd)
            .or_insert_with(HashMap::new);

        if phase_2b_messages_in_rnd.len() > (self.size / 2) && !self.decided {
            // TODO: let caller know of decision
            self.decided = true;

            // TODO: propogate decision
            Some(endpoints)
        } else {
            None
        }
    }
}

fn select_proposal(messages: &[Phase1bMessage], size: usize) -> Vec<Endpoint> {
    // The rule with which a coordinator picks a value proposed. Corresponds
    // Figure 2 of the Fast Paxos paper:
    // https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/tr-2005-112.pdf
    let max_vrnd_so_far = messages.iter().map(|msg| msg.vrnd).max();

    if max_vrnd_so_far.is_none() {
        // Coordingator will not move on to Phase 2 unless a proposal is chosen, so it's
        // safe to return an empty vec
        return vec![];
    }

    let max_vrnd_so_far = max_vrnd_so_far.unwrap();

    // Let k be the largest value of `vr(a)` (Highest valued round the acceptor `a` voted in)
    // for all a in Q (quorum).
    // `V` (collected_vals_set) is the set of all `vv(a)` (vval of acceptor) for all acceptors in Q
    // such that `vr(a) == k`.
    // This basically means that we have a set of all `vval`s of acceptors who's vr is the highest
    // ranked round.
    let collected_vals_set: HashSet<Vec<Endpoint>> = messages
        .iter()
        .filter(|msg| msg.vrnd == max_vrnd_so_far)
        .filter(|msg| msg.vval.is_empty())
        .map(|msg| msg.vval.clone())
        .collect();

    let mut chosen_proposal = None;

    match collected_vals_set.len().cmp(&1) {
        Ordering::Equal => {
            // If there is only one value in the set, choose that.
            chosen_proposal = Some(collected_vals_set.iter().next().unwrap());
        }
        Ordering::Greater => {
            let mut counters = HashMap::new();
            for values in collected_vals_set.iter() {
                let count = counters.entry(values).and_modify(|e| *e += 1).or_insert(0);

                if *count + 1 > (size / 4) {
                    chosen_proposal = Some(values);
                    break;
                }
            }
        }
        Ordering::Less => unreachable!(),
    }

    // At this point, no value has been selected yet and it is safe for the coordinator to pick any proposed value.
    // If none of the 'vvals' contain valid values (are all empty lists), then this method returns an empty
    // list. This can happen because a quorum of acceptors that did not vote in prior rounds may have responded
    // to the coordinator first. This is safe to do here for two reasons:
    //      1) The coordinator will only proceed with phase 2 if it has a valid vote.
    //      2) It is likely that the coordinator (itself being an acceptor) is the only one with a valid vval,
    //         and has not heard a Phase1bMessage from itself yet. Once that arrives, phase1b will be triggered
    //         again.
    //
    if let Some(proposal) = chosen_proposal {
        proposal.to_vec()
    } else {
        // Just grab the first value you find which is not empty.
        messages
            .iter()
            .filter(|msg| msg.vval.is_empty())
            .map(|msg| msg.vval.clone())
            .nth(0)
            .unwrap_or_else(Vec::new)
    }
}

fn hash_str(input: &str) -> u32 {
    let mut hasher = XxHash32::with_seed(0);
    hasher.write(input.as_bytes());

    hasher
        .finish()
        .try_into()
        .expect("Got > 32 bits from 32 bit hasher")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::trace_init;

    const K: usize = 5;
    const NODES: [&str; 5] = ["chicago", "new-york", "boston", "seattle", "la"];
    const CONFIG_ID: i64 = 0;

    macro_rules! extract_message {
        ($pax:expr, $mt:ident) => {
            if let (_, Message::Request(message)) = $pax.messages.pop_front().unwrap() {
                if let RequestKind::Consensus(Consensus::$mt(msg)) = message {
                    msg
                } else {
                    panic!("Incorrect type of request!")
                }
            } else {
                panic!("No messages present in the queue!")
            }
        };
    }

    #[test]
    fn test_paxos_start_phase1a() {
        trace_init();

        let mut pax = Paxos::new(K, "san-francisco".to_string(), CONFIG_ID);

        pax.start_phase_1a(1);
        let msg = extract_message!(pax, Phase1aMessage);

        assert_eq!(msg.config_id, CONFIG_ID);
        assert_eq!(
            msg.rank,
            Rank {
                round: 1,
                node_index: hash_str("san-francisco")
            }
        );
    }

    #[test]
    #[should_panic]
    fn test_paxos_start_phase1a_fail() {
        trace_init();

        let mut pax = Paxos::new(K, "san-francisco".to_string(), CONFIG_ID);

        pax.start_phase_1a(0);
        let msg = extract_message!(pax, Phase1aMessage);

        pax.handle_phase_1a(msg);
        let _msg = extract_message!(pax, Phase1aMessage);

        // At this point, anyone starting a new paxos round needs to have the round at >= 1
        pax.start_phase_1a(0);
        let _msg = extract_message!(pax, Phase1aMessage);
    }

    #[test]
    fn test_paxos_handle_phase1a_safe_path() {
        trace_init();

        let req = Phase1aMessage {
            sender: "chicago".into(),
            config_id: 1,
            rank: Rank {
                round: 3,
                node_index: hash_str("chicago"),
            },
        };

        let mut pax = Paxos::new(K, "san-francisco".to_string(), 1);

        let req = pax.handle_phase_1a(req);
        let msg = extract_message!(pax, Phase1bMessage);

        assert_eq!(msg.config_id, 1);
        assert_eq!(
            msg.rnd,
            Rank {
                round: 0,
                node_index: 0
            }
        );
        assert_eq!(
            msg.vrnd,
            Rank {
                round: 0,
                node_index: 0
            }
        );
    }

    #[test]
    #[should_panic]
    fn test_paxos_handle_phase1a_wrong_config() {
        trace_init();

        let rank = Rank {
            round: 3,
            node_index: hash_str("chicago"),
        };

        let req = Phase1aMessage {
            sender: "chicago".into(),
            config_id: 1,
            rank,
        };

        let mut pax = Paxos::new(K, "san-francisco".to_string(), 2);

        let req = pax.handle_phase_1a(req);
        // test will panic because no message was sent
        let _msg = extract_message!(pax, Phase1bMessage);
    }

    #[test]
    #[should_panic]
    fn test_paxos_handle_phase1a_wrong_rank() {
        trace_init();

        let rank = Rank {
            round: 0,
            node_index: hash_str("chicago"),
        };

        let req = Phase1aMessage {
            sender: "chicago".into(),
            config_id: 2,
            rank,
        };

        let mut pax = Paxos::new(K, "san-francisco".to_string(), 2);

        pax.handle_phase_1a(req);
        let _msg = extract_message!(pax, Phase1bMessage);

        let rank = Rank {
            round: 0,
            node_index: hash_str("chicago"),
        };

        let req = Phase1aMessage {
            sender: "chicago".into(),
            config_id: 2,
            rank,
        };

        pax.handle_phase_1a(req);
        // test will panic because no message was sent
        let _msg = extract_message!(pax, Phase1bMessage);
    }

    #[test]
    fn test_paxos_handle_phase2a() {
        trace_init();

        let PROPOSAL: Vec<Endpoint> = vec![
            "chicago".to_string(),
            "new-york".to_string(),
            "boston".to_string(),
            "seattle".to_string(),
        ];

        let rank = Rank {
            round: 1,
            node_index: hash_str("san-francisco"),
        };

        let req = Phase2aMessage {
            sender: "san-francisco".to_string(),
            config_id: 1,
            rnd: rank,
            vval: PROPOSAL.clone(),
        };

        let mut pax = Paxos::new(K, "san-francisco".to_string(), 1);

        pax.handle_phase_2a(req);

        let msg = extract_message!(pax, Phase2bMessage);

        assert_eq!(msg.config_id, 1);
        assert_eq!(msg.endpoints, PROPOSAL);
    }
}
