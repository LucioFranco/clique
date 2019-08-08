use crate::{
    common::Endpoint,
    transport::proto::{AlertMessage, EdgeStatus},
};
use std::collection::{HashMap, HashSet};

const NUM_MIN: usize = 3;

pub struct MultiNodeCutDetector {
    /// Number of observers per subject and vice versa
    num: usize,
    /// High water mark
    high: usize,
    /// Low water mark
    low: usize,
    proposal_count: usize,
    updates_in_progress: usize,
    reports_per_host: HashMap<Endpoint, HashMap<usize, Endpoint>>,
    proposal: Vec<Endpoint>,
    pre_proposal: HashSet<Endpoint>,
    seen_down_link_events: bool,
}

impl MultiNodeCutDetector {
    pub fn new(num: usize, high: usize, low: usize) -> Self {
        if high > num || low > high || num < NUM_MIN || low <= 0 || high <= 0 {
            panic!("Invalid arguments passed to cut detector")
        }

        Self {
            num,
            high,
            low,
            proposal_count: 0,
            updates_in_progress: 0,
            reports_per_host: HashMap::new(),
            proposal: Vec::new(),
            pre_proposal: HashSet::new(),
            seen_down_link_events: false,
        }
    }

    fn aggregate(message: AlertMessage) -> Vec<Endpoint> {
        msg.ring_number
            .iter()
            .map(|ring_number| {
                self.aggregate_for_proposal(
                    message.edge_src,
                    message.edge_dst,
                    msg.edge_status,
                    ringN,
                )
            })
            .flatten()
            .collect()
    }

    fn aggregate_for_proposal(
        &mut self,
        link_src: Endpoint,
        link_dst: Endpoint,
        edge_status: EdgeStatus,
        ring_number: usize,
    ) -> Vec<Endpoint> {
        debug_assert!(ring_number <= self.num);

        if edge_status == EdgeStatus::Down {
            self.seen_down_link_events = true;
        }

        let reports_per_host = self
            .reports_per_host
            .entry(link_dst)
            .or_insert(HashMap::new());

        if (reports_per_host.contains(ring_number)) {
            // Duplicate announcement, ignore
            return vec![];
        }

        reports_per_host.insert(ring_number, link_src);
        let num_reports_for_host = reports_per_host.len();

        if (num_reports_for_host == self.low) {
            self.updates_in_progress += 1;
            self.pre_proposal.add(link_dst);
        }

        if (num_reports_for_host == self.high) {
            // Enough reports about link_dst have been received that it is safe to act upon,
            // provided there are no other nodes with low < num_reports < high
            self.pre_proposal.remove(link_dst);
            self.proposal.push(link_dst);
            self.updates_in_progress -= 1;

            if (self.updates_in_progress == 0) {
                // No outstanding updates, so all nodes have crossed the H threshold. Reports
                // are not part of a single proposal
                self.proposal_count += 1;
                let return_val = self.proposal.drain().collect();

                self.clear();

                return return_val;
            }
        }

        vec![]
    }

    pub fn invalidate_failing_edges(view: &mut MembershipView) -> Vec<Endpoint> {
        // link invalidation is only required when there are failing nodes
        if !self.seen_down_link_events {
            return vec![];
        }

        let proposals_to_return = vec![];

        for node_in_flux in self.pre_proposal.iter() {
            let observers = if view.is_host_present(node_in_flux) {
                view.get_observers(node_in_flux)
            } else {
                view.get_expected_observers(node_in_flux)
            };

            // Account for all edges between nodes that are past the low threshold
            let mut ring_number = 0;

            for observer in observers.iter() {
                if self.proposal.contains(observer) || self.pre_proposal.contains(observer) {
                    // Implicit detection of edges between observer and node_in_flux
                    let edge_status = if view.is_host_present(node_in_flux) {
                        EdgeStatus::Up
                    } else {
                        EdgeStatus::Down
                    };

                    proposals_to_return.extend(self.aggregate_for_proposal(
                        observer,
                        node_in_flux,
                        edge_status,
                        ring_number,
                    ));
                }
            }

            ring_number += 1;
        }

        return proposals_to_return;
    }

    fn clear(&mut self) {
        self.reports_per_host.clear();
        self.proposal.clear();
        self.updates_in_progress = 0;
        self.proposal_count = 0;
        self.pre_proposal.clear();
        self.seen_down_link_events = false;
    }
}
