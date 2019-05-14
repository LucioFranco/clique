use futures::Future;

use crate::{membership::Membership};

pub type RapidResponseFuture = Box<Future<Item = RapidResponse, Error = ()>>;
pub trait Broadcast {
    fn broadcast(&self, req: RapidRequest) -> RapidResponseFuture;
    fn set_membership(&self, reciepients: Vec<Endpoint>);
}

pub trait Client {
    fn send_message(&self, remote: Endpoint, message: RapidRequest) -> RapidResponseFuture;
    fn send_message_best_effort(&self, remote: Endpoint, message: RapidRequest) -> RapidResponseFuture;
}

pub trait Server {
    fn start(&self);
    fn shutdown(&self);
    fn set_membership_service(&self, membership: Membership);
}

pub struct Endpoint;
pub struct RapidResponse;
pub struct NodeId;
pub struct Metadata;
pub struct RapidRequest;