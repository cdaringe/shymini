use moka::future::Cache;
use std::time::Duration;

use crate::config::Settings;
use crate::domain::{HitId, ServiceId, SessionId};

#[derive(Clone)]
pub struct AppCache {
    /// Cache for service origins (ServiceId -> origins string)
    pub service_origins: Cache<ServiceId, String>,

    /// Cache for script inject content (ServiceId -> script)
    pub script_inject: Cache<ServiceId, String>,

    /// Cache for session associations (hash -> SessionId)
    pub session_associations: Cache<String, SessionId>,

    /// Cache for hit idempotency (idempotency key -> HitId)
    pub hit_idempotency: Cache<String, HitId>,
}

impl AppCache {
    pub fn new(settings: &Settings) -> Self {
        let cache_ttl = Duration::from_secs(settings.cache_ttl_secs);
        let session_ttl = Duration::from_secs(settings.session_memory_timeout_secs);
        let max_entries = settings.cache_max_entries;

        Self {
            service_origins: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(cache_ttl)
                .build(),

            script_inject: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(cache_ttl)
                .build(),

            session_associations: Cache::builder()
                .max_capacity(max_entries * 10)
                .time_to_live(session_ttl)
                .build(),

            hit_idempotency: Cache::builder()
                .max_capacity(max_entries * 100)
                .time_to_live(session_ttl)
                .build(),
        }
    }

    /// Get or insert service origins
    pub async fn get_or_insert_origins<F, Fut>(&self, service_id: ServiceId, f: F) -> Option<String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Option<String>>,
    {
        if let Some(origins) = self.service_origins.get(&service_id).await {
            return Some(origins);
        }

        if let Some(origins) = f().await {
            self.service_origins
                .insert(service_id, origins.clone())
                .await;
            Some(origins)
        } else {
            None
        }
    }

    /// Get or insert script inject
    pub async fn get_or_insert_script_inject<F, Fut>(
        &self,
        service_id: ServiceId,
        f: F,
    ) -> Option<String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Option<String>>,
    {
        if let Some(script) = self.script_inject.get(&service_id).await {
            return Some(script);
        }

        if let Some(script) = f().await {
            self.script_inject.insert(service_id, script.clone()).await;
            Some(script)
        } else {
            None
        }
    }

    /// Get session from association cache
    pub async fn get_session_association(&self, hash: &str) -> Option<SessionId> {
        self.session_associations.get(hash).await
    }

    /// Set session association (and touch TTL if exists)
    pub async fn set_session_association(&self, hash: String, session_id: SessionId) {
        self.session_associations.insert(hash, session_id).await;
    }

    /// Touch session association TTL (re-insert to reset TTL)
    pub async fn touch_session_association(&self, hash: &str) {
        if let Some(session_id) = self.session_associations.get(hash).await {
            // Re-insert to reset TTL
            self.session_associations
                .insert(hash.to_string(), session_id)
                .await;
        }
    }

    /// Get hit from idempotency cache
    pub async fn get_hit_idempotency(&self, key: &str) -> Option<HitId> {
        self.hit_idempotency.get(key).await
    }

    /// Set hit idempotency
    pub async fn set_hit_idempotency(&self, key: String, hit_id: HitId) {
        self.hit_idempotency.insert(key, hit_id).await;
    }

    /// Touch hit idempotency TTL
    pub async fn touch_hit_idempotency(&self, key: &str) {
        if let Some(hit_id) = self.hit_idempotency.get(key).await {
            self.hit_idempotency.insert(key.to_string(), hit_id).await;
        }
    }

    /// Invalidate service-related caches
    pub async fn invalidate_service(&self, service_id: ServiceId) {
        self.service_origins.invalidate(&service_id).await;
        self.script_inject.invalidate(&service_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_settings() -> Settings {
        Settings {
            host: "0.0.0.0".to_string(),
            port: 8080,
            database_url: None,
            database_path: None,
            maxmind_city_db: None,
            maxmind_asn_db: None,
            block_all_ips: false,
            aggressive_hash_salting: false,
            script_heartbeat_frequency_ms: 5000,
            cache_max_entries: 100,
            cache_ttl_secs: 60,
            session_memory_timeout_secs: 30,
        }
    }

    #[tokio::test]
    async fn test_cache_creation() {
        let settings = test_settings();
        let cache = AppCache::new(&settings);
        // Caches should be empty initially
        let service_id = ServiceId::from_uuid(Uuid::new_v4());
        assert!(cache.service_origins.get(&service_id).await.is_none());
    }

    #[tokio::test]
    async fn test_session_association_cache() {
        let settings = test_settings();
        let cache = AppCache::new(&settings);

        let hash = "test_hash_12345".to_string();
        let session_id = SessionId::from_uuid(Uuid::new_v4());

        // Initially empty
        assert!(cache.get_session_association(&hash).await.is_none());

        // Set association
        cache
            .set_session_association(hash.clone(), session_id)
            .await;

        // Should be retrievable
        let retrieved = cache.get_session_association(&hash).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), session_id);
    }

    #[tokio::test]
    async fn test_hit_idempotency_cache() {
        let settings = test_settings();
        let cache = AppCache::new(&settings);

        let key = "idempotency_key_123".to_string();
        let hit_id = HitId(42);

        // Initially empty
        assert!(cache.get_hit_idempotency(&key).await.is_none());

        // Set idempotency
        cache.set_hit_idempotency(key.clone(), hit_id).await;

        // Should be retrievable
        let retrieved = cache.get_hit_idempotency(&key).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().0, 42);
    }

    #[tokio::test]
    async fn test_touch_session_association() {
        let settings = test_settings();
        let cache = AppCache::new(&settings);

        let hash = "test_hash_touch".to_string();
        let session_id = SessionId::from_uuid(Uuid::new_v4());

        cache
            .set_session_association(hash.clone(), session_id)
            .await;

        // Touch should not error
        cache.touch_session_association(&hash).await;

        // Should still be retrievable
        assert!(cache.get_session_association(&hash).await.is_some());
    }

    #[tokio::test]
    async fn test_touch_hit_idempotency() {
        let settings = test_settings();
        let cache = AppCache::new(&settings);

        let key = "idempotency_touch".to_string();
        let hit_id = HitId(100);

        cache.set_hit_idempotency(key.clone(), hit_id).await;

        // Touch should not error
        cache.touch_hit_idempotency(&key).await;

        // Should still be retrievable
        assert!(cache.get_hit_idempotency(&key).await.is_some());
    }

    #[tokio::test]
    async fn test_get_or_insert_origins() {
        let settings = test_settings();
        let cache = AppCache::new(&settings);

        let service_id = ServiceId::from_uuid(Uuid::new_v4());

        // First call should invoke the closure
        let origins = cache
            .get_or_insert_origins(service_id, || async {
                Some("https://example.com".to_string())
            })
            .await;

        assert_eq!(origins, Some("https://example.com".to_string()));

        // Second call should return cached value without invoking closure
        let origins2 = cache
            .get_or_insert_origins(service_id, || async {
                Some("https://other.com".to_string())
            })
            .await;

        // Should still be the original value
        assert_eq!(origins2, Some("https://example.com".to_string()));
    }

    #[tokio::test]
    async fn test_get_or_insert_script_inject() {
        let settings = test_settings();
        let cache = AppCache::new(&settings);

        let service_id = ServiceId::from_uuid(Uuid::new_v4());

        let script = cache
            .get_or_insert_script_inject(service_id, || async {
                Some("console.log('test');".to_string())
            })
            .await;

        assert_eq!(script, Some("console.log('test');".to_string()));
    }

    #[tokio::test]
    async fn test_invalidate_service() {
        let settings = test_settings();
        let cache = AppCache::new(&settings);

        let service_id = ServiceId::from_uuid(Uuid::new_v4());

        // Populate caches
        cache
            .service_origins
            .insert(service_id, "https://example.com".to_string())
            .await;
        cache
            .script_inject
            .insert(service_id, "test script".to_string())
            .await;

        // Verify they exist
        assert!(cache.service_origins.get(&service_id).await.is_some());
        assert!(cache.script_inject.get(&service_id).await.is_some());

        // Invalidate
        cache.invalidate_service(service_id).await;

        // Should be gone
        assert!(cache.service_origins.get(&service_id).await.is_none());
        assert!(cache.script_inject.get(&service_id).await.is_none());
    }

    #[tokio::test]
    async fn test_get_or_insert_origins_returns_none() {
        let settings = test_settings();
        let cache = AppCache::new(&settings);

        let service_id = ServiceId::from_uuid(Uuid::new_v4());

        // Closure returns None
        let origins = cache
            .get_or_insert_origins(service_id, || async { None })
            .await;

        assert!(origins.is_none());
    }
}
