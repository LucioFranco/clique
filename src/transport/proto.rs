use crate::common::{ConfigId, Endpoint, NodeId, RingNumber};
use bytes::Bytes;

use std::collections::HashMap;
use std::default::Default;

#[derive(Debug, Clone, PartialEq)]
pub enum RequestKind {
    PreJoin(PreJoinMessage),
    Join(JoinMessage),
    BatchedAlert(BatchedAlertMessage),
    Probe,
    Consensus(Consensus),
}

pub type Status = i32;

#[derive(Debug, Clone, PartialEq)]
pub enum ResponseKind {
    Join(JoinResponse),
    Response,
    Probe(Status),
    Consensus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Consensus {
    FastRoundPhase2bMessage(FastRoundPhase2bMessage),
    Phase1aMessage(Phase1aMessage),
    Phase1bMessage(Phase1bMessage),
    Phase2aMessage(Phase2aMessage),
    Phase2bMessage(Phase2bMessage),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreJoinMessage {
    pub sender: Endpoint,
    pub node_id: NodeId,
    pub ring_number: Vec<RingNumber>,
    pub config_id: Option<ConfigId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JoinMessage {
    pub sender: Endpoint,
    pub node_id: NodeId,
    pub ring_number: Vec<RingNumber>,
    pub config_id: ConfigId,
    pub metadata: Option<Metadata>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JoinResponse {
    pub sender: Endpoint,
    pub status: JoinStatus,
    pub config_id: ConfigId,
    pub endpoints: Vec<Endpoint>,
    pub identifiers: Vec<NodeId>,
    pub cluster_metadata: HashMap<String, Metadata>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmptyResponse;

#[derive(Debug, PartialEq, Clone)]
pub enum JoinStatus {
    HostnameAlreadyInRing,
    NodeIdAlreadyInRing,
    SafeToJoin,
    ConfigChanged,
    MembershipRejected,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FastRoundPhase2bMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phase1aMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub rank: Rank,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phase1bMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub rnd: Rank,
    pub vrnd: Rank,
    pub vval: Vec<Endpoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phase2aMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub rnd: Rank,
    pub vval: Vec<Endpoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phase2bMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub rnd: Rank,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
    pub metadata: HashMap<String, Bytes>,
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            metadata: HashMap::default(),
        }
    }
}

// An index can never be negative, and rounds always monotonically
// increase from 0
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Rank {
    pub round: u32,
    pub node_index: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchedAlertMessage {
    pub sender: Endpoint,
    pub alerts: Vec<Alert>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub src: Endpoint,
    pub dst: Endpoint,
    pub edge_status: EdgeStatus,
    pub config_id: ConfigId,
    pub ring_number: Vec<RingNumber>,
    pub node_id: Option<NodeId>,
    pub metadata: Option<Metadata>,
}

impl Alert {
    pub fn new(
        src: Endpoint,
        dst: Endpoint,
        edge_status: EdgeStatus,
        config_id: ConfigId,
        ring_number: RingNumber,
    ) -> Self {
        Alert {
            src,
            dst,
            edge_status,
            config_id,
            ring_number: vec![ring_number],
            node_id: None,
            metadata: None,
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum EdgeStatus {
    Up,
    Down,
}
