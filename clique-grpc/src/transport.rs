use crate::{proto::client, server::GrpcServer};
use clique::transport::{Request, Response, Transport};
use futures::Future as Future01;
use futures03::{
    compat::Future01CompatExt,
    future::{self, Ready},
};
use hyper::client::connect::HttpConnector;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use tokio_sync::{mpsc, oneshot};

pub struct GrpcTransport {
    inner: client::MembershipService<tower_hyper::Client<HttpConnector, tower_grpc::BoxBody>>,
    server: GrpcServer,
}

impl Transport<String> for GrpcTransport {
    type Error = crate::Error;

    type ClientFuture = Pin<Box<dyn Future<Output = Result<Response, Self::Error>> + Unpin + Send>>;

    fn send(&mut self, req: Request) -> Self::ClientFuture {
        let req = tower_grpc::Request::new(req.into_inner().into());

        // TODO: destination

        let fut = self
            .inner
            .send_request(req)
            .map(|res| res.into_inner().into())
            .map_err(|_| crate::Error::Unknown)
            .compat();

        Box::pin(fut)
    }

    type ServerFuture = Ready<Result<Self::ServerStream, Self::Error>>;
    type ServerStream = mpsc::Receiver<
        Result<(Request, oneshot::Sender<Result<Response, clique::Error>>), Self::Error>,
    >;

    fn listen_on(&mut self, bind: Self::Target) -> Self::ServerFuture {
        let addr = bind.parse::<SocketAddr>().unwrap();
        let tx = self.server.create(addr);

        future::ready(Ok(tx))
    }
}
