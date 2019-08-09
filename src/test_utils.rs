pub fn trace_init() {
    let subscriber = tracing_fmt::FmtSubscriber::builder().finish();
    let _ = tracing::dispatcher::set_global_default(tracing::Dispatch::new(subscriber));
}
