use super::Monitor;
use crate::{common::Endpoint, transport::Client};
use futures::{future, FutureExt};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio_sync::{mpsc, oneshot};
use tokio_timer::{Delay, Timeout};

pub trait Monitor2 {
    type Future: Future<Output = ()> + Send + 'static;

    fn monitor(
        &mut self,
        subject: Endpoint,
        client: mpsc::Sender<oneshot::Sender<()>>,
    ) -> Self::Future;
}

#[derive(Debug)]
pub struct PingPong {
    timeout: Duration,
    tick_delay: Duration,
    failures: Arc<AtomicUsize>,
}

impl Monitor for PingPong {
    type Future = future::BoxFuture<'static, ()>;

    fn monitor(
        &mut self,
        subject: Endpoint,
        mut client: mpsc::Sender<oneshot::Sender<()>>,
    ) -> Self::Future {
        let timeout = self.timeout;
        let tick_delay = self.tick_delay;
        let failures = self.failures.clone();

        async move {
            loop {
                let (tx, rx) = oneshot::channel();
                client.send(tx);

                if let Err(_) = Timeout::new(rx, timeout).await {
                    failures.fetch_add(1, Ordering::SeqCst);
                    // TODO: probe failed, we should increment the endpoints counter
                    unimplemented!()
                }

                Delay::new(Instant::now() + tick_delay).await;
            }
        }
            .boxed()
    }
}

impl PingPong {
    pub fn new() -> Self {
        unimplemented!()
    }
}

fn request() -> crate::transport::Request {
    unimplemented!()
}
