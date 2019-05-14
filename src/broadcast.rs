use std::cmp::Ordering;
use std::collections::{hash_map::DefaultHasher, BTreeSet};

use bincode::{serialized_size, Result};
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
        if self.transmits < other.transmits {
            return Ordering::Less;
        } else if self.transmits > other.transmits {
            return Ordering::Greater;
        } else if self.id < other.id {
            return Ordering::Greater;
        } else if self.id > other.id {
            return Ordering::Less;
        } else {
            // Never going to happen as id is monotonically increasing.
            Ordering::Equal
        }
    }
}

impl PartialEq for LimitedBroadcast {
    fn eq(&self, other: &LimitedBroadcast) -> bool {
        self.transmits == other.transmits && self.id == other.id
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

    fn gen(transmits: u64, id: u64, broadcast: Broadcast) -> LimitedBroadcast {
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

/// TransmitQueue is used to queue messages to broadcast to the cluster but limits the number of
/// transmits per message. It prioritizes messages with lower transmit counts and newer version of
/// the same message. This means that the items yielded by the queue will be ordered newest first.
impl TransmitQueue {
    pub fn new() -> Self {
        Self {
            filter: CuckooFilter::new(),
            set: BTreeSet::new(),
            gen: 0,
        }
    }

    /// Return the number of elements currently present in the queue.
    pub fn len(&self) -> usize {
        self.set.len()
    }

    /// Add the given `LimitedBroadcast` to the queue.
    fn add(&mut self, val: LimitedBroadcast) {
        self.filter.add(&val.broadcast.uuid);
        self.set.insert(val);
    }

    /// Remove the given `LimitedBroadcast` from the queue.
    fn delete(&mut self, val: &LimitedBroadcast) {
        self.filter.delete(&val.broadcast.uuid);
        self.set.remove(&val);

        if self.set.is_empty() {
            self.gen = 0;
        }
    }

    /// Enqueues a broadcast to be disseminated. If a Broadcast with the same UUID already exists,
    /// it will be removed and the new version will be inserted.
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

        self.add(limited_broadcast);
    }

    /// Returns a list of `Broadcast`s ordered by newest first. The number of values returned
    /// is limited by the `size_limit` parameter which limits the toal size of the resultant Vec.
    pub fn get_broadcasts(&mut self, size_limit: usize) -> Result<Vec<Broadcast>> {
        let int_max = std::u64::MAX;
        let mut used: i64 = 0;

        let mut reinsert = vec![];
        let mut broadcasts = vec![];

        // The size of an empty vec is 8 bytes. Every item added to the vec only increases the size
        // of the vec by the size of the item. Consider this in the calculation.
        used += serialized_size(&broadcasts)? as i64;

        let joined = Broadcast::new(Message::Joined);

        let min_item = LimitedBroadcast::gen(0, int_max, joined.clone());
        let max_item = LimitedBroadcast::gen(int_max, int_max, joined.clone());

        let min = self.set.iter().next().unwrap_or(&min_item).transmits;
        let max = self.set.iter().next_back().unwrap_or(&max_item).transmits;

        // Try to get all transmits within a given tier. We do this by getting the min and max
        // number of transmits in the queue, and iterating over each transmit tier. Each iteration
        // of this loop, we see if there are any broadcasts which fit the remaining size in the
        // given transmit tier and add it to the `broadcasts` vec. These items are added to the
        // prune list. If the item hasn't been transmitted `size_limit` (currently 5) times, it is
        // reinserted into the queue.
        for transmits in min..max + 1 {
            let mut free: i64 = size_limit as i64 - used;

            if free <= 0 {
                break;
            }

            let start = LimitedBroadcast::gen(transmits, int_max, joined.clone());
            // Ranges in Rust are Include(min)..Exclude(max), so we need to add by one to get tier
            // i
            let end = LimitedBroadcast::gen(transmits + 1, int_max, joined.clone());

            let mut prune = vec![];

            for item in self.set.range(start..end) {
                // Drop those broadcasts which cannot be serialized.
                let size = match serialized_size(&item.broadcast) {
                    Ok(n) => n as i64,
                    Err(e) => {
                        error!("Serialization error: {:?}", e);
                        prune.push(item.clone());
                        continue;
                    }
                };

                if size as i64 > free {
                    // Ignore this broadcast as it won't fit
                    continue;
                }

                // Update sizes
                used += size;
                free -= size;

                broadcasts.push(item.broadcast.clone());
                prune.push(item.clone());

                // TODO: make this parameter configurable
                if item.transmits + 1 <= 5 {
                    // We reinsert after the broadcasts vec if filled as if we reinsert into the
                    // queue here, the next iteration of the loop might consider the same item
                    // again.
                    reinsert.push(item.clone())
                }
            }

            for item in prune.iter_mut() {
                self.delete(item);
            }
        }

        for mut item in reinsert {
            item.transmits += 1;
            self.add(item);
        }

        Ok(broadcasts)
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
        (0..n).map(|_| Broadcast::new(rand::random())).collect()
    }

    #[test]
    fn invalidate_existing() {
        let broadcasts = gen_broadcasts(1);
        let mut queue = TransmitQueue::new();

        queue.enqueue(broadcasts[0].clone());
        let packet = queue.get_broadcasts(1500).unwrap();
        assert_eq!(packet.len(), 1);

        queue.enqueue(broadcasts[0].clone());
        let packet = queue.get_broadcasts(1500).unwrap();
        assert_eq!(packet.len(), 1);
    }

    #[test]
    fn get_to_limit() {
        let broadcasts = gen_broadcasts(5);
        let mut queue = TransmitQueue::new();

        broadcasts.into_iter().for_each(|b| queue.enqueue(b));

        let packets = queue.get_broadcasts(100).unwrap();
        assert_eq!(packets.len(), 3);
    }

    #[test]
    fn empty_queue() {
        let broadcasts = gen_broadcasts(5);
        let mut queue = TransmitQueue::new();

        broadcasts.into_iter().for_each(|b| queue.enqueue(b));
        let mut counter = 0;

        while queue.len() > 0 {
            let _packets = queue.get_broadcasts(100).unwrap();
            counter += 1;
        }

        // This is based on the hard coded limit of 5 transmits per broadcast. We have 5 items in
        // the queue, `get_broadcast` call gets 3 items of 1 tier, followed by 2 items of 1 tier
        // and 1 item of the next tier, and this goes on until all items have been transmitted
        // 5 times and takes 10 calls to empty the queue.
        assert_eq!(counter, 10);
    }

    #[test]
    fn correct_ordering() {
        let broadcasts = gen_broadcasts(5);
        let ids: Vec<Uuid> = broadcasts.iter().map(|b| b.uuid).collect();
        let mut queue = TransmitQueue::new();

        broadcasts.into_iter().for_each(|b| queue.enqueue(b));

        let mut queue_ids = vec![];

        while queue.len() > 0 {
            let packets = queue.get_broadcasts(100).unwrap();
            packets.iter().for_each(|p| queue_ids.push(p.uuid));
        }

        // 3 items per round, 10 rounds
        assert_eq!(queue_ids.len(), 30);

        ids.iter()
            .rev()
            .cycle()
            .zip(queue_ids.iter())
            .for_each(|(expected, result)| assert_eq!(expected, result));
    }
}
