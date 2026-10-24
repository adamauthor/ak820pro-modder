//! One-file export/import of every setting, also used for iCloud sync.
//!
//! Each file is parsed into its real type before anything is written, so a
//! damaged or foreign bundle is rejected as a whole instead of half-applied.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

pub const FORMAT: &str = "ak820-pro-modder-settings";
pub const VERSION: u32 = 1;
pub const ICLOUD_NAME: &str = "settings-bundle.json";

/// Settings files included in a bundle.
pub const FILES: [&str; 8] = [
    "mac-controls.json",
    "host-macros.json",
    "automations.json",
    crate::smart_lighting::CONFIG_FILE,
    crate::smart_lighting::BASE_FILE,
    crate::pomodoro::CONFIG_FILE,
    crate::anime_screen::CONFIG_FILE,
    crate::colour_profiles::FILE,
];

#[derive(Debug, Serialize, Deserialize)]
pub struct Bundle {
    pub format: String,
    pub version: u32,
    pub exported_at_ms: i64,
    pub files: BTreeMap<String, Value>,
}

pub fn export(dir: &Path, now_ms: i64) -> Result<Bundle, String> {
    let mut files = BTreeMap::new();
    for name in FILES {
        if let Ok(bytes) = std::fs::read(dir.join(name)) {
            let value = serde_json::from_slice(&bytes)
                .map_err(|e| format!("{name} is not valid JSON: {e}"))?;
            files.insert(name.to_string(), value);
        }
    }
    Ok(Bundle {
        format: FORMAT.into(),
        version: VERSION,
        exported_at_ms: now_ms,
        files,
    })
}

fn check<T: serde::de::DeserializeOwned>(name: &str, value: &Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|e| format!("{name}: {e}"))
}

/// Validates every file of the bundle against its type.
pub fn validate(bundle: &Bundle) -> Result<(), String> {
    if bundle.format != FORMAT || bundle.version > VERSION {
        return Err(
            "This is not an AK820 Pro Modder settings file, or it is from a newer version".into(),
        );
    }
    for (name, value) in &bundle.files {
        match name.as_str() {
            "mac-controls.json" => drop(check::<crate::mac_controls::Config>(name, value)?),
            "host-macros.json" => crate::host_keyboard::validate(&check(name, value)?)?,
            "automations.json" => drop(check::<Vec<crate::automations::Automation>>(name, value)?),
            crate::smart_lighting::CONFIG_FILE => {
                check::<crate::smart_lighting::SmartConfig>(name, value)?.validate()?
            }
            crate::smart_lighting::BASE_FILE => drop(check::<
                Option<ak820_protocol::commands::lighting::LightingConfig>,
            >(name, value)?),
            crate::pomodoro::CONFIG_FILE => {
                check::<crate::pomodoro::PomodoroConfig>(name, value)?.validate()?
            }
            crate::anime_screen::CONFIG_FILE => {
                check::<crate::anime_screen::AnimeConfig>(name, value)?.validate()?
            }
            crate::colour_profiles::FILE => {
                check::<crate::colour_profiles::Store>(name, value)?.validate()?
            }
            other => return Err(format!("Unknown file in settings bundle: {other}")),
        }
    }
    Ok(())
}

/// Writes the files of a validated bundle. Files missing from the bundle
/// are left as they are.
pub fn write(dir: &Path, bundle: &Bundle) -> Result<(), String> {
    validate(bundle)?;
    for (name, value) in &bundle.files {
        crate::json_store::save(dir, name, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_validate_import_round_trip() {
        let dir = std::env::temp_dir().join(format!("ak820-bundle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pomodoro.json"), br#"{"focus_minutes":30}"#).unwrap();
        std::fs::write(dir.join("mac-controls.json"), br#"{"knob_mode":"smart"}"#).unwrap();
        let bundle = export(&dir, 1).unwrap();
        assert_eq!(bundle.files.len(), 2);
        validate(&bundle).unwrap();
        let target = dir.join("restored");
        write(&target, &bundle).unwrap();
        let restored: crate::pomodoro::PomodoroConfig =
            crate::json_store::load(&target, "pomodoro.json");
        assert_eq!(restored.focus_minutes, 30);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn foreign_or_invalid_bundles_are_rejected_whole() {
        let mut bundle = Bundle {
            format: FORMAT.into(),
            version: VERSION,
            exported_at_ms: 0,
            files: BTreeMap::new(),
        };
        bundle.files.insert(
            "pomodoro.json".into(),
            serde_json::json!({"focus_minutes": 0}),
        );
        assert!(validate(&bundle).is_err());
        bundle.files.clear();
        bundle
            .files
            .insert("../../etc/passwd".into(), serde_json::json!({}));
        assert!(validate(&bundle).is_err(), "only known file names");
        bundle.files.clear();
        bundle.version = VERSION + 1;
        assert!(validate(&bundle).is_err());
    }
}
