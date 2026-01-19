use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{info, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use shymini::{
    api, cache::AppCache, config::Settings, dashboard, db, geo::GeoIpLookup, ingress,
    state::AppState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy(),
        )
        .init();

    // Load configuration
    let settings = Settings::new()?;
    info!("Configuration loaded");

    // Determine database URL
    let db_url = settings
        .database_url
        .clone()
        .or_else(|| {
            settings
                .database_path
                .as_ref()
                .map(|p| format!("sqlite:{}", p))
        })
        .unwrap_or_else(|| {
            #[cfg(feature = "postgres")]
            {
                "postgres://localhost/shymini".to_string()
            }
            #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
            {
                "sqlite:shymini.db?mode=rwc".to_string()
            }
        });

    info!("Connecting to database...");
    let pool = db::create_pool(&db_url).await?;
    info!("Database connected");

    // Run migrations
    info!("Running migrations...");
    db::run_migrations(&pool).await?;
    info!("Migrations complete");

    // Initialize GeoIP
    let geo = GeoIpLookup::new(
        settings.maxmind_city_db.as_deref(),
        settings.maxmind_asn_db.as_deref(),
    )?;
    if geo.is_available() {
        info!("GeoIP lookup available");
    } else {
        info!("GeoIP lookup not available (no database files)");
    }

    // Initialize cache
    let cache = AppCache::new(&settings);
    info!("Cache initialized");

    // Create app state
    let state = AppState::new(pool, cache, settings.clone(), geo);

    // CORS layer
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);

    // Build router
    let app = Router::new()
        // Dashboard routes
        .route("/", get(dashboard::dashboard_index))
        .route("/service/new", get(dashboard::service_create_form))
        .route("/service/new", post(dashboard::service_create))
        .route("/service/:id", get(dashboard::service_detail))
        .route("/service/:id/stats", get(dashboard::stats_partial))
        .route("/service/:id/sessions", get(dashboard::session_list))
        .route(
            "/service/:id/sessions/:session_id",
            get(dashboard::session_detail),
        )
        .route("/service/:id/locations", get(dashboard::location_list))
        .route("/service/:id/manage", get(dashboard::service_update_form))
        .route("/service/:id/manage", post(dashboard::service_update))
        .route("/service/:id/delete", get(dashboard::service_delete_form))
        .route("/service/:id/delete", post(dashboard::service_delete))
        // Ingress routes (using non-obvious paths to avoid ad blockers)
        .route(
            "/trace/px_:tracking_id.gif",
            get(ingress::pixel_handler),
        )
        .route(
            "/trace/px_:tracking_id/:identifier.gif",
            get(ingress::pixel_with_id_handler),
        )
        .route(
            "/trace/app_:tracking_id.js",
            get(ingress::script_get_handler).post(ingress::script_post_handler),
        )
        .route(
            "/trace/app_:tracking_id/:identifier.js",
            get(ingress::script_get_with_id_handler).post(ingress::script_post_with_id_handler),
        )
        // API routes
        .route("/api/services", get(api::list_services))
        .route("/api/services/:id", get(api::get_service))
        .route("/api/services/:id/stats", get(api::get_service_stats))
        .route("/api/services/:id/sessions", get(api::list_sessions))
        .route("/api/sessions/:id", get(api::get_session))
        .route("/api/sessions/:id/hits", get(api::list_session_hits))
        // Static files
        .nest_service("/static", ServeDir::new("static"))
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::new(
        settings.host.parse().unwrap_or([0, 0, 0, 0].into()),
        settings.port,
    );
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
