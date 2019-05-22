use futures::{
    stream::futures_unordered::FuturesUnordered,
    sync::{mpsc, oneshot},
    Async, Future, Poll, Stream,
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

use clique_proto::client::{self as ProtoClient, MembershipService};
use clique_proto::RapidRequest;

struct PoolReq {
    uri: Uri,
    req: RapidRequest,
}

impl BackgroundJob for PoolReq {
    type Req = Uri;
    type Resp = Connection<Body>;
}

struct FuturesRunner<T: BackgroundJob> {
    inner: FuturesUnordered<Box<dyn Future<Item = T::Resp, Error = ()>>>,
    rx: mpsc::UnboundedReceiver<(T::Req, oneshot::Sender<T::Resp>)>,
}

impl<T> FuturesRunner<T>
where
    T: BackgroundJob,
{
    pub fn new() -> Enqueuer<T> {
        let (tx, rx) = mpsc::unbounded();
        tokio::spawn(FuturesRunner {
            inner: FuturesUnordered::new(),
            rx,
        });

        Enqueuer { chan: tx }
    }

    fn poll_enqueue(&mut self) {
        self.rx.for_each(|uri| {
            let dst = Destination::try_from_uri(uri.clone()).unwrap();
            let connector = util::Connector::new(HttpConnector::new(4));
            let settings = client::Builder::new().http2_only(true).clone();
            let mut make_client = client::Connect::with_builder(connector, settings);

            let client = make_client
                .make_service(dst) // we need to enqueue
                .map_err(|e| panic!("Unable to connect to destination"));

            self.inner.push(Box::new(client));
        });
    }
}

impl<T> Future for FuturesRunner<T>
where
    T: BackgroundJob,
{
    type Item = T::Resp;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // Check if there are items to be run
        self.poll_enqueue();

        match self.inner.poll() {
            Ok(Async::Ready(None)) => Ok(Async::NotReady), // There are no futures to run
            Ok(Async::Ready(Some(val))) => Ok(Async::Ready(val)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}

trait BackgroundJob {
    type Req: Clone;
    type Resp;
}

#[derive(Debug, Clone)]
struct Enqueuer<T: BackgroundJob> {
    chan: mpsc::UnboundedSender<(T::Req, oneshot::Sender<T::Resp>)>,
}

impl<T> Service<T::Req> for Enqueuer<T>
where
    T: BackgroundJob,
{
    type Response = T::Resp;
    type Error = ();
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.chan.poll_ready()
    }

    fn call(&mut self, req: T::Req) -> Self::Future {
        let (tx, rx) = oneshot::channel();
        self.chan.unbouned_send((req, tx))
    }
}

pub struct UniquePool<Req, T>
where
    Req: BackgroundJob,
    T: Service<Req>,
{
    pool: HashMap<Uri, T>,
    buffering: usize,
    max_buffering: usize,
    enqueuer: Enqueuer<Req>,
}

impl<Req, T> UniquePool<Req, T>
where
    Req: BackgroundJob,
    T: Service<Req>,
{
    pub fn new(max: usize) -> UniquePool<Req, T> {
        UniquePool {
            pool: HashMap::new(),
            buffering: 0,
            max_buffering: max,
            enqueuer: FuturesRunner::new(),
        }
    }
}

impl<Req, T> Service<Req> for UniquePool<Req, T>
where
    T: Service<Req> + Clone,
    Req: BackgroundJob,
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

    fn call(&mut self, req: Req) -> Self::Future {
        if let Some(conn) = self.pool.get(req.uri) {
            // We have already established a connection to this destination
            ProtoClient::MembershipService::new(conn)
                .and_then(|client| client.send_request(req.req))
        } else {
            // The connection doesn't exist, so we need to establish it, and then wait for it to resolve
            // and once it does, we need to make the request.
            // All this has to be done in a non-blocking way.
            self.buffering += 1;

            let request = req as PoolReq;

            self.enqueuer
                .call(request.uri)
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
