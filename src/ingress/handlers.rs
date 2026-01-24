use askama::Template;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::db;
use crate::domain::TrackerType;
use crate::error::Error;
use crate::privacy::{
    get_client_ip, get_origin, get_referrer, get_user_agent, is_dnt_enabled, is_ip_ignored,
};
use crate::state::AppState;

use super::{process_ingress, IngressPayload};

#[derive(Template)]
#[template(path = "ingress/tracker.js", escape = "none")]
struct TrackerScriptTemplate<'a> {
    protocol: &'a str,
    endpoint: &'a str,
    heartbeat_frequency: u64,
    script_inject: &'a str,
}

#[derive(Template)]
#[template(path = "ingress/tracker_dnt.js", escape = "none")]
struct TrackerScriptDntTemplate;

/// Strip file extension suffix from tracking_id if present
fn strip_extension(s: &str) -> &str {
    s.strip_suffix(".js")
        .or_else(|| s.strip_suffix(".gif"))
        .unwrap_or(s)
}

/// Detect the protocol (http/https) from request headers
/// Checks X-Forwarded-Proto header first (for reverse proxy setups),
/// then falls back to the provided default
fn detect_protocol(headers: &HeaderMap, default_https: bool) -> &'static str {
    // Check X-Forwarded-Proto header (common in reverse proxy setups)
    if let Some(proto) = headers.get("x-forwarded-proto") {
        if let Ok(proto_str) = proto.to_str() {
            if proto_str.eq_ignore_ascii_case("https") {
                return "https";
            } else if proto_str.eq_ignore_ascii_case("http") {
                return "http";
            }
        }
    }

    // Check X-Forwarded-Ssl header
    if let Some(ssl) = headers.get("x-forwarded-ssl") {
        if let Ok(ssl_str) = ssl.to_str() {
            if ssl_str.eq_ignore_ascii_case("on") {
                return "https";
            }
        }
    }

    // Fall back to config default
    if default_https {
        "https"
    } else {
        "http"
    }
}

// 1x1 transparent GIF
const PIXEL_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0xff, 0x00, 0xff, 0xff, 0xff,
    0x00, 0x00, 0x00, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
];

#[derive(Debug, Deserialize)]
pub struct ScriptPayload {
    pub idempotency: Option<String>,
    pub location: Option<String>,
    pub referrer: Option<String>,
    #[serde(rename = "loadTime")]
    pub load_time: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ScriptResponse {
    pub status: String,
}

/// GET /trace/px_:tracking_id.gif
pub async fn pixel_handler(
    State(state): State<AppState>,
    Path(tracking_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let tracking_id = strip_extension(&tracking_id).to_string();
    pixel_handler_internal(state, tracking_id, None, headers).await
}

/// GET /trace/px_:tracking_id/:identifier.gif
pub async fn pixel_with_id_handler(
    State(state): State<AppState>,
    Path((tracking_id, identifier)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    // Strip .gif suffix if present
    let identifier = identifier
        .strip_suffix(".gif")
        .unwrap_or(&identifier)
        .to_string();
    pixel_handler_internal(state, tracking_id, Some(identifier), headers).await
}

async fn pixel_handler_internal(
    state: AppState,
    tracking_id: String,
    identifier: Option<String>,
    headers: HeaderMap,
) -> Response {
    info!("Pixel request for tracking_id={}", tracking_id);

    // Validate service and get origins
    let service = match db::get_active_service_by_tracking_id(&state.pool, &tracking_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            error!("Service not found for tracking_id={}", tracking_id);
            return (StatusCode::NOT_FOUND, "Service not found").into_response();
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    info!("Found service: {} ({})", service.name, service.id);

    // Validate origin
    let (allow_origin, origin_valid) = validate_origin(&headers, &service);
    if !origin_valid {
        return (StatusCode::FORBIDDEN, "Invalid origin").into_response();
    }

    // Check DNT
    if is_dnt_enabled(&headers) && service.respect_dnt {
        debug!("Ignoring due to DNT/GPC");
        return pixel_response(allow_origin);
    }

    let ip = get_client_ip(&headers).unwrap_or_else(|| "0.0.0.0".to_string());
    let user_agent = get_user_agent(&headers);
    let location = get_referrer(&headers);

    // Check ignored IPs
    let ignored_networks = service.get_ignored_networks();
    if is_ip_ignored(&ip, &ignored_networks) {
        debug!("Ignoring due to ignored IP");
        return pixel_response(allow_origin);
    }

    // Process ingress asynchronously
    let identifier = identifier.unwrap_or_default();
    let payload = IngressPayload {
        location,
        ..Default::default()
    };

    // Spawn processing in background to not delay response
    tokio::spawn(async move {
        if let Err(e) = process_ingress(
            &state,
            &service,
            TrackerType::Pixel,
            Utc::now(),
            payload,
            &ip,
            &user_agent,
            &identifier,
        )
        .await
        {
            error!("Error processing pixel ingress: {}", e);
        }
    });

    pixel_response(allow_origin)
}

fn pixel_response(allow_origin: String) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/gif"),
            (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, &allow_origin),
        ],
        PIXEL_GIF,
    )
        .into_response()
}

/// GET /trace/app_:tracking_id.js
pub async fn script_get_handler(
    State(state): State<AppState>,
    Path(tracking_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let tracking_id = strip_extension(&tracking_id).to_string();
    script_get_handler_internal(state, tracking_id, None, headers).await
}

/// GET /trace/app_:tracking_id/:identifier.js
pub async fn script_get_with_id_handler(
    State(state): State<AppState>,
    Path((tracking_id, identifier)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    // Strip .js suffix if present
    let identifier = identifier
        .strip_suffix(".js")
        .unwrap_or(&identifier)
        .to_string();
    script_get_handler_internal(state, tracking_id, Some(identifier), headers).await
}

async fn script_get_handler_internal(
    state: AppState,
    tracking_id: String,
    identifier: Option<String>,
    headers: HeaderMap,
) -> Response {
    info!("Script GET request for tracking_id={}", tracking_id);

    // Validate service
    let service = match db::get_active_service_by_tracking_id(&state.pool, &tracking_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            error!("Service not found for tracking_id={}", tracking_id);
            return (StatusCode::NOT_FOUND, "Service not found").into_response();
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    info!("Found service: {} ({})", service.name, service.id);

    // Validate origin
    let (allow_origin, origin_valid) = validate_origin(&headers, &service);
    if !origin_valid {
        return (StatusCode::FORBIDDEN, "Invalid origin").into_response();
    }

    // Check DNT
    let dnt = is_dnt_enabled(&headers) && service.respect_dnt;

    // Generate script - detect protocol from incoming request headers
    let protocol = detect_protocol(&headers, true);

    let endpoint = match &identifier {
        Some(id) => format!("/trace/app_{}/{}.js", tracking_id, id),
        None => format!("/trace/app_{}.js", tracking_id),
    };

    let heartbeat_frequency = state.settings.script_heartbeat_frequency_ms;

    // Get script inject content
    let script_inject = state
        .cache
        .get_or_insert_script_inject(service.id, || async { Some(service.script_inject.clone()) })
        .await
        .unwrap_or_default();

    let script = generate_tracker_script(
        dnt,
        protocol,
        &endpoint,
        heartbeat_frequency,
        &script_inject,
    );

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/javascript"),
            (header::CACHE_CONTROL, "public, max-age=31536000"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, &allow_origin),
        ],
        script,
    )
        .into_response()
}

/// POST /trace/app_:tracking_id.js
pub async fn script_post_handler(
    State(state): State<AppState>,
    Path(tracking_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<ScriptPayload>,
) -> Response {
    let tracking_id = strip_extension(&tracking_id).to_string();
    script_post_handler_internal(state, tracking_id, None, headers, payload).await
}

/// POST /trace/app_:tracking_id/:identifier.js
pub async fn script_post_with_id_handler(
    State(state): State<AppState>,
    Path((tracking_id, identifier)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<ScriptPayload>,
) -> Response {
    // Strip .js suffix if present
    let identifier = identifier
        .strip_suffix(".js")
        .unwrap_or(&identifier)
        .to_string();
    script_post_handler_internal(state, tracking_id, Some(identifier), headers, payload).await
}

async fn script_post_handler_internal(
    state: AppState,
    tracking_id: String,
    identifier: Option<String>,
    headers: HeaderMap,
    payload: ScriptPayload,
) -> Response {
    info!(
        "Script POST request for tracking_id={} payload={:?}",
        tracking_id, payload
    );

    // Validate service
    let service = match db::get_active_service_by_tracking_id(&state.pool, &tracking_id).await {
        Ok(s) => s,
        Err(Error::ServiceNotFound) => {
            error!("Service not found for tracking_id={}", tracking_id);
            return (StatusCode::NOT_FOUND, "Service not found").into_response();
        }
        Err(e) => {
            error!("Error fetching service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    info!("Found service: {} ({})", service.name, service.id);

    // Validate origin
    let (allow_origin, origin_valid) = validate_origin(&headers, &service);
    if !origin_valid {
        return (StatusCode::FORBIDDEN, "Invalid origin").into_response();
    }

    // Check DNT
    if is_dnt_enabled(&headers) && service.respect_dnt {
        debug!("Ignoring due to DNT/GPC");
        return json_response(allow_origin);
    }

    let ip = get_client_ip(&headers).unwrap_or_else(|| "0.0.0.0".to_string());
    let user_agent = get_user_agent(&headers);

    // Check ignored IPs
    let ignored_networks = service.get_ignored_networks();
    if is_ip_ignored(&ip, &ignored_networks) {
        debug!("Ignoring due to ignored IP");
        return json_response(allow_origin);
    }

    let identifier = identifier.unwrap_or_default();
    let ingress_payload = IngressPayload {
        idempotency: payload.idempotency,
        location: payload.location.unwrap_or_default(),
        referrer: payload.referrer.unwrap_or_default(),
        load_time: payload.load_time,
    };

    // Process synchronously for POST requests
    if let Err(e) = process_ingress(
        &state,
        &service,
        TrackerType::Js,
        Utc::now(),
        ingress_payload,
        &ip,
        &user_agent,
        &identifier,
    )
    .await
    {
        error!("Error processing script ingress: {}", e);
    }

    json_response(allow_origin)
}

fn json_response(allow_origin: String) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, &allow_origin),
            (
                header::ACCESS_CONTROL_ALLOW_METHODS,
                "GET,HEAD,OPTIONS,POST",
            ),
            (
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                "Origin, X-Requested-With, Content-Type, Accept, Authorization, Referer",
            ),
        ],
        Json(ScriptResponse {
            status: "OK".to_string(),
        }),
    )
        .into_response()
}

fn validate_origin(headers: &HeaderMap, service: &crate::domain::Service) -> (String, bool) {
    if service.origins == "*" {
        return ("*".to_string(), true);
    }

    let remote_origin = get_origin(headers);

    match remote_origin {
        Some(origin) => {
            if service.is_origin_allowed(&origin) {
                (origin, true)
            } else {
                ("*".to_string(), false)
            }
        }
        None => ("*".to_string(), false),
    }
}

fn generate_tracker_script(
    dnt: bool,
    protocol: &str,
    endpoint: &str,
    heartbeat_frequency: u64,
    script_inject: &str,
) -> String {
    if dnt {
        return TrackerScriptDntTemplate
            .render()
            .unwrap_or_else(|_| "var shymini = { dnt: true };".to_string());
    }

    let template = TrackerScriptTemplate {
        protocol,
        endpoint,
        heartbeat_frequency,
        script_inject,
    };

    template.render().unwrap_or_else(|e| {
        error!("Failed to render tracker script template: {}", e);
        "console.error('Failed to load tracker script');".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_gif_is_valid_gif() {
        // GIF magic bytes are "GIF89a" or "GIF87a"
        assert_eq!(&PIXEL_GIF[0..3], b"GIF");
        assert_eq!(PIXEL_GIF[3], b'8');
        assert_eq!(PIXEL_GIF[4], b'9');
        assert_eq!(PIXEL_GIF[5], b'a');
    }

    #[test]
    fn test_pixel_gif_dimensions() {
        // In GIF format, bytes 6-7 are width (little-endian)
        // and bytes 8-9 are height (little-endian)
        // 1x1 pixel means width = 1, height = 1
        let width = u16::from_le_bytes([PIXEL_GIF[6], PIXEL_GIF[7]]);
        let height = u16::from_le_bytes([PIXEL_GIF[8], PIXEL_GIF[9]]);
        assert_eq!(width, 1);
        assert_eq!(height, 1);
    }

    #[test]
    fn test_generate_tracker_script_dnt() {
        let script = generate_tracker_script(true, "https", "/ingress/uuid/script.js", 5000, "");
        assert_eq!(script, r#"var shymini = { dnt: true };"#);
    }

    #[test]
    fn test_generate_tracker_script_normal() {
        let script = generate_tracker_script(false, "https", "/ingress/uuid/script.js", 5000, "");

        assert!(script.contains("var shymini = (function()"));
        assert!(script.contains("dnt: false"));
        assert!(script.contains("sendHeartbeat"));
        assert!(script.contains("newPageLoad"));
        assert!(script.contains("scriptOrigin")); // Uses script origin instead of window.location.host
        assert!(script.contains("/ingress/uuid/script.js"));
        assert!(script.contains("5000")); // heartbeat frequency
    }

    #[test]
    fn test_generate_tracker_script_http() {
        let script = generate_tracker_script(false, "http", "/ingress/test/script.js", 3000, "");

        assert!(script.contains("http://"));
        assert!(script.contains("3000")); // heartbeat frequency
    }

    #[test]
    fn test_generate_tracker_script_with_inject() {
        let script = generate_tracker_script(
            false,
            "https",
            "/ingress/uuid/script.js",
            5000,
            "console.log('custom code');",
        );

        assert!(script.contains("console.log('custom code');"));
        assert!(script.contains("// The following script is not part of shymini"));
        assert!(script.contains("// -- START --"));
        assert!(script.contains("// -- END --"));
    }

    #[test]
    fn test_generate_tracker_script_empty_inject() {
        let script = generate_tracker_script(false, "https", "/test", 5000, "");

        // Should not contain inject markers
        assert!(!script.contains("// -- START --"));
        assert!(!script.contains("provided by this site's administrator"));
    }

    #[test]
    fn test_script_payload_deserialization() {
        let json = r#"{"idempotency": "abc123", "location": "/home", "referrer": "https://google.com", "loadTime": 150.5}"#;
        let payload: ScriptPayload = serde_json::from_str(json).unwrap();

        assert_eq!(payload.idempotency, Some("abc123".to_string()));
        assert_eq!(payload.location, Some("/home".to_string()));
        assert_eq!(payload.referrer, Some("https://google.com".to_string()));
        assert_eq!(payload.load_time, Some(150.5));
    }

    #[test]
    fn test_script_payload_deserialization_minimal() {
        let json = r#"{}"#;
        let payload: ScriptPayload = serde_json::from_str(json).unwrap();

        assert!(payload.idempotency.is_none());
        assert!(payload.location.is_none());
        assert!(payload.referrer.is_none());
        assert!(payload.load_time.is_none());
    }

    #[test]
    fn test_script_response_serialization() {
        let response = ScriptResponse {
            status: "OK".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(json, r#"{"status":"OK"}"#);
    }

    #[test]
    fn test_generate_tracker_script_contains_fetch() {
        let script = generate_tracker_script(false, "https", "/test", 5000, "");

        // Script should use fetch API
        assert!(script.contains("fetch("));
        assert!(script.contains("method: \"POST\""));
        assert!(script.contains("Content-Type"));
        assert!(script.contains("application/json"));
    }

    #[test]
    fn test_generate_tracker_script_visibility_api() {
        let script = generate_tracker_script(false, "https", "/test", 5000, "");

        // Script should check document visibility
        assert!(script.contains("document.hidden"));
    }

    #[test]
    fn test_generate_tracker_script_sends_correct_data() {
        let script = generate_tracker_script(false, "https", "/test", 5000, "");

        // Script should send idempotency, referrer, location
        assert!(script.contains("idempotency: shymini.idempotency"));
        assert!(script.contains("referrer: document.referrer"));
        assert!(script.contains("location: window.location.href"));
        // loadTime is only sent on first request (when loadTimeSent is false)
        assert!(script.contains("loadTimeSent"));
        assert!(script.contains("payload.loadTime"));
    }

    #[test]
    fn test_detect_protocol_x_forwarded_proto_https() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(detect_protocol(&headers, false), "https");
    }

    #[test]
    fn test_detect_protocol_x_forwarded_proto_http() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", "http".parse().unwrap());
        assert_eq!(detect_protocol(&headers, true), "http");
    }

    #[test]
    fn test_detect_protocol_x_forwarded_ssl_on() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-ssl", "on".parse().unwrap());
        assert_eq!(detect_protocol(&headers, false), "https");
    }

    #[test]
    fn test_detect_protocol_no_headers_default_true() {
        let headers = HeaderMap::new();
        assert_eq!(detect_protocol(&headers, true), "https");
    }

    #[test]
    fn test_detect_protocol_no_headers_default_false() {
        let headers = HeaderMap::new();
        assert_eq!(detect_protocol(&headers, false), "http");
    }

    #[test]
    fn test_detect_protocol_case_insensitive() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", "HTTPS".parse().unwrap());
        assert_eq!(detect_protocol(&headers, false), "https");

        let mut headers2 = HeaderMap::new();
        headers2.insert("x-forwarded-proto", "HTTP".parse().unwrap());
        assert_eq!(detect_protocol(&headers2, true), "http");
    }

    #[test]
    fn test_strip_extension_js() {
        assert_eq!(strip_extension("abc123.js"), "abc123");
    }

    #[test]
    fn test_strip_extension_gif() {
        assert_eq!(strip_extension("abc123.gif"), "abc123");
    }

    #[test]
    fn test_strip_extension_none() {
        assert_eq!(strip_extension("abc123"), "abc123");
    }
}
