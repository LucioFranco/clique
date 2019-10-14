use std::error;
use std::fmt;

use clique::transport::{proto, Request, Response};
use uuid::Uuid;

pub mod membership {
    tonic::include_proto!("clique");
}

mod server;
pub mod transport;

impl From<membership::RapidResponse> for Response {
    fn from(res: membership::RapidResponse) -> Self {
        use membership::rapid_response::Content;

        let membership::RapidResponse {
            content: Some(content),
        } = res;

        match content {
            Content::JoinResponse(res) => Response::new_join(res.into()),
            Content::Response(_) => Response::empty(),
            Content::ConsensusResponse(_) => Response::consensus(),
            Content::ProbeResponse(membership::ProbeResponse { status }) => {
                Response::new_probe(status)
            }
        }
    }
}

impl From<membership::RapidRequest> for Request {
    fn from(_r: membership::RapidRequest) -> Self {
        unimplemented!();
    }
}

impl From<membership::JoinResponse> for proto::JoinResponse {
    fn from(r: membership::JoinResponse) -> Self {
        proto::JoinResponse {
            sender: r.sender.take().unwrap().into(),
            status: r.status_code,
            config_id: r.configuration_id,
            endpoints: r.endpoints.into_iter().map(|pt| pt.into()).collect(),
            identifiers: r.identifiers.into_iter().map(|id| id.into()).collect(),
            cluster_metadata: r.cluster_metadata,
        }
    }
}

impl From<membership::Endpoint> for clique::Endpoint {
    fn from(r: membership::Endpoint) -> Self {
        format!("{}:{}", r.hostname, r.port)
    }
}

impl From<membership::NodeId> for clique::NodeId {
    fn from(r: membership::NodeId) -> Self {
        Uuid::from_bytes(r.uuid.into_bytes()).into()
    }
}

impl From<Response> for membership::RapidResponse {
    fn from(_r: Response) -> Self {
        unimplemented!()
    }
}

impl From<proto::RequestKind> for membership::RapidRequest {
    fn from(_r: proto::RequestKind) -> Self {
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
