//! Background services: native events, battery and the menu-bar state.
//! Everything here runs on the async runtime; the native layer only reports
//! events and renders the menu it is given.
use crate::base_lighting::{self, BaseLighting};
use crate::{ensure_open, mac_controls, ConnState};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

#[derive(Default)]
struct Status {
    battery: Option<u8>,
    battery_source: &'static str,
    low_battery_warned: Option<u8>,
    language: String,
}

pub struct Services {
    app: AppHandle,
    dir: PathBuf,
    pub lighting: Arc<BaseLighting>,
    conn: Arc<ConnState>,
    status: Mutex<Status>,
}

#[derive(Serialize)]
pub struct KeyboardStatus {
    pub usb: bool,
    pub battery: Option<u8>,
    pub battery_source: &'static str,
}

impl Services {
    pub fn new(app: AppHandle, conn: Arc<ConnState>) -> Result<Arc<Self>, String> {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        Ok(Arc::new(Self {
            lighting: BaseLighting::new(conn.clone(), dir.clone()),
            status: Mutex::new(Status {
                language: "en".into(),
                ..Status::default()
            }),
            app,
            dir,
            conn,
        }))
    }

    /// Starts the event loop and the periodic battery check.
    pub fn start(self: &Arc<Self>, mut events: tokio::sync::mpsc::UnboundedReceiver<Value>) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(event) = events.recv().await {
                let me = me.clone();
                tauri::async_runtime::spawn(async move { me.handle(event).await });
            }
        });
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(status) = mac_controls::request(&me.app, json!({"operation": "status"})).await
            {
                tracing::info!(
                    accessibility = ?status["accessibility"],
                    hotkeys = ?status["hotkey_active"],
                    devices = %status["devices"],
                    error = ?status["error"],
                    "Mac controls status"
                );
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            let mut minute = tokio::time::interval(Duration::from_secs(60));
            loop {
                minute.tick().await;
                me.battery_check().await;
            }
        });
    }

    async fn handle(self: &Arc<Self>, event: Value) {
        let text = |key: &str| event[key].as_str().unwrap_or_default().to_string();
        match event["event"].as_str().unwrap_or_default() {
            "log" => tracing::warn!(message = %text("message"), "native"),
            "gesture" => {
                tracing::info!(gesture = %text("gesture"), action = %text("action"), "knob")
            }
            "debug" => tracing::info!(detail = %event["detail"], "native debug"),
            "bluetooth_battery" => {
                if let Some(level) = event["level"].as_u64() {
                    self.battery_update(Some(level.min(100) as u8), "bluetooth")
                        .await;
                }
            }
            "devices" | "profile" => self.push_menu().await,
            "menu" => {
                tracing::info!(command = %text("command"), value = %text("value"), "command");
                if let Err(error) = self.menu(&text("command"), &text("value")).await {
                    tracing::warn!(%error, "command failed");
                    self.notify("AK820", &error).await;
                }
                self.push_menu().await;
            }
            _ => {}
        }
    }

    async fn menu(self: &Arc<Self>, command: &str, value: &str) -> Result<(), String> {
        match command {
            "knob_mode" => {
                let mut config = mac_controls::config(&self.app);
                config.knob_mode =
                    serde_json::from_value(json!(value)).map_err(|e| e.to_string())?;
                mac_controls::save(&self.app, config).await?;
                let _ = self
                    .app
                    .get_webview_window("main")
                    .map(|w| w.eval("window.dispatchEvent(new Event('mac-controls-updated'))"));
            }
            "lighting" if value == "restore" => self
                .lighting
                .restore()
                .await
                .map_err(|e| format!("{e:?}"))?,
            "lighting" => {
                let preset = base_lighting::preset(value).ok_or("Unknown lighting preset")?;
                self.lighting
                    .apply_base(preset)
                    .await
                    .map_err(|e| format!("{e:?}"))?;
            }
            "colour_profile" => {
                crate::apply_colour_profile(&self.app, Some(value))
                    .await
                    .map_err(|e| format!("{e:?}"))?;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn keyboard_status(&self) -> KeyboardStatus {
        let status = self.status.lock().await;
        KeyboardStatus {
            usb: base_lighting::usb_connected(),
            battery: status.battery,
            battery_source: status.battery_source,
        }
    }

    // MARK: battery

    async fn battery_check(self: &Arc<Self>) {
        let legacy = ak820_protocol::enumerate().ok().is_some_and(|devices| {
            devices.iter().any(|d| {
                d.is_control && d.protocol == ak820_protocol::device::DeviceProtocol::LegacyFeature
            })
        });
        if legacy {
            let reading = self
                .conn
                .with(|slot| Ok(ensure_open(slot)?.get_legacy_battery()?))
                .await;
            if let Ok(battery) = reading {
                // FF means wired/full: show 100% rather than a guess.
                let level = battery.percent.or(battery.wired_full.then_some(100));
                self.battery_update(level, "usb").await;
            }
            return;
        }
        let bluetooth = mac_controls::request(&self.app, json!({"operation": "status"}))
            .await
            .ok()
            .and_then(|status| status["devices"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .any(|d| d["transport"].as_str() == Some("Bluetooth"));
        if bluetooth {
            // The reply arrives as a `bluetooth_battery` event.
            let _ = mac_controls::request(&self.app, json!({"operation": "battery"})).await;
        } else {
            self.battery_update(None, "").await;
        }
    }
    async fn battery_update(self: &Arc<Self>, level: Option<u8>, source: &'static str) {
        let warn = {
            let mut status = self.status.lock().await;
            status.battery = level;
            status.battery_source = source;
            match level {
                Some(l) if l > 25 => {
                    status.low_battery_warned = None;
                    None
                }
                Some(l) => {
                    let threshold = if l <= 10 {
                        10
                    } else if l <= 20 {
                        20
                    } else {
                        0
                    };
                    let new =
                        threshold != 0 && status.low_battery_warned.is_none_or(|w| threshold < w);
                    if new {
                        status.low_battery_warned = Some(threshold);
                    }
                    new.then_some((l, status.language == "ru"))
                }
                None => None,
            }
        };
        if let Some((level, russian)) = warn {
            let (title, body) = if russian {
                (
                    "AK820: низкий заряд",
                    format!("Осталось {level}%. Подключите кабель USB-C."),
                )
            } else {
                (
                    "AK820 battery low",
                    format!("{level}% left. Connect the USB-C cable."),
                )
            };
            self.notify(title, &body).await;
        }
        self.push_menu().await;
    }

    // MARK: menu and notifications

    pub async fn set_language(self: &Arc<Self>, language: String) {
        self.status.lock().await.language = if language == "ru" {
            "ru".into()
        } else {
            "en".into()
        };
        self.push_menu().await;
    }
    async fn notify(&self, title: &str, body: &str) {
        let _ = mac_controls::request(
            &self.app,
            json!({"operation": "notify", "title": title, "body": body}),
        )
        .await;
    }
    pub async fn push_menu(&self) {
        let colours = crate::colour_profiles::load(&self.dir);
        let status = self.status.lock().await;
        let state = json!({
            "battery": status.battery,
            "usb": base_lighting::usb_connected(),
            "language": status.language,
            "colour_profiles": colours.profiles.iter().map(|p| &p.name).collect::<Vec<_>>(),
            "colour_active": colours.active,
        });
        drop(status);
        let _ = mac_controls::request(
            &self.app,
            json!({"operation": "menu_state", "state": state}),
        )
        .await;
    }

    /// Re-reads settings files, e.g. after an import or iCloud pull.
    pub async fn reload(self: &Arc<Self>) -> Result<(), String> {
        self.lighting.reload().await;
        self.push_menu().await;
        Ok(())
    }
}
