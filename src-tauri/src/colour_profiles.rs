//! Named per-key colour maps: save, apply, share as JSON files and cycle
//! with a global shortcut. Also remembers the last written map, because
//! feature-report firmware cannot read its colour bank back.
use crate::json_store;
use ak820_protocol::commands::per_key_rgb::{CustomLedMap, LED_COUNT};
use serde::{Deserialize, Serialize};
use std::path::Path;

const FILE: &str = "colour-profiles.json";
const EXPORT_FORMAT: &str = "ak820-colour-profiles/1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColourProfile {
    pub name: String,
    pub map: CustomLedMap,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    /// Last map written to the keyboard.
    pub current: Option<CustomLedMap>,
    pub profiles: Vec<ColourProfile>,
    /// Name of the profile applied last; the cycle continues after it.
    pub active: Option<String>,
    /// Global shortcut, e.g. "Control+Alt+KeyC"; empty disables it.
    pub cycle_shortcut: String,
}

#[derive(Serialize, Deserialize)]
struct Export {
    format: String,
    profiles: Vec<ColourProfile>,
}

pub fn load(dir: &Path) -> Store {
    json_store::load(dir, FILE)
}

pub fn save(dir: &Path, store: &Store) -> Result<(), String> {
    json_store::save(dir, FILE, store)
}

pub fn validate_map(map: &CustomLedMap) -> Result<(), String> {
    if map.leds.len() != LED_COUNT
        || map
            .leds
            .iter()
            .enumerate()
            .any(|(i, l)| usize::from(l.led_id) != i)
    {
        return Err(format!("A colour map needs {LED_COUNT} ordered keys"));
    }
    Ok(())
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("Profile name must be 1-40 characters".into());
    }
    Ok(name.to_string())
}

impl Store {
    /// Adds a profile or replaces the one with the same name.
    pub fn upsert(&mut self, name: &str, map: CustomLedMap) -> Result<(), String> {
        validate_map(&map)?;
        let name = clean_name(name)?;
        match self.profiles.iter_mut().find(|p| p.name == name) {
            Some(existing) => existing.map = map,
            None => self.profiles.push(ColourProfile { name, map }),
        }
        Ok(())
    }

    pub fn remove(&mut self, name: &str) {
        self.profiles.retain(|p| p.name != name);
        if self.active.as_deref() == Some(name) {
            self.active = None;
        }
    }

    pub fn get(&self, name: &str) -> Option<&ColourProfile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    /// The profile after the active one, wrapping around.
    pub fn next(&self) -> Option<&ColourProfile> {
        let after = self
            .active
            .as_deref()
            .and_then(|a| self.profiles.iter().position(|p| p.name == a))
            .map_or(0, |i| i + 1);
        self.profiles
            .get(after % self.profiles.len().max(1))
            .filter(|_| !self.profiles.is_empty())
    }

    pub fn export_json(&self, names: &[String]) -> Result<String, String> {
        let profiles = self
            .profiles
            .iter()
            .filter(|p| names.is_empty() || names.contains(&p.name))
            .cloned()
            .collect();
        serde_json::to_string_pretty(&Export {
            format: EXPORT_FORMAT.into(),
            profiles,
        })
        .map_err(|e| e.to_string())
    }

    /// Imports every valid profile; same names are replaced. Returns names.
    pub fn import_json(&mut self, text: &str) -> Result<Vec<String>, String> {
        let file: Export =
            serde_json::from_str(text).map_err(|e| format!("Not a colour profile file: {e}"))?;
        if file.format != EXPORT_FORMAT {
            return Err(format!("Unsupported colour profile format {}", file.format));
        }
        if file.profiles.is_empty() || file.profiles.len() > 100 {
            return Err("A colour profile file holds 1-100 profiles".into());
        }
        for p in &file.profiles {
            validate_map(&p.map)?;
            clean_name(&p.name)?;
        }
        let mut names = Vec::new();
        for p in file.profiles {
            self.upsert(&p.name, p.map)?;
            names.push(p.name.trim().to_string());
        }
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(r: u8) -> CustomLedMap {
        let mut m = CustomLedMap::default();
        for i in 0..LED_COUNT {
            m.set(i, r, 0, 0);
        }
        m
    }

    #[test]
    fn cycle_wraps_and_survives_removal() {
        let mut s = Store::default();
        assert!(s.next().is_none());
        s.upsert("Work", filled(1)).unwrap();
        s.upsert("Games", filled(2)).unwrap();
        assert_eq!(s.next().unwrap().name, "Work");
        s.active = Some("Work".into());
        assert_eq!(s.next().unwrap().name, "Games");
        s.active = Some("Games".into());
        assert_eq!(s.next().unwrap().name, "Work");
        s.remove("Games");
        assert_eq!(s.active, None);
        assert_eq!(s.next().unwrap().name, "Work");
    }

    #[test]
    fn upsert_replaces_by_name_and_rejects_bad_input() {
        let mut s = Store::default();
        s.upsert(" Work ", filled(1)).unwrap();
        s.upsert("Work", filled(9)).unwrap();
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.profiles[0].map.leds[5].red, 9);
        assert!(s.upsert("", filled(1)).is_err());
        let mut short = filled(1);
        short.leds.pop();
        assert!(s.upsert("Short", short).is_err());
    }

    #[test]
    fn export_import_round_trip_and_rejects_foreign_files() {
        let mut a = Store::default();
        a.upsert("Work", filled(1)).unwrap();
        a.upsert("Games", filled(2)).unwrap();
        let text = a.export_json(&["Games".into()]).unwrap();
        let mut b = Store::default();
        b.upsert("Games", filled(7)).unwrap();
        assert_eq!(b.import_json(&text).unwrap(), vec!["Games".to_string()]);
        assert_eq!(b.profiles.len(), 1);
        assert_eq!(b.profiles[0].map.leds[0].red, 2);
        assert!(b
            .import_json("{\"format\":\"other\",\"profiles\":[]}")
            .is_err());
        assert!(b.import_json("not json").is_err());
    }
}
