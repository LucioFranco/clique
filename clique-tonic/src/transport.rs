use clique::trasnport;
use tokio_sync::{mpsc, oneshot};
use tonic::{
    trasnport::client::{Channel, ResponseFuture},
    Request, Response,
};

use crate::server::{GrpcServer, TransportItem};

pub struct TonicTransport {
    client: Channel,
    server: GrpcServer,
}

impl trasnport::Trasnport<String> for TonicTransport {
    type Error = crate::Error;
    type ClientFuture = ResponseFuture;

    fn send(&mut self, req: transport::Request) -> Self::ClientFuture {
        let req = Request::new(req.into_inner().into());
        self.client.call(req)
    }

    type ServerFuture = Ready<Result<Self::ServerStream, Self::Error>>;
    type ServerStream = mpsc::Receiver<TransportItem>;

    fn listen_on(&mut self, bind: String) -> Self::ServerFuture {
        let stream = self.server.create(bind);

        future::ready(Ok(stream))
    }
}
