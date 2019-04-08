use crate::peer::Peer;
use indexmap::IndexMap;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct PeerList {
    list: IndexMap<SocketAddr, Peer>,
}

impl PeerList {
    pub fn new() -> Self {
        PeerList::from(IndexMap::new())
    }

    pub fn from(list: IndexMap<SocketAddr, Peer>) -> Self {
        PeerList { list }
    }

    pub fn add(&mut self, peer: Peer) {
        self.list.insert(peer.addr(), peer);
    }

    pub fn remove(&mut self, peer: Peer) {
        self.list.remove(&peer.addr());
    }

    pub fn pick_gossip(&mut self) -> Option<Peer> {
        self.list.pop().map(|(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::Peer;

    #[test]
    fn list_gossip_single_node() {
        let mut list = PeerList::new();

        let addr = "127.0.0.1:8888".parse().unwrap();
        let peer = Peer::new("name".into(), addr);

        list.add(peer.clone());

        assert_eq!(list.pick_gossip(), Some(peer));
    }
}
