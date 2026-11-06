//! A store in memory, for tests: nothing is written anywhere, and everything is gone when it is
//! dropped. Never shipped (feature `testing`).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Mutex, PoisonError};

use crate::{Deleted, Secret, SecretError, SecretName, SecretStore};

/// Secrets in memory. [`MemoryStore::unavailable`] stands for a system without a secure store.
pub struct MemoryStore {
    entries: Mutex<BTreeMap<SecretName, Secret>>,
    available: bool,
}

impl MemoryStore {
    /// An empty store.
    pub fn new() -> MemoryStore {
        MemoryStore { entries: Mutex::new(BTreeMap::new()), available: true }
    }

    /// A store that answers every call with [`SecretError::NoSecureStore`], as a system without a
    /// secure store does.
    pub fn unavailable() -> MemoryStore {
        MemoryStore { entries: Mutex::new(BTreeMap::new()), available: false }
    }

    fn entries(&self) -> Result<std::sync::MutexGuard<'_, BTreeMap<SecretName, Secret>>, SecretError> {
        if !self.available {
            return Err(SecretError::NoSecureStore { reason: "the test store stands for a system without one".to_owned() });
        }
        Ok(self.entries.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl Default for MemoryStore {
    fn default() -> MemoryStore {
        MemoryStore::new()
    }
}

impl fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<String> = self.entries.lock().unwrap_or_else(PoisonError::into_inner).keys().map(|n| n.as_str().to_owned()).collect();
        f.debug_struct("MemoryStore").field("names", &names).field("available", &self.available).finish()
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, name: &SecretName) -> Result<Secret, SecretError> {
        self.entries()?.get(name).map(Secret::duplicate).ok_or_else(|| SecretError::NotFound { name: name.as_str().to_owned() })
    }

    fn set(&self, name: &SecretName, secret: &Secret) -> Result<(), SecretError> {
        self.entries()?.insert(name.clone(), secret.duplicate());
        Ok(())
    }

    fn delete(&self, name: &SecretName) -> Result<Deleted, SecretError> {
        Ok(if self.entries()?.remove(name).is_some() { Deleted::Removed } else { Deleted::WasAbsent })
    }
}
