use bytes::Bytes;
use std::collections::HashMap;
use uuid::Uuid;

/// The configuration that we are currently on.
pub type ConfigId = i64;
/// The ring identifier for which ring this node is from.
pub type RingNumber = i32;
/// Represents some _node/destination_ in the system.
pub type Endpoint = String;

/// Represents the NodeId internall it is just a Uuid v4
#[derive(Debug)]
pub struct NodeId(Uuid);

#[derive(Debug)]
pub enum RequestKind {
    PreJoin(PreJoinMessage),
    Join(JoinMessage),
    Probe,
}

#[derive(Debug)]
pub enum ResponseKind {
    Join(JoinResponse),
    Response,
    Probe,
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
