use crate::broadcast::Broadcast;
use serde::{Deserialize, Serialize};

type SeqNum = u32;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum Packet {
    Ping(SeqNum, Vec<Broadcast>),
    Ack(SeqNum, Vec<Broadcast>),
    PingReq(SeqNum, Vec<Broadcast>),
    NAck(SeqNum, Vec<Broadcast>),
}
