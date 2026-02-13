CREATE TABLE IF NOT EXISTS cities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(100) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    center_lat DOUBLE PRECISION NOT NULL,
    center_lng DOUBLE PRECISION NOT NULL,
    bbox_south DOUBLE PRECISION,
    bbox_west DOUBLE PRECISION,
    bbox_north DOUBLE PRECISION,
    bbox_east DOUBLE PRECISION,
    file_size_bytes BIGINT NOT NULL,
    gcs_bucket VARCHAR(255) NOT NULL,
    gcs_object VARCHAR(512) NOT NULL,
    download_url TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_cities_slug ON cities(slug);
