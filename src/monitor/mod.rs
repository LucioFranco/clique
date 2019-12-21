pub mod ping_pong;

use crate::{
    common::{ConfigId, Endpoint},
    transport::Client,
};
use std::future::Future;
use tokio::sync::{mpsc, oneshot};

pub trait Monitor {
    type Future: Future<Output = ()> + Send + 'static;

    fn monitor(
        &mut self,
        subject: Endpoint,
        client: Client,
        current_config_id: ConfigId,
        notification_tx: mpsc::Sender<(Endpoint, ConfigId)>,
        cancellation_rx: oneshot::Receiver<()>,
    ) -> Self::Future;
}
