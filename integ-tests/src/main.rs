use clique::Cluster;
use clique_tonic::transport::TonicTransport;

fn main() {
    let _cluster = Cluster::builder()
        .transport(TonicTransport::new())
        .target("127.0.0.1:54321")
        .finish();
}
