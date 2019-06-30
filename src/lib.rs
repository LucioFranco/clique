#![feature(async_await)]
// #![deny(missing_docs, missing_debug_implementations, rust_2018_idioms)]
// #![cfg_attr(test, deny(warnings))]
// #![doc(test(no_crate_inject, attr(deny(rust_2018_idioms))))]

//! Clique membership library.

pub mod cluster;
mod error;
pub mod transport;

mod membership;

pub use self::cluster::Cluster;
pub use self::error::{Error, Result};

use uuid::Uuid;

/// The configuration that we are currently on.
pub type ConfigId = i64;
/// The ring identifier for which ring this node is from.
pub type RingNumber = i32;
/// Represents some _node/destination_ in the system.
pub type Endpoint = String;

/// Represents the NodeId internall it is just a Uuid v4
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(Uuid);

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
