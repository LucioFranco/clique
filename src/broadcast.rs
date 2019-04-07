use std::cmp::Ordering;
use std::collections::{hash_map::DefaultHasher, BTreeSet};
use std::ops::Bound;

use bincode::serialized_size;
use cuckoofilter::CuckooFilter;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// An empty struct for now.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Message {
    Joined,
    Alive,
    Leave,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq)]
pub struct LimitedBroadcast {
    transmits: u64,
    id: u64,
    broadcast: Broadcast,
}

// Order by number of transmits followed by decreasing id as id increases
// for every generation.
//
// - [transmits=0, ..., transmits=inf]
// - [transmits=0:id=999, ..., transmits=0:id=1, ...]
impl Ord for LimitedBroadcast {
    fn cmp(&self, other: &LimitedBroadcast) -> Ordering {
        dbg!((self.transmits, self.id, other.transmits, other.id));
        if self.transmits < other.transmits {
            dbg!("Returning less");
            return Ordering::Less;
        }

        if self.transmits > other.transmits {
            dbg!("Returning greater");
            return Ordering::Greater;
        }

        dbg!((self.id, other.id));
        if self.id < other.id {
            dbg!("Returning greater");
            return Ordering::Greater;
        } else if self.id > other.id {
            dbg!("Returning less");
            return Ordering::Less;
        }

        Ordering::Equal
    }
}

impl PartialEq for LimitedBroadcast {
    fn eq(&self, other: &LimitedBroadcast) -> bool {
        dbg!(self.transmits == other.transmits && self.id == other.id)
    }
}

impl PartialEq<Broadcast> for LimitedBroadcast {
    fn eq(&self, other: &Broadcast) -> bool {
        self.broadcast.uuid == other.uuid
    }
}

impl PartialOrd for LimitedBroadcast {
    fn partial_cmp(&self, other: &LimitedBroadcast) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl LimitedBroadcast {
    pub fn invalidates(&self, _other: &LimitedBroadcast) -> bool {
        unimplemented!()
    }

    fn broadcast(&self) -> &Broadcast {
        &self.broadcast
    }

    fn transmits(&self) -> u64 {
        self.transmits
    }

    fn gen(transmits: u64, id: u64, broadcast: Broadcast) -> Self {
        Self {
            transmits,
            id,
            broadcast,
        }
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

    fn add(&mut self, val: LimitedBroadcast) {
        self.filter.add(&val.broadcast.uuid);
        self.set.insert(val);
    }

    fn delete(&mut self, val: &LimitedBroadcast) {
        self.filter.delete(&val.broadcast.uuid);
        self.set.remove(&val);

        if self.set.len() == 0 {
            self.gen = 0;
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

    pub fn get_broadcasts(&mut self, limit: u64) -> Vec<LimitedBroadcast> {
        dbg!(&self.set);
        let int_max = std::u64::MAX;
        let mut used = 0;

        let mut reinsert = vec![];
        let mut broadcasts = vec![];

        let joined = Broadcast::new(Message::Joined);

        let min_item = LimitedBroadcast::gen(0, int_max, joined);
        let max_item = LimitedBroadcast::gen(int_max, int_max, joined);

        let min = self.set.iter().next().unwrap_or(&min_item).transmits;
        let max = self.set.iter().next_back().unwrap_or(&max_item).transmits;

        for transmits in min..max + 1 {
            let free = limit - dbg!(used);

            dbg!((transmits, free));

            if free <= 0 {
                break;
            }

            let start = dbg!(LimitedBroadcast::gen(transmits, 0, joined));
            // Ranges in Rust are Include(min)..Exclude(max), so we need to add by one to get tier
            // i
            let end = dbg!(LimitedBroadcast::gen(transmits + 1, 0, joined));

            let mut keep = None;

            for item in self.set.range(start..end) {
                dbg!(item);
                if serialized_size(item).unwrap() > free {
                    continue;
                }
                keep = Some(item.clone());
                break;
            }

            if keep.is_none() {
                continue;
            }

            let mut keep = keep.unwrap();

            used += match serialized_size(&keep.broadcast) {
                Ok(n) => dbg!(n),
                Err(e) => {
                    eprintln!("Error figuring out size: {}", e);
                    0
                }
            };

            broadcasts.push(keep.clone());

            self.delete(&keep);

            // TODO: Make limit depend on number of nodes
            if keep.transmits + 1 > 5 {
                // don't do anything since it's already been deleted.
            } else {
                keep.transmits += 1;
                reinsert.push(keep);
            }
        }

        for item in reinsert {
            self.add(item);
        }

        broadcasts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{
        self,
        distributions::{Distribution, Standard},
        Rng,
    };

    impl Distribution<Message> for Standard {
        fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Message {
            match rng.gen_range(0, 3) {
                0 => Message::Joined,
                1 => Message::Alive,
                _ => Message::Leave,
            }
        }
    }

    fn gen_broadcasts(n: usize) -> Vec<Broadcast> {
        (0..n + 1).map(|_| Broadcast::new(rand::random())).collect()
    }

    #[test]
    fn invalidate_existing() {
        let broadcasts = gen_broadcasts(1);
        let mut queue = TransmitQueue::new();

        queue.enqueue(broadcasts[0]);
        let packet = queue.get_broadcasts(1500);
        assert_eq!(packet.len(), 1);

        queue.enqueue(broadcasts[0].clone());
        let packet = queue.get_broadcasts(1500);
        assert_eq!(packet.len(), 1);
    }

    #[test]
    fn get_to_limit() {
        let broadcasts = gen_broadcasts(5);
        let mut queue = TransmitQueue::new();

        queue.enqueue(broadcasts[0]);
        queue.enqueue(broadcasts[1]);
        queue.enqueue(broadcasts[2]);
        queue.enqueue(broadcasts[3]);
        queue.enqueue(broadcasts[4]);

        eprintln!("---------------------------------------------");
        eprintln!("---------------------------------------------");
        eprintln!("---------------------------------------------");
        eprintln!("---------------------------------------------");

        let packet = queue.get_broadcasts(1500);
        assert_eq!(packet.len(), 5);
    }
}
