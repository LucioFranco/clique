use crate::{common::Endpoint, transport::Metadata};

use std::collections::HashMap;

pub struct MetadataManager {
    role_map: HashMap<Endpoint, Metadata>
}

impl MetadataManager {
    pub fn get(&mut self, key: &Endpoint) -> Option<&Metadata> {
        self.role_map.get(key).or_else_with(Metadata::default)
    }

    pub fn add_metadata(&mut self, roles: HashMap<Endpoint, Metadata>) {

        roles.drain().for_each(|(key, val)| {
            self.role_map.entry(&key).or_insert(val);
        });
    }

    pub fn remove_node(&mut self, node: &Endpoint) {
        self.role_map.remove(&node);
    }

}
