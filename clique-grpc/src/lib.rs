use futures::{
    stream::futures_unordered::FuturesUnordered,
    sync::{mpsc, oneshot},
    Async, Future, Poll,
};
use http::Uri;
use hyper::client::connect::{Destination, HttpConnector};
use tokio;
use tower_hyper::{
    body::Body,
    client::{self, Connection},
    util,
};
use tower_request_modifier;
use tower_service::Service;
use tower_util::MakeService;

use std::collections::HashMap;

pub mod clique_proto {
    include!(concat!(env!("OUT_DIR"), "/messaging.rs"));
}

use clique_proto::client as ProtoClient;
use clique_proto::RapidRequest;

struct PoolReq {
    uri: Uri,
    req: RapidRequest,
}

struct FuturesRunner<T: Future> {
    inner: FuturesUnordered<T>,
    rx: mpsc::UnboundedReceiver<(T, oneshot::Sender<T::Item>)>,
}

impl<T> FuturesRunner<T>
where
    T: Future,
{
    pub fn new() -> Enqueuer<T, T> {
        let (tx, rx) = mpsc::unbounded();
        tokio::spawn(FuturesRunner {
            inner: FuturesUnordered::new(),
            rx,
        });

        Enqueuer { chan: tx }
    }
}

impl<F> Future for FuturesRunner<F>
where
    F: Future<Item = F::Item, Error = F::Error>,
{
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner.poll() {
            Ok(Async::Ready(None)) => Ok(Async::NotReady), // There are no futures to run
            Ok(Async::Ready(Some(val))) => Ok(Async::Ready(val)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug, Clone)]
struct Enqueuer<T, F> {
    chan: mpsc::UnboundedSender<(F, oneshot::Sender<T>)>,
}

impl<T, F, Request> Service<Request> for Enqueuer<T, F>
where
    T: Service<Request> + Clone,
    F: Future,
{
    type Response = T::Response;
    type Error = T::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.chan.poll_ready()
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let (tx, rx) = oneshot::channel();
        self.chan.unbouned_send((req, tx))
    }
}

pub struct UniquePool<T, F> {
    pool: HashMap<Uri, T>,
    buffering: usize,
    max_buffering: usize,
    enqueuer: Enqueuer<T, F>,
}

impl<T, F> UniquePool<T, F> {
    pub fn new(max: usize) -> UniquePool<T, F> {
        UniquePool {
            pool: HashMap::new(),
            buffering: 0,
            max_buffering: max,
            enqueuer: FuturesRunner::new(),
        }
    }
}

impl<T, F, Request> Service<Request> for UniquePool<T, F>
where
    T: Service<Request> + Clone,
    F: Future,
{
    type Response = T::Response;
    type Error = T::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        if self.buffering >= self.max_buffering {
            Err(())
        } else {
            Ok(Async::Ready(()))
        }
    }

    fn call(&mut self, req: Request) -> Self::Future {
        if let Some(conn) = self.pool.get(req.uri) {
            // We have already established a connection to this destination
            ProtoClient::MembershipService::new(conn)
                .and_then(|client| client.send_request(req.req))
        } else {
            // The connection doesn't exist, so we need to establish it, and then wait for it to resolve
            // and once it does, we need to make the request.
            // All this has to be done in a non-blocking way.
            self.buffering += 1;

            let dst = Destination::try_from_uri(req.uri.clone()).unwrap();
            let connector = util::Connector::new(HttpConnector::new(4));
            let settings = client::Builder::new().http2_only(true).clone();
            let mut make_client = client::Connect::with_builder(connector, settings);

            make_client
                .make_service(dst) // we need to enqueue
                .map_err(|e| panic!("Unable to connect to destination"))
                .and_then(|conn| {
                    use ProtoClient::MembershipService;

                    let conn = tower_request_modifier::Builder::new()
                        .set_origin(req.uri)
                        .build(conn)
                        .unwrap();

                    MembershipService::new(conn).ready() // This should be run on the background process
                })
                .and_then(|mut client| client.send_request(req.request))
        }
    }
}
