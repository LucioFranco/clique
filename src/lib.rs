#![feature(async_await)]
#![recursion_limit = "512"]
// #![deny(missing_docs, missing_debug_implementations, rust_2018_idioms)]
// #![cfg_attr(test, deny(warnings))]
// #![doc(test(no_crate_inject, attr(deny(rust_2018_idioms))))]

//! Clique membership library.

mod alert;
mod builder;
mod cluster;
mod common;
mod consensus;
mod error;
mod event;
mod handle;
mod membership;
mod monitor;
pub mod transport;
#[cfg(test)]
mod support;

pub use self::builder::Builder;
pub use self::cluster::Cluster;
pub use self::error::{Error, Result};
pub use self::event::Event;
pub use self::handle::Handle;
pub use self::transport::Transport;
