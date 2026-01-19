use chrono::{DateTime, Utc};
use tracing::debug;

use crate::db::{self, Pool};
use crate::domain::{
    CreateHit, CreateSession, DeviceType, HitId, Service, ServiceId, SessionAssociationHash,
    SessionId, TrackerType,
};
use crate::error::Result;
use crate::state::AppState;
use crate::ua::parse_user_agent;

#[derive(Debug, Default)]
pub struct IngressPayload {
    pub idempotency: Option<String>,
    pub location: String,
    pub referrer: String,
    pub load_time: Option<f64>,
}

#[allow(clippy::too_many_arguments)]
pub async fn process_ingress(
    state: &AppState,
    service: &Service,
    tracker: TrackerType,
    time: DateTime<Utc>,
    payload: IngressPayload,
    ip: &str,
    user_agent: &str,
    identifier: &str,
) -> Result<()> {
    debug!(
        "Processing ingress for service {} with tracker {:?}",
        service.id, tracker
    );

    // Validate and clean payload
    let load_time = payload.load_time.filter(|&t| t > 0.0);

    // Compute session association hash
    let aggressive_salting = state.settings.aggressive_hash_salting;
    let hash = SessionAssociationHash::compute(
        ip,
        user_agent,
        if aggressive_salting {
            Some(&service.id)
        } else {
            None
        },
        aggressive_salting,
    );

    let cache_key = format!("session_{}_{}", service.id, hash);

    // Try to find existing session in cache
    let (session_id, initial) = match state.cache.get_session_association(&cache_key).await {
        Some(session_id) => {
            debug!("Found existing session {} in cache", session_id);
            state.cache.touch_session_association(&cache_key).await;

            // Update session last_seen
            db::update_session_last_seen(&state.pool, session_id, time).await?;

            // Update identifier if provided and session doesn't have one
            if !identifier.is_empty() {
                let session = db::get_session(&state.pool, session_id).await?;
                if session.identifier.is_empty() {
                    db::update_session_identifier(&state.pool, session_id, identifier).await?;
                }
            }

            (session_id, false)
        }
        None => {
            debug!("Creating new session for service {}", service.id);

            // GeoIP lookup
            let geo_data = state.geo.lookup(ip);
            debug!("GeoIP data: {:?}", geo_data);

            // Parse user agent
            let ua_data = parse_user_agent(user_agent);
            debug!("UA data: {:?}", ua_data);

            // Check if we should ignore robots
            if ua_data.device_type == DeviceType::Robot && service.ignore_robots {
                debug!("Ignoring robot");
                return Ok(());
            }

            // Determine IP to store
            let stored_ip = if service.collect_ips && !state.settings.block_all_ips {
                Some(ip.to_string())
            } else {
                None
            };

            // Create session
            let session = db::create_session(
                &state.pool,
                CreateSession {
                    service_id: service.id,
                    identifier: identifier.trim().to_string(),
                    start_time: time,
                    user_agent: user_agent.to_string(),
                    browser: ua_data.browser,
                    device: ua_data.device,
                    device_type: ua_data.device_type,
                    os: ua_data.os,
                    ip: stored_ip,
                    asn: geo_data.asn,
                    country: geo_data.country,
                    longitude: geo_data.longitude,
                    latitude: geo_data.latitude,
                    time_zone: geo_data.time_zone,
                },
            )
            .await?;

            // Cache the session association
            state
                .cache
                .set_session_association(cache_key, session.id)
                .await;

            (session.id, true)
        }
    };

    // Handle hit creation/update
    let idempotency_key = payload.idempotency.as_ref().map(|k| format!("hit_{}", k));

    let hit_id = if let Some(ref key) = idempotency_key {
        if let Some(existing_hit_id) = state.cache.get_hit_idempotency(key).await {
            // This is a heartbeat for an existing hit
            debug!("Heartbeat for existing hit {}", existing_hit_id);
            state.cache.touch_hit_idempotency(key).await;
            db::update_hit_heartbeat(&state.pool, existing_hit_id, time).await?;
            existing_hit_id
        } else {
            // New hit
            create_new_hit(
                &state.pool,
                session_id,
                service.id,
                initial,
                time,
                tracker,
                &payload,
                load_time,
            )
            .await?
        }
    } else {
        // No idempotency key, always create new hit
        create_new_hit(
            &state.pool,
            session_id,
            service.id,
            initial,
            time,
            tracker,
            &payload,
            load_time,
        )
        .await?
    };

    // Cache the hit idempotency if key was provided
    if let Some(key) = idempotency_key {
        state.cache.set_hit_idempotency(key, hit_id).await;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_new_hit(
    pool: &Pool,
    session_id: SessionId,
    service_id: ServiceId,
    initial: bool,
    time: DateTime<Utc>,
    tracker: TrackerType,
    payload: &IngressPayload,
    load_time: Option<f64>,
) -> Result<HitId> {
    debug!("Creating new hit for session {}", session_id);

    let hit = db::create_hit(
        pool,
        CreateHit {
            session_id,
            service_id,
            initial,
            start_time: time,
            tracker,
            location: payload.location.clone(),
            referrer: payload.referrer.clone(),
            load_time,
        },
    )
    .await?;

    // Recalculate bounce status
    db::recalculate_session_bounce(pool, session_id).await?;

    Ok(hit.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ingress_payload_default() {
        let payload = IngressPayload::default();
        assert!(payload.idempotency.is_none());
        assert!(payload.location.is_empty());
        assert!(payload.referrer.is_empty());
        assert!(payload.load_time.is_none());
    }

    #[test]
    fn test_ingress_payload_with_values() {
        let payload = IngressPayload {
            idempotency: Some("abc123".to_string()),
            location: "/home".to_string(),
            referrer: "https://google.com".to_string(),
            load_time: Some(150.5),
        };

        assert_eq!(payload.idempotency, Some("abc123".to_string()));
        assert_eq!(payload.location, "/home");
        assert_eq!(payload.referrer, "https://google.com");
        assert_eq!(payload.load_time, Some(150.5));
    }

    #[test]
    fn test_ingress_payload_debug_format() {
        let payload = IngressPayload::default();
        let debug_str = format!("{:?}", payload);
        assert!(debug_str.contains("IngressPayload"));
        assert!(debug_str.contains("idempotency"));
        assert!(debug_str.contains("location"));
    }

    #[test]
    fn test_load_time_filter() {
        // Test that negative load times are filtered
        let load_time: Option<f64> = Some(-100.0);
        let filtered = load_time.filter(|&t| t > 0.0);
        assert!(filtered.is_none());

        // Test that zero is filtered
        let load_time: Option<f64> = Some(0.0);
        let filtered = load_time.filter(|&t| t > 0.0);
        assert!(filtered.is_none());

        // Test that positive values are kept
        let load_time: Option<f64> = Some(100.0);
        let filtered = load_time.filter(|&t| t > 0.0);
        assert_eq!(filtered, Some(100.0));
    }
}
