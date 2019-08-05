use crate::{
    common::Endpoint,
    error::{Error, Result},
    transport::{proto::RequestKind, Request, Response},
};
use tokio_sync::{mpsc, oneshot};

pub type Channel = mpsc::Sender<(Request, oneshot::Sender<crate::Result<Response>>)>;

#[derive(Debug, Clone)]
pub struct Client {
    inner: Channel,
}

impl Client {
    pub fn new(inner: Channel) -> Self {
        Self { inner }
    }

    pub async fn send(&mut self, target: Endpoint, req: RequestKind) -> Result<Response> {
        let (tx, rx) = oneshot::channel();
        let req = Request::new(target, req);

        self.inner
            .send((req, tx))
            .await
            .map_err(|_| Error::new_broken_pipe(None))?;

        rx.await
            .map_err(|_| Error::new_broken_pipe(None))?
            .map_err(Into::into)
    }

    pub async fn broadcast(&mut self, req: RequestKind) -> Result<()> {
        unimplemented!()
    }
}
