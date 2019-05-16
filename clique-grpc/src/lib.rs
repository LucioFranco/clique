use futures::Future;
use http::Uri;
use hyper::client::connect::{Connect, Destination, HttpConnector};
use tower_hyper::{client, util};
use tower_request_modifier;
use tower_util::MakeService;

pub mod clique_proto {
    include!(concat!(env!("OUT_DIR"), "/messaging.rs"));
}

pub use self::clique_proto::*;

use clique::messaging::{Broadcast, RapidResponseFuture};

struct CliqueClient {
    conn: Connect,
}

impl CliqueClient {
    fn init(uri: &str) -> CliqueClient {
        let dst = Destination::try_from_uri(uri.parse().unwrap().clone()).unwrap();
        let connector = util::Connector::new(HttpConnector::new(4));
        let settings = client::Builder::new().http2_only(true).clone();
        let mut make_client = client::Connect::with_builder(connector, settings);

        let clique_client = make_client
            .make_service(dst)
            .map_err(|e| panic!("Connect error: {:?}", e))
            .and_then(|conn| {
                use clique_proto::client::MembershipService;

                let conn = tower_request_modifier::Builder::new()
                    .set_origin(uri)
                    .build(conn)
                    .unwrap();

                MembershipService::new(conn).ready()
            });

        CliqueClient { conn: make_client }
    }
}
