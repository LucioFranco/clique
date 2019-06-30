use clique::transport::{proto::RequestKind, Response};
use std::{error, fmt};

mod proto {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/clique.rs"));
}

mod client;
mod server;

pub use client::GrpcClient;
pub use server::GrpcServer;

#[derive(Debug)]
// TODO: add actual errors here!
pub enum Error {
    Unknown,
}

impl From<Response> for proto::Response {
    fn from(_r: Response) -> Self {
        unimplemented!()
    }
}

impl From<proto::Request> for RequestKind {
    fn from(_r: proto::Request) -> Self {
        unimplemented!()
    }
}

impl From<RequestKind> for proto::Request {
    fn from(_r: RequestKind) -> Self {
        unimplemented!()
    }
}

impl From<proto::Response> for Response {
    fn from(_r: proto::Response) -> Self {
        unimplemented!()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl error::Error for Error {}
