use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::types::{
    ChartData, CountedItem, DeviceType, HitId, ServiceId, ServiceStatus,
    SessionId, TrackingId, TrackerType,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: ServiceId,
    pub tracking_id: TrackingId,
    pub name: String,
    pub link: String,
    pub origins: String,
    pub status: ServiceStatus,
    pub respect_dnt: bool,
    pub ignore_robots: bool,
    pub collect_ips: bool,
    pub ignored_ips: String,
    pub hide_referrer_regex: String,
    pub script_inject: String,
    pub created_at: DateTime<Utc>,
}

impl Service {
    pub fn get_ignored_networks(&self) -> Vec<ipnetwork::IpNetwork> {
        if self.ignored_ips.trim().is_empty() {
            return Vec::new();
        }

        self.ignored_ips
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    }

    pub fn get_origins_list(&self) -> Vec<String> {
        if self.origins == "*" {
            return vec!["*".to_string()];
        }

        self.origins
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .collect()
    }

    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        if self.origins == "*" {
            return true;
        }

        let origins = self.get_origins_list();
        origins.contains(&origin.to_lowercase())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub service_id: ServiceId,
    pub identifier: String,
    pub start_time: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub user_agent: String,
    pub browser: String,
    pub device: String,
    pub device_type: DeviceType,
    pub os: String,
    pub ip: Option<String>,
    pub asn: String,
    pub country: String,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub time_zone: String,
    pub is_bounce: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub id: HitId,
    pub session_id: SessionId,
    pub service_id: ServiceId,
    pub initial: bool,
    pub start_time: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub heartbeats: i32,
    pub tracker: TrackerType,
    pub location: String,
    pub referrer: String,
    pub load_time: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct CreateService {
    pub name: String,
    pub link: String,
    pub origins: String,
    pub respect_dnt: bool,
    pub ignore_robots: bool,
    pub collect_ips: bool,
    pub ignored_ips: String,
    pub hide_referrer_regex: String,
    pub script_inject: String,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateService {
    pub name: Option<String>,
    pub link: Option<String>,
    pub origins: Option<String>,
    pub status: Option<ServiceStatus>,
    pub respect_dnt: Option<bool>,
    pub ignore_robots: Option<bool>,
    pub collect_ips: Option<bool>,
    pub ignored_ips: Option<String>,
    pub hide_referrer_regex: Option<String>,
    pub script_inject: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateSession {
    pub service_id: ServiceId,
    pub identifier: String,
    pub start_time: DateTime<Utc>,
    pub user_agent: String,
    pub browser: String,
    pub device: String,
    pub device_type: DeviceType,
    pub os: String,
    pub ip: Option<String>,
    pub asn: String,
    pub country: String,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub time_zone: String,
}

#[derive(Debug, Clone)]
pub struct CreateHit {
    pub session_id: SessionId,
    pub service_id: ServiceId,
    pub initial: bool,
    pub start_time: DateTime<Utc>,
    pub tracker: TrackerType,
    pub location: String,
    pub referrer: String,
    pub load_time: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CoreStats {
    pub currently_online: i64,
    pub session_count: i64,
    pub hit_count: i64,
    pub has_hits: bool,
    pub bounce_rate_pct: Option<f64>,
    pub avg_session_duration: Option<f64>,
    pub avg_load_time: Option<f64>,
    pub avg_hits_per_session: Option<f64>,
    pub locations: Vec<CountedItem>,
    pub referrers: Vec<CountedItem>,
    pub countries: Vec<CountedItem>,
    pub operating_systems: Vec<CountedItem>,
    pub browsers: Vec<CountedItem>,
    pub devices: Vec<CountedItem>,
    pub device_types: Vec<CountedItem>,
    pub chart_data: ChartData,
    pub chart_tooltip_format: String,
    pub chart_granularity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare: Option<Box<CoreStats>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn test_service() -> Service {
        Service {
            id: ServiceId(Uuid::new_v4()),
            tracking_id: TrackingId("abc12345".to_string()),
            name: "Test Service".to_string(),
            link: "https://example.com".to_string(),
            origins: "*".to_string(),
            status: ServiceStatus::Active,
            respect_dnt: true,
            ignore_robots: false,
            collect_ips: true,
            ignored_ips: "".to_string(),
            hide_referrer_regex: "".to_string(),
            script_inject: "".to_string(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_service_is_origin_allowed_wildcard() {
        let service = test_service();
        assert!(service.is_origin_allowed("https://example.com"));
        assert!(service.is_origin_allowed("https://anything.com"));
        assert!(service.is_origin_allowed("http://localhost"));
    }

    #[test]
    fn test_service_is_origin_allowed_specific() {
        let mut service = test_service();
        service.origins = "https://example.com, https://test.com".to_string();

        assert!(service.is_origin_allowed("https://example.com"));
        assert!(service.is_origin_allowed("https://test.com"));
        assert!(!service.is_origin_allowed("https://other.com"));
    }

    #[test]
    fn test_service_is_origin_allowed_case_insensitive() {
        let mut service = test_service();
        service.origins = "https://Example.Com".to_string();

        assert!(service.is_origin_allowed("https://example.com"));
        assert!(service.is_origin_allowed("HTTPS://EXAMPLE.COM"));
    }

    #[test]
    fn test_service_get_origins_list_wildcard() {
        let service = test_service();
        let origins = service.get_origins_list();
        assert_eq!(origins, vec!["*"]);
    }

    #[test]
    fn test_service_get_origins_list_multiple() {
        let mut service = test_service();
        service.origins = "https://a.com, https://b.com, https://c.com".to_string();

        let origins = service.get_origins_list();
        assert_eq!(origins.len(), 3);
        assert!(origins.contains(&"https://a.com".to_string()));
        assert!(origins.contains(&"https://b.com".to_string()));
        assert!(origins.contains(&"https://c.com".to_string()));
    }

    #[test]
    fn test_service_get_ignored_networks_empty() {
        let service = test_service();
        let networks = service.get_ignored_networks();
        assert!(networks.is_empty());
    }

    #[test]
    fn test_service_get_ignored_networks_valid() {
        let mut service = test_service();
        service.ignored_ips = "192.168.1.0/24, 10.0.0.0/8".to_string();

        let networks = service.get_ignored_networks();
        assert_eq!(networks.len(), 2);
    }

    #[test]
    fn test_service_get_ignored_networks_with_invalid() {
        let mut service = test_service();
        service.ignored_ips = "192.168.1.0/24, invalid, 10.0.0.0/8".to_string();

        let networks = service.get_ignored_networks();
        assert_eq!(networks.len(), 2); // Invalid entry ignored
    }

    #[test]
    fn test_service_get_ignored_networks_whitespace_only() {
        let mut service = test_service();
        service.ignored_ips = "   ".to_string();

        let networks = service.get_ignored_networks();
        assert!(networks.is_empty());
    }

    #[test]
    fn test_session_creation() {
        let session = Session {
            id: SessionId(Uuid::new_v4()),
            service_id: ServiceId(Uuid::new_v4()),
            identifier: "".to_string(),
            start_time: Utc::now(),
            last_seen: Utc::now(),
            user_agent: "Mozilla/5.0".to_string(),
            browser: "Chrome".to_string(),
            device: "".to_string(),
            device_type: DeviceType::Desktop,
            os: "Windows 10".to_string(),
            ip: Some("192.168.1.1".to_string()),
            asn: "".to_string(),
            country: "US".to_string(),
            longitude: Some(-122.0),
            latitude: Some(37.0),
            time_zone: "America/Los_Angeles".to_string(),
            is_bounce: true,
        };

        assert_eq!(session.browser, "Chrome");
        assert_eq!(session.device_type, DeviceType::Desktop);
        assert!(session.is_bounce);
    }

    #[test]
    fn test_hit_creation() {
        let hit = Hit {
            id: HitId(1),
            session_id: SessionId(Uuid::new_v4()),
            service_id: ServiceId(Uuid::new_v4()),
            initial: true,
            start_time: Utc::now(),
            last_seen: Utc::now(),
            heartbeats: 0,
            tracker: TrackerType::Js,
            location: "/home".to_string(),
            referrer: "https://google.com".to_string(),
            load_time: Some(150.5),
        };

        assert!(hit.initial);
        assert_eq!(hit.heartbeats, 0);
        assert_eq!(hit.tracker, TrackerType::Js);
        assert_eq!(hit.location, "/home");
    }

    #[test]
    fn test_create_service_default() {
        let create = CreateService::default();
        assert!(create.name.is_empty());
        assert!(create.link.is_empty());
        assert!(create.origins.is_empty());
        assert!(!create.respect_dnt);
        assert!(!create.ignore_robots);
        assert!(!create.collect_ips);
    }

    #[test]
    fn test_update_service_default() {
        let update = UpdateService::default();
        assert!(update.name.is_none());
        assert!(update.link.is_none());
        assert!(update.origins.is_none());
        assert!(update.status.is_none());
    }

    #[test]
    fn test_core_stats_default() {
        let stats = CoreStats::default();
        assert_eq!(stats.currently_online, 0);
        assert_eq!(stats.session_count, 0);
        assert_eq!(stats.hit_count, 0);
        assert!(!stats.has_hits);
        assert!(stats.bounce_rate_pct.is_none());
        assert!(stats.avg_session_duration.is_none());
        assert!(stats.locations.is_empty());
        assert!(stats.compare.is_none());
    }

    #[test]
    fn test_create_session_fields() {
        let create = CreateSession {
            service_id: ServiceId(Uuid::new_v4()),
            identifier: "user123".to_string(),
            start_time: Utc::now(),
            user_agent: "Test UA".to_string(),
            browser: "Firefox".to_string(),
            device: "PC".to_string(),
            device_type: DeviceType::Desktop,
            os: "Linux".to_string(),
            ip: None,
            asn: "".to_string(),
            country: "".to_string(),
            longitude: None,
            latitude: None,
            time_zone: "".to_string(),
        };

        assert_eq!(create.identifier, "user123");
        assert_eq!(create.browser, "Firefox");
        assert!(create.ip.is_none());
    }

    #[test]
    fn test_create_hit_fields() {
        let create = CreateHit {
            session_id: SessionId(Uuid::new_v4()),
            service_id: ServiceId(Uuid::new_v4()),
            initial: false,
            start_time: Utc::now(),
            tracker: TrackerType::Pixel,
            location: "/about".to_string(),
            referrer: "".to_string(),
            load_time: None,
        };

        assert!(!create.initial);
        assert_eq!(create.tracker, TrackerType::Pixel);
        assert!(create.load_time.is_none());
    }
}
