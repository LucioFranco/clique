mod client;
pub mod proto;

pub use self::client::Client;

use crate::{
    common::{ConfigId, Endpoint},
    Error,
};
use futures::Stream;
use std::future::Future;
use tokio_sync::oneshot;

pub trait Transport<T> {
    type Error: std::error::Error + Send + 'static;

    type ClientFuture: Future<Output = Result<Response, Self::Error>> + Send;

    fn send(&mut self, req: Request) -> Self::ClientFuture;

    type ServerStream: Stream<
            Item = Result<(Request, oneshot::Sender<crate::Result<Response>>), Self::Error>,
        > + Unpin;
    type ServerFuture: Future<Output = Result<Self::ServerStream, Self::Error>>;

    fn listen_on(&mut self, bind: T) -> Self::ServerFuture;
}

#[derive(Debug)]
pub struct Request {
    target: Endpoint,
    kind: proto::RequestKind,
}

pub struct InboundRequest {
    request: Request,
    response_tx: oneshot::Sender<crate::Result<Response>>,
}

#[derive(Debug)]
pub struct Response {
    kind: proto::ResponseKind,
}

impl Request {
    pub fn new(target: Endpoint, kind: proto::RequestKind) -> Self {
        Self { target, kind }
    }

    pub fn into_inner(self) -> proto::RequestKind {
        self.kind
    }

    pub fn kind(&self) -> &proto::RequestKind {
        &self.kind
    }

    pub fn into_parts(self) -> (Endpoint, proto::RequestKind) {
        (self.target, self.kind)
    }
}

impl Response {
    pub fn new(kind: proto::ResponseKind) -> Self {
        Self { kind }
    }

    pub fn empty() -> Self {
        Self {
            kind: proto::ResponseKind::Response,
        }
    }

    pub fn consensus() -> Self {
        Self {
            kind: proto::ResponseKind::Consensus,
        }
    }

    pub fn new_join(join: proto::JoinResponse) -> Self {
        let kind = proto::ResponseKind::Join(join);
        Self::new(kind)
    }
}
