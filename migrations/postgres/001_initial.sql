-- Services table
CREATE TABLE IF NOT EXISTS services (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(64) NOT NULL,
    link VARCHAR(2048) NOT NULL DEFAULT '',
    origins TEXT NOT NULL DEFAULT '*',
    status VARCHAR(2) NOT NULL DEFAULT 'AC',
    respect_dnt BOOLEAN NOT NULL DEFAULT TRUE,
    ignore_robots BOOLEAN NOT NULL DEFAULT FALSE,
    collect_ips BOOLEAN NOT NULL DEFAULT TRUE,
    ignored_ips TEXT NOT NULL DEFAULT '',
    hide_referrer_regex TEXT NOT NULL DEFAULT '',
    script_inject TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_services_status ON services(status);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id UUID NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    identifier VARCHAR(255) NOT NULL DEFAULT '',
    start_time TIMESTAMPTZ NOT NULL,
    last_seen TIMESTAMPTZ NOT NULL,
    user_agent TEXT NOT NULL DEFAULT '',
    browser VARCHAR(255) NOT NULL DEFAULT '',
    device VARCHAR(255) NOT NULL DEFAULT '',
    device_type VARCHAR(7) NOT NULL DEFAULT 'OTHER',
    os VARCHAR(255) NOT NULL DEFAULT '',
    ip INET,
    asn VARCHAR(255) NOT NULL DEFAULT '',
    country CHAR(2) NOT NULL DEFAULT '',
    longitude DOUBLE PRECISION,
    latitude DOUBLE PRECISION,
    time_zone VARCHAR(64) NOT NULL DEFAULT '',
    is_bounce BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX IF NOT EXISTS idx_sessions_service_start ON sessions(service_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_service_last_seen ON sessions(service_id, last_seen DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_service_identifier ON sessions(service_id, identifier);

-- Hits table
CREATE TABLE IF NOT EXISTS hits (
    id BIGSERIAL PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    service_id UUID NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    initial BOOLEAN NOT NULL DEFAULT FALSE,
    start_time TIMESTAMPTZ NOT NULL,
    last_seen TIMESTAMPTZ NOT NULL,
    heartbeats INTEGER NOT NULL DEFAULT 0,
    tracker VARCHAR(5) NOT NULL DEFAULT 'JS',
    location TEXT NOT NULL DEFAULT '',
    referrer TEXT NOT NULL DEFAULT '',
    load_time DOUBLE PRECISION
);

CREATE INDEX IF NOT EXISTS idx_hits_session_start ON hits(session_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_hits_service_start ON hits(service_id, start_time DESC);
