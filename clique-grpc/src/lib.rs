use futures::{
    stream::futures_unordered::FuturesUnordered,
    sync::{oneshot::channel, Sender},
    Async, Future, Poll,
};
use http::{Request, Response, Uri};
use hyper::client::connect::{Destination, HttpConnector};
use rand::seq::SliceRandom;
use rand::thread_rng;
use tower_hyper::{
    body::Body,
    client::{self, future::ResponseFuture, Connection},
    util,
};
use tower_request_modifier;
use tower_service::Service;
use tower_util::MakeService;

use std::{collections::HashMap, hash::Hash};

pub mod clique_proto {
    include!(concat!(env!("OUT_DIR"), "/messaging.rs"));
}

pub use self::clique_proto::*;

#[derive(Clone)]
struct Conn(Connection<Body>);

struct PoolReq {
    uri: Uri,
    req: Request<Body>,
}

pub struct UniquePool {
    pool: HashMap<PoolReq, Conn>,
    channels: HashMap<PoolReq, Sender<Response>>,
    buffering: usize,
    max_buffering: usize,
    // runner: FuturesUnordered<ResponseFuture>, // Should this be a `ConnectFuture`?
}

impl UniquePool {
    pub fn new(max: usize) -> UniquePool {
        UniquePool {
            pool: HashMap::new(),
            buffering: 0,
            max_buffering: max,
            // runner: FuturesUnordered::new(),
        }
    }
}

impl Service<Request<Body>> for UniquePool {
    type Response = Response<Body>;
    type Error = ();
    type Future = Box<dyn Future<Item = Response<Body>, Error = ()>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        if self.buffering >= self.max_buffering {
            Err(())
        } else {
            Ok(Async::Ready(()))
        }
    }

    fn call(&mut self, req: PoolReq) -> Self::Future {
        if let Some(conn) = self.pool.get(req.uri) {
            // We have already established a connection to this destination
            // We pull the connection out and check if it is ready via `poll_ready`
            //     * if it isn't we wrap it in a future which just checks if the connection is ready
            //       and increase the buffer count
            //          * if it is, notify the pool?
            //          * Or make the reqeust itself?
            //     * If it is, we make the request?
        } else {
            // The connection doesn't exist, so we need to establish it, and then wait for it to resolve
            // and once it does, we need to make the request.
            // All this has to be done in a non-blocking way.
        }
    }
}

struct Broadcaster {
    recipients: Vec<Endpoint>,
}

impl Broadcaster {
    fn init(uri: Uri) -> CliqueClient {
        CliqueClient {
            reciepients: Vec::new(),
        }
    }

    fn set_membership(&mut self, endpoints: Vec<Endpoint>) {
        let mut rng = thread_rng();
        self.recipients = endpoints.drain().collect();
        &self.recipients[..].shuffle(&mut rng);
    }

    fn broadcast(&mut self, request: RapidRequest) {
        self.call(request);
    }
}
