use crate::{cluster::Cluster, transport::Transport};

use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct Builder<T, Target> {
    transport: Option<T>,
    target: Option<Target>,
}

impl<T, Target> Default for Builder<T, Target> {
    fn default() -> Self {
        Builder {
            transport: None,
            target: None,
        }
    }
}

impl<T, Target> Builder<T, Target>
where
    T: Transport<Target> + Send,
    Target: Send + Clone + Into<String>,
{
    pub fn new() -> Self {
        Builder::default()
    }

    pub async fn finish(mut self) -> Cluster<T, Target> {
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

    pub fn target(mut self, target: Target) -> Self {
        self.target = Some(target);
        self
    }
}
