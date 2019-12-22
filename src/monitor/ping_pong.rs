use crate::{
    common::{ConfigId, Endpoint},
    monitor::Monitor,
    transport::{proto, Client},
};
use futures::{future, FutureExt};
use std::time::Duration;
use tokio::{
    sync::{mpsc, oneshot},
    time::{delay_for, timeout},
};

// Number of bootstrapping responses allowed by a node before being treated as a failure condition.
const BOOTSTRAP_COUNT_THRESHOLD: usize = 30;

#[derive(Debug)]
pub struct PingPong {
    timeout: Duration,
    tick_delay: Duration,
}

impl PingPong {
    pub fn new(timeout: Duration, tick_delay: Duration) -> Self {
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
        mut cancellation_rx: oneshot::Receiver<()>,
    ) -> Self::Future {
        let req_timeout = self.timeout;
        let tick_delay = self.tick_delay;

        async move {
            let mut failures: usize = 0;
            let mut bootstraps: usize = 0;

            while let Ok(_) = cancellation_rx.try_recv() {
                let fut = client.send(subject.clone(), proto::RequestKind::Probe);
                let result = timeout(req_timeout, fut).await;

                if result.is_err() {
                    failures += 1;
                }

                if let Ok(Ok(response)) = result {
                    if let proto::ResponseKind::Probe(status) = response.kind() {
                        if *status == proto::NodeStatus::Bootstrapping {
                            bootstraps += 1;

                            if bootstraps > BOOTSTRAP_COUNT_THRESHOLD {
                                failures += 1;
                            }
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

                delay_for(tick_delay).await;
            }
        }
        .boxed()
    }
}
