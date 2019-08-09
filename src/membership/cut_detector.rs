use crate::{
    common::{Endpoint, RingNumber},
    membership::view::View,
    transport::proto::{AlertMessage, EdgeStatus},
    test_utils::trace_init,
};

use std::collections::{HashMap, HashSet};

use tracing_attributes::instrument;

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
    reports_per_host: HashMap<Endpoint, HashMap<RingNumber, Endpoint>>,
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

    #[instrument]
    fn aggregate(&mut self, message: AlertMessage) -> Vec<Endpoint> {
        message
            .ring_number()
            .iter()
            .map(|ring_number| {
                self.aggregate_for_proposal(
                    message.edge_src().clone(),
                    message.edge_dst().clone(),
                    message.edge_status(),
                    *ring_number,
                )
            })
            .flatten()
            .collect()
    }

    #[instrument]
    fn aggregate_for_proposal(
        &mut self,
        link_src: Endpoint,
        link_dst: Endpoint,
        edge_status: EdgeStatus,
        ring_number: RingNumber,
    ) -> Vec<Endpoint> {
        debug_assert!(ring_number <= self.num as RingNumber);


        if edge_status == EdgeStatus::Down {
            self.seen_down_link_events = true;
        }

        let reports_per_host = self
            .reports_per_host
            .entry(link_dst.clone())
            .or_insert(HashMap::new());

        if reports_per_host.contains_key(&ring_number) {
            // Duplicate announcement, ignore
            return vec![];
        }

        reports_per_host.insert(ring_number, link_src);
        let num_reports_for_host = reports_per_host.len();

        if num_reports_for_host == self.low {
            self.updates_in_progress += 1;
            self.pre_proposal.insert(link_dst.clone());
        }

        if num_reports_for_host == self.high {
            // Enough reports about link_dst have been received that it is safe to act upon,
            // provided there are no other nodes with low < num_reports < high
            self.pre_proposal.remove(&link_dst);
            self.proposal.push(link_dst);
            self.updates_in_progress -= 1;

            if self.updates_in_progress == 0 {

                tracing::info!("No more updates in progress");
                // No outstanding updates, so all nodes have crossed the H threshold. Reports
                // are not part of a single proposal
                self.proposal_count += 1;
                let return_val = self.proposal.drain(0..).collect();

                self.proposal.clear();

                return return_val;
            }
        }

        vec![]
    }

    pub fn invalidate_failing_edges(&mut self, view: &mut View) -> Vec<Endpoint> {
        // link invalidation is only required when there are failing nodes
        if !self.seen_down_link_events {
            return vec![];
        }

        let mut proposals_to_return = vec![];

        // Gotta do this, as there is no iter_mut on `HashSet`
        let mut pre_proposal_vec: Vec<Endpoint> = self.pre_proposal.iter().cloned().collect();

        for node_in_flux in pre_proposal_vec.iter_mut() {
            let observers = if view.is_host_present(&node_in_flux) {
                view.get_observers(&node_in_flux)
                    .expect("Node not in ring?")
            } else {
                view.get_expected_observers(&node_in_flux)
            };

            // Account for all edges between nodes that are past the low threshold
            let mut ring_number = 0;

            for observer in observers.iter() {
                if self.proposal.contains(observer) || self.pre_proposal.contains(observer) {
                    // Implicit detection of edges between observer and node_in_flux
                    let edge_status = if view.is_host_present(&node_in_flux) {
                        EdgeStatus::Up
                    } else {
                        EdgeStatus::Down
                    };

                    proposals_to_return.extend(self.aggregate_for_proposal(
                        observer.clone(),
                        node_in_flux.clone(),
                        edge_status,
                        ring_number,
                    ));
                }
                ring_number += 1;
            }
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

    pub(crate) fn get_proposal_count(&mut self) -> usize {
        self.proposal_count
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    const NUM: usize = 10;
    const HIGH: usize = 8;
    const LOW: usize = 2;
    const CONFIG_ID: i64 = -1; // Does not affect the following tests(?)

    #[test]
    fn cut_detection_test() {
        let mut cut_detector = MultiNodeCutDetector::new(NUM, HIGH, LOW);
        let dst = String::from("127.0.0.2:2");

        let mut ret = vec![];

        for i in 0..HIGH - 1 {
            ret = cut_detector.aggregate(AlertMessage::new(
                format!("127.0.0.1:{}", i + 1),
                dst.clone(),
                EdgeStatus::Up,
                CONFIG_ID,
                i.try_into().unwrap(),
            ));

            assert_eq!(0, ret.len());
            assert_eq!(0, cut_detector.get_proposal_count());
        }

        ret = cut_detector.aggregate(AlertMessage::new(
            format!("127.0.0.1:{}", HIGH),
            dst,
            EdgeStatus::Up,
            CONFIG_ID,
            (HIGH - 1).try_into().unwrap(),
        ));

        assert_eq!(1, ret.len());
        assert_eq!(1, cut_detector.get_proposal_count());
    }

    #[test]
    fn cut_detection_test_blocking_one_blocker() {
        trace_init();

        let mut cut_detector = MultiNodeCutDetector::new(NUM, HIGH, LOW);
        let dst1 = String::from("127.0.0.2:2");
        let dst2 = String::from("127.0.0.2:2");

        let mut ret = vec![];

        for i in 0..HIGH - 1 {
            ret = cut_detector.aggregate(AlertMessage::new(
                format!("127.0.0.1:{}", i + 1),
                dst1.clone(),
                EdgeStatus::Up,
                CONFIG_ID,
                i.try_into().unwrap(),
            ));

            assert_eq!(0, ret.len());
            assert_eq!(0, cut_detector.get_proposal_count());
        }

        for i in 0..HIGH - 1 {
            ret = cut_detector.aggregate(AlertMessage::new(
                format!("127.0.0.1:{}", i + 1),
                dst2.clone(),
                EdgeStatus::Up,
                CONFIG_ID,
                i.try_into().unwrap(),
            ));

            assert_eq!(0, ret.len());
            assert_eq!(0, cut_detector.get_proposal_count());
        }

        ret = cut_detector.aggregate(AlertMessage::new(
            format!("127.0.0.1:{}", HIGH),
            dst1,
            EdgeStatus::Up,
            CONFIG_ID,
            (HIGH - 1).try_into().unwrap(),
        ));

        assert_eq!(0, ret.len());
        assert_eq!(0, cut_detector.get_proposal_count());

        ret = cut_detector.aggregate(AlertMessage::new(
            format!("127.0.0.1:{}", HIGH),
            dst2,
            EdgeStatus::Up,
            CONFIG_ID,
            (HIGH - 1).try_into().unwrap(),
        ));

        assert_eq!(2, ret.len());
        assert_eq!(1, cut_detector.get_proposal_count());
    }
}
