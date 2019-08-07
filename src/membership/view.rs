use crate::{
    common::{ConfigId, Endpoint, NodeId},
    error::{Error, Result},
    transport::proto::JoinStatus,
};
use std::{collections::HashSet, hash::Hasher};
use twox_hash::XxHash64;

type Ring = crate::membership::ring::Ring<Endpoint>;

/// Represents the current view of membership.
///
/// The `View` is responsible for representing each view of the current membership. It
/// contains an edge expander graph approximation that allows for a strongly connected toplogy of groups
/// of observers and subjects.
///
/// The to topology is setup of `K` rings each with unique seed values. This provides `K` different
/// pseudo ordered rings. A node has one observer and one subject in each ring. Thus the set of
/// observers and subjects is the successor and precessor in each ring.
///
/// TODO: talk about what observers, subjects and the `K` value.
#[derive(Debug, Clone)]
pub struct View {
    k: i32,
    rings: Vec<Ring>,
    seen: HashSet<NodeId>,
    current_config: Configuration,
    current_config_id: ConfigId,
    should_update_configuration_id: bool,
}

/// Represents the current configuration of the view.
#[derive(Debug, Clone, Default)]
pub struct Configuration {
    node_ids: Vec<NodeId>,
    endpoints: Vec<Endpoint>,
}

#[allow(dead_code)]
impl View {
    /// Create a new `View` with value `K`.
    pub fn new(k: i32) -> Self {
        assert!(k > 0);

        let mut rings = Vec::with_capacity(k as usize);

        for i in 0..k {
            rings.push(Ring::new(i as u64));
        }

        Self {
            k,
            rings,
            seen: HashSet::new(),
            current_config_id: u64::max_value(),
            current_config: Configuration::default(),
            should_update_configuration_id: true,
        }
    }

    /// Bootstrap a view with the provided node ids and endpoints.
    pub fn bootstrap(k: i32, node_ids: Vec<NodeId>, endpoints: Vec<Endpoint>) -> Self {
        let mut view = View::new(k);

        view.seen = node_ids.into_iter().collect();

        for ring in &mut view.rings {
            for endpoint in &endpoints {
                ring.insert(endpoint.clone());
            }
        }

        view
    }

    /// Checks if a node with the accompanying `NodeId` is safe to add to the network.
    pub fn is_safe_to_join(&self, node: &Endpoint, node_id: &NodeId) -> JoinStatus {
        // TODO: what if host is not in the ring?
        if self.rings[0].contains(node.clone()) {
            JoinStatus::HostnameAlreadyInRing
        } else if self.seen.contains(node_id) {
            JoinStatus::NodeIdAlreadyInRing
        } else {
            JoinStatus::SafeToJoin
        }
    }

    /// Add a node to the ring.
    ///
    /// # Errors
    ///
    /// Returns `UuidAlreadySeen` if the `NodeId` has already been seen and returns
    /// `NodeAlreadyInRing` if the node being added already exists.
    pub fn ring_add(&mut self, node: Endpoint, node_id: NodeId) -> Result<()> {
        if self.is_node_id_present(&node_id) {
            return Err(Error::new_uuid_already_seen());
        }

        if self.rings[0].contains(node.clone()) {
            return Err(Error::new_node_already_in_ring());
        }

        for ring in &mut self.rings {
            // Should this be append?
            ring.insert(node.clone());
        }

        self.seen.insert(node_id);
        self.should_update_configuration_id = true;

        Ok(())
    }

    /// Delete a node from the view.
    ///
    /// # Errors
    ///
    /// Returns `NodeNotInRing` if the node doesn't exist in the view already.
    pub fn ring_delete(&mut self, node: &Endpoint) -> Result<()> {
        if !self.rings[0].contains(node.clone()) {
            return Err(Error::new_node_not_in_ring());
        }

        for ring in &mut self.rings {
            ring.remove(node.clone());
        }

        self.should_update_configuration_id = true;

        Ok(())
    }

    /// Get a list of size `K` observers for the provided node.
    ///
    /// # Errors
    ///
    /// Returns `NodeNotInRing` if the provided node doesn't exist already.
    pub fn get_observers(&self, node: &Endpoint) -> Result<Vec<Endpoint>> {
        if !self.rings[0].contains(node.clone()) {
            return Err(Error::new_node_not_in_ring());
        }

        if self.rings[0].len() <= 1 {
            return Ok(Vec::new());
        }

        let mut observers = Vec::new();

        for ring in &self.rings {
            if let Some(successor) = ring.higher(node.clone()) {
                observers.push(successor.clone());
            } else {
                let first = ring.first().unwrap();
                observers.push(first.clone());
            }
        }

        Ok(observers)
    }

    /// Get a list of size `K` subjects for the provided node.
    ///
    /// # Errors
    ///
    /// Returns `NodeNotInRing` if the provided node doesn't exist already.
    pub fn get_subjects(&self, node: &Endpoint) -> Result<Vec<Endpoint>> {
        if !self.rings[0].contains(node.clone()) {
            return Err(Error::new_node_not_in_ring());
        }

        if self.rings[0].len() <= 1 {
            return Ok(Vec::new());
        }

        let predecessors = self.get_predecessors(node);
        Ok(predecessors)
    }

    /// Get the observers that the node would have if it were in the view already.
    pub fn get_expected_observers(&self, node: &Endpoint) -> Vec<Endpoint> {
        self.get_predecessors(node)
    }

    /// Get the `K`th ring.
    pub fn get_ring(&self, k: i32) -> Option<&Ring> {
        self.rings.get(k as usize)
    }

    /// Get the current configuration.
    pub fn get_config(&mut self) -> &mut Configuration {
        if self.should_update_configuration_id {
            let endpoints = self.rings[0].clone().into_iter().collect();
            let seen = self.seen.clone().into_iter().collect();

            let new_conf = Configuration::new(endpoints, seen);
            self.current_config_id = new_conf.config_id();
            self.current_config = new_conf;
            self.should_update_configuration_id = false;
        }

        &mut self.current_config
    }

    /// Get the current size of the membership.
    pub fn get_membership_size(&self) -> usize {
        self.rings[0].len()
    }

    /// Check if the node id has been seen.
    pub fn is_node_id_present(&self, node_id: &NodeId) -> bool {
        self.seen.contains(node_id)
    }

    /// Check if the node is present.
    pub fn is_host_present(&self, node: &Endpoint) -> bool {
        self.rings[0].contains(node.clone())
    }

    fn get_predecessors(&self, node: &Endpoint) -> Vec<Endpoint> {
        if self.rings[0].is_empty() {
            return Vec::new();
        }

        let mut predecessors = Vec::new();

        for ring in &self.rings {
            if let Some(predecessor) = ring.lower(node.clone()) {
                predecessors.push(predecessor.clone());
            } else {
                let last = ring.last().unwrap();
                predecessors.push(last.clone());
            }
        }

        predecessors
    }
}

impl Configuration {
    pub fn new(mut endpoints: Vec<Endpoint>, mut node_ids: Vec<NodeId>) -> Self {
        endpoints.sort();
        endpoints.dedup();

        node_ids.sort();
        node_ids.dedup();

        Self {
            endpoints,
            node_ids,
        }
    }

    pub fn config_id(&self) -> ConfigId {
        let mut hasher = XxHash64::with_seed(0);

        for node in &self.node_ids {
            hasher.write(&node.as_bytes());
        }

        for endpoint in &self.endpoints {
            hasher.write(endpoint.as_ref());
        }

        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::NodeId;
    use crate::error::ErrorKind;
    use std::collections::HashSet;

    const K: i32 = 10;

    #[test]
    fn ring_add_one() {
        let mut view = View::new(K);
        view.ring_add("A".into(), NodeId::new()).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 1);

        for i in 0..K {
            assert_eq!(view.get_ring(i).unwrap().len(), 1);
        }
    }

    #[test]
    fn ring_add_multiple() {
        let mut view = View::new(K);
        let num = 10;

        for i in 0..10 {
            view.ring_add(i.to_string(), NodeId::new()).unwrap();
        }

        for i in 0..K {
            assert_eq!(view.get_ring(i).unwrap().len(), num);
        }

        for i in 0..10 {
            view.ring_add(i.to_string(), NodeId::new()).unwrap_err();
        }
    }

    #[test]
    fn ring_delete() {
        let mut view = View::new(K);
        let node = "A".to_string();
        view.ring_add(node.clone(), NodeId::new()).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 1);

        view.ring_delete(&node).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 0);
    }

    #[test]
    fn ring_re_addition() {
        let mut view = View::new(K);
        view.ring_add("A".into(), NodeId::new()).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 1);

        let err = view.ring_add("A".into(), NodeId::new()).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::NodeAlreadyInRing);
    }

    #[test]
    fn ring_delete_invalid() {
        let mut view = View::new(K);

        let err = view.ring_delete(&"A".to_string()).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::NodeNotInRing);
    }

    #[test]
    fn monitor_edge_one_node() {
        let mut view = View::new(K);

        view.ring_add("A".into(), NodeId::new()).unwrap();

        let obs = view.get_observers(&"A".to_string()).unwrap();
        let sub = view.get_subjects(&"A".to_string()).unwrap();
        assert!(obs.is_empty());
        assert!(sub.is_empty());
    }

    #[test]
    fn monitor_edge_two_nodes() {
        let mut view = View::new(K);

        view.ring_add("A".into(), NodeId::new()).unwrap();
        view.ring_add("B".into(), NodeId::new()).unwrap();

        let obs = view.get_observers(&"A".to_string()).unwrap();
        let sub = view.get_subjects(&"A".to_string()).unwrap();
        assert_eq!(obs.len(), K as usize);
        assert_eq!(sub.len(), K as usize);
    }

    #[test]
    fn monitor_edge_three_nodes() {
        let mut view = View::new(K);

        view.ring_add("A".into(), NodeId::new()).unwrap();
        view.ring_add("B".into(), NodeId::new()).unwrap();
        view.ring_add("C".into(), NodeId::new()).unwrap();

        for node in vec!["A", "B", "C"] {
            let mut obs = view.get_observers(&node.to_string()).unwrap();
            assert_eq!(obs.len(), K as usize);
            obs.sort();
            obs.dedup();
            assert_eq!(obs.len(), 2);

            let mut sub = view.get_subjects(&node.to_string()).unwrap();
            assert_eq!(sub.len(), K as usize);
            sub.sort();
            sub.dedup();
            assert_eq!(sub.len(), 2);
        }

        view.ring_delete(&"B".into()).unwrap();

        let mut obs = view.get_observers(&"A".to_string()).unwrap();
        assert_eq!(obs.len(), K as usize);
        obs.sort();
        obs.dedup();
        assert_eq!(obs.len(), 1);

        let mut sub = view.get_subjects(&"A".to_string()).unwrap();
        assert_eq!(sub.len(), K as usize);
        sub.sort();
        sub.dedup();
        assert_eq!(sub.len(), 1);
    }

    #[test]
    fn monitor_edge_multiple_nodes() {
        let num = 1000;

        let nodes = (0..num)
            .into_iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>();

        let mut view = View::new(K);
        for node in &nodes {
            view.ring_add(node.clone(), NodeId::new()).unwrap();
        }

        for node in &nodes {
            let obs = view.get_observers(&node.to_string()).unwrap();
            assert_eq!(obs.len(), K as usize);
            let sub = view.get_subjects(&node.to_string()).unwrap();
            assert_eq!(sub.len(), K as usize);
        }
    }

    #[test]
    fn monitor_edge_bootstrap() {
        let mut view = View::new(K);

        view.ring_add("A".into(), NodeId::new()).unwrap();

        let exp_obs = view.get_expected_observers(&"".to_string());
        assert_eq!(exp_obs.len(), K as usize);
        let mut exp_obs_dedup = exp_obs.clone();
        exp_obs_dedup.sort();
        exp_obs_dedup.dedup();
        assert_eq!(exp_obs_dedup.len(), 1);
        assert_eq!(exp_obs[0], "A".to_string());
    }

    #[test]
    fn node_unique_no_deletions() {
        let mut view = View::new(K);

        let e1 = "A".to_string();
        let n1 = NodeId::new();

        view.ring_add(e1.clone(), n1.clone()).unwrap();

        let err = view.ring_add(e1.clone(), n1.clone()).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::UuidAlreadySeen);

        let err = view.ring_add(e1.clone(), NodeId::new()).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::NodeAlreadyInRing);

        let err = view.ring_add("B".to_string(), n1.clone()).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::UuidAlreadySeen);

        view.ring_add("C".to_string(), NodeId::new()).unwrap();
    }

    #[test]
    fn node_unique_with_deletions() {
        let mut view = View::new(K);

        let e1 = "A".to_string();
        let n1 = NodeId::new();

        view.ring_add(e1.clone(), n1.clone()).unwrap();

        let e2 = "B".to_string();
        let n2 = NodeId::new();

        view.ring_add(e2.clone(), n2.clone()).unwrap();

        view.ring_delete(&e2).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 1);

        let err = view.ring_add(e2.clone(), n2.clone()).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::UuidAlreadySeen);

        view.ring_add(e2.clone(), NodeId::new()).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 2);
    }

    #[test]
    fn node_config_change() {
        let mut view = View::new(K);

        let num = 1000;
        let mut set = HashSet::new();

        for i in 0..num {
            let n = i.to_string();
            view.ring_add(n, NodeId::new()).unwrap();
            let id = view.get_config().config_id();
            set.insert(id);
        }

        assert_eq!(set.len(), num);
    }

    #[test]
    fn node_config_change_accross_views() {
        let mut view1 = View::new(K);
        let mut view2 = View::new(K);

        let num = 1000;
        let nodes = (0..num)
            .into_iter()
            .map(|i| (i.to_string(), NodeId::new()))
            .collect::<Vec<_>>();

        let mut set1 = Vec::new();
        for (e, i) in &nodes {
            view1.ring_add(e.clone(), i.clone()).unwrap();
            let id = view1.get_config().config_id();
            set1.push(id);
        }

        let mut set2 = Vec::new();
        for (e, i) in nodes.iter().rev() {
            view2.ring_add(e.clone(), i.clone()).unwrap();
            let id = view2.get_config().config_id();
            set2.push(id);
        }

        for (id1, id2) in set1.iter().zip(&set2).take(set1.len() - 1) {
            assert_ne!(id1, id2);
        }

        assert_eq!(set1.pop().unwrap(), set2.pop().unwrap());
    }
}
