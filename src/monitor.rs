use crate::{common::Endpoint, transport::Client};
use futures::{future, FutureExt};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio_sync::{mpsc, oneshot};
use tokio_timer::{Delay, Timeout};

#[derive(Debug)]
pub struct Monitor {
    timeout: Duration,
    tick_delay: Duration,
    failures: Arc<AtomicUsize>,
}

impl Monitor {
    pub fn new() -> Self {
        unimplemented!()
    }

    pub fn monitor(
        &mut self,
        subject: Endpoint,
        mut client: mpsc::Sender<oneshot::Sender<()>>,
    ) -> future::BoxFuture<'static, ()> {
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

fn request() -> crate::transport::Request {
    unimplemented!()
}
