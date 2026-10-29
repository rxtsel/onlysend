use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

/// Coalesces work for one account without blocking unrelated accounts.
#[derive(Default)]
pub(crate) struct AccountLocks {
    locks: Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
}

impl AccountLocks {
    pub(crate) fn get(&self, account_id: &str) -> Result<Arc<tokio::sync::Mutex<()>>, String> {
        let mut locks = self.locks.lock().map_err(|_| "Account refresh lock poisoned")?;
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(account_id).and_then(Weak::upgrade) {
            return Ok(lock);
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(account_id.to_owned(), Arc::downgrade(&lock));
        Ok(lock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_queues_are_reclaimed() {
        let pool = AccountLocks::default();
        drop(pool.get("a").unwrap());
        let _b = pool.get("b").unwrap();
        let locks = pool.locks.lock().unwrap();
        assert!(!locks.contains_key("a"));
        assert!(locks.contains_key("b"));
    }
}
