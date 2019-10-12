use clique::{Cluster, Event};
use clique_tonic::server::Server;

fn main() {
    let cluster = Cluster::builder()
        .transport(GrpcServer::new())
        .target("127.0.0.1:54321")
        .finish();
}
