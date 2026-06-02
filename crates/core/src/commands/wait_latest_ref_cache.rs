use crate::{error::AppError, refs::RefMap, refs_store::RefStore};
use std::time::{Duration, Instant};

pub(crate) struct LatestRefCache<'a> {
    store: &'a RefStore,
    pub(crate) snapshot_id: Option<String>,
    refmap: RefMap,
    pub(crate) last_refresh: Instant,
}

impl<'a> LatestRefCache<'a> {
    pub(crate) fn new(store: &'a RefStore) -> Result<Self, AppError> {
        let snapshot_id = store.latest_snapshot_id();
        let refmap = if let Some(id) = snapshot_id.as_deref() {
            store.load_snapshot(id)?
        } else {
            store.load_latest()?
        };
        Ok(Self {
            store,
            snapshot_id,
            refmap,
            last_refresh: Instant::now() - Duration::from_millis(500),
        })
    }

    pub(crate) fn entry(&self, ref_id: &str) -> Option<crate::refs::RefEntry> {
        self.refmap.get(ref_id).cloned()
    }

    pub(crate) fn refresh_if_due(&mut self) {
        if self.last_refresh.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last_refresh = Instant::now();
        if let Some(snapshot_id) = self.store.latest_snapshot_id() {
            if self.snapshot_id.as_deref() == Some(snapshot_id.as_str()) {
                return;
            }
            if let Ok(refmap) = self.store.load_snapshot(&snapshot_id) {
                self.snapshot_id = Some(snapshot_id);
                self.refmap = refmap;
            }
        } else if let Ok(refmap) = self.store.load_latest() {
            self.refmap = refmap;
            self.snapshot_id = self.store.latest_snapshot_id();
        }
    }
}

#[cfg(test)]
#[path = "wait_latest_ref_cache_tests.rs"]
mod tests;
