use crate::broadcast::Broadcast;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

type SeqNum = u32;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Packet {
    addr: SocketAddr,
    seq: SeqNum,
    kind: PacketKind,
    broadcasts: Vec<Broadcast>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum PacketKind {
    Ping,
    Ack,
    PingReq,
    NAck,
}

impl Packet {
    pub fn new(
        addr: SocketAddr,
        kind: PacketKind,
        seq: SeqNum,
        broadcasts: Vec<Broadcast>,
    ) -> Self {
        Packet {
            addr,
            seq,
            kind,
            broadcasts,
        }
    }

    pub fn ping(addr: SocketAddr, seq: SeqNum, broadcasts: Vec<Broadcast>) -> Self {
        Packet::new(addr, PacketKind::Ping, seq, broadcasts)
    }

    pub fn ack(addr: SocketAddr, seq: SeqNum, broadcasts: Vec<Broadcast>) -> Self {
        Packet::new(addr, PacketKind::Ack, seq, broadcasts)
    }

    pub fn ping_req(addr: SocketAddr, seq: SeqNum, broadcasts: Vec<Broadcast>) -> Self {
        Packet::new(addr, PacketKind::PingReq, seq, broadcasts)
    }

    pub fn nack(addr: SocketAddr, seq: SeqNum, broadcasts: Vec<Broadcast>) -> Self {
        Packet::new(addr, PacketKind::NAck, seq, broadcasts)
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn seq(&self) -> SeqNum {
        self.seq
    }

    pub fn broadcasts(&self) -> &Vec<Broadcast> {
        &self.broadcasts
    }

    pub fn kind(&self) -> &PacketKind {
        &self.kind
    }

    pub fn into_kind(self) -> PacketKind {
        self.kind
    }
}
