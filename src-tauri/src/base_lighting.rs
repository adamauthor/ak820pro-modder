//! The user's own lighting: what the Lighting view, a preset, a colour
//! profile or the menu bar last applied. Remembered so the Lighting view can
//! start from it (FF13 keyboards cannot read lighting back).
//!
//! Every FF13 write is saved to the keyboard's flash, so nothing here writes
//! on its own: no event-driven layers, flashes or reconnect rewrites.
use crate::{ensure_open, json_store, AppError, ConnState};
use ak820_protocol::commands::lighting::{Direction, LightingConfig, Mode};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub const BASE_FILE: &str = "lighting-state.json";

pub fn effect(mode: Mode, color: &str, brightness: u8, speed: u8) -> LightingConfig {
    LightingConfig {
        mode,
        color: color.into(),
        secondary: None,
        color_mode: 0,
        effect_mode_type: 0,
        brightness,
        speed,
        direction: Direction::Left,
    }
}
pub fn solid(color: &str, brightness: u8) -> LightingConfig {
    effect(Mode::Static, color, brightness, 0)
}

/// Named choices for the menu bar.
pub fn preset(name: &str) -> Option<LightingConfig> {
    Some(match name {
        "off" => effect(Mode::Off, "000000", 0, 0),
        "white" => solid("FFFFFF", 4),
        "warm" => solid("FF9A3C", 4),
        "rainbow" => {
            let mut c = effect(Mode::Spectrum, "FFFFFF", 4, 3);
            c.color_mode = 1;
            c
        }
        "breath" => effect(Mode::Breath, "00AAFF", 4, 2),
        _ => return None,
    })
}

pub fn usb_connected() -> bool {
    ak820_protocol::enumerate()
        .map(|devices| devices.iter().any(|d| d.is_control))
        .unwrap_or(false)
}

pub struct BaseLighting {
    conn: Arc<ConnState>,
    dir: PathBuf,
    base: Mutex<Option<LightingConfig>>,
}

impl BaseLighting {
    pub fn new(conn: Arc<ConnState>, dir: PathBuf) -> Arc<Self> {
        let base = json_store::load(&dir, BASE_FILE);
        Arc::new(Self {
            conn,
            dir,
            base: Mutex::new(base),
        })
    }

    pub async fn base(&self) -> Option<LightingConfig> {
        self.base.lock().await.clone()
    }

    /// Remembers lighting the caller has just written to the keyboard.
    pub async fn set_base(&self, config: &LightingConfig) {
        if let Err(error) = json_store::save(&self.dir, BASE_FILE, &Some(config)) {
            tracing::warn!(%error, "could not remember lighting");
        }
        *self.base.lock().await = Some(config.clone());
    }

    pub async fn apply_base(&self, config: LightingConfig) -> Result<(), AppError> {
        let written = config.clone();
        self.conn
            .with(|slot| {
                ensure_open(slot)?.set_lighting(&written)?;
                Ok(())
            })
            .await?;
        tracing::info!(mode = config.mode.name(), color = %config.color, brightness = config.brightness, "lighting applied");
        self.set_base(&config).await;
        Ok(())
    }

    /// Writes the remembered lighting again ("My lighting" in the menu bar).
    pub async fn restore(&self) -> Result<(), AppError> {
        match self.base().await {
            Some(config) => self.apply_base(config).await,
            None => Ok(()),
        }
    }

    /// Re-reads the remembered lighting after a settings import.
    pub async fn reload(&self) {
        *self.base.lock().await = json_store::load(&self.dir, BASE_FILE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_exist_for_every_menu_entry() {
        for name in ["off", "white", "warm", "rainbow", "breath"] {
            assert!(preset(name).is_some(), "{name}");
        }
        assert!(preset("shell").is_none());
    }
}
