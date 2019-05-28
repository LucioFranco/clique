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

type Error = Box<std::error::Error + Send + Sync + 'static>;

impl<M, T, R> Service<R> for UniquePool<M, T, R>
where
    M: MakeService<T, R> + Clone + Send + 'static,
    T: Hash + Eq + Send + 'static,
    R: Key<T> + Send + 'static,
    M::Service: Clone + Send + 'static,
    M::MakeError: Into<Error>,
    M::Error: Into<Error>,
    M::Future: Send + 'static,
    <M::Service as Service<R>>::Future: Send + 'static,
{
    type Response = M::Response;
    type Error = Error; // Need better error handling
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error> + Send + 'static>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(Async::Ready(()))
    }

    fn call(&mut self, req: R) -> Self::Future {
        if let Some(svc) = self.pool.get(&req.get_key()) {
            Box::new(svc.call(req).map_err(Into::into))
        } else {
            let fut = self
                .maker
                .clone()
                .make_service(req.get_key())
                .map_err(Into::into)
                .and_then(|svc| {
                    self.pool.insert(req.get_key(), svc.clone());
                    svc.call(req).map_err(Into::into)
                });

            Box::new(fut)
        }
    }
}
