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

    pub async fn send_no_wait(&mut self, target: Endpoint, req: RequestKind) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let req = Request::new(target, req);

        self.inner
            .send((req, tx))
            .await
            .map_err(|_| Error::new_broken_pipe(None))?;

        // Drop the sender because we dont care about the response.
        drop(rx);

        Ok(())
    }

    pub async fn broadcast(&mut self, req: RequestKind) -> Result<()> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{proto::{RequestKind, ResponseKind}, Response};
    use futures::StreamExt;
    use tokio_sync::mpsc;

    #[tokio::test]
    async fn send() {
        let (tx, mut rx) = mpsc::channel(100);

        let mut client = Client::new(tx);

        tokio::spawn(async move {
            let (req, tx) = rx.next().await.unwrap();

            let res = Response::new(ResponseKind::Probe);

            tx.send(Ok(res)).unwrap();
        });

        let req = RequestKind::Probe;
        client.send("some-addr".into(), req).await.unwrap();
    }

    #[tokio::test]
    async fn send_no_wait() {
        let (tx, mut rx) = mpsc::channel(100);

        let mut client = Client::new(tx);

        let req = RequestKind::Probe;
        client.send_no_wait("some-addr".into(), req.clone()).await.unwrap();

        let (req, tx) = rx.next().await.unwrap();
        assert_eq!(req.kind(), &RequestKind::Probe);

        // This simulates what the server does when it tries to send, it may
        // ignore the error as the sender could be dropped.
        let res = Response::new(ResponseKind::Probe);
        let _ = tx.send(Ok(res));
    }    
}
