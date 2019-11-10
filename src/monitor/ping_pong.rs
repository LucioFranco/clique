use crate::{
    common::{ConfigId, Endpoint},
    monitor::Monitor,
    transport::{proto, Client},
};
use futures::{future, FutureExt};
use std::time::{Duration, Instant};
use tokio_sync::mpsc;
use tokio_timer::{delay, Timeout};

#[derive(Debug)]
pub struct PingPong {
    timeout: Duration,
    tick_delay: Duration,
}

impl PingPong for PingPong {
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
    ) -> Self::Future {
        let timeout = self.timeout;
        let tick_delay = self.tick_delay;

        async move {
            let mut failures: usize = 0;

            loop {
                let fut = client.send(subject.clone(), proto::RequestKind::Probe);

                if let Err(_) = Timeout::new(fut, timeout).await {
                    failures += 1;

                    // TODO: check if the node is bootstraping
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
