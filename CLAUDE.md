# CLAUDE.md - Development Guide for shymini

## Quick Start

```bash
# Run with SQLite (default)
cargo run

# The server starts at http://localhost:8080
# SQLite database is created at ./shymini.db
```

## Project Overview

shymini is a privacy-friendly web analytics platform written in Rust.

### Stack
- **Web Framework:** Axum 0.7
- **Database:** SQLite (default) or PostgreSQL via feature flags
- **Templates:** Askama + HTMX
- **Cache:** Moka (async in-memory)
- **GeoIP:** MaxMind GeoLite2 (optional)
- **UA Parsing:** Woothee

## Configuration

Environment variables (prefix: `SHYMINI__`):

| Variable | Default | Description |
|----------|---------|-------------|
| `SHYMINI__HOST` | `0.0.0.0` | Bind address |
| `SHYMINI__PORT` | `8080` | Port |
| `SHYMINI__DATABASE_PATH` | `shymini.db` | SQLite file path |
| `SHYMINI__DATABASE_URL` | - | Full DB URL (overrides path) |
| `SHYMINI__MAXMIND_CITY_DB` | - | Path to GeoLite2-City.mmdb |
| `SHYMINI__MAXMIND_ASN_DB` | - | Path to GeoLite2-ASN.mmdb |
| `SHYMINI__SCRIPT_HEARTBEAT_FREQUENCY_MS` | `5000` | JS heartbeat interval |
| `SHYMINI__CACHE_MAX_ENTRIES` | `10000` | Max cache entries |
| `SHYMINI__CACHE_TTL_SECS` | `3600` | Cache TTL |

## Building

```bash
# Development (SQLite, default)
cargo build

# Production with PostgreSQL
cargo build --release --no-default-features --features postgres

# Run tests
cargo test

# Run full readiness check (format, test, clippy, docker build)
./scripts/check.sh
```

## Project Structure

```
src/
├── main.rs           # Entry point, router setup
├── lib.rs            # Library exports
├── config.rs         # Environment configuration
├── error.rs          # Error types (thiserror)
├── state.rs          # AppState (pool, cache, settings, geo)
├── db/mod.rs         # All SQLx queries
├── domain/
│   ├── types.rs      # Newtypes (ServiceId, SessionId, HitId)
│   └── models.rs     # Domain models, DTOs
├── cache/mod.rs      # Moka caching layer
├── ingress/
│   ├── handlers.rs   # Pixel/script HTTP handlers
│   └── processor.rs  # Core ingress processing logic
├── dashboard/
│   ├── handlers.rs   # Dashboard route handlers
│   └── templates.rs  # Askama template structs
├── api/mod.rs        # JSON API handlers
├── geo/mod.rs        # MaxMind GeoIP lookup
├── ua/mod.rs         # User-agent parsing (woothee)
└── privacy/mod.rs    # DNT, IP filtering, bot detection

templates/            # Askama HTML templates
static/               # CSS, JS, tracker script
migrations/
├── postgres/         # PostgreSQL migrations
└── sqlite/           # SQLite migrations
```

## Core Flows

### 1. Service Management
- `GET /` - Dashboard index, lists all services
- `GET /service/new` - Create service form
- `POST /service/new` - Create service
- `GET /service/{id}` - Service detail with stats
- `GET /service/{id}/manage` - Edit service
- `POST /service/{id}/manage` - Update service
- `POST /service/{id}/delete` - Delete service

### 2. Tracking Ingress
Routes use non-obvious paths to avoid ad blockers. Services have a short 8-character `tracking_id`:
- `GET /trace/px_{tracking_id}.gif` - 1x1 GIF pixel tracker
- `GET /trace/app_{tracking_id}.js` - Serve tracker JS
- `POST /trace/app_{tracking_id}.js` - Receive tracking data

### 3. Session/Hit Flow
1. Request arrives at ingress endpoint
2. Validate service exists and is active
3. Check privacy (DNT header, IP filtering, bot detection)
4. Compute session hash: SHA256(IP + User-Agent + optional salt)
5. Look up session in cache; if miss, create new session
6. Check hit idempotency cache
7. Create or update hit (heartbeat increments)
8. Update session last_seen

### 4. Stats Aggregation
- Sessions, hits, bounce rate, avg load time, avg session duration
- Top locations, referrers, countries, browsers, OS, devices
- Chart data (hourly if <3 days, daily otherwise)
- Comparison with previous period

## Testing

### Manual Testing

```bash
# Start server
cargo run

# Create a service via UI at http://localhost:8080

# Test pixel tracker (use your service's tracking_id, e.g., "abc12345")
curl -v "http://localhost:8080/trace/px_{TRACKING_ID}.gif"

# Test script endpoint
curl -v "http://localhost:8080/trace/app_{TRACKING_ID}.js"

# Test script POST (tracking data)
curl -X POST "http://localhost:8080/trace/app_{TRACKING_ID}.js" \
  -H "Content-Type: application/json" \
  -d '{"idempotency":"test123","location":"/test","referrer":"","loadTime":100}'
```

### Automated Tests

```bash
cargo test
```

### E2E Browser Tests

End-to-end browser tests using Playwright with TypeScript. Each test file runs against a fresh server instance with an in-memory SQLite database.

```bash
cd e2e
npm install
npx playwright install chromium

# Run all tests
npm test

# Run with UI
npm run test:ui

# Run headed (visible browser)
npm run test:headed

# Run with debug
npm run test:debug
```

**E2E Test Structure:**
```
e2e/
├── package.json              # Node.js deps
├── playwright.config.ts      # Playwright configuration
├── global-setup.ts           # Build server before tests
├── tsconfig.json             # TypeScript configuration
├── tests/
│   ├── dashboard.spec.ts     # Dashboard & empty state
│   ├── service-crud.spec.ts  # Create, update, delete service
│   ├── service-detail.spec.ts # Analytics dashboard
│   ├── sessions.spec.ts      # Session list & detail
│   ├── tracking.spec.ts      # Pixel/script ingestion
│   └── htmx.spec.ts          # HTMX interactions
└── lib/
    ├── server.ts             # Server lifecycle management
    └── fixtures.ts           # Test data helpers
```

**Test Categories:**
- **Dashboard:** Empty state, service grid
- **Service CRUD:** Create, update, delete services
- **Service Detail:** Setup instructions, tracker snippets
- **Sessions:** List, detail, pagination
- **Tracking:** Pixel/script ingestion, DNT, CORS
- **HTMX:** Date range filtering, stats updates

### Development Proxy (Same-Domain Testing)

A Deno reverse proxy with host-based routing for local development. Uses `*.localhost` which works in Chrome/Safari without configuration.

```bash
# Terminal 1: Start shymini backend
cargo run

# Terminal 2: Start the frontend server
cd workspace/frontend && SERVICE_ID=your-service-uuid deno task fe

# Terminal 3: Start the reverse proxy
cd workspace/proxy && deno task proxy
```

**Routing (all on port 3000):**
| URL | Upstream |
|-----|----------|
| http://shymini.localhost:3000 | shymini backend (:8080) |
| http://fe.localhost:3000 | Test frontend (:3333) |
| http://localhost:3000 | shymini backend (default) |

**Proxy endpoints:**
- `/__proxy/status` - Status page with routing info
- `/__proxy/test` - Interactive tracking test page
- `/__proxy/health` - Health check (JSON)

**Environment variables:**
| Variable | Default | Description |
|----------|---------|-------------|
| `PROXY_PORT` | `3000` | Proxy listen port |
| `BACKEND_URL` | `http://127.0.0.1:8080` | shymini backend URL |
| `FRONTEND_URL` | `http://127.0.0.1:3333` | Frontend server URL |
| `VERBOSE` | `false` | Enable verbose logging |

**Firefox Note:** Add `shymini.localhost,fe.localhost` to `network.dns.localDomains` in `about:config`.

### Test Frontend Server

A dummy frontend site with multiple pages for testing the tracker.

```bash
cd workspace/frontend
SERVICE_ID=your-service-uuid deno task fe
```

**Direct access:** http://localhost:3333
**Via proxy:** http://fe.localhost:3000 (recommended)

**Pages:** `/`, `/about`, `/products`, `/products/*`, `/blog`, `/blog/*`, `/contact`

**Environment variables:**
| Variable | Default | Description |
|----------|---------|-------------|
| `FE_PORT` | `3333` | Frontend port |
| `TRACKER_URL` | `http://shymini.localhost:3000` | Tracker script URL |
| `SERVICE_ID` | - | Service UUID (required) |

### Load Testing & Benchmarking

A Rust-based load test for seeding realistic data and benchmarking analytics queries.

```bash
# Seed database with default settings (5 services × 100k hits × 10k sessions each)
cargo run --release --bin loadtest -- seed

# With custom settings
cargo run --release --bin loadtest -- seed --db ./test.db --hits 50000 --sessions 5000 --services 10

# Run benchmarks on existing database
cargo run --release --bin loadtest -- bench --db ./loadtest.db

# Seed and immediately benchmark
cargo run --release --bin loadtest -- seed --bench

# Start server with the seeded database
SHYMINI__DATABASE_PATH=./loadtest.db cargo run --release
```

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `--db <path>` | `loadtest.db` | Database file path |
| `--hits <n>` | `100000` | Hits per service |
| `--sessions <n>` | `10000` | Sessions per service |
| `--services <n>` | `5` | Number of services to create |
| `--days <n>` | `7` | Days of history to generate |
| `--bench` | - | Run benchmarks after seeding |

**Default output:** 5 services × 100k hits = 500k total hits, 5 services × 10k sessions = 50k total sessions, all within 7 days.

**Benchmark queries tested:**
- Session/hit counts
- Top locations
- Bounce rate
- Avg load time
- Browser breakdown
- Daily chart data
- Sessions pagination

**Criterion benchmarks** (more rigorous statistical analysis):
```bash
# First seed a benchmark database
cargo run --release --bin loadtest -- seed --db bench.db

# Run criterion benchmarks
SHYMINI_BENCH_DB=sqlite:bench.db cargo bench
```

### Full Local Dev Setup

```bash
# Terminal 1: Backend
cargo run

# Terminal 2: Frontend
cd workspace/frontend && SERVICE_ID=your-uuid deno task fe

# Terminal 3: Proxy
cd workspace/proxy && deno task proxy

# Then visit:
#   http://shymini.localhost:3000  - shymini dashboard
#   http://fe.localhost:3000      - Test frontend (with tracking)
```

## Database

### SQLite Schema Location
- `migrations/sqlite/001_initial.sql`

### Key Tables
- `services` - Tracked websites
- `sessions` - Visitor sessions (deduplicated by IP+UA hash)
- `hits` - Page views within sessions

### Session Deduplication
Sessions are identified by SHA256 hash of:
- IP address
- User-Agent string
- Optional: Service ID + Date (if aggressive_hash_salting enabled)

## Caching

Uses Moka async cache with TTL:
- `service_origins` - CORS origins per service
- `script_inject` - Custom JS per service
- `session_associations` - Hash -> SessionId mapping
- `hit_idempotency` - Prevents duplicate hits

## Privacy Features

- **DNT/GPC:** Respects Do Not Track header (per-service setting)
- **IP Filtering:** Configurable CIDR ignore list per service
- **Bot Detection:** Skips known bot user agents
- **IP Blocking:** Global option to not store IPs

## Troubleshooting

### Database Errors
```bash
# Reset SQLite database
rm shymini.db && cargo run
```

### Template Errors
Templates are compiled at build time. If you modify templates, rebuild:
```bash
cargo build
```
