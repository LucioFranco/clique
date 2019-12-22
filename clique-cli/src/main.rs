use clique::Builder;
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_log::LogTracer::init()?;
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter("clique=debug,clique_tonic=debug,clique_cli=debug")
        .init();

    let opts = Opts::from_args();

    let listen = match &opts {
        Opts::Start { listen } => listen.clone(),
        Opts::Join { listen, .. } => listen.clone(),
    };

    let transport = clique_tonic::TonicTransport::new();
    let mut cluster = Builder::new()
        .target(listen)
        .transport(transport)
        .finish()
        .await;

    // match opts {
    //     Opts::Start { .. } => cluster.start().await?,
    //     Opts::Join { peer, .. } => cluster.join(peer).await?,
    // }

    Ok(())
}

#[derive(StructOpt, Debug)]
#[structopt(name = "clique-cli", rename_all = "snake_case")]
enum Opts {
    Start { listen: String },
    Join { listen: String, peer: String },
}
