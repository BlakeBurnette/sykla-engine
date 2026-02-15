/// Geographic origin for world<->lat/lon conversion.
/// Inverse of the equirectangular projection used in gpx.rs.
#[derive(Clone)]
pub struct GeoOrigin {
    pub lat0: f64,
    pub lon0: f64,
    cos_lat0: f64,
}

impl GeoOrigin {
    pub fn new(lat0: f64, lon0: f64) -> Self {
        Self {
            lat0,
            lon0,
            cos_lat0: lat0.to_radians().cos(),
        }
    }

    /// World XZ -> (lat, lon). Inverse of gpx::project().
    pub fn world_to_latlon(&self, x: f32, z: f32) -> (f64, f64) {
        let lon = x as f64 / (self.cos_lat0 * 111_320.0) + self.lon0;
        let lat = z as f64 / 110_540.0 + self.lat0;
        (lat, lon)
    }
}

/// Cropped DEM tile with bilinear elevation sampling.
///
/// Binary format (little-endian):
///   f64 lat_min, f64 lon_min, f64 cell_size, u32 rows, u32 cols
///   i16[rows*cols] row-major, north-to-south
pub struct DemTile {
    lat_min: f64,
    lon_min: f64,
    cell_size: f64,
    rows: usize,
    cols: usize,
    data: Vec<i16>,
}

impl DemTile {
    /// Parse a cropped .dem file from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        // Header: 3 x f64 + 2 x u32 = 24 + 8 = 32 bytes
        if bytes.len() < 32 {
            return None;
        }

        let lat_min = f64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let lon_min = f64::from_le_bytes(bytes[8..16].try_into().ok()?);
        let cell_size = f64::from_le_bytes(bytes[16..24].try_into().ok()?);
        let rows = u32::from_le_bytes(bytes[24..28].try_into().ok()?) as usize;
        let cols = u32::from_le_bytes(bytes[28..32].try_into().ok()?) as usize;

        let expected = 32 + rows * cols * 2;
        if bytes.len() < expected {
            return None;
        }

        let mut data = Vec::with_capacity(rows * cols);
        let pixel_bytes = &bytes[32..];
        for i in 0..(rows * cols) {
            let lo = pixel_bytes[i * 2];
            let hi = pixel_bytes[i * 2 + 1];
            data.push(i16::from_le_bytes([lo, hi]));
        }

        Some(Self {
            lat_min,
            lon_min,
            cell_size,
            rows,
            cols,
            data,
        })
    }

    /// Bilinear interpolation at (lat, lon). Returns None for void (-32768) or out-of-bounds.
    pub fn sample(&self, lat: f64, lon: f64) -> Option<f32> {
        // Row 0 = northernmost (lat_min + (rows-1)*cell_size)
        let lat_max = self.lat_min + (self.rows - 1) as f64 * self.cell_size;

        let col_f = (lon - self.lon_min) / self.cell_size;
        let row_f = (lat_max - lat) / self.cell_size; // north-to-south

        if col_f < 0.0 || row_f < 0.0 {
            return None;
        }

        let c0 = col_f.floor() as usize;
        let r0 = row_f.floor() as usize;

        if c0 + 1 >= self.cols || r0 + 1 >= self.rows {
            return None;
        }

        let fc = col_f - c0 as f64;
        let fr = row_f - r0 as f64;

        let v00 = self.get(r0, c0)?;
        let v10 = self.get(r0, c0 + 1)?;
        let v01 = self.get(r0 + 1, c0)?;
        let v11 = self.get(r0 + 1, c0 + 1)?;

        let top = v00 as f64 + fc * (v10 as f64 - v00 as f64);
        let bot = v01 as f64 + fc * (v11 as f64 - v01 as f64);
        let val = top + fr * (bot - top);

        Some(val as f32)
    }

    /// Sample using world coordinates via a GeoOrigin.
    pub fn sample_world(&self, origin: &GeoOrigin, x: f32, z: f32) -> Option<f32> {
        let (lat, lon) = origin.world_to_latlon(x, z);
        self.sample(lat, lon)
    }

    fn get(&self, row: usize, col: usize) -> Option<i16> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let v = self.data[row * self.cols + col];
        if v == -32768 {
            // SRTM void value
            None
        } else {
            Some(v)
        }
    }
}
