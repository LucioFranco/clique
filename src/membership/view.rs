use crate::{
    common::{ConfigId, Endpoint, NodeId},
    error::{Error, Result},
};
use std::collections::HashSet;

type Ring = crate::membership::ring::Ring<Endpoint>;

#[derive(Debug, Clone)]
pub struct View {
    k: i32,
    rings: Vec<Ring>,
    seen: HashSet<NodeId>,
    current_config: ConfigId,
    should_update_configuration_id: bool,
    current_configuration: Configuration,
}

#[derive(Debug, Clone, Default)]
struct Configuration {
    node_ids: Vec<NodeId>,
    endpoints: Vec<Endpoint>,
}

impl View {
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
            current_config: -1,
            current_configuration: Configuration::default(),
            should_update_configuration_id: true,
        }
    }

    pub fn ring_add(&mut self, node: Endpoint, node_id: NodeId) -> Result<()> {
        if self.is_node_present(&node_id) {
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

    pub fn get_expected_observers(&self, node: &Endpoint) -> Result<Vec<Endpoint>> {
        Ok(self.get_predecessors(node))
    }

    pub fn get_ring(&self, k: i32) -> Option<&Ring> {
        self.rings.get(k as usize)
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

    fn is_node_present(&self, node_id: &NodeId) -> bool {
        self.seen.contains(node_id)
    }
}

impl Configuration {
    pub fn config_id(&self) -> ConfigId {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::NodeId;
    use crate::error::ErrorKind;

    const K: i32 = 10;

    #[test]
    fn ring_add() {
        let mut view = View::new(5);
        view.ring_add("A".into(), NodeId::new()).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 1);
    }

    #[test]
    fn ring_delete() {
        let mut view = View::new(5);
        let node = "A".to_string();
        view.ring_add(node.clone(), NodeId::new()).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 1);

        view.ring_delete(&node).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 0);
    }

    #[test]
    fn ring_re_addition() {
        let mut view = View::new(5);
        view.ring_add("A".into(), NodeId::new()).unwrap();
        assert_eq!(view.get_ring(0).unwrap().len(), 1);

        let err = view.ring_add("A".into(), NodeId::new()).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::NodeAlreadyInRing);
    }

    #[test]
    fn ring_delete_invalid() {
        let mut view = View::new(5);

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

        let exp_obs = view.get_expected_observers(&"".to_string()).unwrap();
        assert_eq!(exp_obs.len(), K as usize);
        let mut exp_obs_dedup = exp_obs.clone();
        exp_obs_dedup.sort();
        exp_obs_dedup.dedup();
        assert_eq!(exp_obs_dedup.len(), 1);
        assert_eq!(exp_obs[0], "A".to_string());
    }
}
