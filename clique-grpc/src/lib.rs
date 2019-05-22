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

struct FuturesRunner<Request, T: Service<Request>> {
    inner: FuturesUnordered<Box<dyn Future<Item = T::Response, Error = ()>>>,
    rx: mpsc::UnboundedReceiver<(Request, oneshot::Sender<T::Response>)>,
}

impl<Request, T> FuturesRunner<Request, T>
where
    T: Service<Request> + Clone,
    Request: Address,
{
    pub fn new() -> mpsc::UnboundedSender<(Request, oneshot::Sender<T::Response>)> {
        let (tx, rx) = mpsc::unbounded();
        tokio::spawn(FuturesRunner {
            inner: FuturesUnordered::new(),
            rx,
        });

        tx
    }

    fn poll_enqueue(&mut self) {
        self.rx
            .for_each(|uri| {
                let dst = Destination::try_from_uri(uri.clone()).unwrap();
                let connector = util::Connector::new(HttpConnector::new(4));
                let settings = client::Builder::new().http2_only(true).clone();
                let mut make_client = client::Connect::with_builder(connector, settings);

                let client = make_client
                    .make_service(dst) // we need to enqueue
                    .map_err(|e| panic!("Unable to connect to destination"));

                self.inner.push(Box::new(client));
                Ok(())
            })
            .poll();
    }
}

impl<Request, T> Future for FuturesRunner<Request, T>
where
    T: Service<Request> + Clone,
    Request: Address,
{
    type Item = T::Response;
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

pub struct UniquePool<Request, T>
where
    T: Service<Request> + Clone,
{
    pool: HashMap<Uri, T::Response>,
    buffering: usize,
    max_buffering: usize,
    enqueuer: mpsc::UnboundedSender<(Request, oneshot::Sender<T::Response>)>,
}

impl<Request, T> UniquePool<Request, T>
where
    T: Service<Request> + Clone,
    Request: Address,
{
    pub fn new(max: usize) -> UniquePool<Request, T> {
        UniquePool {
            pool: HashMap::new(),
            buffering: 0,
            max_buffering: max,
            enqueuer: FuturesRunner::new(),
        }
    }
}

trait Address {
    type Address: Clone;
    type Payload: Clone;

    fn get_uri(&self) -> Self::Address;
    fn get_payload(&self) -> Self::Payload;
}

impl<Request, T> Service<Request> for UniquePool<Request, T>
where
    T: Service<Request> + Clone,
    Request: Address,
{
    type Response = T::Response;
    type Error = (); // Need better error handling
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        if self.buffering >= self.max_buffering {
            Err(())
        } else {
            Ok(Async::Ready(()))
        }
    }

    fn call(&mut self, req: Request) -> Self::Future {
        if let Some(conn) = self.pool.get(req.get_uri()) {
            // We have already established a connection to this destination
            ProtoClient::MembershipService::new(conn)
                .and_then(|client| client.send_request(req.req))
        } else {
            // The connection doesn't exist, so we need to establish it, and then wait for it to resolve
            // and once it does, we need to make the request.
            // All this has to be done in a non-blocking way.
            self.buffering += 1;

            self.enqueuer
                .unbounded_send(request.get_uri())
                .map_err(|e| panic!("Unable to connect to destination"))
                .and_then(|conn| {
                    use ProtoClient::MembershipService;

                    let conn = tower_request_modifier::Builder::new()
                        .set_origin(req.get_uri())
                        .build(conn)
                        .unwrap();

                    Ok(MembershipService::new(conn)) // This should be run on the background process
                })
                .and_then(|mut client| client.send_request(req.get_payload()))
                .map_err(|e| println!("ERROR making request"))
                .map(|val| val.ok())
        }
    }
}
