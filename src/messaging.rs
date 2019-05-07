use crate::rpc::proto::{RapidRequest, RapidResponse, Endpoint};
use futures::Future;

type RapidResponseFuture = Future<Item = RapidResponse, Error = ()>;
trait Broadcast {
    fn broadcast(req: RapidRequest) -> RapidResponseFuture;
    fn set_membership(reciepients: Vec<Endpoint>);
}
