use std::time::Duration;
use std::collections::{HashMap, VecDeque};

use crate::messaging::*;

#[derive(Debug)]
pub struct Config {
    batching_window: Duration,
    default_fdinterval_delay: Duration,
    default_fdinterval: Duration
}

pub struct MembershipView;

pub struct MultiNodeCutDetector;

pub struct MetadataManager;

pub struct Membership {
    config: Config,
    cut_detector: MultiNodeCutDetector,
    membership_view: MembershipView,
    my_addr: Endpoint,
    broadcast: Box<dyn Broadcast>,
    client: Box<dyn Client>,
    joiners_to_respond_to: HashMap<Endpoint, VecDeque<RapidResponse>>,
    joiners_uuid: HashMap<Endpoint, NodeId>,
    joiners_meta: HashMap<Endpoint, Metadata>,
    metadataManager: MetadataManager,
} 