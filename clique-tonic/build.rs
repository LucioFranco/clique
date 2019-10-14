use std::env;

fn main() {
    tonic_build::compile_protos(&"proto/clique.proto")
        .unwrap_or_else(|e| panic!("Build error: {:?}", e));
}
