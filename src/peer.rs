use std::fmt;
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug)]
pub struct Peer {
    name: String,
    addr: SocketAddr,
    state: State,
}

#[derive(Debug)]
enum State {
    Alive,
    Suspect,
    Dead,
}

impl Peer {
    pub fn new(name: String, addr: SocketAddr) -> Self {
        Peer {
            name,
            addr,
            state: State::Alive,
        }
    }
}
