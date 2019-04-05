use crate::node::Broadcast;
use serde::{Deserialize, Serialize};

type SeqNum = u32;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Message {
    Ping(SeqNum, Vec<Broadcast>),
    Ack(SeqNum, Vec<Broadcast>),
    PingReq(SeqNum, Vec<Broadcast>),
    NAck(SeqNum, Vec<Broadcast>),
}
