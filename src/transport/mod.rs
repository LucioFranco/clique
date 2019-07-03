pub mod proto;

use crate::Error;
use futures::Stream;
use std::future::Future;
use tokio_sync::oneshot;

pub trait Client {
    type Error: std::error::Error;
    type Future: Future<Output = Result<Response, Self::Error>>;

    fn call(&mut self, req: Request) -> Self::Future;
}

pub trait Server<T> {
    type Error: std::error::Error;
    type Stream: Stream<Item = Result<Request, Self::Error>> + Unpin;
    type Future: Future<Output = Result<Self::Stream, Self::Error>>;

    fn start(&mut self, target: T) -> Self::Future;
}

#[derive(Debug)]
pub struct Request {
    kind: proto::RequestKind,
    res_tx: oneshot::Sender<crate::Result<Response>>,
}

#[derive(Debug)]
pub struct Response {
    kind: proto::ResponseKind,
}

impl Request {
    pub fn new(res_tx: oneshot::Sender<crate::Result<Response>>, kind: proto::RequestKind) -> Self {
        Self { res_tx, kind }
    }

    pub fn into_inner(self) -> proto::RequestKind {
        self.kind
    }

    pub fn respond(self, res: Response) -> crate::Result<()> {
        self.res_tx
            .send(Ok(res))
            // TODO: prob should be transport dropped
            .map_err(|_| Error::new_broken_pipe(None))
    }

    pub fn kind(&self) -> &proto::RequestKind {
        &self.kind
    }
}
