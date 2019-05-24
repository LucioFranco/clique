use futures::{
    stream::futures_unordered::FuturesUnordered,
    sync::{mpsc, oneshot},
    Async, Future, Poll, Stream,
};
use http::Uri;
use hyper::client::connect::{Destination, HttpConnector};
use tokio;
use tower_grpc::Request;
use tower_hyper::{
    body::Body,
    client::{self, Connection},
    util,
};
use tower_request_modifier;
use tower_service::Service;
use tower_util::MakeService;

use std::collections::HashMap;
use std::hash::Hash;

pub mod clique_proto {
    include!(concat!(env!("OUT_DIR"), "/messaging.rs"));
}

use clique_proto::client::{self as ProtoClient, MembershipService};
use clique_proto::RapidRequest;

struct Message {
    uri: Uri,
    req: RapidRequest,
}

pub struct UniquePool<R, T, M>
where
    T: Service<R> + Clone,
    R: PoolRequest<M>,
{
    pool: HashMap<R::Address, T::Response>,
    buffering: usize,
    max_buffering: usize,
}

impl<R, T, M> UniquePool<R, T, M>
where
    T: Service<R> + Clone,
    R: PoolRequest<M>,
{
    pub fn new(max: usize) -> UniquePool<R, T, M> {
        UniquePool {
            pool: HashMap::new(),
            buffering: 0,
            max_buffering: max,
        }
    }
}

trait PoolRequest<T> {
    type Address: Into<Uri> + Clone + Eq + Hash;
    type Payload: Into<tower_grpc::Request<T>> + Clone;

    fn get_uri(&self) -> Self::Address;
    fn get_payload(&self) -> Self::Payload;
}

impl From<RapidRequest> for Request<RapidRequest> {
    fn from(req: RapidRequest) -> Request<RapidRequest> {
        Request::new(req)
    }
}

impl<T> PoolRequest<T> for Message {
    type Address = Uri;
    type Payload = RapidRequest;

    fn get_uri(&self) -> Self::Address {
        self.uri
    }

    fn get_payload(&self) -> Self::Payload {
        self.req.into()
    }
}

impl<R, T, M> Service<R> for UniquePool<R, T, M>
where
    T: Service<R> + Clone,
    R: PoolRequest<M>,
{
    type Response = T::Response;
    type Error = Box<std::error::Error + Send + Sync + 'static>; // Need better error handling
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        if self.buffering >= self.max_buffering {
            Err(Box::new(()))
        } else {
            Ok(Async::Ready(()))
        }
    }

    fn call(&mut self, req: R) -> Self::Future {
        if let Some(conn) = self.pool.get(&req.get_uri().into()) {
            // We have already established a connection to this destination
            ProtoClient::MembershipService::new(conn)
                .and_then(|client| client.send_request(req.get_payload().into()))
        } else {
            // The connection doesn't exist, so we need to establish it, and then wait for it to resolve
            // and once it does, we need to make the request.
            // All this has to be done in a non-blocking way.
            self.buffering += 1;

            let dst = Destination::try_from_uri(req.get_uri().into().clone()).unwrap();
            let connector = util::Connector::new(HttpConnector::new(4));
            let settings = client::Builder::new().http2_only(true).clone();
            let mut make_client = client::Connect::with_builder(connector, settings);

            make_client
                .make_service(dst)
                .map_err(Into::into)
                .and_then(move |conn| {
                    let conn = tower_request_modifier::Builder::new()
                        .set_origin(req.get_uri().into())
                        .build(conn)
                        .unwrap();

                    self.pool.insert(req.get_uri(), conn);
                    Ok(ProtoClient::MembershipService::new(conn))
                })
                .and_then(|mut client| client.send_request(req.get_payload().into()))
                .map_err(Into::into);
        }
    }
}
