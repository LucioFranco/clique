use crate::{
    common::Endpoint,
    transport::proto::{EdgeStatus, Metadata},
};

#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    Members(Vec<Endpoint>),
    Join(Endpoint),
    Leave(Endpoint),
    ViewChange(Vec<NodeStatusChange>),
    ViewChangeProposal(Vec<NodeStatusChange>),
    Start,
}

impl Event {
    pub fn new() -> Self {
        Event::Start
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct NodeStatusChange {
    pub(crate) endpoint: Endpoint,
    pub(crate) status: EdgeStatus,
    pub(crate) metadata: Metadata,
}
