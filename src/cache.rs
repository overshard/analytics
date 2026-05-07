use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;

pub const TTL: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct DashboardCache {
    inner: Cache<String, Arc<serde_json::Value>>,
}

impl DashboardCache {
    pub fn new() -> Self {
        Self {
            inner: Cache::builder()
                .max_capacity(256)
                .time_to_live(TTL)
                .build(),
        }
    }

    pub async fn get(&self, key: &str) -> Option<Arc<serde_json::Value>> {
        self.inner.get(key).await
    }

    pub async fn insert(&self, key: String, value: Arc<serde_json::Value>) {
        self.inner.insert(key, value).await;
    }
}

impl Default for DashboardCache {
    fn default() -> Self {
        Self::new()
    }
}
