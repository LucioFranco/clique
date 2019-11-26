use crate::{
    common::{ConfigId, Endpoint},
    monitor::Monitor,
    transport::{proto, Client},
};
use futures::{future, FutureExt};
use std::time::{Duration, Instant};
use tokio_sync::{mpsc, oneshot};
use tokio_timer::{delay, Timeout};

// Number of bootstrapping responses allowed by a node before being treated as a failure condition.
const BOOTSTRAP_COUNT_THRESHOLD: usize = 30;

#[derive(Debug)]
pub struct PingPong {
    timeout: Duration,
    tick_delay: Duration,
}

impl PingPong for PingPong {
    fn new(timeout: Duration, tick_delay: Duration) -> Self {
        PingPong {
            timeout,
            tick_delay,
        }
    }
}

impl Monitor for PingPong {
    type Future = future::BoxFuture<'static, ()>;

    fn monitor(
        &mut self,
        subject: Endpoint,
        mut client: Client,
        current_config_id: ConfigId,
        mut notification_tx: mpsc::Sender<(Endpoint, ConfigId)>,
        cancellation_rx: oneshot::Receiver<()>,
    ) -> Self::Future {
        let timeout = self.timeout;
        let tick_delay = self.tick_delay;

        async move {
            let mut failures: usize = 0;
            let mut bootstraps: usize = 0;

            loop {
                match cancellation_rx.try_recv() {
                    Ok(_) => continue,
                    Err(_) => return,
                };

                let fut = client.send(subject.clone(), proto::RequestKind::Probe);
                let result = Timeout::new(fut, timeout).await;

                if result.is_err() {
                    failures += 1;
                }

                if let Ok(response) = result {
                    if response.status == proto::NodeStatus::Bootstrapping {
                        bootstraps += 1;

                        if bootstraps > BOOTSTRAP_COUNT_THRESHOLD {
                            failures += 1;
                        }
                    }
                }

                if failures >= 5 {
                    notification_tx
                        .send((subject, current_config_id))
                        .await
                        .unwrap();
                    return;
                }

                delay(Instant::now() + tick_delay).await;
            }
        }
        .boxed()
    }
}
