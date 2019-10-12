use crate::{cluster::Cluster, cluster::Inner, event::Event, handle::Handle, transport::Transport};

use tokio_sync::watch;

#[derive(Debug, Clone)]
pub struct Builder<T, Target> {
    transport: Option<T>,
    target: Option<Target>,
}

impl<T, Target> Builder<T, Target>
where
    T: Transport<Target> + Send,
    Target: Send + Clone + Into<String>,
{
    pub fn new() -> Self {
        Builder {
            transport: None,
            target: None,
        }
    }

    pub async fn finish(mut self) -> Cluster<T, Target> {
        let (event_tx, event_rx) = watch::channel(Event::new());
        let transport = self
            .transport
            .take()
            .unwrap_or_else(|| panic!("Unable to get trasnport"));
        let target = self
            .target
            .take()
            .unwrap_or_else(|| panic!("Unable to get target"));
        let inner = Inner::new(transport, target, event_tx).await;
        let handle = Handle::new(event_rx);

        Cluster::new(handle, inner)
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
