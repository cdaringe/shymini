use askama::Template;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::domain::{CoreStats, CountedItem, Hit, Service, Session, TrackerType};

#[derive(Template)]
#[template(path = "dashboard/index.html")]
pub struct DashboardIndexTemplate {
    pub services: Vec<ServiceWithStats>,
}

pub struct ServiceWithStats {
    pub service: Service,
    pub session_count: i64,
    pub hit_count: i64,
}

#[derive(Template)]
#[template(path = "dashboard/service.html")]
pub struct ServiceDetailTemplate {
    pub service: Service,
    pub stats: CoreStats,
    pub sessions: Vec<Session>,
    pub start_date: String,
    pub end_date: String,
    pub url_pattern: String,
    pub results_limit: i64,
}

#[derive(Template)]
#[template(path = "dashboard/service_create.html")]
pub struct ServiceCreateTemplate {}

#[derive(Template)]
#[template(path = "dashboard/service_update.html")]
pub struct ServiceUpdateTemplate {
    pub service: Service,
}

#[derive(Template)]
#[template(path = "dashboard/service_delete.html")]
pub struct ServiceDeleteTemplate {
    pub service: Service,
}

#[derive(Template)]
#[template(path = "dashboard/session_list.html")]
pub struct SessionListTemplate {
    pub service: Service,
    pub sessions: Vec<Session>,
    pub page: i64,
    pub has_next: bool,
    pub start_date: String,
    pub end_date: String,
    pub url_pattern: String,
}

/// A Hit with pre-formatted timestamps for display in templates
pub struct HitDisplay {
    pub location: String,
    pub referrer: String,
    pub tracker: TrackerType,
    pub load_time: Option<f64>,
    pub heartbeats: i32,
    pub initial: bool,
    /// Formatted start time in user's timezone
    pub start_time: String,
    /// Formatted last seen time in user's timezone
    pub last_seen: String,
}

impl HitDisplay {
    pub fn from_hit(hit: Hit, tz: Tz) -> Self {
        let start_local = hit.start_time.with_timezone(&tz);
        let last_seen_local = hit.last_seen.with_timezone(&tz);

        Self {
            location: hit.location,
            referrer: hit.referrer,
            tracker: hit.tracker,
            load_time: hit.load_time,
            heartbeats: hit.heartbeats,
            initial: hit.initial,
            start_time: start_local.format("%m/%d %H:%M:%S").to_string(),
            last_seen: last_seen_local.format("%m/%d %H:%M:%S").to_string(),
        }
    }
}

#[derive(Template)]
#[template(path = "dashboard/session_detail.html")]
pub struct SessionDetailTemplate {
    pub service: Service,
    pub session: SessionDisplay,
    pub hits: Vec<HitDisplay>,
}

/// A Session with pre-formatted timestamps for display in templates
pub struct SessionDisplay {
    pub id: String,
    pub identifier: String,
    pub start_time: String,
    pub last_seen: String,
    pub user_agent: String,
    pub browser: String,
    pub device: String,
    pub device_type: String,
    pub os: String,
    pub ip: Option<String>,
    pub asn: String,
    pub country: String,
    pub time_zone: String,
    pub is_bounce: bool,
}

impl SessionDisplay {
    pub fn from_session(session: Session, tz: Tz) -> Self {
        let start_local = session.start_time.with_timezone(&tz);
        let last_seen_local = session.last_seen.with_timezone(&tz);

        Self {
            id: session.id.0.to_string(),
            identifier: session.identifier,
            start_time: start_local.format("%Y-%m-%d %H:%M:%S %Z").to_string(),
            last_seen: last_seen_local.format("%Y-%m-%d %H:%M:%S %Z").to_string(),
            user_agent: session.user_agent,
            browser: session.browser,
            device: session.device,
            device_type: session.device_type.to_string(),
            os: session.os,
            ip: session.ip,
            asn: session.asn,
            country: session.country,
            time_zone: session.time_zone,
            is_bounce: session.is_bounce,
        }
    }
}

#[derive(Template)]
#[template(path = "dashboard/location_list.html")]
pub struct LocationListTemplate {
    pub service: Service,
    pub locations: Vec<CountedItem>,
    pub total_hits: i64,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Template)]
#[template(path = "components/stats_partial.html")]
pub struct StatsPartialTemplate {
    pub stats: CoreStats,
    pub service_id: String,
}

#[derive(Template)]
#[template(path = "components/session_table.html")]
pub struct SessionTableTemplate {
    pub sessions: Vec<Session>,
    pub service_id: String,
}

// Template helper functions - use as methods in templates
impl ServiceWithStats {
    pub fn format_count(count: i64) -> String {
        intcomma(count)
    }
}

// Helper functions for templates
#[allow(clippy::manual_is_multiple_of)]
pub fn intcomma(value: i64) -> String {
    let s = value.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }
    result
}

pub fn floatformat(value: Option<f64>, precision: i32) -> String {
    match value {
        Some(v) => {
            if precision < 0 {
                let formatted = format!("{:.prec$}", v, prec = precision.unsigned_abs() as usize);
                formatted
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            } else {
                format!("{:.prec$}", v, prec = precision as usize)
            }
        }
        None => "?".to_string(),
    }
}

pub fn percent(count: i64, total: i64) -> String {
    if total == 0 {
        return "0%".to_string();
    }
    let pct = (count as f64 / total as f64) * 100.0;
    format!("{:.1}%", pct)
}

pub fn naturaldelta(seconds: Option<f64>) -> String {
    match seconds {
        Some(secs) => {
            let total_secs = secs as i64;

            if total_secs < 60 {
                format!("{}s", total_secs)
            } else if total_secs < 3600 {
                let mins = total_secs / 60;
                let secs = total_secs % 60;
                if secs > 0 {
                    format!("{}m {}s", mins, secs)
                } else {
                    format!("{}m", mins)
                }
            } else {
                let hours = total_secs / 3600;
                let mins = (total_secs % 3600) / 60;
                if mins > 0 {
                    format!("{}h {}m", hours, mins)
                } else {
                    format!("{}h", hours)
                }
            }
        }
        None => "?".to_string(),
    }
}

pub fn urldisplay(url: &str) -> String {
    if url.is_empty() {
        return "Unknown".to_string();
    }

    if let Ok(parsed) = url::Url::parse(url) {
        let path = parsed.path();
        if path == "/" || path.is_empty() {
            return parsed.host_str().unwrap_or(url).to_string();
        }
        return format!("{}{}", parsed.host_str().unwrap_or(""), path);
    }

    url.to_string()
}

pub fn country_name(code: &str) -> String {
    match code.to_uppercase().as_str() {
        "US" => "United States",
        "GB" => "United Kingdom",
        "DE" => "Germany",
        "FR" => "France",
        "CA" => "Canada",
        "AU" => "Australia",
        "JP" => "Japan",
        "CN" => "China",
        "IN" => "India",
        "BR" => "Brazil",
        "RU" => "Russia",
        "IT" => "Italy",
        "ES" => "Spain",
        "MX" => "Mexico",
        "KR" => "South Korea",
        "NL" => "Netherlands",
        "SE" => "Sweden",
        "CH" => "Switzerland",
        "PL" => "Poland",
        "BE" => "Belgium",
        "AT" => "Austria",
        "NO" => "Norway",
        "DK" => "Denmark",
        "FI" => "Finland",
        "IE" => "Ireland",
        "NZ" => "New Zealand",
        "SG" => "Singapore",
        "HK" => "Hong Kong",
        "TW" => "Taiwan",
        "" => "Unknown",
        _ => code,
    }
    .to_string()
}

pub fn timeago(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*dt);

    let secs = duration.num_seconds();
    if secs < 60 {
        return "just now".to_string();
    }

    let mins = duration.num_minutes();
    if mins < 60 {
        return format!("{}m ago", mins);
    }

    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{}h ago", hours);
    }

    let days = duration.num_days();
    if days < 30 {
        return format!("{}d ago", days);
    }

    dt.format("%Y-%m-%d").to_string()
}
