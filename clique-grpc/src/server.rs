use crate::{
    proto::{self, server},
    Error,
};
use clique::transport::{Request, Response};
use futures::sync::oneshot as oneshot01;
use futures::{Future as Future01, Stream as Stream01};
use futures03::TryFutureExt;
use std::net::SocketAddr;
use tokio_sync::{mpsc, oneshot};
use tokio_tcp::TcpListener;
use tower_grpc::{Code, Status};
use tower_hyper::server::Http;

pub struct GrpcServer {
    msg_rx: Option<
        mpsc::Receiver<Result<(Request, oneshot::Sender<Result<Response, clique::Error>>), Error>>,
    >,
    target_tx: Option<oneshot01::Sender<SocketAddr>>,
}

#[derive(Clone)]
pub struct Svc {
    msg_tx:
        mpsc::Sender<Result<(Request, oneshot::Sender<Result<Response, clique::Error>>), Error>>,
}

pub struct Background {
    inner: Option<Svc>,
    target_rx: oneshot01::Receiver<SocketAddr>,
    state: State,
}

enum State {
    Waiting,
    Running(Box<dyn Future01<Item = (), Error = ()> + Send>),
}

impl GrpcServer {
    pub fn new() -> (Self, Background) {
        let (msg_tx, msg_rx) = mpsc::channel(100);
        let (target_tx, target_rx) = oneshot01::channel();

        let server = GrpcServer {
            msg_rx: Some(msg_rx),
            target_tx: Some(target_tx),
        };
        let svc = Svc { msg_tx };

        let bg = Background {
            inner: Some(svc),
            target_rx,
            state: State::Waiting,
        };

        (server, bg)
    }

    pub fn create(
        &mut self,
        target: SocketAddr,
    ) -> mpsc::Receiver<Result<(Request, oneshot::Sender<Result<Response, clique::Error>>), Error>>
    {
        self.target_tx.take().unwrap().send(target).unwrap();
        self.msg_rx.take().unwrap()
    }
}

impl server::MembershipService for Svc {
    type SendRequestFuture = Box<
        dyn Future01<Item = tower_grpc::Response<proto::Response>, Error = tower_grpc::Status>
            + Send,
    >;

    fn send_request(
        &mut self,
        request: tower_grpc::Request<proto::Request>,
    ) -> Self::SendRequestFuture {
        let inbound_req = request.into_inner();
        let (res_tx, res_rx) = oneshot::channel();
        let req = Request::new("someendpoint".into(), inbound_req.into());

        // TODO: poll_ready first find way to
        self.msg_tx.try_send(Ok((req, res_tx))).unwrap();

        Box::new(
            res_rx
                .compat()
                .map(|r| tower_grpc::Response::new(r.unwrap().into()))
                .map_err(|_| Status::new(Code::Unknown, "Unknown grpc error")),
        )
    }
}

impl Future01 for Background {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> futures::Poll<(), ()> {
        loop {
            match &mut self.state {
                State::Waiting => {
                    let target = futures::try_ready!(self.target_rx.poll().map_err(|_| ()));

                    let svc = self.inner.take().expect("Started server twice.");

                    let svc = server::MembershipServiceServer::new(svc);
                    let mut server = tower_hyper::Server::new(svc);

                    let http = Http::new().http2_only(true).clone();

                    let bind = TcpListener::bind(&target).expect("bind");

                    let server = bind
                        .incoming()
                        .for_each(move |sock| {
                            if let Err(e) = sock.set_nodelay(true) {
                                return Err(e);
                            }

                            let serve = server.serve_with(sock, http.clone());
                            tokio_executor::spawn(serve.map_err(|_| ()));

                            Ok(())
                        })
                        .map_err(|e| eprintln!("accept error: {}", e));

                    self.state = State::Running(Box::new(server));
                    continue;
                }

                State::Running(fut) => return fut.poll(),
            }
        }
    }
}
