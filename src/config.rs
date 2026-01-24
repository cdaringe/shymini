use config::{Config, Environment};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    pub database_url: Option<String>,
    pub database_path: Option<String>,

    pub maxmind_city_db: Option<String>,
    pub maxmind_asn_db: Option<String>,

    #[serde(default)]
    pub block_all_ips: bool,

    #[serde(default)]
    pub aggressive_hash_salting: bool,

    #[serde(default = "default_heartbeat_frequency")]
    pub script_heartbeat_frequency_ms: u64,

    #[serde(default = "default_cache_max_entries")]
    pub cache_max_entries: u64,

    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_secs: u64,

    #[serde(default = "default_session_memory_timeout")]
    pub session_memory_timeout_secs: u64,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_heartbeat_frequency() -> u64 {
    5000
}

fn default_cache_max_entries() -> u64 {
    10000
}

fn default_cache_ttl() -> u64 {
    3600
}

fn default_session_memory_timeout() -> u64 {
    3600 // 1 hour
}

impl Settings {
    pub fn new() -> Result<Self, config::ConfigError> {
        let _ = dotenvy::dotenv();

        let config = Config::builder()
            .add_source(
                Environment::with_prefix("SHYMINI")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?;

        config.try_deserialize()
    }

    pub fn active_user_timeout_ms(&self) -> u64 {
        self.script_heartbeat_frequency_ms * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings() -> Settings {
        Settings {
            host: "127.0.0.1".to_string(),
            port: 3000,
            database_url: None,
            database_path: Some("test.db".to_string()),
            maxmind_city_db: None,
            maxmind_asn_db: None,
            block_all_ips: false,
            aggressive_hash_salting: true,
            script_heartbeat_frequency_ms: 5000,
            cache_max_entries: 1000,
            cache_ttl_secs: 3600,
            session_memory_timeout_secs: 3600,
        }
    }

    #[test]
    fn test_default_host() {
        assert_eq!(default_host(), "0.0.0.0");
    }

    #[test]
    fn test_default_port() {
        assert_eq!(default_port(), 8080);
    }

    #[test]
    fn test_default_heartbeat_frequency() {
        assert_eq!(default_heartbeat_frequency(), 5000);
    }

    #[test]
    fn test_default_cache_max_entries() {
        assert_eq!(default_cache_max_entries(), 10000);
    }

    #[test]
    fn test_default_cache_ttl() {
        assert_eq!(default_cache_ttl(), 3600);
    }

    #[test]
    fn test_default_session_memory_timeout() {
        assert_eq!(default_session_memory_timeout(), 3600);
    }

    #[test]
    fn test_active_user_timeout_ms() {
        let settings = test_settings();
        assert_eq!(settings.active_user_timeout_ms(), 10000); // 5000 * 2
    }

    #[test]
    fn test_settings_fields() {
        let settings = test_settings();
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, 3000);
        assert!(settings.database_path.is_some());
        assert!(settings.database_url.is_none());
        assert!(!settings.block_all_ips);
        assert!(settings.aggressive_hash_salting);
    }
}
