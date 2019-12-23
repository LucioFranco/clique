pub mod proto;

use crate::common::Endpoint;
use futures::Stream;
use std::future::Future;
use tokio::sync::oneshot;

pub type StreamItem = (Request, oneshot::Sender<crate::Result<Response>>);

pub trait Transport<T> {
    type Error: std::error::Error + Send + 'static;

    type ClientFuture: Future<Output = Result<Response, Self::Error>> + Send + 'static;

    fn send(&mut self, req: Request) -> Self::ClientFuture;

    type ServerStream: Stream<Item = StreamItem> + Send + Unpin;
    type ServerFuture: Future<Output = Result<Self::ServerStream, Self::Error>>;

    fn listen_on(&mut self, bind: T) -> Self::ServerFuture;
}

#[async_trait::async_trait]
pub trait Transport2 {
    type Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>;

    /// This async should not wait for a response.
    async fn send_to(&mut self, dst: Endpoint, message: Message) -> Result<(), Self::Error>;

    async fn recv(&mut self) -> Result<(Endpoint, Message), Self::Error>;
}

#[derive(Debug, Clone)]
pub enum Message {
    Request(proto::RequestKind),
    Response(proto::ResponseKind),
}

impl From<proto::RequestKind> for Message {
    fn from(t: proto::RequestKind) -> Self {
        Message::Request(t)
    }
}

impl From<proto::ResponseKind> for Message {
    fn from(t: proto::ResponseKind) -> Self {
        Message::Response(t)
    }
}

#[derive(Debug)]
pub struct Request {
    target: Endpoint,
    kind: proto::RequestKind,
}

// pub struct InboundRequest {
//     request: Request,
//     response_tx: oneshot::Sender<crate::Result<Response>>,
// }

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

    pub fn kind(&self) -> &proto::ResponseKind {
        &self.kind
    }

    pub fn consensus() -> Self {
        Self {
            kind: proto::ResponseKind::Consensus,
        }
    }

    pub fn into_inner(self) -> proto::ResponseKind {
        self.kind
    }

    pub fn new_join(join: proto::JoinResponse) -> Self {
        let kind = proto::ResponseKind::Join(join);
        Self::new(kind)
    }

    pub fn new_probe(status: proto::NodeStatus) -> Self {
        let kind = proto::ResponseKind::Probe(status);
        Self::new(kind)
    }
}
