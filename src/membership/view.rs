use crate::{ConfigId, Endpoint, Error, NodeId, Result};
use indexmap::IndexSet;
use std::{collections::HashSet, ops::Bound};

type Ring = IndexSet<Endpoint>;

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
        if ring.len() <= 1 {
            return Ok(None);
        }

        let (i, _) = ring.get_full(node).ok_or(Error::new_node_not_in_ring())?;

        let succ = if let Some(succ) = ring.get_index(i + 1) {
            Some(succ)
        } else {
            ring.get_index(0)
        };

        Ok(succ.map(|s| s.clone()))
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
    use crate::error::ErrorKind;
    use crate::NodeId;

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
