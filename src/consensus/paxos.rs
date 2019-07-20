use crate::{
    common::{ConfigId, Endpoint},
    error::{Error, Result},
    transport::{
        proto::{
            self, Consensus, Phase1aMessage, Phase1bMessage, Phase2aMessage, Phase2bMessage, Rank,
            RequestKind,
        },
        Client, Request, Response,
    },
};

use std::{collections::HashMap, convert::TryInto, hash::Hasher};

use tokio_sync::{mpsc, oneshot};
use twox_hash::XxHash32;

pub struct Paxos {
    // TODO: come up with a design for onDecide
    client: mpsc::Sender<(Request, oneshot::Sender<Response>)>,
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
    config_id: ConfigId,
    phase_1b_messages: Vec<Phase1bMessage>,
    phase_2a_messages: Vec<Phase2aMessage>,
    accept_responses: HashMap<Rank, HashMap<Endpoint, Phase2bMessage>>,
    decided: bool,
}

impl Paxos {
    pub fn new(
        client: mpsc::Sender<(Request, oneshot::Sender<Response>)>,
        size: usize,
        my_addr: Endpoint,
        config_id: ConfigId,
    ) -> Paxos {
        Paxos {
            client,
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
        }
    }

    /// Starts a classic paxos round by senidng out a Phase 1a message as the coordinator
    ///
    /// Using ranks as round numbers ensure uniquenes even with multiple rounds happening at the
    /// same time.
    pub async fn start_phase_1a(&mut self, round: u32) -> Result<()> {
        if self.crnd > round {
            // TODO: handle these () returns
            return Ok(());
        }

        let mut hasher = XxHash32::with_seed(0);
        hasher.write(self.my_addr.as_bytes());

        self.crnd = Rank {
            round,
            node_index: hasher
                .finish()
                .try_into()
                .expect("Got > 32 bits from 32 bit hasher"),
        };

        let kind = RequestKind::Consensus(Consensus::Phase1aMessage(Phase1aMessage {
            config_id: self.config_id,
            sender: self.my_addr.clone(),
            rank: self.crnd,
        }));

        let (tx, rx) = oneshot::channel();

        let request = Request::new(tx, kind);

        self.client.send((request, tx));
        rx.await.map_err(|_| Error::new_broken_pipe(None))?;

        Ok(())
    }

    /// At acceptor, handle a Phase 1a message from a coordinator.
    ///
    /// If `crnd` > then we don't respond back.
    pub(crate) async fn handle_phase_1a(&self, request: Request) -> crate::Result<()> {
        let RequestKind::Consensus(Consensus::Phase1aMessage(Phase1aMessage {
            sender,
            config_id,
            rank,
        })) = request.kind();

        if *config_id != self.config_id {
            return Err(Error::new_unexpected_request(None));
        }

        if self.crnd < *rank {
            self.crnd = *rank;
        } else {
            // TODO: new error type for rejecting message due to lower rank
            return Err(Error::new_unexpected_request(None));
        }

        let kind = RequestKind::Consensus(Consensus::Phase1bMessage(Phase1bMessage {
            config_id: self.config_id,
            rnd: self.rnd,
            sender: self.my_addr.clone(),
            vrnd: self.vrnd,
            vval: self.vval,
        }));

        let (tx, rx) = oneshot::channel();
        let request = Request::new(tx, kind);

        self.client.send((request, tx));
        rx.await.map_err(|_| Error::new_broken_pipe(None))?;

        Ok(())
    }

    /// At coordinator, coolect phase 1b messages from acceptors and check if they have already
    /// voted and if a value might have already been chosen
    pub(crate) async fn handle_phase_1b(&self, request: Request) -> crate::Result<()> {
        let RequestKind::Consensus(Consensus::Phase1bMessage(message)) = request.kind().clone();

        let RequestKind::Consensus(Consensus::Phase1bMessage(Phase1bMessage {
            sender,
            config_id,
            rnd,
            vrnd,
            vval,
        })) = request.kind();

        if *config_id != self.config_id {
            return Err(Error::new_unexpected_request(None));
        }

        // Only handle responses where crnd == i
        if *rnd != self.crnd {
            return Err(Error::new_unexpected_request(None));
        }

        self.phase_1b_messages.push(message.clone());

        if self.phase_1b_messages.len() > (self.size / 2) {
            let chosen_proposal = select_proposal(message);
            if self.crnd == *rnd && self.cval.len() == 0 && chosen_proposal.len() > 0 {
                self.cval = chosen_proposal.clone();
                let kind = RequestKind::Consensus(Consensus::Phase2aMessage(Phase2aMessage {
                    sender: self.my_addr.clone(),
                    config_id: self.config_id,
                    rnd: self.crnd,
                    vval: chosen_proposal,
                }));
                let (tx, rx) = oneshot::channel();

                let request = Request::new(tx, kind);

                self.client.send((request, tx));
                rx.await.map_err(|_| Error::new_broken_pipe(None))?;
            }
        }

        Ok(())
    }

    /// At acceptor, accept a phase 2a message.
    pub(crate) async fn handle_phase_2a(&self, request: Request) -> crate::Result<()> {
        let RequestKind::Consensus(Consensus::Phase2aMessage(Phase2aMessage {
            sender,
            config_id,
            rnd,
            vval,
        })) = request.kind();

        if *config_id != self.config_id {
            Err(Error::new_unexpected_request(None));
        }

        if self.rnd <= *rnd && self.vrnd != *rnd {
            self.rnd = *rnd.clone();
            self.vrnd = *rnd;
            self.vval = vval.clone();

            let kind = RequestKind::Consensus(Consensus::Phase2bMessage(Phase2bMessage {
                config_id: *config_id,
                rnd: *rnd,
                sender: self.my_addr.clone(),
                endpoints: *vval,
            }));

            let (tx, rx) = oneshot::channel();

            let request = Request::new(tx, kind);

            self.client.send((request, tx));
            rx.await.map_err(|_| Error::new_broken_pipe(None))?;
        }

        Ok(())
    }

    pub(crate) async fn handle_phase_2b(&self, request: Request) -> crate::Result<()> {
        let RequestKind::Consensus(Consensus::Phase2bMessage(Phase2bMessage {
            config_id,
            rnd,
            sender,
            endpoints,
        })) = request.kind();

        if *config_id != self.config_id {
            return Err(Error::new_unexpected_request(None));
        }

        let phase_2b_messages_in_rnd = self.accept_responses.entry(*rnd).or_insert(HashMap::new());

        if phase_2b_messages_in_rnd.len() > (self.size / 2) && !self.decided {
            let decision = endpoints;
            // TODO: let caller know of decision
            self.decided = true;
        }

        Ok(())
    }
}

fn select_proposal(message: Phase1bMessage) -> Vec<Endpoint> {
    unimplemented!()
}
