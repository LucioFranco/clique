use std::error;
use std::fmt;

use clique::transport::{proto::RequestKind, Response};

pub mod membership {
    tonic::include_proto!("clique");
}

#[derive(Debug)]
pub enum Error {
    //TODO: add actual errors
}

impl From<Response> for membership::RapidRequest {
    fn from(_r: Response) -> Self {
        unimplemented!()
    }
}

impl From<membership::RapidRequest> for RequestKind {
    fn from(_r: membership::RapidRequest) -> Self {
        unimplemented!()
    }
}

impl From<RequestKind> for membership::RapidRequest {
    fn from(_r: RequestKind) -> Self {
        unimplemented!()
    }
}

impl From<membership::RapidResponse> for Response {
    fn from(_r: membership::RapidResponse) -> Self {
        unimplemented!()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl error::Error for Error {}
