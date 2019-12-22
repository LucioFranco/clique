use crate::{
    common::Endpoint,
    error::{Error, Result},
    transport::{proto::RequestKind, Request, Response},
};
use futures::{stream::Fuse, StreamExt};
use tokio::sync::{mpsc, oneshot};

use std::collections::VecDequeue;

pub type RequestStream = Fuse<mpsc::Receiver<RequestType>>;

#[derive(Debug, Clone)]
pub struct Client {
    inner: VecDequeue<RequestType>
}

#[derive(Debug)]
pub enum RequestType {
    Unary(Request),
    UnaryNoWait(Request),
    Broadcast(RequestKind),
}

impl RequestType {
    pub fn new_unary(req: Request) -> Self {
        RequestType::Unary(req)
    }

    pub fn new_unary_nowait(req: Request) -> Self {
        RequestType::UnaryNoWait(req)
    }

    pub fn new_broadcast(kind: RequestKind) -> Self {
        RequestType::Broadcast(kind)
    }
}

impl Client {
    pub fn new(bound: usize) -> (Self, RequestStream) {
        let (inner, rx) = mpsc::channel(bound);
        let me = Self { inner };

        (me, rx.fuse())
    }

    pub fn send(&mut self, target: Endpoint, req: RequestKind) {
        let req = Request::new(target, req);

        self.inner.push_back(req);
    }

    pub fn send_no_wait(&mut self, target: Endpoint, req: RequestKind) {
        let req = Request::new(target, req);
        self.inner.push_back(req)
    }

    pub fn broadcast(&mut self, req: RequestKind) -> Result<()> {
        self.inner.push_back(req)
    }

    pub fn get_next(&mut self) -> Option<RequestType> {
        self.inner.pop_front()
    }
}
