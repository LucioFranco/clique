use crate::{
    error::Result,
    transport::{
        proto::{ConfigId, Endpoint, Phase1bMessage, Phase2aMessage, Rank},
        Client, Request, Response,
    },
};

pub struct Paxos<'a, C> {
    // TODO: come up with a design for onDecide
    client: &'a mut C,
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
}

impl<'a, C> Paxos<'a, C> {
    pub fn new(client: &'a mut C, size: usize, my_addr: Endpoint, config_id: ConfigId) -> Paxos<C>
    where
        C: Client,
    {
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
        }
    }

    pub async fn start_round(&mut self) -> Result<()> {
        unimplemented!()
    }

    pub(crate) async fn handle_phase_1a(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }

    pub(crate) async fn handle_phase_1b(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }

    pub(crate) async fn handle_phase_2a(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }

    pub(crate) async fn handle_phase_2b(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }
}
