use futures::{Async, Future, Poll};
use http::{Response, Uri};
use http_body::Body as HttpBody;
use tower_grpc::client::unary::Once;
use tower_grpc::generic::client::GrpcService;
use tower_grpc::Request as gRequest;
use tower_hyper::client::Connection;
use tower_service::Service;
use tower_util;
use tower_util::MakeService;

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

pub mod clique_proto {
    include!(concat!(env!("OUT_DIR"), "/messaging.rs"));
}

use clique_proto::client as ProtoClient;
use clique_proto::RapidRequest;

pub struct UniquePool<S, T, R>
where
    S: MakeService<T, R> + Clone,
    T: Hash + Eq,
{
    pool: Arc<Mutex<HashMap<T, S::Service>>>,
    maker: S,
}

impl<S, T, R> UniquePool<S, T, R>
where
    S: MakeService<T, R> + Clone,
    T: Hash + Eq,
{
    pub fn new(maker: S) -> UniquePool<S, T, R> {
        UniquePool {
            pool: Arc::new(Mutex::new(HashMap::new())),
            maker,
        }
    }
}

pub trait Key<T> {
    fn get_key(&self) -> T;
}

type Error = Box<std::error::Error + Send + Sync + 'static>;

impl<M, T, R> Service<R> for UniquePool<M, T, R>
where
    M: MakeService<T, R> + Clone + Send + Sync,
    T: Hash + Eq + Send + 'static,
    R: Key<T> + Send + 'static,
    M::Service: Clone + Send + 'static,
    M::MakeError: Into<Error> + 'static,
    M::Error: Into<Error> + 'static,
    M::Future: Send + 'static,
    <M::Service as Service<R>>::Future: Send + 'static,
{
    type Response = M::Response;
    type Error = Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error> + Send + 'static>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(Async::Ready(()))
    }

    fn call(&mut self, req: R) -> Self::Future {
        if let Some(svc) = self.pool.lock().unwrap().get_mut(&req.get_key()) {
            Box::new(svc.call(req).map_err(Into::into))
        } else {
            let pool = self.pool.clone();
            let fut = self
                .maker
                .clone()
                .make_service(req.get_key())
                .map_err(Into::into)
                .and_then(move |mut svc| {
                    pool.lock().unwrap().insert(req.get_key(), svc.clone());
                    svc.call(req).map_err(Into::into)
                });

            Box::new(fut)
        }
    }
}

#[derive(Hash, Eq)]
struct Target {
    uri: Uri,
}

impl Key<Uri> for Target {
    fn get_key(&self) -> Uri {
        self.uri
    }
}

impl PartialEq for Target {
    fn eq(&self, other: &Target) -> bool {
        self.uri == other.uri
    }
}

pub struct GrpcClient<T> {
    conn: ProtoClient::MembershipService<T>,
}

impl<T> GrpcClient<T> {
    pub fn new(conn: T) -> GrpcClient<T> {
        GrpcClient {
            conn: ProtoClient::MembershipService::new(conn),
        }
    }
}

impl<T, R> Service<R> for GrpcClient<T>
where
    T: GrpcService<R>,
    Once<RapidRequest>: tower_grpc::client::Encodable<R>,
{
    type Response = Response<T::ResponseBody>;
    type Error = T::Error;
    type Future = T::Future;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.conn.poll_ready().map_err(Into::into)
    }

    fn call(&mut self, request: R) -> Self::Future {
        self.conn.send_request(gRequest::new(request))
    }
}

#[cfg(test)]
mod tests {
    use futures::Future;
    use rand;
    use rand::prelude::*;
    use tower_service::Service;
    use tower_test::{assert_request_eq, mock};

    use super::{Key, UniquePool};

    use std::cmp::PartialEq;

    #[derive(Eq, PartialEq, Debug, Hash)]
    struct Req(u32);

    impl PartialEq<Req> for u32 {
        fn eq(&self, other: &Req) -> bool {
            *self == other.0
        }
    }

    // Helper to run some code within context of a task
    fn with_task<F: FnOnce() -> U, U>(f: F) -> U {
        use futures::future::lazy;
        lazy(|| Ok::<_, ()>(f())).wait().unwrap()
    }

    fn gen_request() -> Req {
        let mut rng = rand::thread_rng();
        Req(rng.gen())
    }

    impl Key<u32> for Req {
        fn get_key(&self) -> u32 {
            self.0
        }
    }

    #[test]
    fn store_service() {
        let (mut mock, mut handle) = mock::pair::<Req, String>();

        let pool = UniquePool::new(mock);

        let req = gen_request();

        assert!(pool.poll_ready().unwrap().is_ready());
        let mut response = pool.call(req);

        let send_response = assert_request_eq!(handle, req);

        assert_eq!(pool.pool.lock().unwrap().len(), 1);

        with_task(|| {
            assert!(response.poll().unwrap().is_not_ready());
        });

        send_response.send_response("test".into());

        assert_eq!(response.wait().unwrap().as_str(), "test");
    }
}
