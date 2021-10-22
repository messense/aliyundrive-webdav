use std::path::Path;
use std::time::Duration;

use moka::future::{Cache as MokaCache, CacheBuilder};
use tracing::trace;

use crate::drive::AliyunFile;

#[derive(Clone)]
pub struct Cache {
    inner: MokaCache<String, Vec<AliyunFile>>,
}

impl Cache {
    pub fn new(max_capacity: usize, ttl: u64) -> Self {
        let inner = CacheBuilder::new(max_capacity)
            .time_to_live(Duration::from_secs(ttl))
            .build();
        Self { inner }
    }

    #[allow(clippy::ptr_arg)]
    pub fn get(&self, key: &String) -> Option<Vec<AliyunFile>> {
        trace!(key = %key, "cache: get");
        self.inner.get(key)
    }

    pub async fn insert(&self, key: String, value: Vec<AliyunFile>) {
        trace!(key = %key, "cache: insert");
        self.inner.insert(key, value).await;
    }

    pub async fn invalidate(&self, path: &Path) {
        let key = path.to_string_lossy().into_owned();
        trace!(path = %path.display(), key = %key, "cache: invalidate");
        self.inner.invalidate(&key).await;
    }

    pub async fn invalidate_parent(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            self.invalidate(parent).await;
        }
    }

    pub fn invalidate_all(&self) {
        trace!("cache: invalidate all");
        self.inner.invalidate_all();
    }
}
