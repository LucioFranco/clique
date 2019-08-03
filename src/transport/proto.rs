use crate::common::{ConfigId, Endpoint, NodeId, RingNumber};
use bytes::Bytes;
use std::collections::HashMap;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum Consensus {
    FastRoundPhase2bMessage(FastRoundPhase2bMessage),
    Phase1aMessage(Phase1aMessage),
    Phase1bMessage(Phase1bMessage),
    Phase2aMessage(Phase2aMessage),
    Phase2bMessage(Phase2bMessage),
}

#[derive(Debug, Clone)]
pub struct PreJoinMessage {
    pub sender: Endpoint,
    pub node_id: NodeId,
    pub ring_number: RingNumber,
    pub config_id: ConfigId,
}

#[derive(Debug, Clone)]
pub struct JoinMessage {
    pub sender: Endpoint,
    pub node_id: NodeId,
    pub ring_number: Vec<RingNumber>,
    pub config_id: ConfigId,
}

#[derive(Debug, Clone)]
pub struct JoinResponse {
    pub sender: Endpoint,
    pub status: JoinStatus,
    pub config_id: ConfigId,
    pub endpoints: Vec<Endpoint>,
    pub identifiers: Vec<NodeId>,
    pub cluster_metadata: HashMap<String, Metadata>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum JoinStatus {
    HostnameAlreadyInRing,
    NodeIdAlreadyInRing,
    SafeToJoin,
    ConfigChanged,
    MembershipRejected,
}

#[derive(Debug, Clone)]
pub struct FastRoundPhase2bMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone)]
pub struct Phase1aMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub rank: Rank,
}

#[derive(Debug, Clone)]
pub struct Phase1bMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub rnd: Rank,
    pub vrnd: Rank,
    pub vval: Vec<Endpoint>,
}

#[derive(Debug, Clone)]
pub struct Phase2aMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub rnd: Rank,
    pub vval: Vec<Endpoint>,
}

#[derive(Debug, Clone)]
pub struct Phase2bMessage {
    pub sender: Endpoint,
    pub config_id: ConfigId,
    pub rnd: Rank,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub metadata: HashMap<String, Bytes>,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Rank {
    pub round: u32,
    pub node_index: u32,
}
