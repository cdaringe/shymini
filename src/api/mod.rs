use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::db;
use crate::domain::{ServiceId, SessionId};
use crate::error::Error;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct DateRangeQuery {
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    #[serde(rename = "urlPattern")]
    pub url_pattern: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(msg: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.to_string()),
        }
    }
}

/// Parse a datetime string that may be either datetime-local format (YYYY-MM-DDTHH:MM)
/// or date-only format (YYYY-MM-DD). For date-only, uses start/end of day based on is_end.
fn parse_datetime_string(s: &str, is_end: bool) -> Option<chrono::DateTime<Utc>> {
    // Try datetime-local format first (YYYY-MM-DDTHH:MM)
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        return Some(dt.and_utc());
    }
    // Fall back to date-only format (YYYY-MM-DD)
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let time = if is_end {
            d.and_hms_opt(23, 59, 59).unwrap()
        } else {
            d.and_hms_opt(0, 0, 0).unwrap()
        };
        return Some(time.and_utc());
    }
    None
}

fn parse_date_range(query: &DateRangeQuery) -> (chrono::DateTime<Utc>, chrono::DateTime<Utc>) {
    let now = Utc::now();
    let default_start = now - Duration::days(30);

    let start = query
        .start_date
        .as_ref()
        .and_then(|s| parse_datetime_string(s, false))
        .unwrap_or(default_start);

    let end = query
        .end_date
        .as_ref()
        .and_then(|s| parse_datetime_string(s, true))
        .unwrap_or(now);

    // Ensure start <= end; if not, swap them
    if start > end {
        (end, start)
    } else {
        (start, end)
    }
}

fn parse_url_pattern(pattern: &Option<String>) -> Option<Regex> {
    pattern
        .as_ref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Regex::new(s).ok())
}

/// GET /api/services
pub async fn list_services(State(state): State<AppState>) -> Response {
    match db::list_services(&state.pool).await {
        Ok(services) => Json(ApiResponse::success(services)).into_response(),
        Err(e) => {
            error!("Error listing services: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error("Failed to list services")),
            )
                .into_response()
        }
    }
}

/// GET /api/services/:id
pub async fn get_service(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error("Invalid service ID")),
            )
                .into_response()
        }
    };

    match db::get_service(&state.pool, service_id).await {
        Ok(service) => Json(ApiResponse::success(service)).into_response(),
        Err(Error::ServiceNotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Service not found")),
        )
            .into_response(),
        Err(e) => {
            error!("Error fetching service: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error("Failed to fetch service")),
            )
                .into_response()
        }
    }
}

/// GET /api/services/:id/stats
pub async fn get_service_stats(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error("Invalid service ID")),
            )
                .into_response()
        }
    };

    let service = match db::get_service(&state.pool, service_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error("Service not found")),
            )
                .into_response()
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error("Failed to fetch service")),
            )
                .into_response();
        }
    };

    let (start, end) = parse_date_range(&query);
    let url_pattern = parse_url_pattern(&query.url_pattern);

    let hide_referrer_regex = if service.hide_referrer_regex.is_empty() {
        None
    } else {
        Regex::new(&service.hide_referrer_regex).ok()
    };

    match db::get_core_stats(
        &state.pool,
        service_id,
        start,
        end,
        hide_referrer_regex.as_ref(),
        url_pattern.as_ref(),
        state.settings.active_user_timeout_ms(),
    )
    .await
    {
        Ok(stats) => Json(ApiResponse::success(stats)).into_response(),
        Err(e) => {
            error!("Error fetching stats: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error("Failed to fetch stats")),
            )
                .into_response()
        }
    }
}

/// GET /api/services/:id/sessions
pub async fn list_sessions(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error("Invalid service ID")),
            )
                .into_response()
        }
    };

    let (start, end) = parse_date_range(&query);
    let url_pattern = parse_url_pattern(&query.url_pattern);

    match db::list_sessions(&state.pool, service_id, start, end, url_pattern.as_ref(), 100, 0).await {
        Ok(sessions) => Json(ApiResponse::success(sessions)).into_response(),
        Err(e) => {
            error!("Error listing sessions: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error("Failed to list sessions")),
            )
                .into_response()
        }
    }
}

/// GET /api/sessions/:id
pub async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let session_id: SessionId = match session_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error("Invalid session ID")),
            )
                .into_response()
        }
    };

    match db::get_session(&state.pool, session_id).await {
        Ok(session) => Json(ApiResponse::success(session)).into_response(),
        Err(Error::SessionNotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Session not found")),
        )
            .into_response(),
        Err(e) => {
            error!("Error fetching session: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error("Failed to fetch session")),
            )
                .into_response()
        }
    }
}

/// GET /api/sessions/:id/hits
pub async fn list_session_hits(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let session_id: SessionId = match session_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error("Invalid session ID")),
            )
                .into_response()
        }
    };

    match db::list_hits_for_session(&state.pool, session_id, 100, 0).await {
        Ok(hits) => Json(ApiResponse::success(hits)).into_response(),
        Err(e) => {
            error!("Error listing hits: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error("Failed to list hits")),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response = ApiResponse::<String>::error("something went wrong");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_api_response_success_with_struct() {
        #[derive(Serialize, PartialEq, Debug)]
        struct TestData {
            value: i32,
        }

        let data = TestData { value: 42 };
        let response = ApiResponse::success(data);
        assert!(response.success);
        assert_eq!(response.data.unwrap().value, 42);
    }

    #[test]
    fn test_parse_date_range_defaults() {
        let query = DateRangeQuery {
            start_date: None,
            end_date: None,
            url_pattern: None,
        };
        let (start, end) = parse_date_range(&query);

        // Default is last 30 days
        let now = Utc::now();
        let expected_start = now - Duration::days(30);

        // Should be within a second of expected
        assert!((start - expected_start).num_seconds().abs() < 2);
        assert!((end - now).num_seconds().abs() < 2);
    }

    #[test]
    fn test_parse_date_range_with_start() {
        let query = DateRangeQuery {
            start_date: Some("2024-01-01".to_string()),
            end_date: None,
            url_pattern: None,
        };
        let (start, _end) = parse_date_range(&query);

        assert_eq!(start.format("%Y-%m-%d").to_string(), "2024-01-01");
    }

    #[test]
    fn test_parse_date_range_with_end() {
        // Use a future date to avoid swap logic when default start is more recent
        let query = DateRangeQuery {
            start_date: None,
            end_date: Some("2099-12-31".to_string()),
            url_pattern: None,
        };
        let (_start, end) = parse_date_range(&query);

        assert_eq!(end.format("%Y-%m-%d").to_string(), "2099-12-31");
    }

    #[test]
    fn test_parse_date_range_both_dates() {
        let query = DateRangeQuery {
            start_date: Some("2024-06-01".to_string()),
            end_date: Some("2024-06-30".to_string()),
            url_pattern: None,
        };
        let (start, end) = parse_date_range(&query);

        assert_eq!(start.format("%Y-%m-%d").to_string(), "2024-06-01");
        assert_eq!(end.format("%Y-%m-%d").to_string(), "2024-06-30");
    }

    #[test]
    fn test_parse_date_range_invalid_start() {
        let query = DateRangeQuery {
            start_date: Some("not-a-date".to_string()),
            end_date: None,
            url_pattern: None,
        };
        let (start, _end) = parse_date_range(&query);

        // Should fall back to default (30 days ago)
        let now = Utc::now();
        let expected_start = now - Duration::days(30);
        assert!((start - expected_start).num_seconds().abs() < 2);
    }

    #[test]
    fn test_parse_date_range_invalid_end() {
        let query = DateRangeQuery {
            start_date: None,
            end_date: Some("invalid".to_string()),
            url_pattern: None,
        };
        let (_start, end) = parse_date_range(&query);

        // Should fall back to now
        let now = Utc::now();
        assert!((end - now).num_seconds().abs() < 2);
    }

    #[test]
    fn test_parse_date_range_datetime_local_format() {
        let query = DateRangeQuery {
            start_date: Some("2024-06-01T09:30".to_string()),
            end_date: Some("2024-06-30T17:45".to_string()),
            url_pattern: None,
        };
        let (start, end) = parse_date_range(&query);

        assert_eq!(start.format("%Y-%m-%dT%H:%M").to_string(), "2024-06-01T09:30");
        assert_eq!(end.format("%Y-%m-%dT%H:%M").to_string(), "2024-06-30T17:45");
    }

    #[test]
    fn test_parse_date_range_mixed_formats() {
        // Start as datetime-local, end as date-only
        let query = DateRangeQuery {
            start_date: Some("2024-06-01T14:00".to_string()),
            end_date: Some("2024-06-30".to_string()),
            url_pattern: None,
        };
        let (start, end) = parse_date_range(&query);

        assert_eq!(start.format("%Y-%m-%dT%H:%M").to_string(), "2024-06-01T14:00");
        // Date-only end should default to 23:59:59
        assert_eq!(end.format("%Y-%m-%d %H:%M:%S").to_string(), "2024-06-30 23:59:59");
    }

    #[test]
    fn test_parse_date_range_swaps_when_start_after_end() {
        // When start > end, they should be swapped
        let query = DateRangeQuery {
            start_date: Some("2024-12-31T23:59".to_string()),
            end_date: Some("2024-01-01T00:00".to_string()),
            url_pattern: None,
        };
        let (start, end) = parse_date_range(&query);

        // The earlier date should become start, later date should become end
        assert_eq!(start.format("%Y-%m-%dT%H:%M").to_string(), "2024-01-01T00:00");
        assert_eq!(end.format("%Y-%m-%dT%H:%M").to_string(), "2024-12-31T23:59");
    }

    #[test]
    fn test_date_range_query_deserialize() {
        // Test the serde rename attributes work
        let json = r#"{"startDate": "2024-01-01", "endDate": "2024-12-31"}"#;
        let query: DateRangeQuery = serde_json::from_str(json).unwrap();

        assert_eq!(query.start_date, Some("2024-01-01".to_string()));
        assert_eq!(query.end_date, Some("2024-12-31".to_string()));
    }

    #[test]
    fn test_parse_url_pattern_valid() {
        let pattern = Some("/blog/.*".to_string());
        let regex = parse_url_pattern(&pattern);
        assert!(regex.is_some());
        assert!(regex.unwrap().is_match("/blog/post-1"));
    }

    #[test]
    fn test_parse_url_pattern_none() {
        let pattern: Option<String> = None;
        let regex = parse_url_pattern(&pattern);
        assert!(regex.is_none());
    }

    #[test]
    fn test_parse_url_pattern_empty() {
        let pattern = Some("".to_string());
        let regex = parse_url_pattern(&pattern);
        assert!(regex.is_none());
    }

    #[test]
    fn test_parse_url_pattern_invalid() {
        let pattern = Some("[invalid".to_string());
        let regex = parse_url_pattern(&pattern);
        assert!(regex.is_none());
    }

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse::success(vec![1, 2, 3]);
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"data\":[1,2,3]"));
        assert!(!json.contains("\"error\"")); // Should be skipped when None
    }

    #[test]
    fn test_api_response_error_serialization() {
        let response = ApiResponse::<()>::error("test error");
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("\"success\":false"));
        assert!(!json.contains("\"data\"")); // Should be skipped when None
        assert!(json.contains("\"error\":\"test error\""));
    }
}
