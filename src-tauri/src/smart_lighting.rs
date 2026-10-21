//! Event-driven keyboard lighting.
//!
//! Several sources want the LEDs at once, so each one is a layer and the
//! highest active layer wins: Teams call > Pomodoro > app rule > the user's
//! own lighting. Short flashes (Telegram, Pomodoro transitions) play on top
//! and then hand back to the winning layer. Lighting needs the USB control
//! interface; while the keyboard is on Bluetooth the layers are remembered
//! and written as soon as USB returns.
use crate::{ensure_open, json_store, AppError, ConnState};
use ak820_protocol::commands::lighting::{Direction, LightingConfig, Mode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub const CONFIG_FILE: &str = "smart-lighting.json";
pub const BASE_FILE: &str = "lighting-state.json";
pub const TEAMS_COLOR: &str = "7A00FF";
const SETTLE: Duration = Duration::from_millis(2500);
/// The 0.9.0 default, replaced on load when the user never changed it.
const OLD_TEAMS_COLOR: &str = "6264A7";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLighting {
    pub bundle_id: String,
    pub name: String,
    pub lighting: LightingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SmartConfig {
    pub teams_enabled: bool,
    pub teams_lighting: LightingConfig,
    /// Shown while a Teams call is ringing (before it is answered).
    pub teams_ring_lighting: LightingConfig,
    pub telegram_enabled: bool,
    pub telegram_color: String,
    pub telegram_flashes: u8,
    pub apps_enabled: bool,
    pub app_rules: Vec<AppLighting>,
}

impl Default for SmartConfig {
    fn default() -> Self {
        Self {
            teams_enabled: true,
            // A saturated violet: Teams' own #6264A7 is so pale that the
            // LEDs render it as bluish white.
            teams_lighting: effect(Mode::Breath, TEAMS_COLOR, 5, 2),
            // Fast pulse so a ringing call is hard to miss.
            teams_ring_lighting: effect(Mode::Breath, TEAMS_COLOR, 5, 5),
            telegram_enabled: true,
            telegram_color: "0088FF".into(),
            telegram_flashes: 3,
            apps_enabled: false,
            app_rules: Vec::new(),
        }
    }
}

impl SmartConfig {
    pub fn validate(&self) -> Result<(), String> {
        let hex = |s: &str| ak820_protocol::commands::lighting::parse_hex_rgb(s).is_some();
        if !hex(&self.telegram_color) || !(1..=10).contains(&self.telegram_flashes) {
            return Err("Telegram flashes need a hex colour and 1–10 flashes".into());
        }
        if self.app_rules.len() > 64 {
            return Err("At most 64 app lighting rules".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for rule in &self.app_rules {
            if !seen.insert(&rule.bundle_id)
                || rule.bundle_id.is_empty()
                || !hex(&rule.lighting.color)
            {
                return Err("App lighting rules need unique apps and hex colours".into());
            }
        }
        Ok(())
    }
}

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
fn off() -> LightingConfig {
    effect(Mode::Off, "000000", 0, 0)
}
/// Used when a layer ends but the user's own lighting was never set here.
fn fallback() -> LightingConfig {
    solid("FFFFFF", 3)
}

/// Named choices for the menu bar.
pub fn preset(name: &str) -> Option<LightingConfig> {
    Some(match name {
        "off" => off(),
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

#[derive(Default)]
struct Layers {
    base: Option<LightingConfig>,
    app: Option<LightingConfig>,
    pomodoro: Option<LightingConfig>,
    teams: bool,
    ringing: bool,
}

/// The configuration that should currently be on the keyboard.
fn desired(layers: &Layers, config: &SmartConfig) -> Option<LightingConfig> {
    if layers.ringing && config.teams_enabled {
        return Some(config.teams_ring_lighting.clone());
    }
    if layers.teams && config.teams_enabled {
        return Some(config.teams_lighting.clone());
    }
    layers
        .pomodoro
        .clone()
        .or_else(|| layers.app.clone().filter(|_| config.apps_enabled))
        .or_else(|| layers.base.clone())
}

fn key(config: &LightingConfig) -> String {
    serde_json::to_string(config).unwrap_or_default()
}

struct Inner {
    config: SmartConfig,
    layers: Layers,
    frontmost: Option<String>,
    flashing: bool,
    /// What we last wrote, or None when the keyboard state is unknown.
    written: Option<String>,
    /// Set once this session changed the LEDs, so ending a layer must
    /// leave some lighting on even if the user's own was never recorded.
    touched: bool,
    connected: bool,
}

pub struct SmartLighting {
    conn: Arc<ConnState>,
    dir: PathBuf,
    inner: Mutex<Inner>,
    /// Flashes and TFT uploads must not overlap: the FF13 firmware drops
    /// image chunks while lighting reports arrive.
    exclusive: Arc<Mutex<()>>,
    last_write: std::sync::Mutex<Option<std::time::Instant>>,
}

pub fn usb_connected() -> bool {
    ak820_protocol::enumerate()
        .map(|devices| devices.iter().any(|d| d.is_control))
        .unwrap_or(false)
}

impl SmartLighting {
    pub fn new(conn: Arc<ConnState>, dir: PathBuf) -> Arc<Self> {
        let mut config: SmartConfig = json_store::load(&dir, CONFIG_FILE);
        if config
            .teams_lighting
            .color
            .eq_ignore_ascii_case(OLD_TEAMS_COLOR)
        {
            config.teams_lighting.color = TEAMS_COLOR.into();
            let _ = json_store::save(&dir, CONFIG_FILE, &config);
        }
        let base: Option<LightingConfig> = json_store::load(&dir, BASE_FILE);
        Arc::new(Self {
            conn,
            dir,
            exclusive: Arc::new(Mutex::new(())),
            last_write: std::sync::Mutex::new(None),
            inner: Mutex::new(Inner {
                config,
                layers: Layers {
                    base,
                    ..Layers::default()
                },
                frontmost: None,
                flashing: false,
                written: None,
                touched: false,
                connected: false,
            }),
        })
    }

    pub async fn config(&self) -> SmartConfig {
        self.inner.lock().await.config.clone()
    }
    pub async fn base(&self) -> Option<LightingConfig> {
        self.inner.lock().await.layers.base.clone()
    }

    pub async fn set_config(self: &Arc<Self>, config: SmartConfig) -> Result<(), String> {
        config.validate()?;
        json_store::save(&self.dir, CONFIG_FILE, &config)?;
        {
            let mut inner = self.inner.lock().await;
            inner.config = config;
            let bundle = inner.frontmost.clone();
            inner.layers.app = app_rule(&inner.config, bundle.as_deref());
        }
        self.sync().await;
        Ok(())
    }

    /// The user's own lighting, applied from the Lighting view or a preset.
    /// It was just written to the keyboard by the caller.
    pub async fn set_base(&self, config: &LightingConfig) {
        if let Err(error) = json_store::save(&self.dir, BASE_FILE, &Some(config)) {
            tracing::warn!(%error, "could not remember lighting");
        }
        let mut inner = self.inner.lock().await;
        inner.layers.base = Some(config.clone());
        inner.written = Some(key(config));
    }

    pub async fn apply_base(self: &Arc<Self>, config: LightingConfig) -> Result<(), AppError> {
        self.write(&config).await?;
        self.set_base(&config).await;
        Ok(())
    }

    /// Re-asserts whatever layer currently wins.
    pub async fn restore(self: &Arc<Self>) {
        self.inner.lock().await.written = None;
        self.sync().await;
    }

    pub async fn on_frontmost(self: &Arc<Self>, bundle: Option<String>) {
        {
            let mut inner = self.inner.lock().await;
            inner.layers.app = app_rule(&inner.config, bundle.as_deref());
            inner.frontmost = bundle;
        }
        self.sync().await;
    }

    pub async fn set_teams(self: &Arc<Self>, active: bool) {
        self.inner.lock().await.layers.teams = active;
        self.sync().await;
    }
    pub async fn set_ringing(self: &Arc<Self>, ringing: bool) {
        self.inner.lock().await.layers.ringing = ringing;
        self.sync().await;
    }
    /// Waits for a running flash, then pauses lighting writes until
    /// `end_upload`, so a TFT upload has the keyboard to itself.
    pub async fn begin_upload(&self) -> tokio::sync::OwnedMutexGuard<()> {
        let guard = self.exclusive.clone().lock_owned().await;
        self.inner.lock().await.flashing = true;
        // The firmware ignores image chunks for a moment after a lighting
        // change; uploads started right after a flash were rejected.
        let recent = self
            .last_write
            .lock()
            .ok()
            .and_then(|t| *t)
            .map(|t| t.elapsed());
        if let Some(elapsed) = recent.filter(|e| *e < SETTLE) {
            tokio::time::sleep(SETTLE - elapsed).await;
        }
        guard
    }
    pub async fn end_upload(&self, guard: tokio::sync::OwnedMutexGuard<()>) {
        self.inner.lock().await.flashing = false;
        drop(guard);
    }
    pub async fn teams_active(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.layers.teams && inner.config.teams_enabled
    }

    pub async fn set_pomodoro(self: &Arc<Self>, config: Option<LightingConfig>) {
        {
            let mut inner = self.inner.lock().await;
            if inner.layers.pomodoro.as_ref().map(key) == config.as_ref().map(key) {
                return;
            }
            inner.layers.pomodoro = config;
        }
        self.sync().await;
    }

    pub async fn telegram_message(self: &Arc<Self>) {
        let (enabled, color, times) = {
            let inner = self.inner.lock().await;
            (
                inner.config.telegram_enabled,
                inner.config.telegram_color.clone(),
                inner.config.telegram_flashes,
            )
        };
        if enabled {
            self.flash(&color, times).await;
        }
    }

    /// Blinks a colour, then returns to the winning layer. Ignored while
    /// another flash runs so a burst of messages does not queue up.
    pub async fn flash(self: &Arc<Self>, color: &str, times: u8) {
        // Skipped while an upload runs; a flash is not worth a broken image.
        let Ok(_exclusive) = self.exclusive.clone().try_lock_owned() else {
            return;
        };
        {
            let mut inner = self.inner.lock().await;
            if inner.flashing || !usb_connected() {
                return;
            }
            inner.flashing = true;
            inner.touched = true;
        }
        for _ in 0..times {
            if self.write(&solid(color, 5)).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(220)).await;
            if self.write(&off()).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(180)).await;
        }
        {
            let mut inner = self.inner.lock().await;
            inner.flashing = false;
            inner.written = None;
        }
        self.sync().await;
    }

    /// Periodic check: rewrite after a USB reconnect or a failed write.
    pub async fn tick(self: &Arc<Self>) {
        let connected = usb_connected();
        let resync = {
            let mut inner = self.inner.lock().await;
            let reconnected = connected && !inner.connected;
            inner.connected = connected;
            if reconnected {
                inner.written = None;
            }
            connected && inner.written.is_none()
        };
        if resync {
            self.sync().await;
        }
    }

    pub async fn sync(self: &Arc<Self>) {
        let target = {
            let inner = self.inner.lock().await;
            if inner.flashing {
                return;
            }
            match desired(&inner.layers, &inner.config) {
                Some(config) => config,
                // Nothing wants the LEDs: only undo something we drew.
                None if !inner.touched => return,
                None => fallback(),
            }
        };
        let wanted = key(&target);
        if self.inner.lock().await.written.as_deref() == Some(wanted.as_str()) {
            return;
        }
        if !usb_connected() {
            return;
        }
        let ok = self.write(&target).await.is_ok();
        tracing::info!(ok, mode = target.mode.name(), color = %target.color, brightness = target.brightness, "lighting applied");
        let mut inner = self.inner.lock().await;
        inner.written = ok.then_some(wanted);
        inner.touched |= ok;
    }

    async fn write(&self, config: &LightingConfig) -> Result<(), AppError> {
        let config = config.clone();
        let result = self
            .conn
            .with(|slot| {
                ensure_open(slot)?.set_lighting(&config)?;
                Ok(())
            })
            .await;
        if let Err(error) = &result {
            tracing::debug!(?error, "smart lighting write failed; will retry");
        }
        if let Ok(mut last) = self.last_write.lock() {
            *last = Some(std::time::Instant::now());
        }
        result
    }
}

fn app_rule(config: &SmartConfig, bundle: Option<&str>) -> Option<LightingConfig> {
    let bundle = bundle?;
    config
        .app_rules
        .iter()
        .find(|rule| rule.bundle_id == bundle)
        .map(|rule| rule.lighting.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teams_beats_pomodoro_beats_apps_beats_base() {
        let mut config = SmartConfig {
            apps_enabled: true,
            ..SmartConfig::default()
        };
        let mut layers = Layers {
            base: Some(solid("FFFFFF", 3)),
            app: Some(solid("00FF00", 3)),
            pomodoro: Some(solid("FF0000", 5)),
            teams: true,
            ringing: false,
        };
        layers.ringing = true;
        assert_eq!(
            desired(&layers, &config).unwrap().speed,
            5,
            "ringing pulses"
        );
        layers.ringing = false;
        assert_eq!(desired(&layers, &config).unwrap().color, TEAMS_COLOR);
        layers.teams = false;
        assert_eq!(desired(&layers, &config).unwrap().color, "FF0000");
        layers.pomodoro = None;
        assert_eq!(desired(&layers, &config).unwrap().color, "00FF00");
        config.apps_enabled = false;
        assert_eq!(desired(&layers, &config).unwrap().color, "FFFFFF");
        layers.teams = true;
        config.teams_enabled = false;
        assert_eq!(desired(&layers, &config).unwrap().color, "FFFFFF");
    }

    #[test]
    fn config_defaults_and_validation() {
        let config: SmartConfig = serde_json::from_str("{}").unwrap();
        assert!(config.teams_enabled && config.telegram_enabled && !config.apps_enabled);
        assert_eq!(config.telegram_color, "0088FF");
        config.validate().unwrap();
        let bad = SmartConfig {
            telegram_flashes: 0,
            ..SmartConfig::default()
        };
        assert!(bad.validate().is_err());
        let rule = AppLighting {
            bundle_id: "com.example".into(),
            name: "Example".into(),
            lighting: solid("123456", 3),
        };
        let duplicate = SmartConfig {
            app_rules: vec![rule.clone(), rule],
            ..SmartConfig::default()
        };
        assert!(duplicate.validate().is_err());
    }

    #[test]
    fn presets_exist_for_every_menu_entry() {
        for name in ["off", "white", "warm", "rainbow", "breath"] {
            assert!(preset(name).is_some(), "{name}");
        }
        assert!(preset("shell").is_none());
        assert_eq!(
            app_rule(
                &SmartConfig {
                    app_rules: vec![AppLighting {
                        bundle_id: "a.b".into(),
                        name: "A".into(),
                        lighting: solid("ABCDEF", 2)
                    }],
                    ..SmartConfig::default()
                },
                Some("a.b")
            )
            .unwrap()
            .color,
            "ABCDEF"
        );
    }
}
