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

struct Message {
    uri: Uri,
    payload: RapidRequest,
}

pub struct UniquePool<Request, T>
where
    T: Service<Request> + Clone,
{
    pool: HashMap<Uri, T::Response>,
    buffering: usize,
    max_buffering: usize,
}

impl<Message, T> UniquePool<Message, T>
where
    T: Service<Message> + Clone,
{
    pub fn new(max: usize) -> UniquePool<Message, T> {
        UniquePool {
            pool: HashMap::new(),
            buffering: 0,
            max_buffering: max,
        }
    }
}

impl<Message, T> Service<Message> for UniquePool<Message, T>
where
    T: Service<Message> + Clone,
{
    type Response = T::Response;
    type Error = Box<std::error::Error + Send + Sync + 'static>; // Need better error handling
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        if self.buffering >= self.max_buffering {
            Err(().into())
        } else {
            Ok(Async::Ready(()))
        }
    }

    fn call(&mut self, request: Message) -> Self::Future {
        if let Some(conn) = self.pool.get(&request.uri) {
            // We have already established a connection to this destination
            ProtoClient::MembershipService::new(conn)
                .and_then(|client| Box::new(client.send_request(request.payload)))
        } else {
            // The connection doesn't exist, so we need to establish it, and then wait for it to resolve
            // and once it does, we need to make the request.
            // All this has to be done in a non-blocking way.
            self.buffering += 1;

            let dst = Destination::try_from_uri(request.uri.clone()).unwrap();
            let connector = util::Connector::new(HttpConnector::new(4));
            let settings = client::Builder::new().http2_only(true).clone();
            let mut make_client = client::Connect::with_builder(connector, settings);

            make_client
                .make_service(dst)
                .map_err(Into::into)
                .and_then(move |conn| {
                    let conn = tower_request_modifier::Builder::new()
                        .set_origin(request.uri)
                        .build()
                        .unwrap();

                    self.pool.insert(conn);
                    Ok(ProtoClient::MembershipService::new(conn))
                })
                .and_then(|mut client| Box::new(client.send_request(request.payload)))
                .map_err(Into::into)
        }
    }
}
