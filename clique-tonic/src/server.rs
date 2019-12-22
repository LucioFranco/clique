use futures::future::FutureExt;
use tokio::sync::{mpsc, oneshot};
use tonic::{transport::Server, Request, Response, Status};

use clique::transport;

use crate::membership::{
    membership_server::{Membership, MembershipServer},
    RapidRequest, RapidResponse,
};

pub(crate) type TransportItem = (
    transport::Request,
    oneshot::Sender<clique::Result<transport::Response>>,
);

pub(crate) struct GrpcServer {
    req_tx: mpsc::Sender<TransportItem>,
    req_rx: Option<mpsc::Receiver<TransportItem>>,
}

#[tonic::async_trait]
impl Membership for GrpcServer {
    async fn send_request(
        &self,
        req: Request<RapidRequest>,
    ) -> Result<Response<RapidResponse>, Status> {
        tracing::debug!(message = "inbound request.", from = %req.remote_addr().unwrap(), message = ?req.get_ref());

        let (res_tx, res_rx) = oneshot::channel();

        self.req_tx
            .clone()
            .send((req.into_inner().into(), res_tx))
            .await
            .map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Unable to send request: {:?}", e),
                )
            })?;

        let response = res_rx.await.map_err(|e| {
            tonic::Status::new(
                tonic::Code::Unknown,
                format!("Channel receive error: {:?}", e),
            )
        })?;

        tracing::debug!(message = "sending response.", message = ?response);

        match response {
            Err(e) => {
                eprintln!("Error handling request: {:?}", e);
                Err(tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Internal error: {:?}", e),
                ))
            }
            Ok(res) => Ok(Response::new(res.into())),
        }
    }
}

impl GrpcServer {
    pub fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel(100);

        Self {
            req_tx,
            req_rx: Some(req_rx),
        }
    }

    pub fn create(&mut self, target: String) -> mpsc::Receiver<TransportItem> {
        let addr = target.parse().expect("Unable to parse server address");
        let membership = GrpcServer::new();

        let task = async move {
            Server::builder()
                .add_service(MembershipServer::new(membership))
                .serve(addr)
                .await?;

            Ok(())
        };

        tokio::spawn(
            task.map(|val: Result<(), tonic::transport::Error>| match val {
                Ok(_) => (),
                Err(e) => panic!("Server crahsed on: {:?}", e),
            }),
        );

        self.req_rx
            .take()
            .unwrap_or_else(|| panic!("Unable to extract receiver"))
    }
}
