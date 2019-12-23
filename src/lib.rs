#![recursion_limit = "512"]
// #![deny(missing_docs, missing_debug_implementations, rust_2018_idioms)]
// #![cfg_attr(test, deny(warnings))]
// #![doc(test(no_crate_inject, attr(deny(rust_2018_idioms))))]
#![allow(warnings)]

//! Clique membership library.

mod alert;
mod builder;
mod cluster;
mod common;
mod consensus;
mod error;
mod event;
mod join;
mod membership;
mod metadata_manager;
mod monitor;
#[cfg(test)]
mod support;
pub mod transport;

pub use self::builder::Builder;
pub use self::cluster::Cluster;
pub use self::common::{Endpoint, NodeId};
pub use self::error::{Error, Result};
pub use self::event::Event;
pub use self::transport::{Message, Transport, Transport2};
