use clique::messaging::{Broadcast, RapidResponseFuture};
use clique_proto;

use clique_proto::{RapidRequest, Endpoint};
struct Client;

impl Broadcast for Client {
    fn broadcast(&self, req: RapidRequest) -> RapidResponseFuture {
        unimplemented!()
    }
    fn set_membership(&self, reciepients: Vec<Endpoint>) {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
