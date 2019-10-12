use std::error;
use std::fmt;

use clique::transport::{Request, Response};

pub mod membership {
    tonic::include_proto!("clique");
}

mod server;
pub mod transport;

impl From<membership::RapidResponse> for Response {
    fn from(_r: membership::RapidResponse) -> Self {
        unimplemented!()
    }
}

impl From<membership::RapidRequest> for Request {
    fn from(_r: membership::RapidRequest) -> Self {
        unimplemented!();
    }
}

impl From<Response> for membership::RapidResponse {
    fn from(_r: Response) -> Self {
        unimplemented!()
    }
}

impl From<Request> for membership::RapidRequest {
    fn from(_r: Request) -> Self {
        unimplemented!()
    }
}

#[derive(Debug)]
pub enum Error {
    //TODO: add actual errors
    Upstream,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl error::Error for Error {}
