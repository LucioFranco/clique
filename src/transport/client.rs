use crate::{
    common::Endpoint,
    error::{Error, Result},
    transport::{proto::RequestKind, Request, Response},
};
use tokio_sync::{mpsc, oneshot};

pub type Channel = mpsc::Sender<(Request, oneshot::Sender<crate::Result<Response>>)>;
pub type Broadcast = mpsc::Sender<RequestKind>;

#[derive(Debug, Clone)]
pub struct Client {
    channel: Channel,
    broadcast: Broadcast,
}

impl Client {
    pub fn new(channel: Channel, broadcast: Broadcast) -> Self {
        Self { channel, broadcast }
    }

    pub async fn send(&mut self, target: Endpoint, req: RequestKind) -> Result<Response> {
        let (tx, rx) = oneshot::channel();
        let req = Request::new(target, req);

        self.channel
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

        self.channel
            .send((req, tx))
            .await
            .map_err(|_| Error::new_broken_pipe(None))?;

        // Drop the sender because we dont care about the response.
        drop(rx);

        Ok(())
    }

    pub async fn broadcast(&mut self, req: RequestKind) -> Result<()> {
        self.broadcast
            .send(req)
            .await
            .map_err(|_| Error::new_broken_pipe(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{
        proto::{RequestKind, ResponseKind},
        Response,
    };
    use futures::StreamExt;
    use tokio_sync::mpsc;

    #[tokio::test]
    async fn send() {
        let (tx, mut rx) = mpsc::channel(100);
        let (tx1, _rx) = mpsc::channel(100);

        let mut client = Client::new(tx, tx1);

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
        let (tx1, _rx) = mpsc::channel(100);

        let mut client = Client::new(tx, tx1);

        let req = RequestKind::Probe;
        client
            .send_no_wait("some-addr".into(), req.clone())
            .await
            .unwrap();

        let (req, tx) = rx.next().await.unwrap();
        assert_eq!(req.kind(), &RequestKind::Probe);

        // This simulates what the server does when it tries to send, it may
        // ignore the error as the sender could be dropped.
        let res = Response::new(ResponseKind::Probe);
        let _ = tx.send(Ok(res));
    }

    #[tokio::test]
    async fn broadcast() {
        let (tx, rx) = mpsc::channel(100);
        let (tx1, mut rx) = mpsc::channel(100);

        let mut client = Client::new(tx, tx1);

        let req = RequestKind::Probe;
        client.broadcast(req.clone()).await.unwrap();

        let req = rx.next().await.unwrap();
        assert_eq!(req, RequestKind::Probe);
    }
}
