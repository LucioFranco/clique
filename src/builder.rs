use crate::common::Endpoint;
use crate::{cluster::Cluster, transport::Transport2};

#[derive(Debug, Clone)]
pub struct Builder<T> {
    transport: Option<T>,
    target: Option<Endpoint>,
}

impl<T> Default for Builder<T> {
    fn default() -> Self {
        Builder {
            transport: None,
            target: None,
        }
    }
}

impl<T> Builder<T>
where
    T: Transport2 + Send,
{
    pub fn new() -> Self {
        Builder::default()
    }

    pub async fn finish(mut self) -> Cluster<T> {
        // let (event_tx, _event_rx) = broadcast::channel(10);
        // let transport = self
        //     .transport
        //     .take()
        //     .unwrap_or_else(|| panic!("Unable to get trasnport"));
        // let target = self
        //     .target
        //     .take()
        //     .unwrap_or_else(|| panic!("Unable to get target"));
        // let inner = Inner::new(transport, target, event_tx.clone()).await;

        // Cluster::new(event_tx, inner)
        todo!()
    }

    pub fn transport(mut self, transport: T) -> Self {
        self.transport = Some(transport);
        self
    }

    pub fn target(mut self, target: Endpoint) -> Self {
        self.target = Some(target);
        self
    }
}
