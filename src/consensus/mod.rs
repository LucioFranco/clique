use futures::{FutureExt, Stream, StreamExt};

mod paxos;

use paxos::Paxos;

use crate::{
    common::Endpoint,
    error::{Error, Result},
    transport::{proto, Broadcast, Client, Request, Response},
};

pub struct FastPaxos<'a, C, B> {
    broadcast: &'a mut B,
    my_addr: Endpoint,
    size: usize,
    paxos: Paxos<'a, C>,
}

impl<'a, C, B> FastPaxos<'a, C, B> {
    pub fn new(
        my_addr: Endpoint,
        size: usize,
        client: &'a mut C,
        broadcast: &'a mut B,
    ) -> FastPaxos<'a, C, B>
    where
        C: Client,
        B: Broadcast,
    {
        FastPaxos {
            my_addr: my_addr.clone(),
            broadcast,
            size,
            paxos: Paxos::new(client, size, my_addr),
        }
    }

    pub async fn propose(&self, proposal: Vec<Endpoint>) {
        // TODO: register proposal with regular paxos instance
        unimplemented!();
    }

    pub async fn handle_message(&self, request: Request) -> Result<Response> {
        match request.kind() {
            proto::RequestKind::Consensus(_) => self.handle_fast_round(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_1a(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_1b(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_2a(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_2b(request).await,
            _ => Err(Error::new_unexpected_request(None)),
        }
    }

    async fn handle_fast_round(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }
}
