use futures::{FutureExt, Stream, StreamExt};

use crate::transport{Endpoint, Client, Request, Response, proto::{self}};

pub struct FastPaxos<'a, C> {
    client: &'a C,
    my_addr: Endpoint,
    size: usize
}

impl<'a, C> FastPaxos<'a, C> {
    pub fn new<C>(my_addr: Endpoint, size: usize, client: &mut C) -> FastPaxos
    where
        C: Client
    {
        FastPaxos {
            my_addr, size, client
        }
    }

    pub async fn propose(&self, proposal: &[Endpoint]) {
        // TODO: register proposal with regular paxos instance
        unimplemented!();
    }

    pub async fn handle_message(&self, request: Request) -> crate::Result<Response> {
        match request.kind {
            proto::RequestKind::FastRoundPhase1bMessage => handle_fast_round_message(request).await,
            proto::RequestKind::Phase1aMessage => handle_phase_1a_message(request).await,
            proto::RequestKind::Phase1bMessage => handle_phase_1b_message(request).await,
            proto::RequestKind::Phase2aMessage => handle_phase_2a_message(request).await,
            proto::RequestKind::Phase2bMessage => handle_phase_2b_message(request).await,
            _ => Err(crate::Error::new_incorrect_argument(None)),
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
