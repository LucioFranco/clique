use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use futures::{Future, FutureExt, Stream, StreamExt, stream::FuturesUnordered};
use rand::Rng;
use tokio_sync::oneshot;
use tokio_timer::Delay;

mod paxos;

use paxos::Paxos;

use crate::{
    common::Endpoint,
    error::{Error, Result},
    transport::{
        proto::{self, Endpoint, Phase2bMessage},
        Broadcast, Client, Request, Response,
    },
};

const BASE_DELAY: u64 = 1000;

#[derive(Debug)]
enum CancelPaxos {}

pub struct FastPaxos<'a, C, B> {
    broadcast: &'a mut B,
    my_addr: Endpoint,
    size: usize,
    paxos: Paxos<'a, C>,
    decided: AtomicBool,
    /// Channel used to communicate with upstream about decision
    decision_tx: oneshot::Sender<Vec<Endpoint>>,
    /// Channel the paxos instance will use to communicate with us about decision
    // paxos_rx: oneshot::Receiver<Vec<Endpoint>>,
    scheduled_paxos: Option<Box<dyn Future<Output = Result<()>>>>,
    config_id: usize,
}

impl<'a, C, B> FastPaxos<'a, C, B> {
    pub fn new(
        my_addr: Endpoint,
        size: usize,
        client: &'a mut C,
        broadcast: &'a mut B,
        decision_tx: oneshot::Sender<Vec<Endpoint>>,
        proposal_rx: oneshot::Receiver<Vec<Endpoint>>,
        config_id: usize,
    ) -> FastPaxos<'a, C, B>
    where
        C: Client,
        B: Broadcast,
    {
        FastPaxos {
            broadcast,
            decision_tx,
            config_id,
            size: size,
            my_addr: my_addr.clone(),
            paxos: Paxos::new(client, size, my_addr, config_id),
            decided: AtomicBool::new(false),
            scheduled_paxos: None,
        }
    }

    pub async fn propose(&self, proposal: Vec<Endpoint>) -> Result<()> {
        let mut paxos_delay = Delay::new(Instant::now() + self.get_random_delay()).fuse();
        let (cancel_tx, cancel_rx) = oneshot::channel::<CancelPaxos>();
        let mut fut_runner = FuturesUnordered::new();

        fut_runner.push(async {
            futures::select! {
                _ = paxos_delay => {
                    self.start_classic_round().await
                },
                cancel = cancel_rx.fuse().select_next_some() => { Ok(()) }
            };
        });

        self.broadcast.broadcast(Phase2bMessage {}).await;
        fut_runner.await;

        Ok(())
    }

    pub async fn handle_message(&self, request: Request) -> Result<Response> {
        match request.kind() {
            proto::RequestKind::Consensus(_) => self.handle_fast_round(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_1a(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_1b(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_2a(request).await,
            proto::RequestKind::Consensus(_) => self.paxos.handle_phase_2b(request).await,
            _ => Err(Error::new_unexpected_request(None)),
        }
    }

    async fn handle_fast_round(&self, request: Request) -> crate::Result<Response> {
        unimplemented!()
    }

    async fn on_decide(self, hosts: Vec<Endpoint>) -> Result<()> {
        // This is the only place where the value is set to `true`.
        self.decided.store(true, Ordering::SeqCst);

        if let Some(paxos_instance) = &self.scheduled_paxos {
            drop(paxos_instance);
        }

        self.decision_tx
            .send(hosts)
            .map_err(|_| Error::new_broken_pipe(None))
    }

    async fn start_classic_round(&mut self) -> Result<()> {
        if !self.decided.load(Ordering::SeqCst) {
            self.paxos.start_round().await
        } else {
            Ok(())
        }
    }

    fn get_random_delay(&self) -> Duration {
        let jitter_rate: f64 = 1f64 / self.size as f64;
        let mut rng = rand::thread_rng();

        let jitter = ((-1000f64 * (1.0f64 - rng.gen::<f64>()).ln()) / jitter_rate) as u64;

        Duration::from_millis(jitter + BASE_DELAY)
    }
}
