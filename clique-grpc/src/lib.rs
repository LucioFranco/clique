use futures::{future::poll_fn, Async, Future, Poll};
use tower::util::ServiceExt;
use tower_service::Service;
use tower_util::MakeService;

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

pub mod clique_proto {
    include!(concat!(env!("OUT_DIR"), "/messaging.rs"));
}

use clique_proto::client as ProtoClient;
use clique_proto::RapidRequest;

pub struct UniquePool<M, T, R>
where
    M: MakeService<T, R> + Clone,
    T: Hash + Eq,
{
    pool: Arc<Mutex<HashMap<T, M::Service>>>,
    maker: M,
}

impl<M, T, R> UniquePool<M, T, R>
where
    M: MakeService<T, R> + Clone,
    T: Hash + Eq,
    R: Key<T>,
    M::Service: Clone,
    M::MakeError: Into<Error>,
    M::Error: Into<Error>,
{
    pub fn new(maker: M) -> UniquePool<M, T, R> {
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
            let maker = poll_fn(|| self.maker.poll_ready());
            let fut = maker
                .and_then(|mut maker| {
                    self.maker
                        .clone()
                        .make_service(req.get_key())
                        .map_err(Into::into)
                })
                .and_then(move |mut svc| {
                    pool.lock().unwrap().insert(req.get_key(), svc.clone());
                    svc.call(req).map_err(Into::into)
                });

            Box::new(fut)
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::future::ok;
    use futures::{Async, Future, Poll};
    use rand;
    use rand::prelude::*;
    use tower_service::Service;
    use tower_test::{assert_request_eq, mock};

    use super::{Key, UniquePool};

    use std::cmp::PartialEq;

    #[derive(Eq, PartialEq, Debug, Hash, Clone)]
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

    impl Key<Req> for Req {
        fn get_key(&self) -> Req {
            self.clone()
        }
    }

    #[derive(Clone)]
    struct ResponseVal(String);

    impl Service<Req> for ResponseVal {
        type Response = String;
        type Error = Box<std::error::Error + Send + Sync + 'static>;
        type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error> + Send + 'static>;

        fn poll_ready(&mut self) -> Poll<(), Self::Error> {
            Ok(Async::Ready(()))
        }

        fn call(&mut self, _: Req) -> Self::Future {
            Box::new(ok::<String, _>(self.0.clone()))
        }
    }

    #[test]
    fn store_service() {
        let (mut mock, mut handle) = mock::pair::<Req, ResponseVal>();

        assert!(mock.poll_ready().unwrap().is_ready());
        let mut pool = UniquePool::new(mock);

        let req = gen_request();

        assert!(pool.poll_ready().unwrap().is_ready());
        let mut response = pool.call(req.clone());

        let send_response = assert_request_eq!(handle, req);

        assert_eq!(pool.pool.lock().unwrap().len(), 1);

        with_task(|| {
            assert!(response.poll().unwrap().is_not_ready());
        });

        let response_type = ResponseVal("test".to_string());

        send_response.send_response(response_type);

        assert_eq!(response.wait().unwrap().as_str(), "test");
    }
}
