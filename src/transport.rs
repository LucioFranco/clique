use crate::membership::Membership;
use futures::Stream;
use std::future::Future;

pub struct Request;
pub struct Response;

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
