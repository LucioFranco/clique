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
