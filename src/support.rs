use tracing_subscriber::{EnvFilter, FmtSubscriber};

pub fn trace_init() {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    let _ = tracing::dispatcher::set_global_default(tracing::Dispatch::new(subscriber));
}
