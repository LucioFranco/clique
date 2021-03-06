use futures::stream::FuturesUnordered;
use std::{future::Future, pin::Pin};
use uuid::Uuid;

/// The configuration that we are currently on.
pub type ConfigId = i64;
/// The ring identifier for which ring this node is from.
pub type RingNumber = i32;
/// Represents some _node/destination_ in the system.
pub type Endpoint = String;

pub type Scheduler = FuturesUnordered<Pin<Box<dyn Future<Output = SchedulerEvents> + Send>>>;

pub enum SchedulerEvents {
    /// Event to trigger the start of a classic paxos round
    StartClassicRound,
    /// An event that has no return type
    None,
    /// An event to surface consensus decision
    Decision(Vec<Endpoint>),
}

/// Represents the NodeId internall it is just a Uuid v4
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NodeId(Uuid);

impl NodeId {
    pub fn new() -> Self {
        NodeId(Uuid::new_v4())
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn to_string(&self) -> String {
        self.0.to_hyphenated().to_string()
    }
}

impl From<Uuid> for NodeId {
    fn from(t: Uuid) -> Self {
        NodeId(t)
    }
}
