use crate::common::Endpoint;

#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    Members(Vec<Endpoint>),
    Join(Endpoint),
    Leave(Endpoint),
    ViewChange,
    Start,
}

impl Event {
    pub fn new() -> Self {
        Event::Start
    }
}
