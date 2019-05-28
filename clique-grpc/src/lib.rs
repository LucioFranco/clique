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

pub trait Key<T> {
    fn get_key(&self) -> T;
}

impl<M, T, R> Service<R> for UniquePool<M, T, R>
where
    M: MakeService<T, R> + Clone,
    T: Hash + Eq,
    R: Key<T>,
    M::Service: Clone,
{
    type Response = M::Response;
    type Error = (); // Need better error handling
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(Async::Ready(()))
    }

    fn call(&mut self, req: R) -> Self::Future {
        if let Some(svc) = self.pool.get(&req.get_key()) {
            Box::new(svc.call(req).map_err(|_| ()))
        } else {
            let fut = self
                .maker
                .clone()
                .make_service(req.get_key())
                .map_err(|_| ())
                .and_then(|svc| {
                    self.pool.insert(req.get_key(), svc.clone());
                    svc.call(req)
                })
                .map_err(|_| ());
            Box::new(fut)
        }
    }
}
