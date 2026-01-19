//! Criterion benchmarks for shymini analytics queries
//!
//! Run with: cargo bench
//!
//! Requires a seeded database. Create one with:
//!   cargo run --release --bin loadtest -- seed --db bench.db
//!
//! Set the database path:
//!   SHYMINI_BENCH_DB=./bench.db cargo bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use tokio::runtime::Runtime;

async fn create_pool(db_path: &str) -> Pool<Sqlite> {
    let options = SqliteConnectOptions::from_str(db_path)
        .unwrap()
        .create_if_missing(false);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("Failed to connect to database")
}

async fn get_top_service(pool: &Pool<Sqlite>) -> String {
    let (id,): (String,) = sqlx::query_as(
        "SELECT id FROM services ORDER BY (SELECT COUNT(*) FROM hits WHERE hits.service_id = services.id) DESC LIMIT 1"
    )
    .fetch_one(pool)
    .await
    .expect("No services found - run seeding first");
    id
}

fn bench_session_count(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let db_path =
        std::env::var("SHYMINI_BENCH_DB").unwrap_or_else(|_| "sqlite:bench.db".to_string());
    let pool = rt.block_on(create_pool(&db_path));
    let service_id = rt.block_on(get_top_service(&pool));

    let now = chrono::Utc::now();
    let thirty_days_ago = now - chrono::Duration::days(30);
    let seven_days_ago = now - chrono::Duration::days(7);

    let mut group = c.benchmark_group("session_count");

    group.bench_function(BenchmarkId::new("30_days", "high_traffic"), |b| {
        b.to_async(&rt).iter(|| async {
            let count: i32 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ?"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_one(&pool)
            .await
            .unwrap();
            black_box(count)
        });
    });

    group.bench_function(BenchmarkId::new("7_days", "high_traffic"), |b| {
        b.to_async(&rt).iter(|| async {
            let count: i32 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ?"
            )
            .bind(&service_id)
            .bind(seven_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_one(&pool)
            .await
            .unwrap();
            black_box(count)
        });
    });

    group.finish();
}

fn bench_hit_count(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let db_path =
        std::env::var("SHYMINI_BENCH_DB").unwrap_or_else(|_| "sqlite:bench.db".to_string());
    let pool = rt.block_on(create_pool(&db_path));
    let service_id = rt.block_on(get_top_service(&pool));

    let now = chrono::Utc::now();
    let thirty_days_ago = now - chrono::Duration::days(30);

    c.bench_function("hit_count_30d", |b| {
        b.to_async(&rt).iter(|| async {
            let count: i32 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ?"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_one(&pool)
            .await
            .unwrap();
            black_box(count)
        });
    });
}

fn bench_top_locations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let db_path =
        std::env::var("SHYMINI_BENCH_DB").unwrap_or_else(|_| "sqlite:bench.db".to_string());
    let pool = rt.block_on(create_pool(&db_path));
    let service_id = rt.block_on(get_top_service(&pool));

    let now = chrono::Utc::now();
    let thirty_days_ago = now - chrono::Duration::days(30);

    c.bench_function("top_locations_30d", |b| {
        b.to_async(&rt).iter(|| async {
            let locations: Vec<(String, i32)> = sqlx::query_as(
                "SELECT location, COUNT(*) as count FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ? GROUP BY location ORDER BY count DESC LIMIT 10"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_all(&pool)
            .await
            .unwrap();
            black_box(locations)
        });
    });
}

fn bench_browser_breakdown(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let db_path =
        std::env::var("SHYMINI_BENCH_DB").unwrap_or_else(|_| "sqlite:bench.db".to_string());
    let pool = rt.block_on(create_pool(&db_path));
    let service_id = rt.block_on(get_top_service(&pool));

    let now = chrono::Utc::now();
    let thirty_days_ago = now - chrono::Duration::days(30);

    c.bench_function("browser_breakdown_30d", |b| {
        b.to_async(&rt).iter(|| async {
            let browsers: Vec<(String, i32)> = sqlx::query_as(
                "SELECT browser, COUNT(*) as count FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ? GROUP BY browser ORDER BY count DESC"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_all(&pool)
            .await
            .unwrap();
            black_box(browsers)
        });
    });
}

fn bench_daily_chart(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let db_path =
        std::env::var("SHYMINI_BENCH_DB").unwrap_or_else(|_| "sqlite:bench.db".to_string());
    let pool = rt.block_on(create_pool(&db_path));
    let service_id = rt.block_on(get_top_service(&pool));

    let now = chrono::Utc::now();
    let thirty_days_ago = now - chrono::Duration::days(30);

    c.bench_function("daily_chart_30d", |b| {
        b.to_async(&rt).iter(|| async {
            let data: Vec<(String, i32, i32)> = sqlx::query_as(
                r#"
                SELECT
                    date(start_time) as day,
                    COUNT(DISTINCT session_id) as sessions,
                    COUNT(*) as hits
                FROM hits
                WHERE service_id = ? AND start_time >= ? AND start_time < ?
                GROUP BY day
                ORDER BY day
                "#,
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_all(&pool)
            .await
            .unwrap();
            black_box(data)
        });
    });
}

fn bench_sessions_list(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let db_path =
        std::env::var("SHYMINI_BENCH_DB").unwrap_or_else(|_| "sqlite:bench.db".to_string());
    let pool = rt.block_on(create_pool(&db_path));
    let service_id = rt.block_on(get_top_service(&pool));

    let mut group = c.benchmark_group("sessions_list");

    group.bench_function(BenchmarkId::new("page", "1"), |b| {
        b.to_async(&rt).iter(|| async {
            let sessions: Vec<(String, String, String, String, String)> = sqlx::query_as(
                "SELECT id, browser, os, country, device_type FROM sessions WHERE service_id = ? ORDER BY start_time DESC LIMIT 25 OFFSET 0"
            )
            .bind(&service_id)
            .fetch_all(&pool)
            .await
            .unwrap();
            black_box(sessions)
        });
    });

    group.bench_function(BenchmarkId::new("page", "10"), |b| {
        b.to_async(&rt).iter(|| async {
            let sessions: Vec<(String, String, String, String, String)> = sqlx::query_as(
                "SELECT id, browser, os, country, device_type FROM sessions WHERE service_id = ? ORDER BY start_time DESC LIMIT 25 OFFSET 225"
            )
            .bind(&service_id)
            .fetch_all(&pool)
            .await
            .unwrap();
            black_box(sessions)
        });
    });

    group.finish();
}

fn bench_full_dashboard_stats(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let db_path =
        std::env::var("SHYMINI_BENCH_DB").unwrap_or_else(|_| "sqlite:bench.db".to_string());
    let pool = rt.block_on(create_pool(&db_path));
    let service_id = rt.block_on(get_top_service(&pool));

    let now = chrono::Utc::now();
    let thirty_days_ago = now - chrono::Duration::days(30);

    // This benchmarks all the queries needed for a full dashboard load
    c.bench_function("full_dashboard_30d", |b| {
        b.to_async(&rt).iter(|| async {
            // Session count
            let session_count: i32 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ?"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_one(&pool)
            .await
            .unwrap();

            // Hit count
            let hit_count: i32 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ?"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_one(&pool)
            .await
            .unwrap();

            // Bounce count
            let bounce_count: i32 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ? AND is_bounce = 1"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_one(&pool)
            .await
            .unwrap();

            // Avg load time
            let avg_load: Option<f64> = sqlx::query_scalar(
                "SELECT AVG(load_time) FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ? AND load_time IS NOT NULL"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_one(&pool)
            .await
            .unwrap();

            // Top locations
            let locations: Vec<(String, i32)> = sqlx::query_as(
                "SELECT location, COUNT(*) as count FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ? GROUP BY location ORDER BY count DESC LIMIT 10"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_all(&pool)
            .await
            .unwrap();

            // Browser breakdown
            let browsers: Vec<(String, i32)> = sqlx::query_as(
                "SELECT browser, COUNT(*) as count FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ? GROUP BY browser ORDER BY count DESC"
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_all(&pool)
            .await
            .unwrap();

            // Daily chart
            let chart: Vec<(String, i32, i32)> = sqlx::query_as(
                r#"
                SELECT
                    date(start_time) as day,
                    COUNT(DISTINCT session_id) as sessions,
                    COUNT(*) as hits
                FROM hits
                WHERE service_id = ? AND start_time >= ? AND start_time < ?
                GROUP BY day
                ORDER BY day
                "#
            )
            .bind(&service_id)
            .bind(thirty_days_ago.to_rfc3339())
            .bind(now.to_rfc3339())
            .fetch_all(&pool)
            .await
            .unwrap();

            black_box((session_count, hit_count, bounce_count, avg_load, locations, browsers, chart))
        });
    });
}

criterion_group!(
    benches,
    bench_session_count,
    bench_hit_count,
    bench_top_locations,
    bench_browser_breakdown,
    bench_daily_chart,
    bench_sessions_list,
    bench_full_dashboard_stats,
);

criterion_main!(benches);
