use crate::{
    common::{ConfigId, Endpoint, NodeId},
    error::{Error, Result},
};
use indexmap::IndexSet;
use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashSet},
    hash::Hasher,
    ops::Bound,
};
use twox_hash::XxHash64;

type Ring = BTreeSet<SeededEndpoint>;

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

#[derive(Debug, Clone, Eq)]
pub struct SeededEndpoint {
    seed: u64,
    endpoint: Endpoint,
}

impl View {
    pub fn new(k: i32) -> Self {
        assert!(k > 0);

        let mut rings = Vec::with_capacity(k as usize);

        for _ in 0..k {
            rings.push(Ring::new());
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

        if self.rings[0].contains(&node) {
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
        if !self.rings[0].contains(node) {
            return Err(Error::new_node_not_in_ring());
        }

        for ring in &mut self.rings {
            ring.remove(node);
        }

        self.should_update_configuration_id = true;

        Ok(())
    }

    pub fn get_observers(&self, node: &Endpoint) -> Result<Vec<Endpoint>> {
        let mut observers = Vec::new();

        for ring in &self.rings {
            let successor = if let Some(s) = self.get_successor(ring, node)? {
                s
            } else {
                return Ok(Vec::new());
            };

            observers.push(successor);
        }

        Ok(observers)
    }

    pub fn get_ring(&self, k: i32) -> Option<&Ring> {
        self.rings.get(k as usize)
    }

    fn get_successor(&self, ring: &Ring, node: &Endpoint) -> Result<Option<Endpoint>> {
        // if ring.len() <= 1 {
        //     return Ok(None);
        // }

        // let (i, _) = ring.get_full(node).ok_or(Error::new_node_not_in_ring())?;

        // let succ = if let Some(succ) = ring.get_index(i + 1) {
        //     Some(succ)
        // } else {
        //     ring.get_index(0)
        // };

        // Ok(succ.map(|s| s.clone()))
        unimplemented!()
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

impl SeededEndpoint {
    pub fn new(endpoint: Endpoint, seed: u64) -> Self {
        Self { endpoint, seed }
    }

    fn hash(&self) -> u64 {
        let mut hash = XxHash64::with_seed(self.seed);
        hash.write(self.endpoint.as_bytes());
        hash.finish()
    }
}

impl PartialOrd for SeededEndpoint {
    fn partial_cmp(&self, other: &SeededEndpoint) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SeededEndpoint {
    fn cmp(&self, other: &SeededEndpoint) -> Ordering {
        self.hash().cmp(&other.hash())
    }
}

impl PartialEq for SeededEndpoint {
    fn eq(&self, other: &SeededEndpoint) -> bool {
        self.hash() == other.hash()
    }
}

impl From<SeededEndpoint> for Endpoint {
    fn from(t: SeededEndpoint) -> Endpoint {
        t.endpoint
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::NodeId;
    use crate::error::ErrorKind;

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
    #[ignore]
    fn observers() {
        let mut view = View::new(3);

        view.ring_add("A".into(), NodeId::new()).unwrap();

        // view.ring_add("C".into(), NodeId::new()).unwrap();
        // view.ring_add("D".into(), NodeId::new()).unwrap();
        // view.ring_add("E".into(), NodeId::new()).unwrap();

        assert_eq!(view.get_observers(&"A".to_string()).unwrap().len(), 0);

        view.ring_add("B".into(), NodeId::new()).unwrap();

        let obs = view.get_observers(&"A".to_string()).unwrap();
        assert_eq!(obs, vec!["B".to_string()]);
    }
}
