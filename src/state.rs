use std::sync::Arc;

use crate::cache::AppCache;
use crate::config::Settings;
use crate::db::Pool;
use crate::geo::GeoIpLookup;

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
    pub cache: AppCache,
    pub settings: Arc<Settings>,
    pub geo: Arc<GeoIpLookup>,
}

impl AppState {
    pub fn new(pool: Pool, cache: AppCache, settings: Settings, geo: GeoIpLookup) -> Self {
        Self {
            pool,
            cache,
            settings: Arc::new(settings),
            geo: Arc::new(geo),
        }
    }
}
