use crate::{common::Endpoint, transport::proto::Metadata};

use std::collections::HashMap;

pub struct MetadataManager {
    role_map: HashMap<Endpoint, Metadata>,
}

#[allow(dead_code)]
impl MetadataManager {
    pub fn new() -> Self {
        MetadataManager {
            role_map: HashMap::<Endpoint, Metadata>::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&Metadata> {
        // self.role_map.get(key).or(Some(Metadata::default()))
        self.role_map.get(key)
    }

    pub fn add_metadata(&mut self, mut roles: HashMap<Endpoint, Metadata>) {
        roles.drain().for_each(|(key, val)| {
            self.role_map.entry(key).or_insert(val);
        });
    }

    pub fn remove_node(&mut self, node: &str) {
        self.role_map.remove(node);
    }
}
