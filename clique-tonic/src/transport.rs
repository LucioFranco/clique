use std::pin::Pin;

use clique::transport;
use futures::{future, Future, TryFutureExt, FutureExt};
use tokio::sync::mpsc;
use tonic::{transport::Channel, Request, Response};

use crate::{
    membership::{client::MembershipClient, RapidRequest},
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
    type ClientFuture = Pin<Box<dyn Future<Output = Result<clique::transport::Response, crate::Error>>>>;

    fn send(&mut self, req: transport::Request) -> Self::ClientFuture {
        let transport::Request{target, kind } = req;

        let channel = Channel::from_static(&target).channel();
        let mut client = MembershipClient::new(channel);

        let req = Request::new(kind.into());

        let task = async {
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
