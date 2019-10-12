use clique::transport;
use futures::future;
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
    type ClientFuture = Result<clique::transport::Response, Error>;

    fn send(&mut self, req: transport::Request) -> Self::ClientFuture {
        let req = Request::new(req.into_inner().into());
        self.client.send_request(req)
    }

    type ServerFuture = future::Ready<Result<Self::ServerStream, Self::Error>>;
    type ServerStream = mpsc::Receiver<TransportItem>;

    fn listen_on(&mut self, bind: T) -> Self::ServerFuture {
        let stream = self.server.create(bind);
        future::ready(Ok(stream))
    }
}
