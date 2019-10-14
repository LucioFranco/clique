use std::error;
use std::fmt;

use bytes::Bytes;
use clique::transport::{proto, Request, Response};
use uuid::Uuid;

pub mod membership {
    tonic::include_proto!("clique");
}

mod server;
pub mod transport;

/// Simple macro to extract an optional field from proto buf messgae.
/// Keep in mind, the field needs to impl From<T> for U where T is the
/// proto buf field type and U, the internal struct field type.
macro_rules! extract_optional {
    ($st:expr, $field:ident, $msg:tt) => {
        $st.$field.take().expect($msg).into()
    };
}

impl From<membership::RapidResponse> for Response {
    fn from(mut res: membership::RapidResponse) -> Self {
        use membership::rapid_response::Content;

        let content = res.content.take().expect("Did not get a valid message");

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

impl From<membership::JoinResponse> for proto::JoinResponse {
    fn from(mut r: membership::JoinResponse) -> Self {
        let status_code = match r.status_code {
            0 => proto::JoinStatus::HostnameAlreadyInRing,
            1 => proto::JoinStatus::NodeIdAlreadyInRing,
            2 => proto::JoinStatus::SafeToJoin,
            3 => proto::JoinStatus::ConfigChanged,
            4 => proto::JoinStatus::MembershipRejected,
            _ => panic!("This should never happen"),
        };
        proto::JoinResponse {
            sender: r.sender.take().unwrap().into(),
            status: status_code,
            config_id: r.configuration_id,
            endpoints: r.endpoints.into_iter().map(|pt| pt.into()).collect(),
            identifiers: r.identifiers.into_iter().map(|id| id.into()).collect(),
            cluster_metadata: r
                .cluster_metadata
                .into_iter()
                .map(|(key, val)| (key, val.into()))
                .collect(),
        }
    }
}

impl From<membership::Metadata> for proto::Metadata {
    fn from(r: membership::Metadata) -> Self {
        proto::Metadata {
            metadata: r
                .metadata
                .into_iter()
                .map(|(key, val)| (key, Bytes::from(val)))
                .collect(),
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
        Uuid::parse_str(&r.uuid)
            .expect("Unable to parse UUID")
            .into()
    }
}

impl From<membership::RapidRequest> for Request {
    fn from(_r: membership::RapidRequest) -> Self {
        unimplemented!();
    }
}

impl From<membership::PreJoinMessage> for proto::PreJoinMessage {
    fn from(r: membership::PreJoinMessage) -> Self {
        // we only care about these two fields for some reason
        proto::PreJoinMessage {
            sender: extract_optional!(r, sender, "Unable to get sender from PreJoinMessage"),
            node_id: extract_optional!(r, node_id, "Unable to get node Id from PreJoinMessage"),
        }
    }
}

impl From<membership::JoinMessage> for proto::JoinMessage {
    fn from(r: membership::JoinMessage) -> Self {
        proto::JoinMessage {
            sender: extract_optional!(r, sender, "Unable to get sender from JoinMessage"),
            node_id: extract_optional!(r, node_id, "Unable to get node Id from PreJoinMessage"),
            ring_number: r.ring_number,
            config_id: r.configuraion_id,
        }
    }
}

impl From<membership::BatchedAlertMessage> for proto::BatchedAlertMessage {
    fn from(r: membershipBatchedAlertMessage) -> Self {
        proto::BatchedAlertMessage {
            sender: extract_optional!(
                r,
                sender,
                "Umable to extract sender from BatchedAlertMessage"
            ),
            alerts: r.messgaes.into_iter().map(|m| m.into()).collect(),
        }
    }
}

impl From<membership::AlertMessage> for proto::Alert {
    fn from(r: membership::AlertMessage) -> Self {
        proto::Alert {
            src: extract_optional!(r, edge_src, "Unable to extract edge_src from AlertMessage"),
            dst: extract_optional!(r, edge_dst, "Unable to extract edge_dst from AlertMessage"),
            edge_status: extract_optional!(
                r,
                edge_status,
                "Unable to extract edge_status from AlertMessage"
            ),
            config_id: r.configuraion_id,
            ring_number: r.ring_number,
            node_id: extract_optional!(r, node_id, "Unable to extract node Id from AlertMessage"),
            metadata: extract_optional!(r, metadata, "Unable to extract metdata from AlertMessage"),
        }
    }
}

impl from<membership::EdgeStatus> for proto::EdgeStatus {
    fn from(r: membership::EdgeStatus) -> Self {
        match r {
            membership::EdgeStatus::Up => proto::EdgeStatus::Up,
            membership::EdgeStatus::Down => proto::EdgeStatus::Down,
        }
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
