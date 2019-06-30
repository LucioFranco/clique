use crate::Error;
use futures::Stream;
use std::future::Future;
use tokio_sync::oneshot;

pub trait Client {
    type Error: std::error::Error;
    type Future: Future<Output = Result<Response, Self::Error>>;

    fn call(&mut self, req: Request) -> Self::Future;
}

pub trait Server<T, C> {
    type Error: std::error::Error;
    type Stream: Stream<Item = Result<Request, Self::Error>> + Unpin;
    type Future: Future<Output = Result<Self::Stream, Self::Error>>;

    fn start(&self, target: T) -> Self::Future;
}

pub struct Request {
    res_tx: oneshot::Sender<crate::Result<Response>>,
}

pub struct Response;

impl Request {
    pub fn respond(self, res: Response) -> crate::Result<()> {
        self.res_tx
            .send(Ok(res))
            // TODO: prob should be transport dropped
            .map_err(|_| Error::new_broken_pipe(None))
    }
}
