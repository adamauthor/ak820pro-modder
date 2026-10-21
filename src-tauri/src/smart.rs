//! Background services: native events, Pomodoro, battery, anime screen and
//! the menu-bar state. Everything here runs on the async runtime; the
//! native layer only reports events and renders the menu it is given.
use crate::anime_screen::{self, AnimeConfig};
use crate::pomodoro::{self, Phase, PomodoroConfig, Timer};
use crate::smart_lighting::{self, SmartConfig, SmartLighting};
use crate::{ensure_open, json_store, mac_controls, ConnState};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

#[derive(Default)]
struct Status {
    battery: Option<u8>,
    battery_source: &'static str,
    low_battery_warned: Option<u8>,
    language: String,
    last_anime: Option<Instant>,
    rotation: usize,
    last_track: Option<String>,
    /// The TFT shows a Pomodoro card that should be replaced when it ends.
    pomodoro_card: bool,
}

pub struct Smart {
    app: AppHandle,
    dir: PathBuf,
    pub lighting: Arc<SmartLighting>,
    conn: Arc<ConnState>,
    timer: Mutex<Timer>,
    pomodoro_config: Mutex<PomodoroConfig>,
    anime_config: Mutex<AnimeConfig>,
    status: Mutex<Status>,
}

#[derive(Serialize)]
pub struct SmartStatus {
    pub config: SmartConfig,
    pub teams_active: bool,
    pub usb: bool,
    pub battery: Option<u8>,
    pub battery_source: &'static str,
}

impl Smart {
    pub fn new(app: AppHandle, conn: Arc<ConnState>) -> Result<Arc<Self>, String> {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        Ok(Arc::new(Self {
            lighting: SmartLighting::new(conn.clone(), dir.clone()),
            pomodoro_config: Mutex::new(json_store::load(&dir, pomodoro::CONFIG_FILE)),
            anime_config: Mutex::new(json_store::load(&dir, anime_screen::CONFIG_FILE)),
            timer: Mutex::new(Timer::default()),
            status: Mutex::new(Status {
                language: "en".into(),
                ..Status::default()
            }),
            app,
            dir,
            conn,
        }))
    }

    /// Starts the event loop and the periodic tasks.
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
            // Automatic anime cards start one interval after launch, so
            // restarts do not rewrite the keyboard's flash each time.
            me.status.lock().await.last_anime = Some(Instant::now());
            me.push_detectors().await;
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
            let mut second = tokio::time::interval(Duration::from_secs(1));
            let mut ticks: u64 = 0;
            loop {
                second.tick().await;
                me.pomodoro_tick().await;
                if ticks % 5 == 0 {
                    me.lighting.tick().await;
                }
                if ticks % 15 == 5 {
                    me.track_check().await;
                }
                if ticks % 60 == 2 {
                    me.battery_check().await;
                    me.anime_auto().await;
                }
                ticks += 1;
            }
        });
    }

    async fn handle(self: &Arc<Self>, event: Value) {
        let text = |key: &str| event[key].as_str().unwrap_or_default().to_string();
        match event["event"].as_str().unwrap_or_default() {
            "frontmost" => {
                self.lighting
                    .on_frontmost(event["bundle"].as_str().map(String::from))
                    .await
            }
            "teams_ringing" => {
                tracing::info!(active = ?event["active"], "Teams incoming call");
                self.lighting
                    .set_ringing(event["active"].as_bool() == Some(true))
                    .await;
            }
            "teams_call" => {
                tracing::info!(active = ?event["active"], "Teams call");
                self.lighting
                    .set_teams(event["active"].as_bool() == Some(true))
                    .await;
            }
            "telegram_message" => {
                tracing::info!(unread = ?event["unread"], "Telegram message: flashing");
                self.lighting.telegram_message().await
            }
            "log" => tracing::warn!(message = %text("message"), "native"),
            "gesture" => {
                tracing::info!(gesture = %text("gesture"), action = %text("action"), "knob")
            }
            "debug" => tracing::info!(detail = %event["detail"], "native debug"),
            "knob_action" if text("action") == "pomodoro" => self.pomodoro_toggle().await,
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
            "lighting" if value == "restore" => self.lighting.restore().await,
            "lighting" => {
                let preset = smart_lighting::preset(value).ok_or("Unknown lighting preset")?;
                self.lighting
                    .apply_base(preset)
                    .await
                    .map_err(|e| format!("{e:?}"))?;
            }
            "smart" => {
                let mut config = self.lighting.config().await;
                match value {
                    "teams" => config.teams_enabled = !config.teams_enabled,
                    "telegram" => config.telegram_enabled = !config.telegram_enabled,
                    "apps" => config.apps_enabled = !config.apps_enabled,
                    _ => return Err("Unknown smart lighting option".into()),
                }
                self.save_smart(config).await?;
            }
            "pomodoro" => self.pomodoro_command(value).await?,
            "test" => match value {
                "telegram" => self.lighting.telegram_message().await,
                "teams_ring" | "teams" => {
                    let ring = value == "teams_ring";
                    if ring {
                        self.lighting.set_ringing(true).await
                    } else {
                        self.lighting.set_teams(true).await
                    }
                    tokio::time::sleep(Duration::from_secs(6)).await;
                    if ring {
                        self.lighting.set_ringing(false).await
                    } else {
                        self.lighting.set_teams(false).await
                    }
                }
                _ => return Err("Unknown test".into()),
            },
            "anime" => self.anime_upload(value).await?,
            _ => {}
        }
        Ok(())
    }

    // MARK: smart lighting

    pub async fn smart_status(&self) -> SmartStatus {
        let status = self.status.lock().await;
        SmartStatus {
            config: self.lighting.config().await,
            teams_active: self.lighting.teams_active().await,
            usb: smart_lighting::usb_connected(),
            battery: status.battery,
            battery_source: status.battery_source,
        }
    }
    pub async fn save_smart(self: &Arc<Self>, config: SmartConfig) -> Result<(), String> {
        self.lighting.set_config(config).await?;
        self.push_detectors().await;
        self.push_menu().await;
        Ok(())
    }
    async fn push_detectors(&self) {
        let config = self.lighting.config().await;
        let _ = mac_controls::request(
            &self.app,
            json!({"operation": "detectors", "teams": config.teams_enabled, "telegram": config.telegram_enabled}),
        )
        .await;
    }

    // MARK: Pomodoro

    pub async fn pomodoro_status(&self) -> (pomodoro::Snapshot, PomodoroConfig) {
        (
            self.timer.lock().await.snapshot(),
            self.pomodoro_config.lock().await.clone(),
        )
    }
    pub async fn save_pomodoro(&self, config: PomodoroConfig) -> Result<(), String> {
        config.validate()?;
        json_store::save(&self.dir, pomodoro::CONFIG_FILE, &config)?;
        *self.pomodoro_config.lock().await = config;
        Ok(())
    }
    async fn pomodoro_toggle(self: &Arc<Self>) {
        let command = {
            let timer = self.timer.lock().await;
            match (timer.phase, timer.paused) {
                (Phase::Idle, _) => "start",
                (_, true) => "resume",
                _ => "pause",
            }
        };
        let _ = self.pomodoro_command(command).await;
    }
    pub async fn pomodoro_command(self: &Arc<Self>, command: &str) -> Result<(), String> {
        let config = self.pomodoro_config.lock().await.clone();
        let transition = {
            let mut timer = self.timer.lock().await;
            match command {
                "start" => {
                    timer.start(&config);
                    None
                }
                "pause" => {
                    timer.pause();
                    None
                }
                "resume" => {
                    timer.resume();
                    None
                }
                "stop" => {
                    timer.stop();
                    drop(timer);
                    self.replace_pomodoro_card();
                    None
                }
                "skip" => timer.skip(&config),
                _ => return Err("Unknown Pomodoro command".into()),
            }
        };
        if let Some(transition) = transition {
            self.pomodoro_transition(transition, &config).await;
        }
        self.pomodoro_lighting(&config).await;
        self.push_menu().await;
        Ok(())
    }
    async fn pomodoro_tick(self: &Arc<Self>) {
        let config = self.pomodoro_config.lock().await.clone();
        let (transition, active) = {
            let mut timer = self.timer.lock().await;
            let active = timer.phase != Phase::Idle && !timer.paused;
            (timer.tick(&config), active)
        };
        if let Some(transition) = transition {
            self.pomodoro_transition(transition, &config).await;
        }
        if active {
            self.pomodoro_lighting(&config).await;
            self.push_menu().await;
        }
    }
    async fn pomodoro_lighting(self: &Arc<Self>, config: &PomodoroConfig) {
        let lighting = self.timer.lock().await.lighting(config);
        self.lighting.set_pomodoro(lighting).await;
    }
    async fn pomodoro_transition(
        self: &Arc<Self>,
        t: pomodoro::Transition,
        config: &PomodoroConfig,
    ) {
        let russian = self.status.lock().await.language == "ru";
        let waiting = self.timer.lock().await.waiting.is_some();
        // Refresh the layer before the flash so it hands back to the new phase.
        self.pomodoro_lighting(config).await;
        let lighting = self.lighting.clone();
        let color = config.color.clone();
        // The flash and the card share the keyboard's single transaction
        // lock, so the card waits until the flash has finished.
        let card = config.anime_screen.then(|| self.pomodoro_scene(&t, config));
        // A single frame uploads in about two seconds instead of ten, so the
        // keyboard is busy (and its lighting paused) only briefly.
        let anime = AnimeConfig {
            animated: false,
            ..self.anime_config.lock().await.clone()
        };
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            lighting.flash(&color, 3).await;
            if let Some(scene) = card.flatten() {
                let exclusive = lighting.begin_upload().await;
                let result = tauri::async_runtime::spawn_blocking(move || {
                    anime_screen::upload(&scene, &anime)
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r);
                match result {
                    Ok(()) => {
                        tracing::info!("Pomodoro anime card uploaded");
                        me.status.lock().await.pomodoro_card = true;
                    }
                    Err(error) => tracing::warn!(%error, "Pomodoro anime card"),
                }
                lighting.end_upload(exclusive).await;
                lighting.restore().await;
            }
        });
        if config.notifications {
            let next = pomodoro::phase_name(t.next, russian);
            let body = match (waiting, russian) {
                (true, true) => format!("Дальше: {next}. Нажмите «Начать», когда будете готовы."),
                (true, false) => format!("Next: {next}. Start it when you are ready."),
                (false, true) => format!("Начинается: {next}"),
                (false, false) => format!("Starting: {next}"),
            };
            let title = format!("🍅 {}", pomodoro::phase_name(t.finished, russian));
            self.notify(&title, &body).await;
        }
    }
    /// After Stop, swaps the Pomodoro card for the regular greeting card.
    fn replace_pomodoro_card(self: &Arc<Self>) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let shown = std::mem::take(&mut me.status.lock().await.pomodoro_card);
            if shown && smart_lighting::usb_connected() {
                if let Err(error) = me.anime_upload("daily").await {
                    tracing::warn!(%error, "replacing the Pomodoro card");
                }
            }
        });
    }
    /// The card for the phase that starts now; None without USB.
    fn pomodoro_scene(
        &self,
        t: &pomodoro::Transition,
        config: &PomodoroConfig,
    ) -> Option<ak820_anime::Scene> {
        if !smart_lighting::usb_connected() {
            return None;
        }
        let (phase, minutes) = match t.next {
            Phase::Focus => ("focus", config.focus_minutes),
            Phase::ShortBreak => ("break", config.short_break_minutes),
            _ => ("long_break", config.long_break_minutes),
        };
        let round = self.timer.try_lock().map(|t| t.round).unwrap_or(0);
        Some(ak820_anime::Scene::Pomodoro {
            phase: phase.into(),
            minutes,
            round,
        })
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

    // MARK: anime screen

    pub async fn anime_config(&self) -> AnimeConfig {
        self.anime_config.lock().await.clone()
    }
    pub async fn save_anime(&self, config: AnimeConfig) -> Result<(), String> {
        config.validate()?;
        json_store::save(&self.dir, anime_screen::CONFIG_FILE, &config)?;
        *self.anime_config.lock().await = config;
        Ok(())
    }
    /// YouTube Music has no AppleScript; its player bar is read natively.
    async fn youtube_playing(&self, scene: &str) -> Option<Value> {
        if scene != "now_playing" {
            return None;
        }
        let status = mac_controls::request(&self.app, json!({"operation": "now_playing"}))
            .await
            .ok()?;
        let playing = status.get("now_playing").filter(|v| !v.is_null()).cloned();
        tracing::info!(?playing, "YouTube Music now playing");
        playing
    }
    pub async fn anime_preview(&self, scene: String) -> Result<Vec<String>, String> {
        let config = self.anime_config().await;
        let youtube = self.youtube_playing(&scene).await;
        tauri::async_runtime::spawn_blocking(move || {
            let built = anime_screen::build_scene(&scene, &config, youtube)?;
            Ok(anime_screen::preview(&built, &config))
        })
        .await
        .map_err(|e| e.to_string())?
    }
    pub async fn anime_upload(&self, scene: &str) -> Result<(), String> {
        let config = self.anime_config().await;
        let youtube = self.youtube_playing(scene).await;
        let name = scene.to_string();
        let exclusive = self.lighting.begin_upload().await;
        let result = tauri::async_runtime::spawn_blocking(move || {
            let built = anime_screen::build_scene(&name, &config, youtube)?;
            anime_screen::upload(&built, &config)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
        self.lighting.end_upload(exclusive).await;
        // The FF13 image transfer can change the LEDs; redraw the winning layer.
        self.lighting.restore().await;
        match &result {
            Ok(()) => {
                tracing::info!(scene, "anime card uploaded");
                self.status.lock().await.pomodoro_card = false;
            }
            Err(error) => tracing::warn!(scene, %error, "anime card failed"),
        }
        result?;
        self.status.lock().await.last_anime = Some(Instant::now());
        Ok(())
    }
    async fn anime_auto(&self) {
        let config = self.anime_config().await;
        if !config.auto || !config.consent || !smart_lighting::usb_connected() {
            return;
        }
        let scene = {
            let mut status = self.status.lock().await;
            let due = status.last_anime.is_none_or(|t| {
                t.elapsed() >= Duration::from_secs(u64::from(config.refresh_minutes) * 60)
            });
            if !due {
                return;
            }
            status.rotation = (status.rotation + 1) % config.rotation.len();
            config.rotation[status.rotation].clone()
        };
        if let Err(error) = self.anime_upload(&scene).await {
            tracing::warn!(%error, scene, "automatic anime card");
            // Do not hammer a failing source; try again next interval.
            self.status.lock().await.last_anime = Some(Instant::now());
        }
    }
    async fn track_check(&self) {
        let config = self.anime_config().await;
        if !config.on_track_change || !config.consent {
            return;
        }
        let Ok(Some(playing)) =
            tauri::async_runtime::spawn_blocking(crate::now_playing::fetch).await
        else {
            return;
        };
        if !playing.is_playing {
            return;
        }
        let track = format!("{:?}|{:?}", playing.title, playing.artist);
        let changed = {
            let mut status = self.status.lock().await;
            let changed = status.last_track.as_deref() != Some(track.as_str());
            let rested = status
                .last_anime
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(5 * 60));
            if changed {
                status.last_track = Some(track);
            }
            changed && rested
        };
        if changed && smart_lighting::usb_connected() {
            if let Err(error) = self.anime_upload("now_playing").await {
                tracing::warn!(%error, "now-playing anime card");
            }
        }
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
        let (snapshot, label) = {
            let timer = self.timer.lock().await;
            (timer.snapshot(), timer.label())
        };
        let config = self.lighting.config().await;
        let status = self.status.lock().await;
        let russian = status.language == "ru";
        let pomodoro_state = match (snapshot.phase, snapshot.paused) {
            (Phase::Idle, _) => "idle",
            (_, true) => "paused",
            _ => "running",
        };
        let pomodoro_status = if snapshot.phase == Phase::Idle {
            String::new()
        } else {
            format!(
                "🍅 {} — {label}",
                pomodoro::phase_name(snapshot.phase, russian)
            )
        };
        let state = json!({
            "battery": status.battery,
            "pomodoro_label": label,
            "pomodoro": pomodoro_state,
            "pomodoro_status": pomodoro_status,
            "smart": {"teams": config.teams_enabled, "telegram": config.telegram_enabled, "apps": config.apps_enabled},
            "usb": smart_lighting::usb_connected(),
            "language": status.language,
        });
        drop(status);
        let _ = mac_controls::request(
            &self.app,
            json!({"operation": "menu_state", "state": state}),
        )
        .await;
    }
}

impl Smart {
    /// Re-reads every settings file, e.g. after an import or iCloud pull.
    pub async fn reload(self: &Arc<Self>) -> Result<(), String> {
        let smart: SmartConfig = json_store::load(&self.dir, smart_lighting::CONFIG_FILE);
        self.lighting.set_config(smart).await?;
        *self.pomodoro_config.lock().await = json_store::load(&self.dir, pomodoro::CONFIG_FILE);
        *self.anime_config.lock().await = json_store::load(&self.dir, anime_screen::CONFIG_FILE);
        self.push_detectors().await;
        self.push_menu().await;
        Ok(())
    }
}
