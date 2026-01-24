use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use chrono::{Duration, TimeZone, Utc};
use chrono_tz::Tz;
use regex::Regex;
use serde::Deserialize;
use tracing::error;

use crate::db;
use crate::domain::{CreateService, ServiceId, SessionId, UpdateService};
use crate::error::Error;
use crate::state::AppState;

use super::templates::*;

const PAGE_SIZE: i64 = 50;
const RESULTS_LIMIT: i64 = 300;

#[derive(Debug, Deserialize)]
pub struct DateRangeQuery {
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    #[serde(rename = "urlPattern")]
    pub url_pattern: Option<String>,
    /// Timezone for interpreting dates and displaying results (e.g., "America/New_York")
    pub tz: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    #[serde(rename = "urlPattern")]
    pub url_pattern: Option<String>,
    /// Timezone for interpreting dates and displaying results (e.g., "America/New_York")
    pub tz: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceForm {
    pub name: String,
    pub link: Option<String>,
    pub origins: Option<String>,
    pub respect_dnt: Option<String>,
    pub ignore_robots: Option<String>,
    pub collect_ips: Option<String>,
    pub ignored_ips: Option<String>,
    pub hide_referrer_regex: Option<String>,
    pub script_inject: Option<String>,
}

/// Parse a timezone string, defaulting to Pacific Time if invalid or not provided
fn parse_timezone(tz_str: Option<&str>) -> Tz {
    tz_str
        .and_then(|s| s.parse::<Tz>().ok())
        .unwrap_or(chrono_tz::America::Los_Angeles)
}

/// Parse a date/datetime string, interpreting it in the given timezone,
/// and convert to UTC. Supports:
/// - ISO 8601 with timezone (2024-01-19T15:30:00.000Z)
/// - datetime-local (YYYY-MM-DDTHH:MM)
/// - date-only (YYYY-MM-DD)
fn parse_datetime_string(s: &str, is_end: bool, tz: Tz) -> Option<chrono::DateTime<Utc>> {
    // Try full ISO 8601 / RFC 3339 format first (already includes timezone)
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try ISO 8601 with seconds but no timezone (interpret in user's tz)
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return tz
            .from_local_datetime(&dt)
            .single()
            .map(|dt| dt.with_timezone(&Utc));
    }

    // Try datetime-local format (YYYY-MM-DDTHH:MM)
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        // Interpret the naive datetime as being in the specified timezone
        return tz
            .from_local_datetime(&dt)
            .single()
            .map(|dt| dt.with_timezone(&Utc));
    }

    // Fall back to date-only format (YYYY-MM-DD)
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let time = if is_end {
            d.and_hms_opt(23, 59, 59).unwrap()
        } else {
            d.and_hms_opt(0, 0, 0).unwrap()
        };
        // Interpret the naive datetime as being in the specified timezone
        return tz
            .from_local_datetime(&time)
            .single()
            .map(|dt| dt.with_timezone(&Utc));
    }

    None
}

fn parse_date_range(query: &DateRangeQuery) -> (chrono::DateTime<Utc>, chrono::DateTime<Utc>, Tz) {
    let tz = parse_timezone(query.tz.as_deref());
    let now = Utc::now();
    let default_start = now - Duration::days(30);

    let start = query
        .start_date
        .as_ref()
        .and_then(|s| parse_datetime_string(s, false, tz))
        .unwrap_or(default_start);

    let end = query
        .end_date
        .as_ref()
        .and_then(|s| parse_datetime_string(s, true, tz))
        .unwrap_or(now);

    (start, end, tz)
}

fn parse_url_pattern(pattern: &Option<String>) -> Option<Regex> {
    pattern
        .as_ref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Regex::new(s).ok())
}

/// GET /
pub async fn dashboard_index(State(state): State<AppState>) -> Response {
    let services = match db::list_services(&state.pool).await {
        Ok(s) => s,
        Err(e) => {
            error!("Error listing services: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let now = Utc::now();
    let day_ago = now - Duration::days(1);

    let mut services_with_stats = Vec::new();
    for service in services {
        // Get basic daily stats
        let (session_count, hit_count): (i64, i64) =
            get_basic_counts(&state, service.id, day_ago, now)
                .await
                .unwrap_or_default();

        services_with_stats.push(ServiceWithStats {
            service,
            session_count,
            hit_count,
        });
    }

    let template = DashboardIndexTemplate {
        services: services_with_stats,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

async fn get_basic_counts(
    state: &AppState,
    service_id: ServiceId,
    start: chrono::DateTime<Utc>,
    end: chrono::DateTime<Utc>,
) -> Result<(i64, i64), Error> {
    #[cfg(feature = "postgres")]
    {
        let session_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE service_id = $1 AND start_time >= $2 AND start_time < $3"
        )
        .bind(service_id.0)
        .bind(start)
        .bind(end)
        .fetch_one(&state.pool)
        .await?;

        let hit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM hits WHERE service_id = $1 AND start_time >= $2 AND start_time < $3"
        )
        .bind(service_id.0)
        .bind(start)
        .bind(end)
        .fetch_one(&state.pool)
        .await?;

        Ok((session_count, hit_count))
    }

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    {
        let session_count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ?"
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_one(&state.pool)
        .await?;

        let hit_count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ?",
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_one(&state.pool)
        .await?;

        Ok((session_count as i64, hit_count as i64))
    }
}

/// GET /service/:id
pub async fn service_detail(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid service ID").into_response(),
    };

    let service = match db::get_service(&state.pool, service_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            return (StatusCode::NOT_FOUND, "Service not found").into_response()
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let (start, end, tz) = parse_date_range(&query);
    let url_pattern = parse_url_pattern(&query.url_pattern);

    let hide_referrer_regex = if service.hide_referrer_regex.is_empty() {
        None
    } else {
        Regex::new(&service.hide_referrer_regex).ok()
    };

    let stats = match db::get_core_stats(
        &state.pool,
        service_id,
        start,
        end,
        hide_referrer_regex.as_ref(),
        url_pattern.as_ref(),
        state.settings.active_user_timeout_ms(),
        tz,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("Error fetching stats: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let sessions = match db::list_sessions(
        &state.pool,
        service_id,
        start,
        end,
        url_pattern.as_ref(),
        10,
        0,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("Error fetching sessions: {}", e);
            Vec::new()
        }
    };

    // Format start/end dates in user's timezone for the form inputs
    let start_local = start.with_timezone(&tz);
    let end_local = end.with_timezone(&tz);

    let template = ServiceDetailTemplate {
        service,
        stats,
        sessions,
        start_date: start_local.format("%Y-%m-%dT%H:%M").to_string(),
        end_date: end_local.format("%Y-%m-%dT%H:%M").to_string(),
        url_pattern: query.url_pattern.clone().unwrap_or_default(),
        results_limit: RESULTS_LIMIT,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

/// GET /service/:id/sessions
pub async fn session_list(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
    Query(query): Query<PaginationQuery>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid service ID").into_response(),
    };

    let service = match db::get_service(&state.pool, service_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            return (StatusCode::NOT_FOUND, "Service not found").into_response()
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let date_query = DateRangeQuery {
        start_date: query.start_date.clone(),
        end_date: query.end_date.clone(),
        url_pattern: query.url_pattern.clone(),
        tz: query.tz.clone(),
    };
    let (start, end, tz) = parse_date_range(&date_query);
    let url_pattern = parse_url_pattern(&query.url_pattern);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * PAGE_SIZE;

    let sessions = match db::list_sessions(
        &state.pool,
        service_id,
        start,
        end,
        url_pattern.as_ref(),
        PAGE_SIZE + 1,
        offset,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("Error fetching sessions: {}", e);
            Vec::new()
        }
    };

    let has_next = sessions.len() > PAGE_SIZE as usize;
    let sessions: Vec<_> = sessions
        .into_iter()
        .take(PAGE_SIZE as usize)
        .map(|s| SessionDisplay::from_session(s, tz))
        .collect();

    // Format start/end dates in user's timezone for the form inputs
    let start_local = start.with_timezone(&tz);
    let end_local = end.with_timezone(&tz);

    let template = SessionListTemplate {
        service,
        sessions,
        page,
        has_next,
        start_date: start_local.format("%Y-%m-%dT%H:%M").to_string(),
        end_date: end_local.format("%Y-%m-%dT%H:%M").to_string(),
        url_pattern: query.url_pattern.clone().unwrap_or_default(),
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

/// Query parameters for timezone
#[derive(Debug, Deserialize)]
pub struct TzQuery {
    pub tz: Option<String>,
}

/// GET /service/:id/sessions/:session_id
pub async fn session_detail(
    State(state): State<AppState>,
    Path((service_id, session_id)): Path<(String, String)>,
    Query(query): Query<TzQuery>,
) -> Response {
    let tz = parse_timezone(query.tz.as_deref());

    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid service ID").into_response(),
    };

    let session_id: SessionId = match session_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid session ID").into_response(),
    };

    let service = match db::get_service(&state.pool, service_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            return (StatusCode::NOT_FOUND, "Service not found").into_response()
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let session = match db::get_session(&state.pool, session_id).await {
        Ok(s) => s,
        Err(Error::SessionNotFound) => {
            return (StatusCode::NOT_FOUND, "Session not found").into_response()
        }
        Err(e) => {
            error!("Error fetching session: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let hits = match db::list_hits_for_session(&state.pool, session_id, 100, 0).await {
        Ok(h) => h,
        Err(e) => {
            error!("Error fetching hits: {}", e);
            Vec::new()
        }
    };

    // Convert to display structs with formatted timestamps
    let session_display = SessionDisplay::from_session(session, tz);
    let hits_display: Vec<HitDisplay> = hits
        .into_iter()
        .map(|h| HitDisplay::from_hit(h, tz))
        .collect();

    let template = SessionDetailTemplate {
        service,
        session: session_display,
        hits: hits_display,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

/// GET /service/:id/locations
pub async fn location_list(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid service ID").into_response(),
    };

    let service = match db::get_service(&state.pool, service_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            return (StatusCode::NOT_FOUND, "Service not found").into_response()
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let (start, end, tz) = parse_date_range(&query);
    let url_pattern = parse_url_pattern(&query.url_pattern);

    let hide_referrer_regex = if service.hide_referrer_regex.is_empty() {
        None
    } else {
        Regex::new(&service.hide_referrer_regex).ok()
    };

    let stats = match db::get_core_stats(
        &state.pool,
        service_id,
        start,
        end,
        hide_referrer_regex.as_ref(),
        url_pattern.as_ref(),
        state.settings.active_user_timeout_ms(),
        tz,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("Error fetching stats: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    // Format start/end dates in user's timezone for the form inputs
    let start_local = start.with_timezone(&tz);
    let end_local = end.with_timezone(&tz);

    let template = LocationListTemplate {
        service,
        locations: stats.locations,
        total_hits: stats.hit_count,
        start_date: start_local.format("%Y-%m-%dT%H:%M").to_string(),
        end_date: end_local.format("%Y-%m-%dT%H:%M").to_string(),
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

/// GET /service/new
pub async fn service_create_form() -> Response {
    let template = ServiceCreateTemplate {};

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

/// POST /service/new
pub async fn service_create(
    State(state): State<AppState>,
    Form(form): Form<ServiceForm>,
) -> Response {
    let input = CreateService {
        name: form.name,
        link: form.link.unwrap_or_default(),
        origins: form.origins.unwrap_or_else(|| "*".to_string()),
        respect_dnt: form.respect_dnt.is_some(),
        ignore_robots: form.ignore_robots.is_some(),
        collect_ips: form.collect_ips.is_some(),
        ignored_ips: form.ignored_ips.unwrap_or_default(),
        hide_referrer_regex: form.hide_referrer_regex.unwrap_or_default(),
        script_inject: form.script_inject.unwrap_or_default(),
    };

    match db::create_service(&state.pool, input).await {
        Ok(service) => Redirect::to(&format!("/service/{}", service.id)).into_response(),
        Err(e) => {
            error!("Error creating service: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create service",
            )
                .into_response()
        }
    }
}

/// GET /service/:id/manage
pub async fn service_update_form(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid service ID").into_response(),
    };

    let service = match db::get_service(&state.pool, service_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            return (StatusCode::NOT_FOUND, "Service not found").into_response()
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let template = ServiceUpdateTemplate { service };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

/// POST /service/:id/manage
pub async fn service_update(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
    Form(form): Form<ServiceForm>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid service ID").into_response(),
    };

    let input = UpdateService {
        name: Some(form.name),
        link: form.link,
        origins: form.origins,
        status: None,
        respect_dnt: Some(form.respect_dnt.is_some()),
        ignore_robots: Some(form.ignore_robots.is_some()),
        collect_ips: Some(form.collect_ips.is_some()),
        ignored_ips: form.ignored_ips,
        hide_referrer_regex: form.hide_referrer_regex,
        script_inject: form.script_inject,
    };

    match db::update_service(&state.pool, service_id, input).await {
        Ok(_) => {
            // Invalidate cache
            state.cache.invalidate_service(service_id).await;
            Redirect::to(&format!("/service/{}", service_id)).into_response()
        }
        Err(e) => {
            error!("Error updating service: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update service",
            )
                .into_response()
        }
    }
}

/// GET /service/:id/delete
pub async fn service_delete_form(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid service ID").into_response(),
    };

    let service = match db::get_service(&state.pool, service_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            return (StatusCode::NOT_FOUND, "Service not found").into_response()
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let template = ServiceDeleteTemplate { service };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

/// POST /service/:id/delete
pub async fn service_delete(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid service ID").into_response(),
    };

    match db::delete_service(&state.pool, service_id).await {
        Ok(_) => {
            state.cache.invalidate_service(service_id).await;
            Redirect::to("/").into_response()
        }
        Err(e) => {
            error!("Error deleting service: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete service",
            )
                .into_response()
        }
    }
}

/// GET /service/:id/stats (HTMX partial)
pub async fn stats_partial(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let service_id: ServiceId = match service_id.parse() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid service ID").into_response(),
    };

    let service = match db::get_service(&state.pool, service_id).await {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, "Service not found").into_response(),
    };

    let (start, end, tz) = parse_date_range(&query);
    let url_pattern = parse_url_pattern(&query.url_pattern);

    let hide_referrer_regex = if service.hide_referrer_regex.is_empty() {
        None
    } else {
        Regex::new(&service.hide_referrer_regex).ok()
    };

    let stats = match db::get_core_stats(
        &state.pool,
        service_id,
        start,
        end,
        hide_referrer_regex.as_ref(),
        url_pattern.as_ref(),
        state.settings.active_user_timeout_ms(),
        tz,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("Error fetching stats: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let template = StatsPartialTemplate {
        stats,
        service_id: service_id.0.to_string(),
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}
