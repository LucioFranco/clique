use clique::transport;
use futures::{future, Future};
use tokio::sync::mpsc;
use tonic::{
    transport::{channel::ResponseFuture, Channel},
    Request, Response,
};

use crate::{
    membership::{client::MembershipClient, RapidRequest},
    server::{GrpcServer, TransportItem},
};

pub struct TonicTransport {
    client: MembershipClient<RapidRequest>,
    server: GrpcServer,
}

impl<T> transport::Transport<T> for TonicTransport
where
    T: Into<String>,
{
    type Error = crate::Error;
    type ClientFuture = Box<dyn Future<Output = Result<clique::transport::Response, crate::Error>>>;

    fn send(&mut self, req: transport::Request) -> Self::ClientFuture {
        let req = Request::new(req.into_inner().into());
        Box::new(async {
            self.client
            .send_request(req)
            .await
            .map_err(|_| crate::Error::Upstream)?
            .into_inner()
        })
    }

    type ServerFuture = future::Ready<Result<Self::ServerStream, Self::Error>>;
    type ServerStream = mpsc::Receiver<TransportItem>;

    fn listen_on(&mut self, bind: T) -> Self::ServerFuture {
        let stream = self.server.create(bind);
        future::ready(Ok(stream))
    }
}
