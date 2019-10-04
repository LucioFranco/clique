use std::net::ToSocketAddr;

use tokio::sync::{mpsc, onoeshot};
use tonic::{transport::Server, Request, Response, Status};

use clique::transport;

use crate::{Error, membership::{server::{Membership, MembershipServer}, RapidRequest, RapidResponse}};


pub(crate) type TransportItem = Result<(transport::Request, oneshot::Sender<clique::Result<transport::Response, clique::Error>>, Error)>;

struct GrpcServer {
    msg_tx: mpsc::Sender<TransportItem>,
    msg_rx: Option<mpsc::Receiver<TransportItem>>
}

#[tonic::async_trait]
impl Membership for GrpcServer {
    async fn send_request(req: Request<RapidRequest>) -> Result<Response<RapidResponse>, Status> {
        unimplemented!()
    }
}

impl GrpcServer {
    fn new() -> Self {
        let (msg_tx, msg_rx) = mpsc::channel();

        Self { msg_tx, msg_rx }
    }


    fn create(&mut self, target: String) -> mpsc::Receiver<Request<RapidRequest>>{
        let addr = target.parse::<ToSocketAddr>().expect("Unable to parse server address");

        let membership = GrpcServer::new();

        let msg_rx = membership.get_rx();

        let task = async move {
            Server.builder()
                .serve(addr, MembershipServer::new(membership))
                .await?;

            Ok(())
        };

        tokio::spawn(task.map(|_| ()).map_err(|e| panic!("Server crashed on: {:?}", e)));

        msg_rx

    }

    fn get_rx(&mut self) -> mpsc::Receiver<Request<RapidRequest>> {
        self.msg_rx.take().expect("Unable to return server stream")
    }

}


