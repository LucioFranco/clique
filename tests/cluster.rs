use clique::{Cluster, Endpoint, Message, Transport2};
use std::collections::HashMap;
use tokio::sync::mpsc;

type Sender = mpsc::Sender<(Endpoint, Message)>;
type Receiver = mpsc::Receiver<(Endpoint, Message)>;

#[derive(Debug)]
struct SimulatedCluster {
    nodes: HashMap<Endpoint, Sender>,
    messages: Receiver,
}

impl SimulatedCluster {
    pub fn new(size: usize) -> Self {
        let (tx, rx) = mpsc::channel(1024);

        let mut nodes = HashMap::new();

        for i in 0..size {
            let endpoint = format!("node:{}", i);
            let (tx1, rx1) = mpsc::channel(1024);
            let network = Network {
                send: tx.clone(),
                recv: rx1,
            };

            nodes.insert(endpoint.clone(), tx1);

            if i == 0 {
                tokio::spawn(async move { Cluster::start(network, endpoint).await });
            }
        }

        Self {
            nodes,
            messages: rx,
        }
    }
}

#[derive(Debug)]
struct Network {
    send: Sender,
    recv: Receiver,
}

#[async_trait::async_trait]
impl Transport2 for Network {
    type Error = std::io::Error;

    async fn send_to(&mut self, dst: Endpoint, msg: Message) -> Result<(), Self::Error> {
        self.send.send((dst, msg)).await.unwrap();
        Ok(())
    }

    async fn recv(&mut self) -> Result<(Endpoint, Message), Self::Error> {
        Ok(self.recv.recv().await.expect("message queue finished"))
    }
}

#[tokio::test]
async fn single_node() {
    SimulatedCluster::new(1);

    // loop {}
}
