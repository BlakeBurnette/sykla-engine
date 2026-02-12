use std::fs;
use std::path::Path;
use sykla_world::city::CityData;

/// Serialize CityData to a .sykla binary file using bincode.
pub fn write_city_data(data: &CityData, path: &Path) -> Result<(), String> {
    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directories: {e}"))?;
    }

    let bytes = bincode::serialize(data).map_err(|e| format!("Serialization error: {e}"))?;
    fs::write(path, &bytes).map_err(|e| format!("Write error: {e}"))?;

    Ok(())
}

/// Read and deserialize CityData from a .sykla binary file.
pub fn read_city_data(path: &Path) -> Result<CityData, String> {
    let bytes = fs::read(path).map_err(|e| format!("Read error: {e}"))?;
    let data: CityData =
        bincode::deserialize(&bytes).map_err(|e| format!("Deserialization error: {e}"))?;
    Ok(data)
}
