//! Host-only controls linked into the app's process and Accessibility identity.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KnobMode {
    #[default]
    Original,
    PlayPause,
    YoutubeMusic,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    pub knob_mode: KnobMode,
    pub launch_at_login: bool,
    pub youtube_app_id: Option<String>,
}
#[derive(Default)]
pub struct State {
    config: std::sync::Mutex<Config>,
    save_lock: tokio::sync::Mutex<()>,
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("mac-controls.json"))
}
fn read_config(path: &Path) -> Result<Config, String> {
    if !path.exists() {
        return Ok(Config::default());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid Mac controls configuration: {e}"))
}
fn write_config(path: &Path, config: &Config) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().ok_or("Invalid config path")?)
        .map_err(|e| e.to_string())?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn configure_request(app: &AppHandle, config: &Config) -> Result<Value, String> {
    Ok(json!({"operation":"configure", "mode":config.knob_mode,
        "youtube_app_id":config.youtube_app_id,
        "directory": app.path().app_data_dir().map_err(|e| e.to_string())?}))
}

/// Setup and shutdown are called by Tauri on the AppKit main thread.
pub fn initialize(app: &AppHandle) -> Result<(), String> {
    let config = read_config(&config_path(app)?)?;
    *app.state::<State>()
        .config
        .lock()
        .map_err(|e| e.to_string())? = config.clone();
    native::request(configure_request(app, &config)?)?;
    Ok(())
}
pub fn shutdown() {
    let _ = native::request(json!({"operation":"shutdown"}));
}
pub fn f20_reserved(app: &AppHandle) -> bool {
    app.state::<State>()
        .config
        .lock()
        .map(|c| c.knob_mode == KnobMode::YoutubeMusic)
        .unwrap_or(false)
}

async fn on_main(app: &AppHandle, request: Value) -> Result<Value, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(native::request(request));
    })
    .map_err(|e| e.to_string())?;
    receiver.await.map_err(|e| e.to_string())?
}
fn with_config(app: &AppHandle, mut status: Value) -> Result<Value, String> {
    status["config"] = serde_json::to_value(
        app.state::<State>()
            .config
            .lock()
            .map_err(|e| e.to_string())?
            .clone(),
    )
    .map_err(|e| e.to_string())?;
    Ok(status)
}
#[tauri::command]
pub async fn mac_controls_status(app: AppHandle) -> Result<Value, String> {
    with_config(&app, on_main(&app, json!({"operation":"status"})).await?)
}
#[tauri::command]
pub async fn mac_controls_refresh(app: AppHandle) -> Result<Value, String> {
    with_config(&app, on_main(&app, json!({"operation":"refresh"})).await?)
}
#[tauri::command]
pub async fn mac_controls_action(app: AppHandle, liked: bool) -> Result<Value, String> {
    with_config(
        &app,
        on_main(&app, json!({"operation":if liked {"liked"} else {"press"}})).await?,
    )
}
#[tauri::command]
pub async fn mac_controls_save(app: AppHandle, config: Config) -> Result<Value, String> {
    let state = app.state::<State>();
    let _guard = state.save_lock.lock().await;
    if config
        .youtube_app_id
        .as_ref()
        .is_some_and(|id| id.len() > 200 || !id.starts_with("com.apple.Safari.WebApp."))
    {
        return Err("Choose an installed YouTube Music Safari app".into());
    }
    if config.knob_mode == KnobMode::YoutubeMusic {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        if crate::automations::load(dir)?
            .iter()
            .any(|a| a.marker_hid == Some(111))
        {
            return Err(
                "F20 is assigned to an automation. Unassign it before enabling the music knob"
                    .into(),
            );
        }
    }
    let previous = state.config.lock().map_err(|e| e.to_string())?.clone();
    let status = on_main(&app, configure_request(&app, &config)?).await?;
    let save = (|| {
        set_autostart(&app, config.launch_at_login)?;
        write_config(&config_path(&app)?, &config)
    })();
    if let Err(error) = save {
        let _ = on_main(&app, configure_request(&app, &previous)?).await;
        let _ = set_autostart(&app, previous.launch_at_login);
        return Err(error);
    }
    *state.config.lock().map_err(|e| e.to_string())? = config;
    with_config(&app, status)
}

#[cfg(any(target_os = "macos", test))]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
#[cfg(any(target_os = "macos", test))]
fn launch_agent_plist(bundle: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Label</key><string>io.github.wsclx.ak820pro-modder.login</string>
<key>ProgramArguments</key><array><string>/usr/bin/open</string><string>-g</string><string>{}</string><string>--args</string><string>--background</string></array>
<key>RunAtLoad</key><true/><key>ProcessType</key><string>Background</string></dict></plist>
"#,
        xml_escape(bundle)
    )
}
#[cfg(target_os = "macos")]
fn set_autostart(_: &AppHandle, enabled: bool) -> Result<(), String> {
    let home = std::env::var_os("HOME").ok_or("Cannot locate the user's home")?;
    let agent = PathBuf::from(home)
        .join("Library/LaunchAgents/io.github.wsclx.ak820pro-modder.login.plist");
    if !enabled {
        if agent.exists() {
            std::fs::remove_file(agent).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = executable
        .ancestors()
        .nth(3)
        .filter(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or("Install the built .app before enabling launch at login")?;
    std::fs::create_dir_all(agent.parent().unwrap()).map_err(|e| e.to_string())?;
    let temporary = agent.with_extension("plist.tmp");
    std::fs::write(&temporary, launch_agent_plist(&bundle.to_string_lossy()))
        .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, agent).map_err(|e| e.to_string())
}
#[cfg(not(target_os = "macos"))]
fn set_autostart(_: &AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        Err("Mac controls require macOS".into())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::ffi::{c_char, c_void, CStr, CString};
    extern "C" {
        fn ak820_mac_controls_request(input: *const c_char) -> *mut c_char;
        fn ak820_mac_controls_free(pointer: *mut c_void);
        #[cfg(test)]
        pub(super) fn ak820_mac_controls_selftest() -> i32;
    }
    pub fn request(value: Value) -> Result<Value, String> {
        let input = CString::new(value.to_string()).map_err(|e| e.to_string())?;
        // Swift owns the allocated result. Copy before freeing with the same
        // allocator; no borrowed pointers escape this FFI boundary.
        let pointer = unsafe { ak820_mac_controls_request(input.as_ptr()) };
        if pointer.is_null() {
            return Err("Native Mac controls allocation failed".into());
        }
        let bytes = unsafe { CStr::from_ptr(pointer).to_bytes().to_vec() };
        unsafe { ak820_mac_controls_free(pointer.cast()) };
        let result: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if result.get("supported").is_none() {
            return Err(result["error"]
                .as_str()
                .unwrap_or("Native Mac controls failed")
                .to_string());
        }
        Ok(result)
    }
}
#[cfg(not(target_os = "macos"))]
mod native {
    use super::*;
    pub fn request(_: Value) -> Result<Value, String> {
        Ok(
            json!({"supported":false,"youtube_supported":false,"accessibility":false,"hotkey_active":false,"devices":[],"youtube_apps":[],"youtube_app_id":null,"message":"Mac controls require macOS","error":null}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_arguments_are_xml_values_not_shell_commands() {
        let plist = launch_agent_plist("/Users/A & B/Apps/AK820 <test>.app");
        assert!(plist.contains("A &amp; B/Apps/AK820 &lt;test&gt;.app"));
        assert!(!plist.contains("/bin/sh"));
        assert!(plist.contains("<string>--background</string>"));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn native_mapping_ownership_and_playback_decision_fixtures() {
        assert_eq!(unsafe { native::ak820_mac_controls_selftest() }, 0);
    }
}
