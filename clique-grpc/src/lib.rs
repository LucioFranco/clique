pub mod clique_proto {
    include!(concat!(env!("OUT_DIR"), "/messaging.rs"));
}

pub use self::clique_proto::*;

use clique::messaging::{Broadcast, RapidResponseFuture};

struct CliqueClient;

impl Broadcast for CliqueClient {
    fn broadcast(&self, req: RapidRequest) -> RapidResponseFuture {
        unimplemented!()
    }
    fn set_membership(&self, reciepients: Vec<Endpoint>) {
        unimplemented!()
    }
}