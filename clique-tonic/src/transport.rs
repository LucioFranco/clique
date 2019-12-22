use std::pin::Pin;

use clique::transport;
use futures::{future, Future};
use tokio::sync::mpsc;
use tonic::{transport::Channel, Request};

use crate::{
    membership::membership_client::MembershipClient,
    server::{GrpcServer, TransportItem},
};

pub struct TonicTransport {
    server: GrpcServer,
}

impl<T> transport::Transport<T> for TonicTransport
where
    T: Into<String>,
{
    type Error = crate::Error;
    type ClientFuture = Pin<
        Box<
            dyn Future<Output = Result<clique::transport::Response, crate::Error>> + Send + 'static,
        >,
    >;

    fn send(&mut self, req: transport::Request) -> Self::ClientFuture {
        let (target, kind) = req.into_parts();
        let channel = Channel::from_shared(target).unwrap();

        let task = async move {
            let channel = channel.connect().await.unwrap();
            let mut client = MembershipClient::new(channel);

            let req = Request::new(kind.into());

            client
                .send_request(req)
                .await
                .map_err(|_| crate::Error::Upstream)
                .map(|res| res.into_inner().into())
        };

        Box::pin(task)
    }

    type ServerFuture = future::Ready<Result<Self::ServerStream, Self::Error>>;
    type ServerStream = mpsc::Receiver<TransportItem>;

    fn listen_on(&mut self, bind: T) -> Self::ServerFuture {
        let stream = self.server.create(bind.into());
        future::ready(Ok(stream))
    }
}

impl TonicTransport {
    pub fn new() -> Self {
        TonicTransport {
            server: GrpcServer::new(),
        }
    }
}
