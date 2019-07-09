use crate::proto::client;
use clique::transport::{Client, Request, Response};
use futures::Future as Future01;
use futures03::compat::Future01CompatExt;
use hyper::client::connect::HttpConnector;
use std::future::Future;

pub struct GrpcClient {
    inner: client::MembershipService<tower_hyper::Client<HttpConnector, tower_grpc::BoxBody>>,
}

impl Client for GrpcClient {
    type Error = crate::Error;
    type Future = Box<dyn Future<Output = Result<Response, Self::Error>> + Unpin>;

    fn call(&mut self, req: Request) -> Self::Future {
        let req = tower_grpc::Request::new(req.into_inner().into());

        // TODO: destination

        let fut = self
            .inner
            .send_request(req)
            .map(|res| res.into_inner().into())
            .map_err(|_| crate::Error::Unknown)
            .compat();

        Box::new(fut)
    }
}
