use std::error;
use std::fmt;

use bytes::Bytes;
use clique::transport::{proto, Request, Response};
use uuid::Uuid;

pub mod membership {
    tonic::include_proto!("clique");
}

mod server;
mod transport;

pub use transport::TonicTransport;

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
            Content::ProbeResponse(membership::ProbeResponse { status: _ }) => {
                todo!()
                // Response::new_probe(status)
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

impl From<membership::RapidRequest> for Request {
    fn from(mut req: membership::RapidRequest) -> Self {
        use membership::rapid_request::Content;

        let content = req.content.take().expect("Did not get a valid message");

        let kind = match content {
            Content::PreJoinMessage(msg) => proto::RequestKind::PreJoin(msg.into()),
            Content::JoinMessage(msg) => proto::RequestKind::Join(msg.into()),
            Content::BatchedAlertMessage(msg) => proto::RequestKind::BatchedAlert(msg.into()),
            Content::ProbeMessage(_) => proto::RequestKind::Probe,
            Content::FastRoundPhase2bMessage(msg) => {
                proto::RequestKind::Consensus(proto::Consensus::FastRoundPhase2bMessage(msg.into()))
            }
            Content::Phase1aMessage(msg) => {
                proto::RequestKind::Consensus(proto::Consensus::Phase1aMessage(msg.into()))
            }
            Content::Phase1bMessage(msg) => {
                proto::RequestKind::Consensus(proto::Consensus::Phase1bMessage(msg.into()))
            }
            Content::Phase2aMessage(msg) => {
                proto::RequestKind::Consensus(proto::Consensus::Phase2aMessage(msg.into()))
            }
            Content::Phase2bMessage(msg) => {
                proto::RequestKind::Consensus(proto::Consensus::Phase2bMessage(msg.into()))
            }
        };

        // The target here is the current node, so we don't need to populate it.
        // TODO: FIXME, need to represent this better
        Request::new("".into(), kind)
    }
}

impl From<membership::PreJoinMessage> for proto::PreJoinMessage {
    fn from(mut r: membership::PreJoinMessage) -> Self {
        // we only care about these two fields for some reason
        proto::PreJoinMessage {
            sender: extract_optional!(r, sender, "Unable to get sender from PreJoinMessage"),
            node_id: extract_optional!(r, node_id, "Unable to get node Id from PreJoinMessage"),
            ring_number: r.ring_number,
            config_id: Some(r.configuration_id),
        }
    }
}

impl From<membership::JoinMessage> for proto::JoinMessage {
    fn from(mut r: membership::JoinMessage) -> Self {
        proto::JoinMessage {
            sender: extract_optional!(r, sender, "Unable to get sender from JoinMessage"),
            node_id: extract_optional!(r, node_id, "Unable to get node Id from PreJoinMessage"),
            ring_number: r.ring_number,
            config_id: r.configuration_id,
            metadata: r.metadata.map(|m| m.into()),
        }
    }
}

impl From<membership::BatchedAlertMessage> for proto::BatchedAlertMessage {
    fn from(mut r: membership::BatchedAlertMessage) -> Self {
        proto::BatchedAlertMessage {
            sender: extract_optional!(
                r,
                sender,
                "Umable to extract sender from BatchedAlertMessage"
            ),
            alerts: r.messages.into_iter().map(|m| m.into()).collect(),
        }
    }
}

impl From<membership::AlertMessage> for proto::Alert {
    fn from(mut r: membership::AlertMessage) -> Self {
        let status = match r.edge_status {
            0 => proto::EdgeStatus::Up,
            1 => proto::EdgeStatus::Down,
            _ => panic!("Incorrect edge status received"),
        };

        proto::Alert {
            src: extract_optional!(r, edge_src, "Unable to extract edge_src from AlertMessage"),
            dst: extract_optional!(r, edge_dst, "Unable to extract edge_dst from AlertMessage"),
            edge_status: status,
            config_id: r.configuration_id,
            ring_number: r.ring_number,
            node_id: r.node_id.map(|n| n.into()),
            metadata: r.metadata.map(|m| m.into()),
        }
    }
}

impl From<membership::EdgeStatus> for proto::EdgeStatus {
    fn from(r: membership::EdgeStatus) -> Self {
        match r {
            membership::EdgeStatus::Up => proto::EdgeStatus::Up,
            membership::EdgeStatus::Down => proto::EdgeStatus::Down,
        }
    }
}

impl From<membership::FastRoundPhase2bMessage> for proto::FastRoundPhase2bMessage {
    fn from(mut r: membership::FastRoundPhase2bMessage) -> Self {
        proto::FastRoundPhase2bMessage {
            sender: extract_optional!(
                r,
                sender,
                "Unable to extract sender from FastRoundPhase2bMessage"
            ),
            config_id: r.configuration_id,
            endpoints: r.endpoints.into_iter().map(|pt| pt.into()).collect(),
        }
    }
}

impl From<membership::Phase1aMessage> for proto::Phase1aMessage {
    fn from(mut r: membership::Phase1aMessage) -> Self {
        proto::Phase1aMessage {
            sender: extract_optional!(r, sender, "Unable to extract sender from Phase1aMessage"),
            config_id: r.configuration_id,
            rank: extract_optional!(r, rank, "Unable to extract rank from Phase1aMessage"),
        }
    }
}

impl From<membership::Phase1bMessage> for proto::Phase1bMessage {
    fn from(mut r: membership::Phase1bMessage) -> Self {
        proto::Phase1bMessage {
            sender: extract_optional!(r, sender, "Unable to extract sender from Phase1bMessage"),
            config_id: r.configuration_id,
            rnd: extract_optional!(r, rnd, "Unable to extract rnd from Phase1aMessage"),
            vrnd: extract_optional!(r, vrnd, "Unable to extract vrnd from Phase1aMessage"),
            vval: r.vval.into_iter().map(|val| val.into()).collect(),
        }
    }
}

impl From<membership::Phase2aMessage> for proto::Phase2aMessage {
    fn from(mut r: membership::Phase2aMessage) -> Self {
        proto::Phase2aMessage {
            sender: extract_optional!(r, sender, "Unable to extract sender from Phase1bMessage"),
            config_id: r.configuration_id,
            rnd: extract_optional!(r, rnd, "Unable to extract rnd from Phase1aMessage"),
            vval: r.vval.into_iter().map(|val| val.into()).collect(),
        }
    }
}

impl From<membership::Phase2bMessage> for proto::Phase2bMessage {
    fn from(mut r: membership::Phase2bMessage) -> Self {
        proto::Phase2bMessage {
            sender: extract_optional!(r, sender, "Unable to extract sender from Phase1bMessage"),
            config_id: r.configuration_id,
            rnd: extract_optional!(r, rnd, "Unable to extract rnd from Phase1aMessage"),
            endpoints: r.endpoints.into_iter().map(|val| val.into()).collect(),
        }
    }
}

impl From<membership::Rank> for proto::Rank {
    fn from(r: membership::Rank) -> Self {
        proto::Rank {
            round: r.round,
            node_index: r.node_index,
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
        println!("uuid: {:?}", r.uuid);
        Uuid::parse_str(&r.uuid)
            .expect("Unable to parse UUID")
            .into()
    }
}

// And the other way around

impl From<Response> for membership::RapidResponse {
    fn from(r: Response) -> Self {
        use membership::rapid_response::Content;
        use proto::ResponseKind;

        let content = match r.into_inner() {
            ResponseKind::Join(m) => Content::JoinResponse(m.into()),
            ResponseKind::Response => Content::Response(membership::EmptyResponse {}),
            ResponseKind::Consensus => Content::ConsensusResponse(membership::ConsensusResponse {}),
            ResponseKind::Probe(_status) => {
                todo!()
                // Content::ProbeResponse(membership::ProbeResponse { status })
            }
        };

        membership::RapidResponse {
            content: Some(content),
        }
    }
}

impl From<proto::JoinResponse> for membership::JoinResponse {
    fn from(j: proto::JoinResponse) -> Self {
        let status_code = match j.status {
            proto::JoinStatus::HostnameAlreadyInRing => 0,
            proto::JoinStatus::NodeIdAlreadyInRing => 1,
            proto::JoinStatus::SafeToJoin => 2,
            proto::JoinStatus::ConfigChanged => 3,
            proto::JoinStatus::MembershipRejected => 4,
        };

        membership::JoinResponse {
            sender: Some(j.sender.into()),
            status_code,
            configuration_id: j.config_id,
            endpoints: j.endpoints.into_iter().map(|val| val.into()).collect(),
            identifiers: j.identifiers.into_iter().map(|id| id.into()).collect(),
            cluster_metadata: j
                .cluster_metadata
                .into_iter()
                .map(|(key, val)| (key, val.into()))
                .collect(),
        }
    }
}

impl From<proto::RequestKind> for membership::RapidRequest {
    fn from(r: proto::RequestKind) -> Self {
        use membership::rapid_request::Content;
        use proto::{Consensus, RequestKind};

        let content = match r {
            RequestKind::PreJoin(m) => Content::PreJoinMessage(m.into()),
            RequestKind::Join(m) => Content::JoinMessage(m.into()),
            RequestKind::BatchedAlert(m) => Content::BatchedAlertMessage(m.into()),
            RequestKind::Probe => Content::ProbeMessage(membership::ProbeMessage {
                sender: None,
                payload: vec![],
            }),
            RequestKind::Consensus(Consensus::FastRoundPhase2bMessage(m)) => {
                Content::FastRoundPhase2bMessage(m.into())
            }
            RequestKind::Consensus(Consensus::Phase1aMessage(m)) => {
                Content::Phase1aMessage(m.into())
            }
            RequestKind::Consensus(Consensus::Phase1bMessage(m)) => {
                Content::Phase1bMessage(m.into())
            }
            RequestKind::Consensus(Consensus::Phase2aMessage(m)) => {
                Content::Phase2aMessage(m.into())
            }
            RequestKind::Consensus(Consensus::Phase2bMessage(m)) => {
                Content::Phase2bMessage(m.into())
            }
        };

        membership::RapidRequest {
            content: Some(content),
        }
    }
}

impl From<proto::PreJoinMessage> for membership::PreJoinMessage {
    fn from(p: proto::PreJoinMessage) -> Self {
        membership::PreJoinMessage {
            sender: Some(p.sender.into()),
            node_id: Some(p.node_id.into()),
            ring_number: p.ring_number,
            configuration_id: p.config_id.unwrap_or(-1),
        }
    }
}

impl From<proto::JoinMessage> for membership::JoinMessage {
    fn from(j: proto::JoinMessage) -> Self {
        membership::JoinMessage {
            sender: Some(j.sender.into()),
            node_id: Some(j.node_id.into()),
            ring_number: j.ring_number,
            configuration_id: j.config_id,
            metadata: j.metadata.map(|m| m.into()),
        }
    }
}

impl From<proto::BatchedAlertMessage> for membership::BatchedAlertMessage {
    fn from(b: proto::BatchedAlertMessage) -> Self {
        membership::BatchedAlertMessage {
            sender: Some(b.sender.into()),
            messages: b.alerts.into_iter().map(|m| m.into()).collect(),
        }
    }
}

impl From<proto::Alert> for membership::AlertMessage {
    fn from(a: proto::Alert) -> Self {
        let status = match a.edge_status {
            proto::EdgeStatus::Up => 0,
            proto::EdgeStatus::Down => 1,
        };

        membership::AlertMessage {
            edge_src: Some(a.src.into()),
            edge_dst: Some(a.dst.into()),
            edge_status: status,
            ring_number: a.ring_number,
            node_id: a.node_id.map(|n| n.into()),
            metadata: a.metadata.map(|m| m.into()),
            configuration_id: a.config_id,
        }
    }
}

impl From<proto::EdgeStatus> for membership::EdgeStatus {
    fn from(p: proto::EdgeStatus) -> Self {
        match p {
            proto::EdgeStatus::Up => membership::EdgeStatus::Up,
            proto::EdgeStatus::Down => membership::EdgeStatus::Down,
        }
    }
}

impl From<proto::FastRoundPhase2bMessage> for membership::FastRoundPhase2bMessage {
    fn from(p: proto::FastRoundPhase2bMessage) -> Self {
        membership::FastRoundPhase2bMessage {
            sender: Some(p.sender.into()),
            configuration_id: p.config_id,
            endpoints: p.endpoints.into_iter().map(|val| val.into()).collect(),
        }
    }
}
impl From<proto::Phase1aMessage> for membership::Phase1aMessage {
    fn from(p: proto::Phase1aMessage) -> Self {
        membership::Phase1aMessage {
            sender: Some(p.sender.into()),
            configuration_id: p.config_id,
            rank: Some(p.rank.into()),
        }
    }
}

impl From<proto::Phase1bMessage> for membership::Phase1bMessage {
    fn from(p: proto::Phase1bMessage) -> Self {
        membership::Phase1bMessage {
            sender: Some(p.sender.into()),
            configuration_id: p.config_id,
            rnd: Some(p.rnd.into()),
            vrnd: Some(p.vrnd.into()),
            vval: p.vval.into_iter().map(|val| val.into()).collect(),
        }
    }
}

impl From<proto::Phase2aMessage> for membership::Phase2aMessage {
    fn from(p: proto::Phase2aMessage) -> Self {
        membership::Phase2aMessage {
            sender: Some(p.sender.into()),
            configuration_id: p.config_id,
            rnd: Some(p.rnd.into()),
            vval: p.vval.into_iter().map(|val| val.into()).collect(),
        }
    }
}

impl From<proto::Phase2bMessage> for membership::Phase2bMessage {
    fn from(p: proto::Phase2bMessage) -> Self {
        membership::Phase2bMessage {
            sender: Some(p.sender.into()),
            configuration_id: p.config_id,
            rnd: Some(p.rnd.into()),
            endpoints: p.endpoints.into_iter().map(|val| val.into()).collect(),
        }
    }
}

impl From<proto::Rank> for membership::Rank {
    fn from(r: proto::Rank) -> Self {
        membership::Rank {
            round: r.round,
            node_index: r.node_index,
        }
    }
}

impl From<proto::Metadata> for membership::Metadata {
    fn from(m: proto::Metadata) -> Self {
        membership::Metadata {
            metadata: m
                .metadata
                .into_iter()
                .map(|(key, val)| (key, val.into_iter().collect()))
                .collect(),
        }
    }
}

impl From<clique::Endpoint> for membership::Endpoint {
    fn from(e: clique::Endpoint) -> Self {
        let addr: std::net::SocketAddr = e.parse().expect("Unable to parse endpoint");

        membership::Endpoint {
            hostname: format!("{}", addr.ip()),
            port: addr.port() as i32,
        }
    }
}

impl From<clique::NodeId> for membership::NodeId {
    fn from(n: clique::NodeId) -> Self {
        membership::NodeId {
            uuid: n.to_string(),
        }
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
