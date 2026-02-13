-- Sykla initial schema

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    display_name VARCHAR(100) NOT NULL,
    weight_kg REAL,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE IF NOT EXISTS routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    gpx_data TEXT NOT NULL,
    distance_m REAL NOT NULL,
    elevation_gain_m REAL NOT NULL,
    min_elevation_m REAL,
    max_elevation_m REAL,
    start_lat DOUBLE PRECISION,
    start_lng DOUBLE PRECISION,
    city VARCHAR(100),
    is_public BOOLEAN DEFAULT false,
    source VARCHAR(50) DEFAULT 'recorded',
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE IF NOT EXISTS route_points (
    id BIGSERIAL PRIMARY KEY,
    route_id UUID REFERENCES routes(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    lat DOUBLE PRECISION NOT NULL,
    lng DOUBLE PRECISION NOT NULL,
    elevation_m REAL NOT NULL,
    distance_from_start_m REAL NOT NULL,
    grade_percent REAL,
    UNIQUE(route_id, seq)
);

CREATE TABLE IF NOT EXISTS rides (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),
    route_id UUID REFERENCES routes(id),
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    duration_secs INTEGER,
    avg_power_watts REAL,
    avg_speed_kmh REAL,
    avg_cadence_rpm REAL,
    calories INTEGER,
    is_indoor BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE IF NOT EXISTS live_positions (
    user_id UUID REFERENCES users(id) PRIMARY KEY,
    route_id UUID REFERENCES routes(id),
    lat DOUBLE PRECISION,
    lng DOUBLE PRECISION,
    distance_m REAL,
    speed_kmh REAL,
    is_indoor BOOLEAN,
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_routes_user_id ON routes(user_id);
CREATE INDEX IF NOT EXISTS idx_routes_is_public ON routes(is_public);
CREATE INDEX IF NOT EXISTS idx_route_points_route_id ON route_points(route_id);
CREATE INDEX IF NOT EXISTS idx_rides_user_id ON rides(user_id);
CREATE INDEX IF NOT EXISTS idx_rides_route_id ON rides(route_id);
CREATE INDEX IF NOT EXISTS idx_live_positions_route_id ON live_positions(route_id);
