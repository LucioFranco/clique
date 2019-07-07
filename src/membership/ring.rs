use std::{cmp::Ordering, collections::BTreeSet, hash::Hasher};
use twox_hash::XxHash64;

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

impl<T> Ring<T> {
    pub fn new(seed: u64) -> Self
    where
        T: AsRef<[u8]>,
    {
        Self {
            seed,
            set: BTreeSet::new(),
        }
    }

    pub fn contains(&self, key: T) -> bool
    where
        T: AsRef<[u8]>,
    {
        // TODO(lucio): Remove this clone since we only need a `AsRef<[u8]>` to
        // do the `Ord` impl.
        let seeded_key = SeededKey::new(key, self.seed);
        self.set.contains(&seeded_key)
    }

    pub fn insert(&mut self, key: T) -> bool
    where
        T: AsRef<[u8]>,
    {
        let seeded_key = SeededKey::new(key, self.seed);
        self.set.insert(seeded_key)
    }

    pub fn remove(&mut self, key: T) -> bool
    where
        T: AsRef<[u8]>,
    {
        // TODO(lucio): Remove this clone since we only need a `AsRef<[u8]>` to
        // do the `Ord` impl.
        let seeded_key = SeededKey::new(key, self.seed);
        self.set.remove(&seeded_key)
    }

    // pub fn get_full(&self, key: T) ->

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
