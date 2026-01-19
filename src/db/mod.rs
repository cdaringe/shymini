use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use std::collections::HashMap;

use crate::domain::{
    ChartData, CoreStats, CountedItem, CreateHit, CreateService, CreateSession, DeviceType, Hit,
    HitId, Service, ServiceId, ServiceStatus, Session, SessionId, TrackerType, TrackingId,
    UpdateService,
};
use crate::error::{Error, Result};

#[cfg(feature = "postgres")]
pub type Pool = sqlx::PgPool;
#[cfg(feature = "postgres")]
pub type PoolOptions = sqlx::postgres::PgPoolOptions;

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
pub type Pool = sqlx::SqlitePool;
#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
pub type PoolOptions = sqlx::sqlite::SqlitePoolOptions;

const RESULTS_LIMIT: i64 = 300;

pub async fn create_pool(url: &str) -> Result<Pool> {
    let pool = PoolOptions::new().max_connections(10).connect(url).await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &Pool) -> Result<()> {
    #[cfg(feature = "postgres")]
    {
        let sql = include_str!("../../migrations/postgres/001_initial.sql");
        sqlx::raw_sql(sql).execute(pool).await?;

        // Check if tracking_id column already exists
        let has_tracking_id: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'services' AND column_name = 'tracking_id')"
        )
        .fetch_one(pool)
        .await?;

        if !has_tracking_id {
            let sql = include_str!("../../migrations/postgres/002_tracking_id.sql");
            sqlx::raw_sql(sql).execute(pool).await?;
        }
    }

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    {
        let sql = include_str!("../../migrations/sqlite/001_initial.sql");
        sqlx::raw_sql(sql).execute(pool).await?;

        // Check if tracking_id column already exists
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('services') WHERE name = 'tracking_id'",
        )
        .fetch_all(pool)
        .await?;

        if columns.is_empty() {
            let sql = include_str!("../../migrations/sqlite/002_tracking_id.sql");
            sqlx::raw_sql(sql).execute(pool).await?;
        }
    }

    Ok(())
}

// Service queries
pub async fn get_service(pool: &Pool, id: ServiceId) -> Result<Service> {
    #[cfg(feature = "postgres")]
    let row: ServiceRow = sqlx::query_as(
        r#"SELECT id, tracking_id, name, link, origins, status, respect_dnt, ignore_robots,
           collect_ips, ignored_ips, hide_referrer_regex, script_inject, created_at
           FROM services WHERE id = $1"#,
    )
    .bind(id.0)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::ServiceNotFound)?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let row: ServiceRow = sqlx::query_as(
        r#"SELECT id, tracking_id, name, link, origins, status, respect_dnt, ignore_robots,
           collect_ips, ignored_ips, hide_referrer_regex, script_inject, created_at
           FROM services WHERE id = ?"#,
    )
    .bind(id.0.to_string())
    .fetch_optional(pool)
    .await?
    .ok_or(Error::ServiceNotFound)?;

    // Check if tracking_id needs to be generated and persisted BEFORE converting
    let needs_backfill = row.tracking_id.is_none();
    let mut service: Service = row.into();

    if needs_backfill {
        let new_tracking_id = TrackingId::new();
        backfill_tracking_id(pool, id, &new_tracking_id).await?;
        service.tracking_id = new_tracking_id;
    }

    Ok(service)
}

/// Persist a tracking_id for a service that doesn't have one
async fn backfill_tracking_id(pool: &Pool, id: ServiceId, tracking_id: &TrackingId) -> Result<()> {
    #[cfg(feature = "postgres")]
    sqlx::query("UPDATE services SET tracking_id = $1 WHERE id = $2 AND tracking_id IS NULL")
        .bind(&tracking_id.0)
        .bind(id.0)
        .execute(pool)
        .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    sqlx::query("UPDATE services SET tracking_id = ? WHERE id = ? AND tracking_id IS NULL")
        .bind(&tracking_id.0)
        .bind(id.0.to_string())
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn get_active_service(pool: &Pool, id: ServiceId) -> Result<Service> {
    let service = get_service(pool, id).await?;
    if service.status != ServiceStatus::Active {
        return Err(Error::ServiceNotFound);
    }
    Ok(service)
}

pub async fn get_service_by_tracking_id(pool: &Pool, tracking_id: &str) -> Result<Service> {
    #[cfg(feature = "postgres")]
    let row: ServiceRow = sqlx::query_as(
        r#"SELECT id, tracking_id, name, link, origins, status, respect_dnt, ignore_robots,
           collect_ips, ignored_ips, hide_referrer_regex, script_inject, created_at
           FROM services WHERE tracking_id = $1"#,
    )
    .bind(tracking_id)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::ServiceNotFound)?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let row: ServiceRow = sqlx::query_as(
        r#"SELECT id, tracking_id, name, link, origins, status, respect_dnt, ignore_robots,
           collect_ips, ignored_ips, hide_referrer_regex, script_inject, created_at
           FROM services WHERE tracking_id = ?"#,
    )
    .bind(tracking_id)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::ServiceNotFound)?;

    Ok(row.into())
}

pub async fn get_active_service_by_tracking_id(pool: &Pool, tracking_id: &str) -> Result<Service> {
    let service = get_service_by_tracking_id(pool, tracking_id).await?;
    if service.status != ServiceStatus::Active {
        return Err(Error::ServiceNotFound);
    }
    Ok(service)
}

pub async fn list_services(pool: &Pool) -> Result<Vec<Service>> {
    #[cfg(feature = "postgres")]
    let rows: Vec<ServiceRow> = sqlx::query_as(
        r#"SELECT id, tracking_id, name, link, origins, status, respect_dnt, ignore_robots,
           collect_ips, ignored_ips, hide_referrer_regex, script_inject, created_at
           FROM services ORDER BY name, id"#,
    )
    .fetch_all(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let rows: Vec<ServiceRow> = sqlx::query_as(
        r#"SELECT id, tracking_id, name, link, origins, status, respect_dnt, ignore_robots,
           collect_ips, ignored_ips, hide_referrer_regex, script_inject, created_at
           FROM services ORDER BY name, id"#,
    )
    .fetch_all(pool)
    .await?;

    // Backfill tracking_ids for services that don't have one
    let mut services = Vec::with_capacity(rows.len());
    for row in rows {
        let needs_backfill = row.tracking_id.is_none();

        #[cfg(feature = "postgres")]
        let service_id = ServiceId(row.id);
        #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
        let service_id = ServiceId(row.id.parse().unwrap_or_default());

        let mut service: Service = row.into();

        if needs_backfill {
            let new_tracking_id = TrackingId::new();
            let _ = backfill_tracking_id(pool, service_id, &new_tracking_id).await;
            service.tracking_id = new_tracking_id;
        }

        services.push(service);
    }

    Ok(services)
}

pub async fn create_service(pool: &Pool, input: CreateService) -> Result<Service> {
    let id = ServiceId::new();
    let tracking_id = TrackingId::new();
    let now = Utc::now();

    #[cfg(feature = "postgres")]
    sqlx::query(
        r#"INSERT INTO services (id, tracking_id, name, link, origins, respect_dnt, ignore_robots,
           collect_ips, ignored_ips, hide_referrer_regex, script_inject, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
    )
    .bind(id.0)
    .bind(&tracking_id.0)
    .bind(&input.name)
    .bind(&input.link)
    .bind(&input.origins)
    .bind(input.respect_dnt)
    .bind(input.ignore_robots)
    .bind(input.collect_ips)
    .bind(&input.ignored_ips)
    .bind(&input.hide_referrer_regex)
    .bind(&input.script_inject)
    .bind(now)
    .execute(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    sqlx::query(
        r#"INSERT INTO services (id, tracking_id, name, link, origins, respect_dnt, ignore_robots,
           collect_ips, ignored_ips, hide_referrer_regex, script_inject, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(id.0.to_string())
    .bind(&tracking_id.0)
    .bind(&input.name)
    .bind(&input.link)
    .bind(&input.origins)
    .bind(input.respect_dnt)
    .bind(input.ignore_robots)
    .bind(input.collect_ips)
    .bind(&input.ignored_ips)
    .bind(&input.hide_referrer_regex)
    .bind(&input.script_inject)
    .bind(now.to_rfc3339())
    .execute(pool)
    .await?;

    get_service(pool, id).await
}

pub async fn update_service(pool: &Pool, id: ServiceId, input: UpdateService) -> Result<Service> {
    let service = get_service(pool, id).await?;

    let name = input.name.unwrap_or(service.name);
    let link = input.link.unwrap_or(service.link);
    let origins = input.origins.unwrap_or(service.origins);
    let status = input.status.unwrap_or(service.status);
    let respect_dnt = input.respect_dnt.unwrap_or(service.respect_dnt);
    let ignore_robots = input.ignore_robots.unwrap_or(service.ignore_robots);
    let collect_ips = input.collect_ips.unwrap_or(service.collect_ips);
    let ignored_ips = input.ignored_ips.unwrap_or(service.ignored_ips);
    let hide_referrer_regex = input
        .hide_referrer_regex
        .unwrap_or(service.hide_referrer_regex);
    let script_inject = input.script_inject.unwrap_or(service.script_inject);

    #[cfg(feature = "postgres")]
    sqlx::query(
        r#"UPDATE services SET name = $1, link = $2, origins = $3, status = $4,
           respect_dnt = $5, ignore_robots = $6, collect_ips = $7, ignored_ips = $8,
           hide_referrer_regex = $9, script_inject = $10
           WHERE id = $11"#,
    )
    .bind(&name)
    .bind(&link)
    .bind(&origins)
    .bind(status.as_str())
    .bind(respect_dnt)
    .bind(ignore_robots)
    .bind(collect_ips)
    .bind(&ignored_ips)
    .bind(&hide_referrer_regex)
    .bind(&script_inject)
    .bind(id.0)
    .execute(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    sqlx::query(
        r#"UPDATE services SET name = ?, link = ?, origins = ?, status = ?,
           respect_dnt = ?, ignore_robots = ?, collect_ips = ?, ignored_ips = ?,
           hide_referrer_regex = ?, script_inject = ?
           WHERE id = ?"#,
    )
    .bind(&name)
    .bind(&link)
    .bind(&origins)
    .bind(status.as_str())
    .bind(respect_dnt)
    .bind(ignore_robots)
    .bind(collect_ips)
    .bind(&ignored_ips)
    .bind(&hide_referrer_regex)
    .bind(&script_inject)
    .bind(id.0.to_string())
    .execute(pool)
    .await?;

    get_service(pool, id).await
}

pub async fn delete_service(pool: &Pool, id: ServiceId) -> Result<()> {
    #[cfg(feature = "postgres")]
    sqlx::query("DELETE FROM services WHERE id = $1")
        .bind(id.0)
        .execute(pool)
        .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    sqlx::query("DELETE FROM services WHERE id = ?")
        .bind(id.0.to_string())
        .execute(pool)
        .await?;

    Ok(())
}

// Session queries
pub async fn get_session(pool: &Pool, id: SessionId) -> Result<Session> {
    #[cfg(feature = "postgres")]
    let row: SessionRow = sqlx::query_as(
        r#"SELECT id, service_id, identifier, start_time, last_seen, user_agent,
           browser, device, device_type, os, ip::TEXT, asn, country, longitude,
           latitude, time_zone, is_bounce
           FROM sessions WHERE id = $1"#,
    )
    .bind(id.0)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::SessionNotFound)?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let row: SessionRow = sqlx::query_as(
        r#"SELECT id, service_id, identifier, start_time, last_seen, user_agent,
           browser, device, device_type, os, ip, asn, country, longitude,
           latitude, time_zone, is_bounce
           FROM sessions WHERE id = ?"#,
    )
    .bind(id.0.to_string())
    .fetch_optional(pool)
    .await?
    .ok_or(Error::SessionNotFound)?;

    Ok(row.into())
}

pub async fn create_session(pool: &Pool, input: CreateSession) -> Result<Session> {
    let id = SessionId::new();

    #[cfg(feature = "postgres")]
    {
        // Use a query that casts the IP string to INET type
        sqlx::query(
            r#"INSERT INTO sessions (id, service_id, identifier, start_time, last_seen,
               user_agent, browser, device, device_type, os, ip, asn, country,
               longitude, latitude, time_zone, is_bounce)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::INET, $12, $13, $14, $15, $16, $17)"#
        )
        .bind(id.0)
        .bind(input.service_id.0)
        .bind(&input.identifier)
        .bind(input.start_time)
        .bind(input.start_time)
        .bind(&input.user_agent)
        .bind(&input.browser)
        .bind(&input.device)
        .bind(input.device_type.as_str())
        .bind(&input.os)
        .bind(&input.ip)  // Pass as string, cast in query
        .bind(&input.asn)
        .bind(&input.country)
        .bind(input.longitude)
        .bind(input.latitude)
        .bind(&input.time_zone)
        .bind(true)
        .execute(pool)
        .await?;
    }

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    sqlx::query(
        r#"INSERT INTO sessions (id, service_id, identifier, start_time, last_seen,
           user_agent, browser, device, device_type, os, ip, asn, country,
           longitude, latitude, time_zone, is_bounce)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(id.0.to_string())
    .bind(input.service_id.0.to_string())
    .bind(&input.identifier)
    .bind(input.start_time.to_rfc3339())
    .bind(input.start_time.to_rfc3339())
    .bind(&input.user_agent)
    .bind(&input.browser)
    .bind(&input.device)
    .bind(input.device_type.as_str())
    .bind(&input.os)
    .bind(&input.ip)
    .bind(&input.asn)
    .bind(&input.country)
    .bind(input.longitude)
    .bind(input.latitude)
    .bind(&input.time_zone)
    .bind(true)
    .execute(pool)
    .await?;

    get_session(pool, id).await
}

pub async fn update_session_last_seen(
    pool: &Pool,
    id: SessionId,
    last_seen: DateTime<Utc>,
) -> Result<()> {
    #[cfg(feature = "postgres")]
    sqlx::query("UPDATE sessions SET last_seen = $1 WHERE id = $2")
        .bind(last_seen)
        .bind(id.0)
        .execute(pool)
        .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    sqlx::query("UPDATE sessions SET last_seen = ? WHERE id = ?")
        .bind(last_seen.to_rfc3339())
        .bind(id.0.to_string())
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_session_identifier(pool: &Pool, id: SessionId, identifier: &str) -> Result<()> {
    #[cfg(feature = "postgres")]
    sqlx::query("UPDATE sessions SET identifier = $1 WHERE id = $2")
        .bind(identifier)
        .bind(id.0)
        .execute(pool)
        .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    sqlx::query("UPDATE sessions SET identifier = ? WHERE id = ?")
        .bind(identifier)
        .bind(id.0.to_string())
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn recalculate_session_bounce(pool: &Pool, session_id: SessionId) -> Result<()> {
    #[cfg(feature = "postgres")]
    {
        let hit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hits WHERE session_id = $1")
            .bind(session_id.0)
            .fetch_one(pool)
            .await?;

        let is_bounce = hit_count <= 1;
        sqlx::query("UPDATE sessions SET is_bounce = $1 WHERE id = $2")
            .bind(is_bounce)
            .bind(session_id.0)
            .execute(pool)
            .await?;
    }

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    {
        let hit_count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM hits WHERE session_id = ?")
            .bind(session_id.0.to_string())
            .fetch_one(pool)
            .await?;

        let is_bounce = hit_count <= 1;
        sqlx::query("UPDATE sessions SET is_bounce = ? WHERE id = ?")
            .bind(is_bounce)
            .bind(session_id.0.to_string())
            .execute(pool)
            .await?;
    }

    Ok(())
}

pub async fn list_sessions(
    pool: &Pool,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    url_pattern: Option<&Regex>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Session>> {
    // If URL pattern is provided, we need to filter sessions that have matching hits
    if let Some(pattern) = url_pattern {
        return list_sessions_with_url_filter(pool, service_id, start, end, pattern, limit, offset)
            .await;
    }

    #[cfg(feature = "postgres")]
    let rows: Vec<SessionRow> = sqlx::query_as(
        r#"SELECT id, service_id, identifier, start_time, last_seen, user_agent,
           browser, device, device_type, os, ip::TEXT, asn, country, longitude,
           latitude, time_zone, is_bounce
           FROM sessions
           WHERE service_id = $1 AND start_time >= $2 AND start_time < $3
           ORDER BY start_time DESC
           LIMIT $4 OFFSET $5"#,
    )
    .bind(service_id.0)
    .bind(start)
    .bind(end)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let rows: Vec<SessionRow> = sqlx::query_as(
        r#"SELECT id, service_id, identifier, start_time, last_seen, user_agent,
           browser, device, device_type, os, ip, asn, country, longitude,
           latitude, time_zone, is_bounce
           FROM sessions
           WHERE service_id = ? AND start_time >= ? AND start_time < ?
           ORDER BY start_time DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(service_id.0.to_string())
    .bind(start.to_rfc3339())
    .bind(end.to_rfc3339())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

async fn list_sessions_with_url_filter(
    pool: &Pool,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    url_pattern: &Regex,
    limit: i64,
    offset: i64,
) -> Result<Vec<Session>> {
    // Get session IDs that have hits matching the URL pattern
    #[cfg(feature = "postgres")]
    let session_ids: Vec<(uuid::Uuid,)> = sqlx::query_as(
        r#"SELECT DISTINCT session_id FROM hits
           WHERE service_id = $1 AND start_time >= $2 AND start_time < $3"#,
    )
    .bind(service_id.0)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let session_ids: Vec<(String,)> = sqlx::query_as(
        r#"SELECT DISTINCT session_id FROM hits
           WHERE service_id = ? AND start_time >= ? AND start_time < ?"#,
    )
    .bind(service_id.0.to_string())
    .bind(start.to_rfc3339())
    .bind(end.to_rfc3339())
    .fetch_all(pool)
    .await?;

    // For each session, check if it has hits matching the pattern
    let mut matching_session_ids = Vec::new();
    for (session_id,) in session_ids {
        #[cfg(feature = "postgres")]
        let hits: Vec<(String,)> =
            sqlx::query_as("SELECT location FROM hits WHERE session_id = $1")
                .bind(session_id)
                .fetch_all(pool)
                .await?;

        #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
        let hits: Vec<(String,)> = sqlx::query_as("SELECT location FROM hits WHERE session_id = ?")
            .bind(&session_id)
            .fetch_all(pool)
            .await?;

        if hits.iter().any(|(loc,)| url_pattern.is_match(loc)) {
            matching_session_ids.push(session_id);
        }
    }

    if matching_session_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Fetch the actual sessions with pagination
    // We need to do pagination in Rust since we filtered in-memory
    let skip = offset as usize;
    let take = limit as usize;
    let paginated_ids: Vec<_> = matching_session_ids
        .into_iter()
        .skip(skip)
        .take(take)
        .collect();

    if paginated_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for session_id in paginated_ids {
        #[cfg(feature = "postgres")]
        let row: Option<SessionRow> = sqlx::query_as(
            r#"SELECT id, service_id, identifier, start_time, last_seen, user_agent,
               browser, device, device_type, os, ip::TEXT, asn, country, longitude,
               latitude, time_zone, is_bounce
               FROM sessions WHERE id = $1"#,
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

        #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
        let row: Option<SessionRow> = sqlx::query_as(
            r#"SELECT id, service_id, identifier, start_time, last_seen, user_agent,
               browser, device, device_type, os, ip, asn, country, longitude,
               latitude, time_zone, is_bounce
               FROM sessions WHERE id = ?"#,
        )
        .bind(&session_id)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            sessions.push(row.into());
        }
    }

    // Sort by start_time DESC
    sessions.sort_by(|a: &Session, b: &Session| b.start_time.cmp(&a.start_time));

    Ok(sessions)
}

// Hit queries
pub async fn get_hit(pool: &Pool, id: HitId) -> Result<Hit> {
    #[cfg(feature = "postgres")]
    let row: HitRow = sqlx::query_as(
        r#"SELECT id, session_id, service_id, initial, start_time, last_seen,
           heartbeats, tracker, location, referrer, load_time
           FROM hits WHERE id = $1"#,
    )
    .bind(id.0)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::Internal("Hit not found".to_string()))?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let row: HitRow = sqlx::query_as(
        r#"SELECT id, session_id, service_id, initial, start_time, last_seen,
           heartbeats, tracker, location, referrer, load_time
           FROM hits WHERE id = ?"#,
    )
    .bind(id.0)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::Internal("Hit not found".to_string()))?;

    Ok(row.into())
}

pub async fn create_hit(pool: &Pool, input: CreateHit) -> Result<Hit> {
    #[cfg(feature = "postgres")]
    let id: i64 = sqlx::query_scalar(
        r#"INSERT INTO hits (session_id, service_id, initial, start_time, last_seen,
           heartbeats, tracker, location, referrer, load_time)
           VALUES ($1, $2, $3, $4, $5, 0, $6, $7, $8, $9)
           RETURNING id"#,
    )
    .bind(input.session_id.0)
    .bind(input.service_id.0)
    .bind(input.initial)
    .bind(input.start_time)
    .bind(input.start_time)
    .bind(input.tracker.as_str())
    .bind(&input.location)
    .bind(&input.referrer)
    .bind(input.load_time)
    .fetch_one(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let id: i64 = {
        sqlx::query(
            r#"INSERT INTO hits (session_id, service_id, initial, start_time, last_seen,
               heartbeats, tracker, location, referrer, load_time)
               VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?, ?)"#,
        )
        .bind(input.session_id.0.to_string())
        .bind(input.service_id.0.to_string())
        .bind(input.initial)
        .bind(input.start_time.to_rfc3339())
        .bind(input.start_time.to_rfc3339())
        .bind(input.tracker.as_str())
        .bind(&input.location)
        .bind(&input.referrer)
        .bind(input.load_time)
        .execute(pool)
        .await?;

        sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
            .fetch_one(pool)
            .await?
    };

    get_hit(pool, HitId(id)).await
}

pub async fn update_hit_heartbeat(pool: &Pool, id: HitId, last_seen: DateTime<Utc>) -> Result<()> {
    #[cfg(feature = "postgres")]
    sqlx::query("UPDATE hits SET heartbeats = heartbeats + 1, last_seen = $1 WHERE id = $2")
        .bind(last_seen)
        .bind(id.0)
        .execute(pool)
        .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    sqlx::query("UPDATE hits SET heartbeats = heartbeats + 1, last_seen = ? WHERE id = ?")
        .bind(last_seen.to_rfc3339())
        .bind(id.0)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_hits_for_session(
    pool: &Pool,
    session_id: SessionId,
    limit: i64,
    offset: i64,
) -> Result<Vec<Hit>> {
    #[cfg(feature = "postgres")]
    let rows: Vec<HitRow> = sqlx::query_as(
        r#"SELECT id, session_id, service_id, initial, start_time, last_seen,
           heartbeats, tracker, location, referrer, load_time
           FROM hits WHERE session_id = $1
           ORDER BY start_time DESC
           LIMIT $2 OFFSET $3"#,
    )
    .bind(session_id.0)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let rows: Vec<HitRow> = sqlx::query_as(
        r#"SELECT id, session_id, service_id, initial, start_time, last_seen,
           heartbeats, tracker, location, referrer, load_time
           FROM hits WHERE session_id = ?
           ORDER BY start_time DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(session_id.0.to_string())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

// Stats queries
pub async fn get_core_stats(
    pool: &Pool,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    hide_referrer_regex: Option<&Regex>,
    url_pattern: Option<&Regex>,
    active_user_timeout_ms: u64,
) -> Result<CoreStats> {
    let main_stats = get_relative_stats(
        pool,
        service_id,
        start,
        end,
        hide_referrer_regex,
        url_pattern,
        active_user_timeout_ms,
    )
    .await?;

    let duration = end - start;
    let compare_start = start - duration;
    let compare_stats = get_relative_stats(
        pool,
        service_id,
        compare_start,
        start,
        hide_referrer_regex,
        url_pattern,
        active_user_timeout_ms,
    )
    .await?;

    Ok(CoreStats {
        compare: Some(Box::new(compare_stats)),
        ..main_stats
    })
}

async fn get_relative_stats(
    pool: &Pool,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    hide_referrer_regex: Option<&Regex>,
    url_pattern: Option<&Regex>,
    active_user_timeout_ms: u64,
) -> Result<CoreStats> {
    // If URL pattern is provided, use filtered stats
    if let Some(pattern) = url_pattern {
        return get_relative_stats_with_url_filter(
            pool,
            service_id,
            start,
            end,
            hide_referrer_regex,
            pattern,
            active_user_timeout_ms,
        )
        .await;
    }

    let now = Utc::now();
    let active_cutoff = now - Duration::milliseconds(active_user_timeout_ms as i64);

    // Currently online count
    #[cfg(feature = "postgres")]
    let currently_online: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE service_id = $1 AND last_seen > $2",
    )
    .bind(service_id.0)
    .bind(active_cutoff)
    .fetch_one(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let currently_online: i64 = {
        let count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND last_seen > ?",
        )
        .bind(service_id.0.to_string())
        .bind(active_cutoff.to_rfc3339())
        .fetch_one(pool)
        .await?;
        count as i64
    };

    // Session count
    #[cfg(feature = "postgres")]
    let session_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE service_id = $1 AND start_time >= $2 AND start_time < $3"
    )
    .bind(service_id.0)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let session_count: i64 = {
        let count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ?"
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_one(pool)
        .await?;
        count as i64
    };

    // Hit count
    #[cfg(feature = "postgres")]
    let hit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM hits WHERE service_id = $1 AND start_time >= $2 AND start_time < $3",
    )
    .bind(service_id.0)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let hit_count: i64 = {
        let count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ?",
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_one(pool)
        .await?;
        count as i64
    };

    // Has any hits ever
    #[cfg(feature = "postgres")]
    let has_hits: bool = {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hits WHERE service_id = $1 LIMIT 1")
                .bind(service_id.0)
                .fetch_one(pool)
                .await?;
        count > 0
    };

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let has_hits: bool = {
        let count: i32 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hits WHERE service_id = ? LIMIT 1")
                .bind(service_id.0.to_string())
                .fetch_one(pool)
                .await?;
        count > 0
    };

    // Bounce count
    #[cfg(feature = "postgres")]
    let bounce_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE service_id = $1 AND start_time >= $2 AND start_time < $3 AND is_bounce = true"
    )
    .bind(service_id.0)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let bounce_count: i64 = {
        let count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ? AND is_bounce = 1"
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_one(pool)
        .await?;
        count as i64
    };

    let bounce_rate_pct = if session_count > 0 {
        Some(((bounce_count as f64 / session_count as f64) * 1000.0).round() / 10.0)
    } else {
        None
    };

    // Average load time
    #[cfg(feature = "postgres")]
    let avg_load_time: Option<f64> = {
        let raw: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(load_time) FROM hits WHERE service_id = $1 AND start_time >= $2 AND start_time < $3 AND load_time IS NOT NULL"
        )
        .bind(service_id.0)
        .bind(start)
        .bind(end)
        .fetch_one(pool)
        .await?;
        raw.map(|v| v.round())
    };

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let avg_load_time: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(load_time) FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ? AND load_time IS NOT NULL"
    )
    .bind(service_id.0.to_string())
    .bind(start.to_rfc3339())
    .bind(end.to_rfc3339())
    .fetch_one(pool)
    .await?;

    // Round avg_load_time to integer
    let avg_load_time = avg_load_time.map(|v| v.round());

    let avg_hits_per_session = if session_count > 0 {
        Some(((hit_count as f64 / session_count as f64) * 10.0).round() / 10.0)
    } else {
        None
    };

    // Average session duration (in seconds)
    #[cfg(feature = "postgres")]
    let avg_session_duration: Option<f64> = {
        let raw: Option<f64> = sqlx::query_scalar(
            r#"SELECT AVG(EXTRACT(EPOCH FROM (last_seen - start_time)))
               FROM sessions WHERE service_id = $1 AND start_time >= $2 AND start_time < $3"#,
        )
        .bind(service_id.0)
        .bind(start)
        .bind(end)
        .fetch_one(pool)
        .await?;
        raw.map(|v| v.round())
    };

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let avg_session_duration: Option<f64> = {
        // SQLite doesn't have easy date arithmetic, compute manually
        let durations: Vec<(String, String)> = sqlx::query_as(
            "SELECT start_time, last_seen FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ?"
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_all(pool)
        .await?;

        if durations.is_empty() {
            None
        } else {
            let total: f64 = durations
                .iter()
                .filter_map(|(s, e)| {
                    let start_dt = DateTime::parse_from_rfc3339(s).ok()?.with_timezone(&Utc);
                    let end_dt = DateTime::parse_from_rfc3339(e).ok()?.with_timezone(&Utc);
                    Some((end_dt - start_dt).num_seconds() as f64)
                })
                .sum();
            Some((total / durations.len() as f64).round())
        }
    };

    // Locations (top pages)
    let locations = get_counted_field(
        pool,
        "hits",
        "location",
        service_id,
        start,
        end,
        RESULTS_LIMIT,
    )
    .await?;

    // Referrers (filter by regex if provided)
    let mut referrers = get_counted_field_initial(
        pool,
        "hits",
        "referrer",
        service_id,
        start,
        end,
        RESULTS_LIMIT,
    )
    .await?;

    if let Some(regex) = hide_referrer_regex {
        referrers.retain(|r| !regex.is_match(&r.value));
    }

    // Countries
    let countries = get_counted_field(
        pool,
        "sessions",
        "country",
        service_id,
        start,
        end,
        RESULTS_LIMIT,
    )
    .await?;

    // Operating systems
    let operating_systems = get_counted_field(
        pool,
        "sessions",
        "os",
        service_id,
        start,
        end,
        RESULTS_LIMIT,
    )
    .await?;

    // Browsers
    let browsers = get_counted_field(
        pool,
        "sessions",
        "browser",
        service_id,
        start,
        end,
        RESULTS_LIMIT,
    )
    .await?;

    // Devices
    let devices = get_counted_field(
        pool,
        "sessions",
        "device",
        service_id,
        start,
        end,
        RESULTS_LIMIT,
    )
    .await?;

    // Device types
    let device_types = get_counted_field(
        pool,
        "sessions",
        "device_type",
        service_id,
        start,
        end,
        RESULTS_LIMIT,
    )
    .await?;

    // Chart data
    let (chart_data, chart_tooltip_format, chart_granularity) =
        get_chart_data(pool, service_id, start, end, now).await?;

    Ok(CoreStats {
        currently_online,
        session_count,
        hit_count,
        has_hits,
        bounce_rate_pct,
        avg_session_duration,
        avg_load_time,
        avg_hits_per_session,
        locations,
        referrers,
        countries,
        operating_systems,
        browsers,
        devices,
        device_types,
        chart_data,
        chart_tooltip_format,
        chart_granularity,
        compare: None,
    })
}

async fn get_relative_stats_with_url_filter(
    pool: &Pool,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    hide_referrer_regex: Option<&Regex>,
    url_pattern: &Regex,
    active_user_timeout_ms: u64,
) -> Result<CoreStats> {
    let now = Utc::now();
    let active_cutoff = now - Duration::milliseconds(active_user_timeout_ms as i64);

    // Get all hits in the date range
    #[cfg(feature = "postgres")]
    let all_hits: Vec<(
        i64,
        uuid::Uuid,
        String,
        Option<f64>,
        bool,
        String,
        DateTime<Utc>,
    )> = sqlx::query_as(
        r#"SELECT id, session_id, location, load_time, initial, referrer, start_time
           FROM hits WHERE service_id = $1 AND start_time >= $2 AND start_time < $3"#,
    )
    .bind(service_id.0)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let all_hits: Vec<(i64, String, String, Option<f64>, bool, String, String)> = sqlx::query_as(
        r#"SELECT id, session_id, location, load_time, initial, referrer, start_time
           FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ?"#,
    )
    .bind(service_id.0.to_string())
    .bind(start.to_rfc3339())
    .bind(end.to_rfc3339())
    .fetch_all(pool)
    .await?;

    // Filter hits by URL pattern
    let filtered_hits: Vec<_> = all_hits
        .into_iter()
        .filter(|(_, _, location, _, _, _, _)| url_pattern.is_match(location))
        .collect();

    let hit_count = filtered_hits.len() as i64;

    // Get unique session IDs from filtered hits
    #[cfg(feature = "postgres")]
    let matching_session_ids: std::collections::HashSet<uuid::Uuid> = filtered_hits
        .iter()
        .map(|(_, session_id, _, _, _, _, _)| *session_id)
        .collect();

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let matching_session_ids: std::collections::HashSet<String> = filtered_hits
        .iter()
        .map(|(_, session_id, _, _, _, _, _)| session_id.clone())
        .collect();

    let session_count = matching_session_ids.len() as i64;

    // Has any hits ever (unfiltered)
    #[cfg(feature = "postgres")]
    let has_hits: bool = {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hits WHERE service_id = $1 LIMIT 1")
                .bind(service_id.0)
                .fetch_one(pool)
                .await?;
        count > 0
    };

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let has_hits: bool = {
        let count: i32 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hits WHERE service_id = ? LIMIT 1")
                .bind(service_id.0.to_string())
                .fetch_one(pool)
                .await?;
        count > 0
    };

    // Calculate filtered stats
    let avg_load_time: Option<f64> = {
        let load_times: Vec<f64> = filtered_hits
            .iter()
            .filter_map(|(_, _, _, load_time, _, _, _)| *load_time)
            .collect();
        if load_times.is_empty() {
            None
        } else {
            Some((load_times.iter().sum::<f64>() / load_times.len() as f64).round())
        }
    };

    let avg_hits_per_session = if session_count > 0 {
        Some(((hit_count as f64 / session_count as f64) * 10.0).round() / 10.0)
    } else {
        None
    };

    // Count locations from filtered hits
    let mut location_counts: HashMap<String, i64> = HashMap::new();
    for (_, _, location, _, _, _, _) in &filtered_hits {
        *location_counts.entry(location.clone()).or_insert(0) += 1;
    }
    let mut locations: Vec<CountedItem> = location_counts
        .into_iter()
        .map(|(value, count)| CountedItem { value, count })
        .collect();
    locations.sort_by(|a, b| b.count.cmp(&a.count));
    locations.truncate(RESULTS_LIMIT as usize);

    // Count referrers from filtered initial hits
    let mut referrer_counts: HashMap<String, i64> = HashMap::new();
    for (_, _, _, _, initial, referrer, _) in &filtered_hits {
        if *initial {
            *referrer_counts.entry(referrer.clone()).or_insert(0) += 1;
        }
    }
    let mut referrers: Vec<CountedItem> = referrer_counts
        .into_iter()
        .map(|(value, count)| CountedItem { value, count })
        .collect();
    if let Some(regex) = hide_referrer_regex {
        referrers.retain(|r| !regex.is_match(&r.value));
    }
    referrers.sort_by(|a, b| b.count.cmp(&a.count));
    referrers.truncate(RESULTS_LIMIT as usize);

    // Get session data for matching sessions to compute other stats
    let mut countries: HashMap<String, i64> = HashMap::new();
    let mut operating_systems: HashMap<String, i64> = HashMap::new();
    let mut browsers: HashMap<String, i64> = HashMap::new();
    let mut devices: HashMap<String, i64> = HashMap::new();
    let mut device_types: HashMap<String, i64> = HashMap::new();
    let mut bounce_count: i64 = 0;
    let mut session_durations: Vec<f64> = Vec::new();
    let mut currently_online: i64 = 0;

    for session_id in &matching_session_ids {
        #[cfg(feature = "postgres")]
        let session: Option<(
            String,
            String,
            String,
            String,
            String,
            bool,
            DateTime<Utc>,
            DateTime<Utc>,
        )> = sqlx::query_as(
            r#"SELECT country, os, browser, device, device_type, is_bounce, start_time, last_seen
               FROM sessions WHERE id = $1"#,
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

        #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
        let session: Option<(
            String,
            String,
            String,
            String,
            String,
            bool,
            String,
            String,
        )> = sqlx::query_as(
            r#"SELECT country, os, browser, device, device_type, is_bounce, start_time, last_seen
               FROM sessions WHERE id = ?"#,
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

        if let Some((
            country,
            os,
            browser,
            device,
            device_type,
            is_bounce,
            session_start,
            last_seen,
        )) = session
        {
            *countries.entry(country).or_insert(0) += 1;
            *operating_systems.entry(os).or_insert(0) += 1;
            *browsers.entry(browser).or_insert(0) += 1;
            *devices.entry(device).or_insert(0) += 1;
            *device_types.entry(device_type).or_insert(0) += 1;
            if is_bounce {
                bounce_count += 1;
            }

            #[cfg(feature = "postgres")]
            {
                let duration = (last_seen - session_start).num_seconds() as f64;
                session_durations.push(duration);
                if last_seen > active_cutoff {
                    currently_online += 1;
                }
            }

            #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
            {
                if let (Ok(start_dt), Ok(end_dt)) = (
                    DateTime::parse_from_rfc3339(&session_start),
                    DateTime::parse_from_rfc3339(&last_seen),
                ) {
                    let duration = (end_dt.with_timezone(&Utc) - start_dt.with_timezone(&Utc))
                        .num_seconds() as f64;
                    session_durations.push(duration);
                    if end_dt.with_timezone(&Utc) > active_cutoff {
                        currently_online += 1;
                    }
                }
            }
        }
    }

    let bounce_rate_pct = if session_count > 0 {
        Some(((bounce_count as f64 / session_count as f64) * 1000.0).round() / 10.0)
    } else {
        None
    };

    let avg_session_duration = if session_durations.is_empty() {
        None
    } else {
        Some((session_durations.iter().sum::<f64>() / session_durations.len() as f64).round())
    };

    // Convert hashmaps to sorted vectors
    fn to_counted_items(map: HashMap<String, i64>, limit: i64) -> Vec<CountedItem> {
        let mut items: Vec<_> = map
            .into_iter()
            .map(|(value, count)| CountedItem { value, count })
            .collect();
        items.sort_by(|a, b| b.count.cmp(&a.count));
        items.truncate(limit as usize);
        items
    }

    let countries = to_counted_items(countries, RESULTS_LIMIT);
    let operating_systems = to_counted_items(operating_systems, RESULTS_LIMIT);
    let browsers = to_counted_items(browsers, RESULTS_LIMIT);
    let devices = to_counted_items(devices, RESULTS_LIMIT);
    let device_types = to_counted_items(device_types, RESULTS_LIMIT);

    // Chart data with URL filter - extract hit times for chart
    #[cfg(feature = "postgres")]
    let hit_times: Vec<DateTime<Utc>> = filtered_hits
        .iter()
        .map(|(_, _, _, _, _, _, start_time)| *start_time)
        .collect();

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let hit_times: Vec<DateTime<Utc>> = filtered_hits
        .iter()
        .filter_map(|(_, _, _, _, _, _, start_time)| {
            DateTime::parse_from_rfc3339(start_time)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        })
        .collect();

    let (chart_data, chart_tooltip_format, chart_granularity) =
        get_chart_data_filtered_sync(start, end, now, &hit_times, session_count);

    Ok(CoreStats {
        currently_online,
        session_count,
        hit_count,
        has_hits,
        bounce_rate_pct,
        avg_session_duration,
        avg_load_time,
        avg_hits_per_session,
        locations,
        referrers,
        countries,
        operating_systems,
        browsers,
        devices,
        device_types,
        chart_data,
        chart_tooltip_format,
        chart_granularity,
        compare: None,
    })
}

async fn get_counted_field(
    pool: &Pool,
    table: &str,
    field: &str,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<CountedItem>> {
    #[cfg(feature = "postgres")]
    let rows: Vec<CountedRow> = {
        let query = format!(
            "SELECT {field} as value, COUNT(*) as count FROM {table}
             WHERE service_id = $1 AND start_time >= $2 AND start_time < $3
             GROUP BY {field} ORDER BY count DESC LIMIT $4"
        );
        sqlx::query_as(&query)
            .bind(service_id.0)
            .bind(start)
            .bind(end)
            .bind(limit)
            .fetch_all(pool)
            .await?
    };

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let rows: Vec<CountedRow> = {
        let query = format!(
            "SELECT {field} as value, COUNT(*) as count FROM {table}
             WHERE service_id = ? AND start_time >= ? AND start_time < ?
             GROUP BY {field} ORDER BY count DESC LIMIT ?"
        );
        sqlx::query_as(&query)
            .bind(service_id.0.to_string())
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .bind(limit)
            .fetch_all(pool)
            .await?
    };

    Ok(rows.into_iter().map(Into::into).collect())
}

async fn get_counted_field_initial(
    pool: &Pool,
    table: &str,
    field: &str,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<CountedItem>> {
    #[cfg(feature = "postgres")]
    let rows: Vec<CountedRow> = {
        let query = format!(
            "SELECT {field} as value, COUNT(*) as count FROM {table}
             WHERE service_id = $1 AND start_time >= $2 AND start_time < $3 AND initial = true
             GROUP BY {field} ORDER BY count DESC LIMIT $4"
        );
        sqlx::query_as(&query)
            .bind(service_id.0)
            .bind(start)
            .bind(end)
            .bind(limit)
            .fetch_all(pool)
            .await?
    };

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let rows: Vec<CountedRow> = {
        let query = format!(
            "SELECT {field} as value, COUNT(*) as count FROM {table}
             WHERE service_id = ? AND start_time >= ? AND start_time < ? AND initial = 1
             GROUP BY {field} ORDER BY count DESC LIMIT ?"
        );
        sqlx::query_as(&query)
            .bind(service_id.0.to_string())
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .bind(limit)
            .fetch_all(pool)
            .await?
    };

    Ok(rows.into_iter().map(Into::into).collect())
}

async fn get_chart_data(
    pool: &Pool,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(ChartData, String, String)> {
    let duration = end - start;
    let use_hourly = duration.num_days() < 3;

    if use_hourly {
        get_hourly_chart_data(pool, service_id, start, end, now).await
    } else {
        get_daily_chart_data(pool, service_id, start, end, now).await
    }
}

async fn get_hourly_chart_data(
    pool: &Pool,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(ChartData, String, String)> {
    let mut data: HashMap<String, (i64, i64)> = HashMap::new();

    // Sessions per hour
    #[cfg(feature = "postgres")]
    {
        let rows: Vec<(DateTime<Utc>, i64)> = sqlx::query_as(
            "SELECT date_trunc('hour', start_time) as hour, COUNT(*) as count
             FROM sessions WHERE service_id = $1 AND start_time >= $2 AND start_time < $3
             GROUP BY hour ORDER BY hour",
        )
        .bind(service_id.0)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await?;

        for (hour, count) in rows {
            let key = hour.format("%Y-%m-%d %H:00").to_string();
            data.entry(key).or_insert((0, 0)).0 = count;
        }

        // Hits per hour
        let rows: Vec<(DateTime<Utc>, i64)> = sqlx::query_as(
            "SELECT date_trunc('hour', start_time) as hour, COUNT(*) as count
             FROM hits WHERE service_id = $1 AND start_time >= $2 AND start_time < $3
             GROUP BY hour ORDER BY hour",
        )
        .bind(service_id.0)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await?;

        for (hour, count) in rows {
            let key = hour.format("%Y-%m-%d %H:00").to_string();
            data.entry(key).or_insert((0, 0)).1 = count;
        }
    }

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT strftime('%Y-%m-%d %H:00', start_time) as hour, COUNT(*) as count
             FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ?
             GROUP BY hour ORDER BY hour",
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_all(pool)
        .await?;

        for (hour, count) in rows {
            data.entry(hour).or_insert((0, 0)).0 = count;
        }

        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT strftime('%Y-%m-%d %H:00', start_time) as hour, COUNT(*) as count
             FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ?
             GROUP BY hour ORDER BY hour",
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_all(pool)
        .await?;

        for (hour, count) in rows {
            data.entry(hour).or_insert((0, 0)).1 = count;
        }
    }

    // Fill in missing hours
    let hours = ((end - start).num_hours() + 1) as usize;
    for i in 0..hours {
        let hour = start + Duration::hours(i as i64);
        if hour <= now {
            let key = hour.format("%Y-%m-%d %H:00").to_string();
            data.entry(key).or_insert((0, 0));
        }
    }

    let mut sorted: Vec<_> = data.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let chart_data = ChartData {
        labels: sorted.iter().map(|(k, _)| k.clone()).collect(),
        sessions: sorted.iter().map(|(_, v)| v.0).collect(),
        hits: sorted.iter().map(|(_, v)| v.1).collect(),
    };

    Ok((chart_data, "MM/dd HH:mm".to_string(), "hourly".to_string()))
}

async fn get_daily_chart_data(
    pool: &Pool,
    service_id: ServiceId,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(ChartData, String, String)> {
    let mut data: HashMap<String, (i64, i64)> = HashMap::new();

    #[cfg(feature = "postgres")]
    {
        let rows: Vec<(chrono::NaiveDate, i64)> = sqlx::query_as(
            "SELECT date_trunc('day', start_time)::date as day, COUNT(*) as count
             FROM sessions WHERE service_id = $1 AND start_time >= $2 AND start_time < $3
             GROUP BY day ORDER BY day",
        )
        .bind(service_id.0)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await?;

        for (day, count) in rows {
            let key = day.format("%Y-%m-%d").to_string();
            data.entry(key).or_insert((0, 0)).0 = count;
        }

        let rows: Vec<(chrono::NaiveDate, i64)> = sqlx::query_as(
            "SELECT date_trunc('day', start_time)::date as day, COUNT(*) as count
             FROM hits WHERE service_id = $1 AND start_time >= $2 AND start_time < $3
             GROUP BY day ORDER BY day",
        )
        .bind(service_id.0)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await?;

        for (day, count) in rows {
            let key = day.format("%Y-%m-%d").to_string();
            data.entry(key).or_insert((0, 0)).1 = count;
        }
    }

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT date(start_time) as day, COUNT(*) as count
             FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ?
             GROUP BY day ORDER BY day",
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_all(pool)
        .await?;

        for (day, count) in rows {
            data.entry(day).or_insert((0, 0)).0 = count;
        }

        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT date(start_time) as day, COUNT(*) as count
             FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ?
             GROUP BY day ORDER BY day",
        )
        .bind(service_id.0.to_string())
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_all(pool)
        .await?;

        for (day, count) in rows {
            data.entry(day).or_insert((0, 0)).1 = count;
        }
    }

    // Fill in missing days
    let days = (end - start).num_days() + 1;
    for i in 0..days {
        let day = (start + Duration::days(i)).date_naive();
        if day <= now.date_naive() {
            let key = day.format("%Y-%m-%d").to_string();
            data.entry(key).or_insert((0, 0));
        }
    }

    let mut sorted: Vec<_> = data.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let chart_data = ChartData {
        labels: sorted.iter().map(|(k, _)| k.clone()).collect(),
        sessions: sorted.iter().map(|(_, v)| v.0).collect(),
        hits: sorted.iter().map(|(_, v)| v.1).collect(),
    };

    Ok((chart_data, "MMM d".to_string(), "daily".to_string()))
}

fn get_chart_data_filtered_sync(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
    hit_times: &[DateTime<Utc>],
    session_count: i64,
) -> (ChartData, String, String) {
    let duration = end - start;
    let use_hourly = duration.num_days() < 3;

    let mut data: HashMap<String, (i64, i64)> = HashMap::new();

    if use_hourly {
        // Count hits per hour
        for hit_time in hit_times {
            let key = hit_time.format("%Y-%m-%d %H:00").to_string();
            data.entry(key).or_insert((0, 0)).1 += 1;
        }

        // Distribute sessions across hours with data
        let hours_with_data = data.len().max(1) as i64;
        for key in data.keys().cloned().collect::<Vec<_>>() {
            data.entry(key).or_insert((0, 0)).0 = session_count / hours_with_data;
        }

        // Fill in missing hours
        let hours = ((end - start).num_hours() + 1) as usize;
        for i in 0..hours {
            let hour = start + Duration::hours(i as i64);
            if hour <= now {
                let key = hour.format("%Y-%m-%d %H:00").to_string();
                data.entry(key).or_insert((0, 0));
            }
        }

        let mut sorted: Vec<_> = data.into_iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let chart_data = ChartData {
            labels: sorted.iter().map(|(k, _)| k.clone()).collect(),
            sessions: sorted.iter().map(|(_, v)| v.0).collect(),
            hits: sorted.iter().map(|(_, v)| v.1).collect(),
        };

        (chart_data, "MM/dd HH:mm".to_string(), "hourly".to_string())
    } else {
        // Count hits per day
        for hit_time in hit_times {
            let key = hit_time.format("%Y-%m-%d").to_string();
            data.entry(key).or_insert((0, 0)).1 += 1;
        }

        // Distribute sessions across days with data
        let days_with_data = data.len().max(1) as i64;
        for key in data.keys().cloned().collect::<Vec<_>>() {
            data.entry(key).or_insert((0, 0)).0 = session_count / days_with_data;
        }

        // Fill in missing days
        let days = (end - start).num_days() + 1;
        for i in 0..days {
            let day = (start + Duration::days(i)).date_naive();
            if day <= now.date_naive() {
                let key = day.format("%Y-%m-%d").to_string();
                data.entry(key).or_insert((0, 0));
            }
        }

        let mut sorted: Vec<_> = data.into_iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let chart_data = ChartData {
            labels: sorted.iter().map(|(k, _)| k.clone()).collect(),
            sessions: sorted.iter().map(|(_, v)| v.0).collect(),
            hits: sorted.iter().map(|(_, v)| v.1).collect(),
        };

        (chart_data, "MMM d".to_string(), "daily".to_string())
    }
}

// Row types for SQLx mapping - PostgreSQL versions
#[cfg(feature = "postgres")]
#[derive(sqlx::FromRow)]
struct ServiceRow {
    id: uuid::Uuid,
    tracking_id: Option<String>,
    name: String,
    link: String,
    origins: String,
    status: String,
    respect_dnt: bool,
    ignore_robots: bool,
    collect_ips: bool,
    ignored_ips: String,
    hide_referrer_regex: String,
    script_inject: String,
    created_at: DateTime<Utc>,
}

#[cfg(feature = "postgres")]
impl From<ServiceRow> for Service {
    fn from(row: ServiceRow) -> Self {
        Self {
            id: ServiceId(row.id),
            tracking_id: TrackingId(row.tracking_id.unwrap_or_else(|| TrackingId::new().0)),
            name: row.name,
            link: row.link,
            origins: row.origins,
            status: ServiceStatus::from_str(&row.status).unwrap_or(ServiceStatus::Active),
            respect_dnt: row.respect_dnt,
            ignore_robots: row.ignore_robots,
            collect_ips: row.collect_ips,
            ignored_ips: row.ignored_ips,
            hide_referrer_regex: row.hide_referrer_regex,
            script_inject: row.script_inject,
            created_at: row.created_at,
        }
    }
}

#[cfg(feature = "postgres")]
#[derive(sqlx::FromRow)]
struct SessionRow {
    id: uuid::Uuid,
    service_id: uuid::Uuid,
    identifier: String,
    start_time: DateTime<Utc>,
    last_seen: DateTime<Utc>,
    user_agent: String,
    browser: String,
    device: String,
    device_type: String,
    os: String,
    ip: Option<String>,
    asn: String,
    country: String,
    longitude: Option<f64>,
    latitude: Option<f64>,
    time_zone: String,
    is_bounce: bool,
}

#[cfg(feature = "postgres")]
impl From<SessionRow> for Session {
    fn from(row: SessionRow) -> Self {
        Self {
            id: SessionId(row.id),
            service_id: ServiceId(row.service_id),
            identifier: row.identifier,
            start_time: row.start_time,
            last_seen: row.last_seen,
            user_agent: row.user_agent,
            browser: row.browser,
            device: row.device,
            device_type: DeviceType::from_str(&row.device_type),
            os: row.os,
            ip: row.ip,
            asn: row.asn,
            country: row.country,
            longitude: row.longitude,
            latitude: row.latitude,
            time_zone: row.time_zone,
            is_bounce: row.is_bounce,
        }
    }
}

#[cfg(feature = "postgres")]
#[derive(sqlx::FromRow)]
struct HitRow {
    id: i64,
    session_id: uuid::Uuid,
    service_id: uuid::Uuid,
    initial: bool,
    start_time: DateTime<Utc>,
    last_seen: DateTime<Utc>,
    heartbeats: i32,
    tracker: String,
    location: String,
    referrer: String,
    load_time: Option<f64>,
}

#[cfg(feature = "postgres")]
impl From<HitRow> for Hit {
    fn from(row: HitRow) -> Self {
        Self {
            id: HitId(row.id),
            session_id: SessionId(row.session_id),
            service_id: ServiceId(row.service_id),
            initial: row.initial,
            start_time: row.start_time,
            last_seen: row.last_seen,
            heartbeats: row.heartbeats,
            tracker: TrackerType::from_str(&row.tracker),
            location: row.location,
            referrer: row.referrer,
            load_time: row.load_time,
        }
    }
}

// Row types for SQLx mapping - SQLite versions (UUIDs stored as TEXT)
#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
#[derive(sqlx::FromRow)]
struct ServiceRow {
    id: String,
    tracking_id: Option<String>,
    name: String,
    link: String,
    origins: String,
    status: String,
    respect_dnt: bool,
    ignore_robots: bool,
    collect_ips: bool,
    ignored_ips: String,
    hide_referrer_regex: String,
    script_inject: String,
    created_at: String,
}

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
impl From<ServiceRow> for Service {
    fn from(row: ServiceRow) -> Self {
        Self {
            id: ServiceId(row.id.parse().unwrap_or_default()),
            tracking_id: TrackingId(row.tracking_id.unwrap_or_else(|| TrackingId::new().0)),
            name: row.name,
            link: row.link,
            origins: row.origins,
            status: ServiceStatus::from_str(&row.status).unwrap_or(ServiceStatus::Active),
            respect_dnt: row.respect_dnt,
            ignore_robots: row.ignore_robots,
            collect_ips: row.collect_ips,
            ignored_ips: row.ignored_ips,
            hide_referrer_regex: row.hide_referrer_regex,
            script_inject: row.script_inject,
            created_at: DateTime::parse_from_rfc3339(&row.created_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }
    }
}

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
#[derive(sqlx::FromRow)]
struct SessionRow {
    id: String,
    service_id: String,
    identifier: String,
    start_time: String,
    last_seen: String,
    user_agent: String,
    browser: String,
    device: String,
    device_type: String,
    os: String,
    ip: Option<String>,
    asn: String,
    country: String,
    longitude: Option<f64>,
    latitude: Option<f64>,
    time_zone: String,
    is_bounce: bool,
}

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
impl From<SessionRow> for Session {
    fn from(row: SessionRow) -> Self {
        Self {
            id: SessionId(row.id.parse().unwrap_or_default()),
            service_id: ServiceId(row.service_id.parse().unwrap_or_default()),
            identifier: row.identifier,
            start_time: DateTime::parse_from_rfc3339(&row.start_time)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            last_seen: DateTime::parse_from_rfc3339(&row.last_seen)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            user_agent: row.user_agent,
            browser: row.browser,
            device: row.device,
            device_type: DeviceType::from_str(&row.device_type),
            os: row.os,
            ip: row.ip,
            asn: row.asn,
            country: row.country,
            longitude: row.longitude,
            latitude: row.latitude,
            time_zone: row.time_zone,
            is_bounce: row.is_bounce,
        }
    }
}

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
#[derive(sqlx::FromRow)]
struct HitRow {
    id: i64,
    session_id: String,
    service_id: String,
    initial: bool,
    start_time: String,
    last_seen: String,
    heartbeats: i32,
    tracker: String,
    location: String,
    referrer: String,
    load_time: Option<f64>,
}

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
impl From<HitRow> for Hit {
    fn from(row: HitRow) -> Self {
        Self {
            id: HitId(row.id),
            session_id: SessionId(row.session_id.parse().unwrap_or_default()),
            service_id: ServiceId(row.service_id.parse().unwrap_or_default()),
            initial: row.initial,
            start_time: DateTime::parse_from_rfc3339(&row.start_time)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            last_seen: DateTime::parse_from_rfc3339(&row.last_seen)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            heartbeats: row.heartbeats,
            tracker: TrackerType::from_str(&row.tracker),
            location: row.location,
            referrer: row.referrer,
            load_time: row.load_time,
        }
    }
}

#[derive(sqlx::FromRow)]
struct CountedRow {
    value: Option<String>,
    count: i64,
}

impl From<CountedRow> for CountedItem {
    fn from(row: CountedRow) -> Self {
        Self {
            value: row.value.unwrap_or_default(),
            count: row.count,
        }
    }
}
