//! Load Test Data Seeder and Benchmark for shymini
//!
//! Creates services with configurable hits and sessions per service.
//! Default: 5 services × 100k hits/service × 10k sessions/service = 500k total hits.
//!
//! # Usage
//!
//! ```bash
//! # Seed a persistent database (default: loadtest.db)
//! cargo run --release --bin loadtest -- seed
//!
//! # Seed with custom settings
//! cargo run --release --bin loadtest -- seed --db ./my-test.db --hits 200000 --sessions 20000 --services 5
//!
//! # Run benchmarks on existing database
//! cargo run --release --bin loadtest -- bench --db ./loadtest.db
//!
//! # Seed and immediately benchmark
//! cargo run --release --bin loadtest -- seed --bench
//!
//! # Then start the server with this database:
//! SHYMINI__DATABASE_PATH=./loadtest.db cargo run --release
//! ```

use chrono::{DateTime, Duration, Utc};
use rand::prelude::*;
use rand_distr::Exp;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

const SERVICE_NAMES: &[&str] = &[
    "Main Website",
    "Blog",
    "Documentation",
    "API Portal",
    "Customer Dashboard",
    "Marketing Site",
    "Landing Page A",
    "Landing Page B",
    "Product Demo",
    "Support Portal",
    "Developer Hub",
    "Community Forum",
    "Status Page",
    "Changelog",
    "Partner Portal",
    "Mobile App Site",
    "Enterprise Portal",
    "Free Tier Site",
    "Pricing Calculator",
    "ROI Tool",
    "Webinar Page",
    "Event Registration",
    "Newsletter Signup",
    "Careers Page",
    "Press Room",
    "Investor Relations",
    "Security Info",
    "Compliance Docs",
    "Training Portal",
    "Integrations",
];

const PAGES: &[&str] = &[
    "/",
    "/about",
    "/pricing",
    "/features",
    "/contact",
    "/blog",
    "/docs",
    "/blog/getting-started",
    "/blog/advanced-tips",
    "/blog/release-notes",
    "/docs/api",
    "/docs/quickstart",
    "/docs/faq",
    "/docs/troubleshooting",
    "/products",
    "/products/pro",
    "/products/enterprise",
    "/products/free",
    "/signup",
    "/login",
    "/dashboard",
    "/settings",
    "/profile",
    "/demo",
    "/tour",
    "/case-studies",
    "/testimonials",
    "/team",
];

const REFERRERS: &[&str] = &[
    "",
    "",
    "",
    "",
    "https://google.com/search?q=analytics",
    "https://google.com/search?q=web+tracking",
    "https://google.com/search?q=privacy+analytics",
    "https://duckduckgo.com/?q=shymini",
    "https://bing.com/search?q=analytics",
    "https://twitter.com/someone/status/123",
    "https://reddit.com/r/selfhosted",
    "https://reddit.com/r/webdev",
    "https://news.ycombinator.com/item?id=12345",
    "https://github.com/cdaringe/shymini",
    "https://linkedin.com/feed",
    "https://facebook.com",
    "https://dev.to/article/analytics",
    "https://medium.com/@author/post",
];

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/120.0.0.0",
    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 13; SM-S918B) AppleWebKit/537.36 Mobile Safari/537.36",
];

const COUNTRIES: &[&str] = &[
    "US", "US", "US", "US", "US", // US weighted higher
    "GB", "GB", "DE", "DE", "FR", "CA", "AU", "NL", "SE", "JP", "BR", "IN", "MX", "ES", "IT",
];

const BROWSERS: &[&str] = &[
    "Chrome",
    "Chrome",
    "Chrome",
    "Chrome", // Chrome weighted higher
    "Firefox",
    "Firefox",
    "Safari",
    "Safari",
    "Edge",
    "Mobile Safari",
    "Chrome Mobile",
];

const OPERATING_SYSTEMS: &[&str] = &[
    "Windows", "Windows", "Windows", // Windows weighted higher
    "macOS", "macOS", "Linux", "iOS", "Android", "Android",
];

const DEVICE_TYPES: &[&str] = &[
    "Desktop", "Desktop", "Desktop", "Desktop", // Desktop weighted higher
    "Mobile", "Mobile", "Tablet",
];

/// Generate a random IP address with some clustering
fn random_ip(rng: &mut impl Rng) -> String {
    let subnets = [
        "192.168.1",
        "10.0.0",
        "172.16.0",
        "45.33",
        "104.236",
        "159.89",
        "167.99",
        "68.183",
        "35.192",
        "34.102",
        "52.14",
        "18.216",
    ];
    let subnet = subnets[rng.gen_range(0..subnets.len())];
    format!("{}.{}", subnet, rng.gen_range(1..255))
}

/// Generate a random datetime within the last N days, weighted toward recent
fn random_recent_datetime(rng: &mut impl Rng, days_back: u32) -> DateTime<Utc> {
    let now = Utc::now();
    let max_ms = (days_back as i64) * 24 * 60 * 60 * 1000;
    // Exponential distribution favoring recent dates
    let exp = Exp::new(3.0 / max_ms as f64).unwrap();
    let offset_ms = (exp.sample(rng) as i64).min(max_ms);
    now - Duration::milliseconds(offset_ms)
}

struct ServiceData {
    id: Uuid,
    tracking_id: String,
    name: String,
}

/// Generate a random 8-character alphanumeric tracking ID
fn generate_tracking_id() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[derive(Clone)]
struct SessionData {
    id: Uuid,
    service_id: Uuid,
    ip: Option<String>,
    user_agent: String,
    browser: String,
    os: String,
    device_type: String,
    country: String,
    start_time: DateTime<Utc>,
}

async fn create_pool(db_path: &str) -> Pool<Sqlite> {
    let options = SqliteConnectOptions::from_str(db_path)
        .unwrap()
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(options)
        .await
        .expect("Failed to create pool")
}

async fn run_migrations(pool: &Pool<Sqlite>) {
    sqlx::migrate!("./migrations/sqlite")
        .run(pool)
        .await
        .expect("Failed to run migrations");

    // Optimize for bulk inserts
    sqlx::query("PRAGMA cache_size = -64000") // 64MB cache
        .execute(pool)
        .await
        .expect("Failed to set cache_size");
    sqlx::query("PRAGMA temp_store = MEMORY")
        .execute(pool)
        .await
        .expect("Failed to set temp_store");
}

async fn seed_database(
    pool: &Pool<Sqlite>,
    num_services: usize,
    hits_per_service: u64,
    sessions_per_service: usize,
    days_back: u32,
) -> Vec<ServiceData> {
    let mut rng = rand::thread_rng();

    println!("Creating {} services...", num_services);
    let start = Instant::now();

    // Create services (no weighted distribution - each service gets same amount)
    let mut services: Vec<ServiceData> = Vec::with_capacity(num_services);

    for i in 0..num_services {
        let id = Uuid::new_v4();
        let tracking_id = generate_tracking_id();
        let name = SERVICE_NAMES
            .get(i)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Service {}", i + 1));

        sqlx::query(
            r#"
            INSERT INTO services (id, tracking_id, name, link, origins, status, respect_dnt, ignore_robots, collect_ips,
                                  ignored_ips, hide_referrer_regex, script_inject, created_at)
            VALUES (?, ?, ?, '', '*', 'AC', 1, 1, 0, '', '', '', datetime('now'))
            "#,
        )
        .bind(id.to_string())
        .bind(&tracking_id)
        .bind(&name)
        .execute(pool)
        .await
        .expect("Failed to create service");

        services.push(ServiceData {
            id,
            tracking_id,
            name,
        });
    }

    println!(
        "  Created {} services in {:?}",
        num_services,
        start.elapsed()
    );

    // Print plan
    let total_hits = hits_per_service * num_services as u64;
    let total_sessions = sessions_per_service * num_services;
    println!("\nData plan:");
    println!(
        "  {} hits per service x {} services = {} total hits",
        hits_per_service, num_services, total_hits
    );
    println!(
        "  {} sessions per service x {} services = {} total sessions",
        sessions_per_service, num_services, total_sessions
    );
    println!(
        "  {} hits per session average",
        hits_per_service / sessions_per_service as u64
    );
    println!("  Time range: last {} days", days_back);
    println!();

    // Pre-generate session pools per service (same count for each)
    println!(
        "Generating session pools ({} sessions per service)...",
        sessions_per_service
    );
    let mut session_pools: HashMap<Uuid, Vec<SessionData>> = HashMap::new();
    for service in &services {
        let mut sessions = Vec::with_capacity(sessions_per_service);

        for _ in 0..sessions_per_service {
            let ip = random_ip(&mut rng);
            let user_agent = USER_AGENTS[rng.gen_range(0..USER_AGENTS.len())].to_string();
            let browser = BROWSERS[rng.gen_range(0..BROWSERS.len())].to_string();
            let os = OPERATING_SYSTEMS[rng.gen_range(0..OPERATING_SYSTEMS.len())].to_string();
            let device_type = DEVICE_TYPES[rng.gen_range(0..DEVICE_TYPES.len())].to_string();
            let country = COUNTRIES[rng.gen_range(0..COUNTRIES.len())].to_string();
            let start_time = random_recent_datetime(&mut rng, days_back);

            sessions.push(SessionData {
                id: Uuid::new_v4(),
                service_id: service.id,
                ip: Some(ip),
                user_agent,
                browser,
                os,
                device_type,
                country,
                start_time,
            });
        }
        session_pools.insert(service.id, sessions);
    }
    println!(
        "  Generated {} total sessions",
        session_pools.values().map(|v| v.len()).sum::<usize>()
    );

    // Insert all sessions in batch
    println!("\nInserting sessions...");
    let session_start = Instant::now();
    let mut total_sessions = 0u64;

    for sessions in session_pools.values() {
        for chunk in sessions.chunks(500) {
            let mut query = String::from(
                "INSERT INTO sessions (id, service_id, identifier, start_time, last_seen, user_agent, browser, device, device_type, os, ip, asn, country, is_bounce) VALUES "
            );
            let mut values: Vec<Option<String>> = Vec::new();
            for (i, s) in chunk.iter().enumerate() {
                if i > 0 {
                    query.push_str(", ");
                }
                query.push_str("(?, ?, '', ?, ?, ?, ?, '', ?, ?, ?, '', ?, 1)");
                values.push(Some(s.id.to_string()));
                values.push(Some(s.service_id.to_string()));
                values.push(Some(s.start_time.to_rfc3339()));
                values.push(Some(s.start_time.to_rfc3339()));
                values.push(Some(s.user_agent.clone()));
                values.push(Some(s.browser.clone()));
                values.push(Some(s.device_type.clone()));
                values.push(Some(s.os.clone()));
                values.push(s.ip.clone());
                values.push(Some(s.country.clone()));
            }

            let mut q = sqlx::query(&query);
            for v in &values {
                q = q.bind(v);
            }
            q.execute(pool).await.expect("Failed to insert sessions");
            total_sessions += chunk.len() as u64;
        }
    }
    println!(
        "  Inserted {} sessions in {:?}",
        total_sessions,
        session_start.elapsed()
    );

    // Generate and insert hits (hits_per_service for EACH service)
    let total_hits = hits_per_service * services.len() as u64;
    println!(
        "\nGenerating {} hits ({} per service)...",
        total_hits, hits_per_service
    );
    let hit_start = Instant::now();
    let completed = Arc::new(AtomicU64::new(0));
    let batch_size = 1000u64;

    // (service_id, session_id, location, referrer, load_time, start_time)
    let mut hits_batch: Vec<(Uuid, Uuid, String, String, i32, DateTime<Utc>)> =
        Vec::with_capacity(batch_size as usize);

    // Track hits per session for bounce calculation
    let mut hits_per_session_count: HashMap<Uuid, u32> = HashMap::new();

    // Generate hits for each service
    for service in &services {
        let sessions = session_pools.get(&service.id).unwrap();

        for _ in 0..hits_per_service {
            // Select random session from this service
            let session = &sessions[rng.gen_range(0..sessions.len())];

            // Generate hit data
            let location = format!(
                "https://{}.example.com{}",
                service.name.to_lowercase().replace(' ', "-"),
                PAGES[rng.gen_range(0..PAGES.len())]
            );
            let referrer = REFERRERS[rng.gen_range(0..REFERRERS.len())].to_string();
            let load_time = rng.gen_range(100..2100);

            // Hit time is sometime after session start
            let session_age_ms = (Utc::now() - session.start_time).num_milliseconds().max(1);
            let hit_offset_ms = rng.gen_range(0..session_age_ms.min(3600000)); // up to 1 hour after session start
            let hit_time = session.start_time + Duration::milliseconds(hit_offset_ms);

            hits_batch.push((
                service.id, session.id, location, referrer, load_time, hit_time,
            ));

            *hits_per_session_count.entry(session.id).or_insert(0) += 1;

            // Insert batch
            if hits_batch.len() >= batch_size as usize {
                let mut query = String::from(
                    "INSERT INTO hits (session_id, service_id, initial, start_time, last_seen, heartbeats, tracker, location, referrer, load_time) VALUES "
                );
                for (j, _) in hits_batch.iter().enumerate() {
                    if j > 0 {
                        query.push_str(", ");
                    }
                    query.push_str("(?, ?, 0, ?, ?, 1, 'JS', ?, ?, ?)");
                }

                let mut q = sqlx::query(&query);
                for h in &hits_batch {
                    q = q
                        .bind(h.1.to_string()) // session_id
                        .bind(h.0.to_string()) // service_id
                        .bind(h.5.to_rfc3339()) // start_time
                        .bind(h.5.to_rfc3339()) // last_seen
                        .bind(&h.2) // location
                        .bind(&h.3) // referrer
                        .bind(h.4); // load_time
                }
                q.execute(pool).await.expect("Failed to insert hits");

                let done = completed.fetch_add(hits_batch.len() as u64, Ordering::Relaxed)
                    + hits_batch.len() as u64;
                let elapsed = hit_start.elapsed().as_secs_f64();
                let rate = done as f64 / elapsed;
                let remaining = (total_hits - done) as f64 / rate;

                print!(
                    "\r  Progress: {}/{} ({:.1}%) | {:.0} hits/sec | ETA: {:.0}s    ",
                    done,
                    total_hits,
                    (done as f64 / total_hits as f64) * 100.0,
                    rate,
                    remaining
                );

                hits_batch.clear();
            }
        }
    }

    // Insert any remaining hits
    if !hits_batch.is_empty() {
        let mut query = String::from(
            "INSERT INTO hits (session_id, service_id, initial, start_time, last_seen, heartbeats, tracker, location, referrer, load_time) VALUES "
        );
        for (j, _) in hits_batch.iter().enumerate() {
            if j > 0 {
                query.push_str(", ");
            }
            query.push_str("(?, ?, 0, ?, ?, 1, 'JS', ?, ?, ?)");
        }

        let mut q = sqlx::query(&query);
        for h in &hits_batch {
            q = q
                .bind(h.1.to_string())
                .bind(h.0.to_string())
                .bind(h.5.to_rfc3339())
                .bind(h.5.to_rfc3339())
                .bind(&h.2)
                .bind(&h.3)
                .bind(h.4);
        }
        q.execute(pool).await.expect("Failed to insert hits");
    }

    println!(
        "\n  Inserted {} hits in {:?}",
        total_hits,
        hit_start.elapsed()
    );

    // Update bounce status for sessions with multiple hits
    println!("\nUpdating bounce status...");
    let bounce_start = Instant::now();
    let non_bounce_sessions: Vec<String> = hits_per_session_count
        .iter()
        .filter(|(_, &count)| count > 1)
        .map(|(id, _)| id.to_string())
        .collect();

    for chunk in non_bounce_sessions.chunks(500) {
        let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
        let query = format!(
            "UPDATE sessions SET is_bounce = 0 WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query(&query);
        for id in chunk {
            q = q.bind(id);
        }
        q.execute(pool)
            .await
            .expect("Failed to update bounce status");
    }
    println!(
        "  Updated {} sessions with multiple hits in {:?}",
        non_bounce_sessions.len(),
        bounce_start.elapsed()
    );

    // Summary
    let total_time = start.elapsed();
    println!("\n{}", "=".repeat(60));
    println!("Seeding complete!");
    println!("{}", "=".repeat(60));
    println!("  Total time: {:?}", total_time);
    println!("  Services: {}", num_services);
    println!("  Sessions: {}", total_sessions);
    println!("  Hits: {}", total_hits);
    println!(
        "  Insert rate: {:.0} hits/sec",
        total_hits as f64 / total_time.as_secs_f64()
    );

    services
}

async fn run_benchmarks(pool: &Pool<Sqlite>) {
    // Get services for benchmarking
    let services: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM services ORDER BY (SELECT COUNT(*) FROM hits WHERE hits.service_id = services.id) DESC"
    )
    .fetch_all(pool)
    .await
    .expect("Failed to fetch services");

    if services.is_empty() {
        eprintln!("No services found. Run seeding first.");
        return;
    }

    let top_service = &services[0];
    let mid_service = &services[services.len() / 2];
    let low_service = services.last().unwrap();

    println!("\n{}", "=".repeat(70));
    println!("Running Benchmarks");
    println!("{}", "=".repeat(70));
    println!("Test services:");
    println!("  High traffic: {} ({})", top_service.1, top_service.0);
    println!("  Mid traffic:  {} ({})", mid_service.1, mid_service.0);
    println!("  Low traffic:  {} ({})", low_service.1, low_service.0);
    println!();

    let iterations = 50;
    let now = Utc::now();
    let thirty_days_ago = now - Duration::days(30);

    struct BenchResult {
        name: String,
        times: Vec<f64>,
    }

    impl BenchResult {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                times: Vec::new(),
            }
        }

        fn mean(&self) -> f64 {
            self.times.iter().sum::<f64>() / self.times.len() as f64
        }

        fn median(&self) -> f64 {
            let mut sorted = self.times.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            sorted[sorted.len() / 2]
        }

        fn p95(&self) -> f64 {
            let mut sorted = self.times.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            sorted[(sorted.len() as f64 * 0.95) as usize]
        }

        fn p99(&self) -> f64 {
            let mut sorted = self.times.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            sorted[(sorted.len() as f64 * 0.99).min(sorted.len() as f64 - 1.0) as usize]
        }

        fn max(&self) -> f64 {
            *self
                .times
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap()
        }
    }

    let mut results: Vec<BenchResult> = Vec::new();

    // Benchmark: Session count query
    println!("1/8 Session count (high traffic, 30 days)...");
    let mut bench = BenchResult::new("Session count (30d)");
    for _ in 0..iterations {
        let start = Instant::now();
        let _: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ?"
        )
        .bind(&top_service.0)
        .bind(thirty_days_ago.to_rfc3339())
        .bind(now.to_rfc3339())
        .fetch_one(pool)
        .await
        .unwrap();
        bench.times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    results.push(bench);

    // Benchmark: Hit count query
    println!("2/8 Hit count (high traffic, 30 days)...");
    let mut bench = BenchResult::new("Hit count (30d)");
    for _ in 0..iterations {
        let start = Instant::now();
        let _: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ?",
        )
        .bind(&top_service.0)
        .bind(thirty_days_ago.to_rfc3339())
        .bind(now.to_rfc3339())
        .fetch_one(pool)
        .await
        .unwrap();
        bench.times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    results.push(bench);

    // Benchmark: Top locations
    println!("3/8 Top locations (high traffic, 30 days)...");
    let mut bench = BenchResult::new("Top locations (30d)");
    for _ in 0..iterations {
        let start = Instant::now();
        let _: Vec<(String, i32)> = sqlx::query_as(
            "SELECT location, COUNT(*) as count FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ? GROUP BY location ORDER BY count DESC LIMIT 10"
        )
        .bind(&top_service.0)
        .bind(thirty_days_ago.to_rfc3339())
        .bind(now.to_rfc3339())
        .fetch_all(pool)
        .await
        .unwrap();
        bench.times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    results.push(bench);

    // Benchmark: Bounce rate calculation
    println!("4/8 Bounce rate (high traffic, 30 days)...");
    let mut bench = BenchResult::new("Bounce rate (30d)");
    for _ in 0..iterations {
        let start = Instant::now();
        let _: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ? AND is_bounce = 1"
        )
        .bind(&top_service.0)
        .bind(thirty_days_ago.to_rfc3339())
        .bind(now.to_rfc3339())
        .fetch_one(pool)
        .await
        .unwrap();
        bench.times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    results.push(bench);

    // Benchmark: Average load time
    println!("5/8 Avg load time (high traffic, 30 days)...");
    let mut bench = BenchResult::new("Avg load time (30d)");
    for _ in 0..iterations {
        let start = Instant::now();
        let _: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(load_time) FROM hits WHERE service_id = ? AND start_time >= ? AND start_time < ? AND load_time IS NOT NULL"
        )
        .bind(&top_service.0)
        .bind(thirty_days_ago.to_rfc3339())
        .bind(now.to_rfc3339())
        .fetch_one(pool)
        .await
        .unwrap();
        bench.times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    results.push(bench);

    // Benchmark: Browser breakdown
    println!("6/8 Browser breakdown (high traffic, 30 days)...");
    let mut bench = BenchResult::new("Browser breakdown (30d)");
    for _ in 0..iterations {
        let start = Instant::now();
        let _: Vec<(String, i32)> = sqlx::query_as(
            "SELECT browser, COUNT(*) as count FROM sessions WHERE service_id = ? AND start_time >= ? AND start_time < ? GROUP BY browser ORDER BY count DESC"
        )
        .bind(&top_service.0)
        .bind(thirty_days_ago.to_rfc3339())
        .bind(now.to_rfc3339())
        .fetch_all(pool)
        .await
        .unwrap();
        bench.times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    results.push(bench);

    // Benchmark: Daily chart data
    println!("7/8 Daily chart data (high traffic, 30 days)...");
    let mut bench = BenchResult::new("Daily chart (30d)");
    for _ in 0..iterations {
        let start = Instant::now();
        let _: Vec<(String, i32, i32)> = sqlx::query_as(
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
        .bind(&top_service.0)
        .bind(thirty_days_ago.to_rfc3339())
        .bind(now.to_rfc3339())
        .fetch_all(pool)
        .await
        .unwrap();
        bench.times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    results.push(bench);

    // Benchmark: Sessions list with pagination
    println!("8/8 Sessions list (page 1, limit 25)...");
    let mut bench = BenchResult::new("Sessions list (pg 1)");
    for _ in 0..iterations {
        let start = Instant::now();
        let _: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, browser, os, country, device_type FROM sessions WHERE service_id = ? ORDER BY start_time DESC LIMIT 25 OFFSET 0"
        )
        .bind(&top_service.0)
        .fetch_all(pool)
        .await
        .unwrap();
        bench.times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    results.push(bench);

    // Print results
    println!("\n{}", "=".repeat(80));
    println!("BENCHMARK RESULTS ({} iterations each)", iterations);
    println!("{}", "=".repeat(80));
    println!(
        "{:30} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Query", "Mean", "Median", "P95", "P99", "Max"
    );
    println!("{}", "-".repeat(80));

    for r in &results {
        println!(
            "{:30} {:>9.2}ms {:>9.2}ms {:>9.2}ms {:>9.2}ms {:>9.2}ms",
            r.name,
            r.mean(),
            r.median(),
            r.p95(),
            r.p99(),
            r.max()
        );
    }
    println!("{}", "-".repeat(80));

    // Summary
    let total_mean: f64 = results.iter().map(|r| r.mean()).sum();
    println!("\nTotal dashboard load (sum of means): {:.2}ms", total_mean);

    if total_mean < 100.0 {
        println!("Performance: EXCELLENT (< 100ms total)");
    } else if total_mean < 500.0 {
        println!("Performance: GOOD (< 500ms total)");
    } else if total_mean < 1000.0 {
        println!("Performance: ACCEPTABLE (< 1s total)");
    } else {
        println!("Performance: NEEDS OPTIMIZATION (> 1s total)");
    }

    let slowest = results
        .iter()
        .max_by(|a, b| a.mean().partial_cmp(&b.mean()).unwrap())
        .unwrap();
    println!(
        "\nSlowest query: {} ({:.2}ms)",
        slowest.name,
        slowest.mean()
    );
}

fn print_usage() {
    eprintln!(
        r#"
Usage: loadtest <command> [options]

Commands:
  seed     Seed the database with test data
  bench    Run benchmarks on existing database

Options for 'seed':
  --db <path>       Database path (default: loadtest.db)
  --hits <n>        Hits PER SERVICE (default: 100000)
  --sessions <n>    Sessions PER SERVICE (default: 10000)
  --services <n>    Number of services (default: 5)
  --days <n>        Days of history to generate (default: 7)
  --bench           Run benchmarks after seeding

Options for 'bench':
  --db <path>       Database path (default: loadtest.db)

Examples:
  cargo run --release --bin loadtest -- seed
  cargo run --release --bin loadtest -- seed --hits 100000 --sessions 10000 --services 5 --bench
  cargo run --release --bin loadtest -- bench --db ./loadtest.db

After seeding, start the server with:
  SHYMINI__DATABASE_PATH=./loadtest.db cargo run --release
"#
    );
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let command = &args[1];
    let mut db_path = PathBuf::from("loadtest.db");
    let mut hits_per_service = 100_000u64;
    let mut num_services = 5usize;
    let mut days_back = 7u32;
    let mut sessions_per_service = 10_000usize;
    let mut run_bench = false;

    // Parse arguments
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--db" => {
                i += 1;
                db_path = PathBuf::from(&args[i]);
            }
            "--hits" => {
                i += 1;
                hits_per_service = args[i].parse().expect("Invalid hits count");
            }
            "--services" => {
                i += 1;
                num_services = args[i].parse().expect("Invalid services count");
            }
            "--days" => {
                i += 1;
                days_back = args[i].parse().expect("Invalid days count");
            }
            "--sessions" => {
                i += 1;
                sessions_per_service = args[i].parse().expect("Invalid sessions count");
            }
            "--bench" => {
                run_bench = true;
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                print_usage();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let db_url = format!("sqlite:{}", db_path.display());

    match command.as_str() {
        "seed" => {
            println!("{}", "=".repeat(60));
            println!("Shymini Load Test - Data Seeder");
            println!("{}", "=".repeat(60));
            println!("Database: {}", db_path.display());
            println!("Services: {}", num_services);
            println!("Hits per service: {}", hits_per_service);
            println!("Sessions per service: {}", sessions_per_service);
            println!("Days of history: {}", days_back);
            println!();

            let pool = create_pool(&db_url).await;
            run_migrations(&pool).await;

            let services = seed_database(
                &pool,
                num_services,
                hits_per_service,
                sessions_per_service,
                days_back,
            )
            .await;

            println!("\nTop service for viewing:");
            println!(
                "  {} (tracking_id: {})",
                services[0].name, services[0].tracking_id
            );
            println!("\nTracker script URL:");
            println!(
                "  http://localhost:8080/trace/app_{}.js",
                services[0].tracking_id
            );
            println!("\nStart the server with:");
            println!(
                "  SHYMINI__DATABASE_PATH={} cargo run --release",
                db_path.display()
            );

            if run_bench {
                run_benchmarks(&pool).await;
            }
        }
        "bench" => {
            if !db_path.exists() {
                eprintln!("Database not found: {}", db_path.display());
                eprintln!("Run seeding first: cargo run --release --bin loadtest -- seed");
                std::process::exit(1);
            }

            let pool = create_pool(&db_url).await;
            run_benchmarks(&pool).await;
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
            std::process::exit(1);
        }
    }
}
