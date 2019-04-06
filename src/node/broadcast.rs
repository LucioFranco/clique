use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;

use cuckoofilter::CuckooFilter;
use uuid::Uuid;

// An empty struct for now.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Message {
    Joined,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Broadcast {
    uuid: Uuid,
    message: Message,
}

impl Broadcast {
    pub fn new(message: Message) -> Self {
        Self {
            message,
            uuid: Uuid::new_v4(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitedBroadcast {
    pub transmits: u64,
    id: u64,
    pub broadcast: Broadcast,
}

// Order by number of transmits followed by decreasing id as id increases
// for every generation.
//
// - [transmits=0, ..., transmits=inf]
// - [transmits=0:id=999, ..., transmits=0:id=1, ...]
impl Ord for LimitedBroadcast {
    fn cmp(&self, other: &LimitedBroadcast) -> Ordering {
        if self.transmits < other.transmits {
            return Ordering::Less;
        } else if self.transmits > other.transmits {
            return Ordering::Greater;
        }

        self.id.cmp(&other.id)
    }
}

impl PartialOrd for LimitedBroadcast {
    fn partial_cmp(&self, other: &LimitedBroadcast) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl LimitedBroadcast {
    pub fn invalidates(&self, other: &LimitedBroadcast) -> bool {
        unimplemented!()
    }
}

pub struct TransmitQueue {
    set: BTreeSet<LimitedBroadcast>,
    filter: CuckooFilter<DefaultHasher>,
    gen: u64,
}

impl TransmitQueue {
    pub fn new() -> Self {
        Self {
            filter: CuckooFilter::new(),
            set: BTreeSet::new(),
            gen: 0,
        }
    }

    pub fn enqueue(&mut self, broadcast: Broadcast) {
        let uuid = broadcast.uuid;

        self.gen = self.gen.wrapping_add(1);

        let limited_broadcast = LimitedBroadcast {
            broadcast,
            transmits: 0,
            id: self.gen,
        };

        if self.filter.contains(&uuid) {
            let remove: Vec<LimitedBroadcast> = self
                .set
                .iter()
                .filter(|&lb| lb.broadcast.uuid == uuid)
                .map(|b| b.clone())
                .collect();

            for item in &remove {
                self.set.remove(&item);
            }
        }

        self.filter.add(&limited_broadcast.broadcast.uuid);
        self.set.insert(limited_broadcast);
    }

    pub fn get_broadcasts(&mut self) -> Vec<&LimitedBroadcast> {
        self.set.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidate_exisitng() {
        let broadcast = Broadcast::new(Message::Joined);
        let clone = broadcast.clone();

        let mut queue = TransmitQueue::new();

        queue.enqueue(broadcast);

        let broadcasts = queue.get_broadcasts();

        assert_eq!(broadcasts.len(), 1);

        queue.enqueue(clone);

        let broadcasts = queue.get_broadcasts();

        assert_eq!(broadcasts.len(), 1);
    }
}
