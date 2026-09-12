use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use super::{CanonicalTypeIdentity, StableInterfaceDeclarationId};

pub(super) static NEXT_SUBSTITUTION_STORE_OWNER: AtomicU32 = AtomicU32::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SubstitutionId(u64);

impl SubstitutionId {
    pub(super) fn new(owner: u32, index: u32) -> Self {
        Self((u64::from(owner) << 32) | u64::from(index))
    }
}

#[derive(Debug)]
pub(super) struct SubstitutionEntry {
    pub(super) declaration: StableInterfaceDeclarationId,
    pub(super) arguments: Arc<[CanonicalTypeIdentity]>,
    pub(super) id: SubstitutionId,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct SubstitutionStoreStats {
    pub(crate) requests: u64,
    pub(crate) hits: u64,
    pub(crate) unique: u64,
    pub(crate) input_arguments: u64,
    pub(crate) stored_arguments: u64,
    pub(crate) argument_storage_avoided: u64,
}

#[derive(Debug)]
pub(crate) struct SubstitutionStore {
    pub(super) owner: u32,
    pub(super) next_index: AtomicU32,
    pub(super) requests: AtomicU64,
    pub(super) hits: AtomicU64,
    pub(super) input_arguments: AtomicU64,
    pub(super) stored_arguments: AtomicU64,
    pub(super) argument_storage_avoided: AtomicU64,
    pub(super) entries: Mutex<HashMap<u64, Vec<SubstitutionEntry>>>,
}

impl SubstitutionStore {
    pub(super) fn new() -> Arc<Self> {
        let owner = NEXT_SUBSTITUTION_STORE_OWNER.fetch_add(1, Ordering::Relaxed);
        assert_ne!(owner, 0, "substitution-store owner space exhausted");
        Arc::new(Self {
            owner,
            next_index: AtomicU32::new(1),
            requests: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            input_arguments: AtomicU64::new(0),
            stored_arguments: AtomicU64::new(0),
            argument_storage_avoided: AtomicU64::new(0),
            entries: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn intern(
        &self,
        declaration: StableInterfaceDeclarationId,
        arguments: Vec<CanonicalTypeIdentity>,
    ) -> SubstitutionId {
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.input_arguments
            .fetch_add(arguments.len() as u64, Ordering::Relaxed);
        let argument_count = arguments.len() as u64;
        let mut hasher = surge_ts_types::fx::FxHasher::default();
        declaration.hash(&mut hasher);
        arguments.hash(&mut hasher);
        let key = hasher.finish();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let bucket = entries.entry(key).or_default();
        for entry in bucket.iter() {
            if entry.declaration == declaration && entry.arguments.as_ref() == arguments.as_slice()
            {
                self.hits.fetch_add(1, Ordering::Relaxed);
                self.argument_storage_avoided
                    .fetch_add(argument_count, Ordering::Relaxed);
                return entry.id;
            }
        }
        let id = SubstitutionId::new(self.owner, self.next_index.fetch_add(1, Ordering::Relaxed));
        self.stored_arguments
            .fetch_add(argument_count, Ordering::Relaxed);
        bucket.push(SubstitutionEntry {
            declaration,
            arguments: Arc::from(arguments),
            id,
        });
        id
    }

    pub(crate) fn stats(&self) -> SubstitutionStoreStats {
        SubstitutionStoreStats {
            requests: self.requests.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            unique: self
                .entries
                .lock()
                .map(|entries| entries.values().map(Vec::len).sum::<usize>() as u64)
                .unwrap_or_default(),
            input_arguments: self.input_arguments.load(Ordering::Relaxed),
            stored_arguments: self.stored_arguments.load(Ordering::Relaxed),
            argument_storage_avoided: self.argument_storage_avoided.load(Ordering::Relaxed),
        }
    }

    pub(super) fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}
