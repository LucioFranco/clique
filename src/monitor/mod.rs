pub mod ping_pong;

use crate::common::Endpoint;
use std::future::Future;
use tokio_sync::{mpsc, oneshot};

pub trait Monitor {
    type Future: Future<Output = ()> + Send + 'static;

    fn monitor(
        &mut self,
        subject: Endpoint,
        client: mpsc::Sender<oneshot::Sender<()>>,
    ) -> Self::Future;
}
