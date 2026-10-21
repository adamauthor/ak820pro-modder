//! Small JSON files in the app-data directory, written atomically.
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

/// Missing or unreadable files fall back to defaults; a corrupt file is
/// logged instead of blocking startup.
pub fn load<T: DeserializeOwned + Default>(dir: &Path, name: &str) -> T {
    let path = dir.join(name);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            tracing::warn!(%error, file = name, "invalid settings file; using defaults");
            T::default()
        }),
        Err(_) => T::default(),
    }
}

pub fn save<T: Serialize>(dir: &Path, name: &str, value: &T) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let temporary = dir.join(format!("{name}.tmp"));
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, dir.join(name)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
    struct Sample {
        value: u8,
    }

    #[test]
    fn round_trip_and_defaults() {
        let dir = std::env::temp_dir().join(format!("ak820-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(super::load::<Sample>(&dir, "s.json"), Sample::default());
        super::save(&dir, "s.json", &Sample { value: 7 }).unwrap();
        assert_eq!(super::load::<Sample>(&dir, "s.json").value, 7);
        std::fs::write(dir.join("s.json"), b"{broken").unwrap();
        assert_eq!(super::load::<Sample>(&dir, "s.json"), Sample::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
