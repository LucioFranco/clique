pub mod clique_proto {
    include!(concat!(env!("OUT_DIR"), "/messaging.rs"));
}

pub use self::clique_proto::{server, client, RapidRequest, RapidResponse, Endpoint};