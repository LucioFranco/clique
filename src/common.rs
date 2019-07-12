use uuid::Uuid;

/// The configuration that we are currently on.
pub type ConfigId = u64;
/// The ring identifier for which ring this node is from.
pub type RingNumber = i32;
/// Represents some _node/destination_ in the system.
pub type Endpoint = String;

/// Represents the NodeId internall it is just a Uuid v4
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(Uuid);

impl NodeId {
    pub fn new() -> Self {
        NodeId(Uuid::new_v4())
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<Uuid> for NodeId {
    fn from(t: Uuid) -> Self {
        NodeId(t)
    }
}
