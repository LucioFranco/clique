use crate::common::{ConfigId, Endpoint, NodeId, RingNumber};
use bytes::Bytes;
use std::collections::HashMap;

#[derive(Debug)]
pub enum RequestKind {
    PreJoin(PreJoinMessage),
    Join(JoinMessage),
    Probe,
    Consensus(Consensus),
}

#[derive(Debug)]
pub enum ResponseKind {
    Join(JoinResponse),
    Response,
    Probe,
    Consensus(Consensus),
}

#[derive(Debug)]
pub enum Consensus {
    FastRoundPhase1bMessage(FastRoundPhase1bMessage),
    Phase1aMessage(Phase1aMessage),
    Phase1bMessage(Phase1bMessage),
    Phase2aMessage(Phase2aMessage),
    Phase2bMessage(Phase2bMessage),
}

#[derive(Debug)]
pub struct PreJoinMessage {
    pub sender: Endpoint,
    pub node_id: NodeId,
    pub ring_number: RingNumber,
    pub config_id: ConfigId,
}

#[derive(Debug)]
pub struct JoinMessage {
    pub sender: Endpoint,
    pub node_id: NodeId,
    pub ring_number: Vec<RingNumber>,
    pub config_id: ConfigId,
}

#[derive(Debug)]
pub struct JoinResponse {
    pub sender: Endpoint,
    pub status: JoinStatus,
    pub config_id: ConfigId,
    pub endpoints: Vec<Endpoint>,
    pub identifiers: Vec<NodeId>,
    pub cluster_metadata: HashMap<String, Metadata>,
}

#[derive(Debug)]
pub enum JoinStatus {
    HostnameAlreadyInRing,
    NodeIdAlreadyInRing,
    SafeToJoin,
    ConfigChanged,
    MembershipRejected,
}

#[derive(Debug)]
pub struct FastRoundPhase1bMessage;

#[derive(Debug)]
pub struct Phase1aMessage;

#[derive(Debug)]
pub struct Phase1bMessage;

#[derive(Debug)]
pub struct Phase2aMessage;

#[derive(Debug)]
pub struct Phase2bMessage;

#[derive(Debug)]
pub struct Metadata {
    pub metadata: HashMap<String, Bytes>,
}

impl NodeId {
    pub fn new() -> Self {
        NodeId(Uuid::new_v4())
    }
}

impl From<Uuid> for NodeId {
    fn from(t: Uuid) -> Self {
        NodeId(t)
    }
}

#[derive(Debug)]
pub struct Rank {
    pub round: u32,
    pub node_index: u32,
}
