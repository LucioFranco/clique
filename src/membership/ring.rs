use std::{cmp::Ordering, collections::BTreeSet, hash::Hasher, ops::Bound};
use twox_hash::XxHash64;

/// A tree that sorts based on a seeded hash value.
///
/// This is responsible for storing the membership in the `View` module. It allows
/// `O(log n)` access to the all the queries. The order is decided via the seeded hash
/// of the `AsRef<[u8]>`. This is used to create pseudo sorted trees.
#[derive(Debug, Clone)]
pub struct Ring<T> {
    seed: u64,
    set: BTreeSet<SeededKey<T>>,
}

#[derive(Debug, Clone)]
struct SeededKey<T> {
    key: T,
    seed: u64,
}

impl<T: AsRef<[u8]>> Ring<T> {
    /// Create a new `Ring` from the seed.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            set: BTreeSet::new(),
        }
    }

    /// Check if the value exists already within the ring.
    pub fn contains(&self, key: T) -> bool {
        // TODO(lucio): Remove this clone since we only need a `AsRef<[u8]>` to
        // do the `Ord` impl.
        let seeded_key = SeededKey::new(key, self.seed);
        self.set.contains(&seeded_key)
    }

    /// Insert a value into the ring.
    ///
    /// If the set did not have this value present, `true` is returned.
    /// If the set did have this value present, `false` is returned, and the entry is not updated.
    pub fn insert(&mut self, key: T) -> bool {
        let seeded_key = SeededKey::new(key, self.seed);
        self.set.insert(seeded_key)
    }

    /// Returns a reference to the value in the ring. `None` if it doesn't exist.
    pub fn get(&mut self, key: T) -> Option<&T> {
        let seeded_key = SeededKey::new(key, self.seed);
        self.set.get(&seeded_key).map(|v| &v.key)
    }

    /// Remove a value from the ring.
    ///
    /// Returns `true` if the value was removed, `false` if the value didnt exist.
    pub fn remove(&mut self, key: T) -> bool {
        // TODO(lucio): Remove this clone since we only need a `AsRef<[u8]>` to
        // do the `Ord` impl.
        let seeded_key = SeededKey::new(key, self.seed);
        self.set.remove(&seeded_key)
    }

    /// Return the value that is the next value beyond the provided value.
    ///
    /// Returns `None` if the value is the last item in the ring.
    pub fn higher(&self, key: T) -> Option<&T> {
        let seeded_key = SeededKey::new(key, self.seed);
        let range = (Bound::Excluded(seeded_key), Bound::Unbounded);
        self.set.range(range).next().map(|v| &v.key)
    }

    /// Return the value that is behind the provided value.
    ///
    /// Returns `None` if the value is the first one in the ring.
    pub fn lower(&self, key: T) -> Option<&T> {
        let seeded_key = SeededKey::new(key, self.seed);
        let range = (Bound::Unbounded, Bound::Excluded(seeded_key));
        self.set.range(range).next().map(|v| &v.key)
    }

    /// Get the first value in the ring.
    pub fn first(&self) -> Option<&T> {
        self.set.iter().next().map(|v| &v.key)
    }

    /// Get the last value in the ring.
    ///
    /// Returns `None` if there are no
    pub fn last(&self) -> Option<&T> {
        self.set.iter().next_back().map(|v| &v.key)
    }

    /// Check if the ring is empty or not.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the amount of values in the ring.
    pub fn len(&self) -> usize {
        self.set.len()
    }
}

impl<T: AsRef<[u8]>> SeededKey<T> {
    pub fn new(key: T, seed: u64) -> Self {
        Self { key, seed }
    }

    fn hash(&self) -> u64 {
        let mut hash = XxHash64::with_seed(self.seed);
        hash.write(self.key.as_ref());
        hash.finish()
    }
}

impl<T: AsRef<[u8]>> PartialOrd for SeededKey<T> {
    fn partial_cmp(&self, other: &SeededKey<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: AsRef<[u8]>> Ord for SeededKey<T> {
    fn cmp(&self, other: &SeededKey<T>) -> Ordering {
        self.hash().cmp(&other.hash())
    }
}

impl<T: AsRef<[u8]>> PartialEq for SeededKey<T> {
    fn eq(&self, other: &SeededKey<T>) -> bool {
        self.hash() == other.hash()
    }
}

impl<T: AsRef<[u8]>> Eq for SeededKey<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert() {
        let mut ring = Ring::new(0);
        assert!(ring.insert("test"));
        assert!(ring.contains("test"));
    }

}
