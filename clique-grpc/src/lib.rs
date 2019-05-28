use futures::{Async, Future, Poll, Stream};
use http::Uri;
use hyper::client::connect::{Destination, HttpConnector};
use tower_grpc::Request;
use tower_hyper::{
    client::{self, Connection},
    util,
};
use tower_request_modifier;
use tower_service::Service;
use tower_util;
use tower_util::MakeService;

use std::collections::HashMap;
use std::hash::Hash;

pub mod clique_proto {
    include!(concat!(env!("OUT_DIR"), "/messaging.rs"));
}

use clique_proto::client as ProtoClient;
use clique_proto::{RapidRequest, RapidResponse};

struct Message {
    uri: Uri,
    payload: RapidRequest,
}

trait PoolRequest<T> {
    type Address: Into<Uri> + Clone + Eq + Hash;
    type Payload: Into<tower_grpc::Request<T>> + Clone;

    fn get_uri(&self) -> Self::Address;
    fn get_payload(&self) -> Self::Payload;
}

impl<T> PoolRequest<T> for Message
where
    tower_grpc::Request<T>: From<RapidRequest>,
{
    type Address = Uri;
    type Payload = RapidRequest;

    fn get_uri(&self) -> Self::Address {
        self.uri
    }

    fn get_payload(&self) -> Self::Payload {
        self.payload
    }
}

impl From<RapidRequest> for tower_grpc::Request<RapidRequest> {
    fn from(req: RapidRequest) -> Request<RapidRequest> {
        Request::new(req)
    }
}

pub struct UniquePool<S, T, R>
where
    S: MakeService<T, R> + Clone,
    T: Hash + Eq,
{
    pool: HashMap<T, S::Service>,
    maker: S,
}

impl<S, T, R> UniquePool<S, T, R>
where
    S: MakeService<T, R> + Clone,
    T: Hash + Eq,
{
    pub fn new(maker: S) -> UniquePool<S, T, R> {
        UniquePool {
            pool: HashMap::new(),
            maker,
        }
    }
}

trait Key<T> {
    fn get_key(&self) -> T;
}

impl<S, T, R> Service<R> for UniquePool<S, T, R>
where
    S: MakeService<T, R> + Clone,
    T: Hash + Eq,
    R: Key<T>,
    S::Service: Clone,
{
    type Response = S::Response;
    type Error = S::Error; // Need better error handling
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(Async::Ready(()))
    }

    fn call(&mut self, req: R) -> Self::Future {
        if let Some(svc) = self.pool.get(&req.get_key()) {
            Box::new(svc.call(req))
        } else {
            Box::new(
                self.maker
                    .clone()
                    .make_service(req.get_key())
                    .map_err(Into::into)
                    .map(|svc| {
                        self.pool.insert(req.get_key(), svc.clone());
                        svc.call(req)
                    }),
            )
        }
    }
}
