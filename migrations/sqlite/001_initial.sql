-- Services table
CREATE TABLE IF NOT EXISTS services (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    link TEXT NOT NULL DEFAULT '',
    origins TEXT NOT NULL DEFAULT '*',
    status TEXT NOT NULL DEFAULT 'AC',
    respect_dnt INTEGER NOT NULL DEFAULT 1,
    ignore_robots INTEGER NOT NULL DEFAULT 0,
    collect_ips INTEGER NOT NULL DEFAULT 1,
    ignored_ips TEXT NOT NULL DEFAULT '',
    hide_referrer_regex TEXT NOT NULL DEFAULT '',
    script_inject TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_services_status ON services(status);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    identifier TEXT NOT NULL DEFAULT '',
    start_time TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    user_agent TEXT NOT NULL DEFAULT '',
    browser TEXT NOT NULL DEFAULT '',
    device TEXT NOT NULL DEFAULT '',
    device_type TEXT NOT NULL DEFAULT 'OTHER',
    os TEXT NOT NULL DEFAULT '',
    ip TEXT,
    asn TEXT NOT NULL DEFAULT '',
    country TEXT NOT NULL DEFAULT '',
    longitude REAL,
    latitude REAL,
    time_zone TEXT NOT NULL DEFAULT '',
    is_bounce INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_sessions_service_start ON sessions(service_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_service_last_seen ON sessions(service_id, last_seen DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_service_identifier ON sessions(service_id, identifier);

-- Hits table
CREATE TABLE IF NOT EXISTS hits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    service_id TEXT NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    initial INTEGER NOT NULL DEFAULT 0,
    start_time TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    heartbeats INTEGER NOT NULL DEFAULT 0,
    tracker TEXT NOT NULL DEFAULT 'JS',
    location TEXT NOT NULL DEFAULT '',
    referrer TEXT NOT NULL DEFAULT '',
    load_time REAL
);

CREATE INDEX IF NOT EXISTS idx_hits_session_start ON hits(session_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_hits_service_start ON hits(service_id, start_time DESC);
