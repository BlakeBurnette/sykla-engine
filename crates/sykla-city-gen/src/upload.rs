use google_cloud_storage::client::Storage;
use sqlx::PgPool;

/// Upload a file to Google Cloud Storage using Application Default Credentials.
pub async fn upload_to_gcs(
    bucket: &str,
    object_path: &str,
    data: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Storage::builder().build().await?;

    let payload = bytes::Bytes::from(data);

    client
        .write_object(bucket, object_path, payload)
        .send_buffered()
        .await?;

    println!("  Uploaded to gs://{bucket}/{object_path}");
    Ok(())
}

/// Insert or update a city record in the database.
/// On conflict (same slug), bumps the version and updates all fields.
pub async fn register_city(
    db_url: &str,
    slug: &str,
    name: &str,
    center_lat: f64,
    center_lng: f64,
    bbox: (f64, f64, f64, f64),
    file_size: i64,
    bucket: &str,
    object: &str,
    download_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect(db_url).await?;

    sqlx::query(
        r#"
        INSERT INTO cities (slug, name, center_lat, center_lng, bbox_south, bbox_west, bbox_north, bbox_east, file_size_bytes, gcs_bucket, gcs_object, download_url, version)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 1)
        ON CONFLICT (slug) DO UPDATE SET
            name = EXCLUDED.name,
            center_lat = EXCLUDED.center_lat,
            center_lng = EXCLUDED.center_lng,
            bbox_south = EXCLUDED.bbox_south,
            bbox_west = EXCLUDED.bbox_west,
            bbox_north = EXCLUDED.bbox_north,
            bbox_east = EXCLUDED.bbox_east,
            file_size_bytes = EXCLUDED.file_size_bytes,
            gcs_bucket = EXCLUDED.gcs_bucket,
            gcs_object = EXCLUDED.gcs_object,
            download_url = EXCLUDED.download_url,
            version = cities.version + 1
        "#,
    )
    .bind(slug)
    .bind(name)
    .bind(center_lat)
    .bind(center_lng)
    .bind(bbox.0) // south
    .bind(bbox.1) // west
    .bind(bbox.2) // north
    .bind(bbox.3) // east
    .bind(file_size)
    .bind(bucket)
    .bind(object)
    .bind(download_url)
    .execute(&pool)
    .await?;

    println!("  Registered city '{slug}' in database");
    Ok(())
}
