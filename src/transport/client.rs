use crate::{
    common::Endpoint,
    error::{Error, Result},
    transport::{proto::RequestKind, Request, Response},
};
use futures::{stream::Fuse, StreamExt};
use tokio_sync::{mpsc, oneshot};

pub type RequestStream = Fuse<mpsc::Receiver<RequestType>>;

#[derive(Debug, Clone)]
pub struct Client {
    inner: mpsc::Sender<RequestType>,
}

#[derive(Debug)]
pub enum RequestType {
    Unary(Request, oneshot::Sender<crate::Result<Response>>),
    Broadcast(RequestKind),
}

impl Client {
    pub fn new(bound: usize) -> (Self, RequestStream) {
        let (inner, rx) = mpsc::channel(bound);
        let me = Self { inner };

        (me, rx.fuse())
    }

    pub async fn send(&mut self, target: Endpoint, req: RequestKind) -> Result<Response> {
        let (tx, rx) = oneshot::channel();
        let req = Request::new(target, req);

        self.inner
            .send(RequestType::Unary(req, tx))
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
            .send(RequestType::Unary(req, tx))
            .await
            .map_err(|_| Error::new_broken_pipe(None))?;

        // Drop the sender because we dont care about the response.
        drop(rx);

        Ok(())
    }

    pub async fn broadcast(&mut self, req: RequestKind) -> Result<()> {
        self.inner
            .send(RequestType::Broadcast(req))
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
    
    #[tokio::test]
    async fn send() {
        let (mut client, mut rx) = Client::new(10);

        tokio::spawn(async move {
            match rx.next().await.unwrap() {
                RequestType::Unary(_req, tx) => {
                    let res = Response::new(ResponseKind::Probe);

                    tx.send(Ok(res)).unwrap();
                }
                _ => panic!("wrong request type"),
            }
        });

        let req = RequestKind::Probe;
        client.send("some-addr".into(), req).await.unwrap();
    }

    #[tokio::test]
    async fn send_no_wait() {
        let (mut client, mut rx) = Client::new(100);

        let req = RequestKind::Probe;
        client
            .send_no_wait("some-addr".into(), req.clone())
            .await
            .unwrap();

        match rx.next().await.unwrap() {
            RequestType::Unary(req, tx) => {
                assert_eq!(req.kind(), &RequestKind::Probe);

                // This simulates what the server does when it tries to send, it may
                // ignore the error as the sender could be dropped.
                let res = Response::new(ResponseKind::Probe);
                let _ = tx.send(Ok(res));
            }
            _ => panic!("wrong request type"),
        }
    }

    #[tokio::test]
    async fn broadcast() {
        let (mut client, mut rx) = Client::new(100);

        let req = RequestKind::Probe;
        client.broadcast(req.clone()).await.unwrap();

        match rx.next().await.unwrap() {
            RequestType::Broadcast(req) => assert_eq!(req, RequestKind::Probe),
            _ => panic!("wrong request type"),
        };
    }
}
