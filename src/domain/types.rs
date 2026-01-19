use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

/// Short alphanumeric tracking ID for use in tracker URLs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrackingId(pub String);

impl TrackingId {
    const CHARSET: &'static [u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    const DEFAULT_LENGTH: usize = 8;

    /// Generate a new random tracking ID
    pub fn new() -> Self {
        Self::with_length(Self::DEFAULT_LENGTH)
    }

    /// Generate a tracking ID with custom length
    pub fn with_length(len: usize) -> Self {
        let mut rng = rand::thread_rng();
        let id: String = (0..len)
            .map(|_| {
                let idx = rng.gen_range(0..Self::CHARSET.len());
                Self::CHARSET[idx] as char
            })
            .collect();
        Self(id)
    }
}

impl Default for TrackingId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TrackingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TrackingId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServiceId(pub Uuid);

impl ServiceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ServiceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ServiceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SessionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HitId(pub i64);

impl fmt::Display for HitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceStatus {
    Active,
    Archived,
}

impl ServiceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "AC",
            Self::Archived => "AR",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "AC" => Some(Self::Active),
            "AR" => Some(Self::Archived),
            _ => None,
        }
    }
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Archived => write!(f, "Archived"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DeviceType {
    Phone,
    Tablet,
    Desktop,
    Robot,
    #[default]
    Other,
}

impl DeviceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Phone => "PHONE",
            Self::Tablet => "TABLET",
            Self::Desktop => "DESKTOP",
            Self::Robot => "ROBOT",
            Self::Other => "OTHER",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "PHONE" => Self::Phone,
            "TABLET" => Self::Tablet,
            "DESKTOP" => Self::Desktop,
            "ROBOT" => Self::Robot,
            _ => Self::Other,
        }
    }
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Phone => write!(f, "Phone"),
            Self::Tablet => write!(f, "Tablet"),
            Self::Desktop => write!(f, "Desktop"),
            Self::Robot => write!(f, "Robot"),
            Self::Other => write!(f, "Other"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackerType {
    Js,
    Pixel,
}

impl TrackerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Js => "JS",
            Self::Pixel => "PIXEL",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "JS" => Self::Js,
            _ => Self::Pixel,
        }
    }
}

impl fmt::Display for TrackerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Js => write!(f, "JavaScript"),
            Self::Pixel => write!(f, "Pixel"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionAssociationHash(pub String);

impl SessionAssociationHash {
    pub fn compute(
        ip: &str,
        user_agent: &str,
        service_id: Option<&ServiceId>,
        aggressive_salting: bool,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(ip.as_bytes());
        hasher.update(user_agent.as_bytes());

        if aggressive_salting {
            if let Some(sid) = service_id {
                hasher.update(sid.0.as_bytes());
            }
            let date = Utc::now().format("%Y-%m-%d").to_string();
            hasher.update(date.as_bytes());
        }

        let result = hasher.finalize();
        Self(hex::encode(result))
    }
}

impl fmt::Display for SessionAssociationHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChartData {
    pub sessions: Vec<i64>,
    pub hits: Vec<i64>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartGranularity {
    Hourly,
    Daily,
}

impl ChartGranularity {
    pub fn tooltip_format(&self) -> &'static str {
        match self {
            Self::Hourly => "MM/dd HH:mm",
            Self::Daily => "MMM d",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountedItem {
    pub value: String,
    pub count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_id_new() {
        let id1 = ServiceId::new();
        let id2 = ServiceId::new();
        assert_ne!(id1, id2, "Each new ServiceId should be unique");
    }

    #[test]
    fn test_service_id_parse() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id: ServiceId = uuid_str.parse().unwrap();
        assert_eq!(id.to_string(), uuid_str);
    }

    #[test]
    fn test_service_id_invalid_parse() {
        let result: Result<ServiceId, _> = "not-a-uuid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_session_id_new() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_service_status_roundtrip() {
        assert_eq!(ServiceStatus::from_str("AC"), Some(ServiceStatus::Active));
        assert_eq!(ServiceStatus::from_str("AR"), Some(ServiceStatus::Archived));
        assert_eq!(ServiceStatus::from_str("XX"), None);

        assert_eq!(ServiceStatus::Active.as_str(), "AC");
        assert_eq!(ServiceStatus::Archived.as_str(), "AR");
    }

    #[test]
    fn test_device_type_roundtrip() {
        assert_eq!(DeviceType::from_str("PHONE"), DeviceType::Phone);
        assert_eq!(DeviceType::from_str("TABLET"), DeviceType::Tablet);
        assert_eq!(DeviceType::from_str("DESKTOP"), DeviceType::Desktop);
        assert_eq!(DeviceType::from_str("ROBOT"), DeviceType::Robot);
        assert_eq!(DeviceType::from_str("OTHER"), DeviceType::Other);
        assert_eq!(DeviceType::from_str("unknown"), DeviceType::Other);

        // Case insensitive
        assert_eq!(DeviceType::from_str("phone"), DeviceType::Phone);
        assert_eq!(DeviceType::from_str("Phone"), DeviceType::Phone);
    }

    #[test]
    fn test_tracker_type_roundtrip() {
        assert_eq!(TrackerType::from_str("JS"), TrackerType::Js);
        assert_eq!(TrackerType::from_str("PIXEL"), TrackerType::Pixel);
        assert_eq!(TrackerType::from_str("anything"), TrackerType::Pixel);

        assert_eq!(TrackerType::Js.as_str(), "JS");
        assert_eq!(TrackerType::Pixel.as_str(), "PIXEL");
    }

    #[test]
    fn test_session_hash_deterministic() {
        let hash1 = SessionAssociationHash::compute("192.168.1.1", "Mozilla/5.0", None, false);
        let hash2 = SessionAssociationHash::compute("192.168.1.1", "Mozilla/5.0", None, false);
        assert_eq!(hash1, hash2, "Same inputs should produce same hash");
    }

    #[test]
    fn test_session_hash_different_ip() {
        let hash1 = SessionAssociationHash::compute("192.168.1.1", "Mozilla/5.0", None, false);
        let hash2 = SessionAssociationHash::compute("192.168.1.2", "Mozilla/5.0", None, false);
        assert_ne!(
            hash1, hash2,
            "Different IPs should produce different hashes"
        );
    }

    #[test]
    fn test_session_hash_different_ua() {
        let hash1 = SessionAssociationHash::compute("192.168.1.1", "Mozilla/5.0", None, false);
        let hash2 = SessionAssociationHash::compute("192.168.1.1", "Chrome/91.0", None, false);
        assert_ne!(
            hash1, hash2,
            "Different user agents should produce different hashes"
        );
    }

    #[test]
    fn test_session_hash_with_service_id() {
        let service_id = ServiceId::new();
        let hash1 =
            SessionAssociationHash::compute("192.168.1.1", "Mozilla/5.0", Some(&service_id), true);
        let hash2 = SessionAssociationHash::compute("192.168.1.1", "Mozilla/5.0", None, true);
        assert_ne!(
            hash1, hash2,
            "Service ID should affect hash when aggressive salting enabled"
        );
    }

    #[test]
    fn test_chart_data_default() {
        let data = ChartData::default();
        assert!(data.sessions.is_empty());
        assert!(data.hits.is_empty());
        assert!(data.labels.is_empty());
    }

    #[test]
    fn test_counted_item() {
        let item = CountedItem {
            value: "test".to_string(),
            count: 42,
        };
        assert_eq!(item.value, "test");
        assert_eq!(item.count, 42);
    }
}
