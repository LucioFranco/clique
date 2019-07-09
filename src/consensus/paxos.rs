use crate::{
    error::{Error, Result},
    common::Endpoint,
    transport::{Client, Request, Response},
};

pub struct Paxos<'a, C> {
    client: &'a mut C,
    size: usize,
    my_addr: Endpoint,
    // TODO: come up with a design for onDecide
}

impl<'a, C> Paxos<'a, C> {
    pub fn new(client: &'a mut C, size: usize, my_addr: Endpoint) -> Paxos<C> {
        Paxos {
            client,
            size,
            my_addr,
        }
    }

    async fn start_round(&mut self) -> Result<Response> {
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
