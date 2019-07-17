#![feature(async_await)]
#![recursion_limit = "128"]
// #![deny(missing_docs, missing_debug_implementations, rust_2018_idioms)]
// #![cfg_attr(test, deny(warnings))]
// #![doc(test(no_crate_inject, attr(deny(rust_2018_idioms))))]

//! Clique membership library.

// TODO: remove this when were ready :)
#![allow(unused)]

mod alert;
pub mod cluster;
mod common;
mod consensus;
mod error;
mod membership;
mod monitor;
pub mod transport;

pub use self::cluster::Cluster;
pub use self::error::{Error, Result};
