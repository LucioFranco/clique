use futures::{FutureExt, Stream, StreamExt};

use crate::{
    error::{Error, Result},
    transport::{
        proto::{self, Endpoint},
        Client, Request, Response,
    },
};

pub struct FastPaxos<'a, C> {
    client: &'a C,
    my_addr: Endpoint,
    size: usize,
}

impl<'a, C> FastPaxos<'a, C> {
    pub fn new(my_addr: Endpoint, size: usize, client: &mut C) -> FastPaxos<C>
    where
        C: Client,
    {
        FastPaxos {
            my_addr,
            size,
            client,
        }
    }

    pub async fn propose(&self, proposal: Vec<Endpoint>) {
        // TODO: register proposal with regular paxos instance
        unimplemented!();
    }

    pub async fn handle_message(&self, request: Request) -> Result<Response> {
        match request.kind() {
            proto::RequestKind::Consensus(_) => self.handle_fast_round_message(request).await,
            proto::RequestKind::Consensus(_) => self.handle_phase_1a_message(request).await,
            proto::RequestKind::Consensus(_) => self.handle_phase_1b_message(request).await,
            proto::RequestKind::Consensus(_) => self.handle_phase_2a_message(request).await,
            proto::RequestKind::Consensus(_) => self.handle_phase_2b_message(request).await,
            _ => Err(Error::new_unexpected_request(None)),
        }
    }

    async fn handle_fast_round_message(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }

    async fn handle_phase_1a_message(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }

    async fn handle_phase_1b_message(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }

    async fn handle_phase_2a_message(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }

    async fn handle_phase_2b_message(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }
}
