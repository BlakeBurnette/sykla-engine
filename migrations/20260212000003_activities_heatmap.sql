-- Activities: recorded outdoor rides with full GPS tracks and sensor data.
-- Distinct from rides (indoor simulator sessions on pre-existing routes).

CREATE TABLE IF NOT EXISTS activities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name VARCHAR(255) NOT NULL DEFAULT 'Untitled Activity',
    source VARCHAR(50) NOT NULL DEFAULT 'recorded',  -- 'recorded' | 'gpx_import'
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    duration_secs REAL,
    distance_m REAL,
    elevation_gain_m REAL,
    avg_power_watts REAL,
    max_power_watts REAL,
    avg_speed_kmh REAL,
    max_speed_kmh REAL,
    avg_heart_rate_bpm SMALLINT,
    max_heart_rate_bpm SMALLINT,
    avg_cadence_rpm REAL,
    max_cadence_rpm REAL,
    calories INT,
    bbox_south DOUBLE PRECISION,
    bbox_west DOUBLE PRECISION,
    bbox_north DOUBLE PRECISION,
    bbox_east DOUBLE PRECISION,
    generated_route_id UUID REFERENCES routes(id) ON DELETE SET NULL,
    gpx_data TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_activities_user_id ON activities(user_id);
CREATE INDEX IF NOT EXISTS idx_activities_started_at ON activities(started_at DESC);

-- Per-second telemetry points for each activity.
CREATE TABLE IF NOT EXISTS activity_points (
    id BIGSERIAL PRIMARY KEY,
    activity_id UUID NOT NULL REFERENCES activities(id) ON DELETE CASCADE,
    seq INT NOT NULL,
    timestamp_ms BIGINT NOT NULL,       -- ms since activity start
    lat DOUBLE PRECISION NOT NULL,
    lng DOUBLE PRECISION NOT NULL,
    elevation_m REAL,
    speed_kmh REAL,
    power_watts REAL,
    heart_rate_bpm SMALLINT,
    cadence_rpm REAL,
    distance_from_start_m REAL NOT NULL DEFAULT 0,
    grade_percent REAL,
    UNIQUE(activity_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_activity_points_activity_id ON activity_points(activity_id);

-- Pre-aggregated spatial grid for heat map visualization.
-- Uses Web Mercator tile math at zoom level 15 (~50m cells).
CREATE TABLE IF NOT EXISTS heatmap_cells (
    zoom INT NOT NULL,
    grid_x INT NOT NULL,
    grid_y INT NOT NULL,
    ride_count INT NOT NULL DEFAULT 0,
    unique_riders INT NOT NULL DEFAULT 0,
    total_points INT NOT NULL DEFAULT 0,
    last_updated TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (zoom, grid_x, grid_y)
);

CREATE INDEX IF NOT EXISTS idx_heatmap_cells_active ON heatmap_cells(ride_count) WHERE ride_count > 0;

-- Per-activity cell contributions for undo/recount when activities are deleted.
CREATE TABLE IF NOT EXISTS heatmap_contributions (
    activity_id UUID NOT NULL REFERENCES activities(id) ON DELETE CASCADE,
    zoom INT NOT NULL,
    grid_x INT NOT NULL,
    grid_y INT NOT NULL,
    point_count INT NOT NULL DEFAULT 1,
    PRIMARY KEY (activity_id, zoom, grid_x, grid_y)
);

-- Auto-detected route corridors from heat map analysis.
CREATE TABLE IF NOT EXISTS detected_corridors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    popularity_score REAL NOT NULL DEFAULT 0,
    is_loop BOOLEAN NOT NULL DEFAULT false,
    distance_m REAL,
    polyline_json TEXT NOT NULL,  -- JSON array of [lat, lng] pairs
    promoted_route_id UUID REFERENCES routes(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_detected_corridors_score ON detected_corridors(popularity_score DESC);
