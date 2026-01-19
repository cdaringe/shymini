use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

// Helper to create test app with shared pool for multi-request tests
async fn create_test_app() -> Router {
    let (router, _) = create_test_app_with_pool().await;
    router
}

async fn create_test_app_with_pool() -> (Router, shymini::db::Pool) {
    use axum::routing::{get, post};
    use shymini::{
        api, cache::AppCache, config::Settings, dashboard, db, geo::GeoIpLookup, ingress,
        state::AppState,
    };

    // Create in-memory SQLite database
    let pool = db::create_pool("sqlite::memory:").await.unwrap();
    db::run_migrations(&pool).await.unwrap();

    // Create minimal settings
    let settings = Settings::new().unwrap_or_else(|_| {
        // Fallback for tests
        Settings {
            host: "127.0.0.1".to_string(),
            port: 8080,
            database_url: None,
            database_path: None,
            maxmind_city_db: None,
            maxmind_asn_db: None,
            block_all_ips: false,
            aggressive_hash_salting: false,
            script_heartbeat_frequency_ms: 5000,
            cache_max_entries: 1000,
            cache_ttl_secs: 3600,
            session_memory_timeout_secs: 1800,
        }
    });

    let cache = AppCache::new(&settings);
    let geo = GeoIpLookup::new(None, None).unwrap();
    let state = AppState::new(pool.clone(), cache, settings, geo);

    let router = Router::new()
        .route("/", get(dashboard::dashboard_index))
        .route("/service/new", get(dashboard::service_create_form))
        .route("/service/new", post(dashboard::service_create))
        .route("/service/:id", get(dashboard::service_detail))
        // New tracking routes
        .route("/trace/px_:tracking_id.gif", get(ingress::pixel_handler))
        .route(
            "/trace/app_:tracking_id.js",
            get(ingress::script_get_handler).post(ingress::script_post_handler),
        )
        .route("/api/services", get(api::list_services))
        .route("/api/services/:id", get(api::get_service))
        .with_state(state);

    (router, pool)
}

#[tokio::test]
async fn test_dashboard_index() {
    let app = create_test_app().await;

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_create_service_form() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/service/new")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_list_services_empty() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/services")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_service_not_found() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/service/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_pixel_service_not_found() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/trace/px_notfound.gif")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 404 for unknown tracking_id
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_script_get_service_not_found() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/trace/app_notfound.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Script endpoint should return 404 for unknown tracking_id
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_tracker_script_served_for_valid_service() {
    use shymini::db;
    use shymini::domain::CreateService;

    let (app, pool) = create_test_app_with_pool().await;

    // Create a service
    let service = db::create_service(
        &pool,
        CreateService {
            name: "Test Service".to_string(),
            link: "https://example.com".to_string(),
            origins: "*".to_string(),
            respect_dnt: false,
            ignore_robots: false,
            collect_ips: true,
            ignored_ips: String::new(),
            hide_referrer_regex: String::new(),
            script_inject: String::new(),
        },
    )
    .await
    .unwrap();

    // Request the tracker script using the tracking_id
    let uri = format!("/trace/app_{}.js", service.tracking_id);
    let response = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("Origin", "https://example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Expected 200 OK for tracking_id {}, got {}",
        service.tracking_id,
        response.status()
    );

    // Verify it's JavaScript content
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("javascript"),
        "Expected JavaScript content-type, got {}",
        content_type
    );

    // Verify the script body contains expected content
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(
        body_str.contains("Shymini"),
        "Script should contain 'Shymini'"
    );
    assert!(
        body_str.contains("sendHeartbeat"),
        "Script should contain 'sendHeartbeat'"
    );
    assert!(
        body_str.contains(&format!("/trace/app_{}.js", service.tracking_id)),
        "Script should contain the tracking endpoint URL"
    );
}

#[tokio::test]
async fn test_pixel_served_for_valid_service() {
    use shymini::db;
    use shymini::domain::CreateService;

    let (app, pool) = create_test_app_with_pool().await;

    // Create a service
    let service = db::create_service(
        &pool,
        CreateService {
            name: "Pixel Test Service".to_string(),
            link: "https://example.com".to_string(),
            origins: "*".to_string(),
            respect_dnt: false,
            ignore_robots: false,
            collect_ips: true,
            ignored_ips: String::new(),
            hide_referrer_regex: String::new(),
            script_inject: String::new(),
        },
    )
    .await
    .unwrap();

    // Request the pixel using the tracking_id
    let uri = format!("/trace/px_{}.gif", service.tracking_id);
    let response = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("Origin", "https://example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Expected 200 OK for pixel tracking_id {}, got {}",
        service.tracking_id,
        response.status()
    );

    // Verify it's a GIF
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("image/gif"),
        "Expected image/gif content-type, got {}",
        content_type
    );

    // Verify the body is a valid GIF (starts with GIF magic bytes)
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(body.len() > 6, "GIF should have at least 6 bytes");
    assert_eq!(&body[0..3], b"GIF", "Should start with GIF magic bytes");
}

#[tokio::test]
async fn test_tracking_id_is_stable() {
    use shymini::db;
    use shymini::domain::CreateService;

    let (_, pool) = create_test_app_with_pool().await;

    // Create a service
    let service = db::create_service(
        &pool,
        CreateService {
            name: "Stability Test".to_string(),
            link: String::new(),
            origins: "*".to_string(),
            respect_dnt: false,
            ignore_robots: false,
            collect_ips: true,
            ignored_ips: String::new(),
            hide_referrer_regex: String::new(),
            script_inject: String::new(),
        },
    )
    .await
    .unwrap();

    let original_tracking_id = service.tracking_id.clone();

    // Fetch the service again multiple times
    for _ in 0..5 {
        let fetched = db::get_service(&pool, service.id).await.unwrap();
        assert_eq!(
            fetched.tracking_id.0, original_tracking_id.0,
            "tracking_id should be stable across fetches"
        );
    }

    // Also test via list_services
    let all_services = db::list_services(&pool).await.unwrap();
    let found = all_services.iter().find(|s| s.id == service.id).unwrap();
    assert_eq!(
        found.tracking_id.0, original_tracking_id.0,
        "tracking_id should be stable in list_services"
    );
}

#[tokio::test]
async fn test_migrations_are_idempotent() {
    use shymini::db;

    // Create in-memory SQLite database
    let pool = db::create_pool("sqlite::memory:").await.unwrap();

    // Run migrations multiple times - should not fail
    db::run_migrations(&pool)
        .await
        .expect("First migration run should succeed");
    db::run_migrations(&pool)
        .await
        .expect("Second migration run should succeed");
    db::run_migrations(&pool)
        .await
        .expect("Third migration run should succeed");

    // Verify the schema is correct by creating a service
    let service = db::create_service(
        &pool,
        shymini::domain::CreateService {
            name: "Idempotency Test".to_string(),
            link: String::new(),
            origins: "*".to_string(),
            respect_dnt: false,
            ignore_robots: false,
            collect_ips: true,
            ignored_ips: String::new(),
            hide_referrer_regex: String::new(),
            script_inject: String::new(),
        },
    )
    .await
    .expect("Should be able to create service after multiple migrations");

    // Verify tracking_id was set
    assert!(
        !service.tracking_id.0.is_empty(),
        "Service should have a tracking_id"
    );
}
