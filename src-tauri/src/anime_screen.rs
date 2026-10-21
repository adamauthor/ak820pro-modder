//! Anime cards for the TFT: a greeting for the time of day, the weather and
//! the current track, drawn by `ak820-anime`.
//!
//! On the FF13 revision every upload replaces the stored animation in the
//! keyboard's flash, so automatic refreshes need an explicit opt-in and are
//! rate limited (at least 15 minutes apart).
use ak820_anime::{Character, Scene, Weather};
use ak820_protocol::commands::tft::{TftAnimation, TftFrame};
use ak820_protocol::Connection;
use serde::{Deserialize, Serialize};
use std::process::Command;

pub const CONFIG_FILE: &str = "anime-screen.json";
pub const MIN_REFRESH_MINUTES: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AnimeConfig {
    pub character: Character,
    pub animated: bool,
    /// City for the weather card; resolved through Open-Meteo geocoding.
    pub city: String,
    /// Allows uploads that replace the keyboard's stored animation.
    pub consent: bool,
    pub auto: bool,
    /// Kept for settings files from 0.9.0; see `ambient_minutes`.
    pub refresh_minutes: u32,
    /// Scenes cycled by the automatic refresh.
    pub rotation: Vec<String>,
    /// Kept for settings files from 0.9.0; context mode replaced it.
    pub on_track_change: bool,
    /// While music plays, how often the track is checked and the card
    /// refreshed when it changed.
    pub music_minutes: u32,
    /// Without music, how often the greeting and weather cards alternate.
    pub ambient_minutes: u32,
    /// Animate automatic cards too. The keyboard's lighting pauses for the
    /// whole transfer: about 1.5 s for one frame, 8 s for an animation.
    pub auto_animated: bool,
}

impl Default for AnimeConfig {
    fn default() -> Self {
        Self {
            character: Character::Sakura,
            animated: true,
            city: String::new(),
            consent: false,
            auto: false,
            refresh_minutes: 60,
            rotation: vec!["daily".into(), "weather".into(), "now_playing".into()],
            on_track_change: false,
            music_minutes: 2,
            ambient_minutes: 30,
            auto_animated: false,
        }
    }
}

impl AnimeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=30).contains(&self.music_minutes)
            || !(MIN_REFRESH_MINUTES..=240).contains(&self.ambient_minutes)
        {
            return Err(format!(
                "Music check every 1–30 minutes, greeting/weather every {MIN_REFRESH_MINUTES}–240 minutes"
            ));
        }
        if self.city.chars().count() > 80
            || self.rotation.is_empty()
            || self.rotation.iter().any(|s| !SCENES.contains(&s.as_str()))
        {
            return Err("Choose valid anime scenes and a city name up to 80 characters".into());
        }
        Ok(())
    }
}

pub const SCENES: [&str; 3] = ["daily", "weather", "now_playing"];

fn curl(url: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("/usr/bin/curl")
        .args(["-fsSL", "--max-time", "8", "--proto", "=https", url])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !output.status.success() {
        return Err("Network request failed; check the connection".into());
    }
    Ok(output.stdout)
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// WMO weather interpretation codes used by Open-Meteo.
pub fn weather_from_code(code: i64) -> Weather {
    match code {
        0 | 1 => Weather::Clear,
        2 | 3 => Weather::Cloudy,
        45 | 48 => Weather::Fog,
        71..=77 | 85 | 86 => Weather::Snow,
        95..=99 => Weather::Storm,
        _ => Weather::Rain,
    }
}

/// Current temperature and weather for a city name. Only the city name is
/// sent, to Open-Meteo (no account, no key).
pub fn fetch_weather(city: &str) -> Result<(f32, Weather, String), String> {
    let fallback = timezone_city();
    let city = if city.trim().is_empty() {
        fallback
            .as_deref()
            .ok_or("Enter a city for the weather card")?
    } else {
        city
    };
    let geo: serde_json::Value = serde_json::from_slice(&curl(&format!(
        "https://geocoding-api.open-meteo.com/v1/search?count=1&name={}",
        percent_encode(city.trim())
    ))?)
    .map_err(|e| e.to_string())?;
    let place = geo["results"]
        .get(0)
        .ok_or("City not found for the weather card")?;
    let (lat, lon) = (
        place["latitude"]
            .as_f64()
            .ok_or("Invalid city coordinates")?,
        place["longitude"]
            .as_f64()
            .ok_or("Invalid city coordinates")?,
    );
    let name = place["name"].as_str().unwrap_or(city).to_string();
    let forecast: serde_json::Value = serde_json::from_slice(&curl(&format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.3}&longitude={lon:.3}&current=temperature_2m,weather_code"
    ))?)
    .map_err(|e| e.to_string())?;
    let temp = forecast["current"]["temperature_2m"]
        .as_f64()
        .ok_or("Weather service returned no temperature")? as f32;
    let code = forecast["current"]["weather_code"].as_i64().unwrap_or(3);
    Ok((temp, weather_from_code(code), name))
}

/// The Mac's time-zone city ("Asia/Almaty" → "Almaty"), used when no city
/// is set. No location permission or lookup is involved.
pub fn timezone_city() -> Option<String> {
    let target = std::fs::read_link("/etc/localtime").ok()?;
    city_from_zone(&target.to_string_lossy())
}
fn city_from_zone(path: &str) -> Option<String> {
    let zone = path.split("zoneinfo/").nth(1)?;
    let city = zone.rsplit('/').next()?.replace('_', " ");
    (!city.is_empty() && zone.contains('/')).then_some(city)
}

/// Extracts the bytes of an AppleScript `«data TYPEhex»` literal.
pub fn parse_applescript_data(output: &str) -> Option<Vec<u8>> {
    let start = output.find("«data ")? + "«data ".len();
    let rest = &output[start..];
    let end = rest.find('»')?;
    let hex = rest[..end].get(4..)?; // skip the four-character type code
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

fn osascript(script: &str) -> Option<String> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Cover art of the playing track, scaled to 128×128 RGB.
pub fn fetch_cover(source: &str) -> Option<Vec<u8>> {
    let bytes = match source {
        "Spotify" => {
            let url = osascript(
                r#"tell application "Spotify" to if it is running then return artwork url of current track"#,
            )?;
            url.starts_with("https://").then(|| curl(&url).ok())??
        }
        "Music" => parse_applescript_data(&osascript(
            r#"tell application "Music" to if it is running then return raw data of artwork 1 of current track"#,
        )?)?,
        _ => return None,
    };
    cover_from(&bytes)
}

fn cover_from(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?;
    let side = ak820_anime::SIZE;
    Some(
        image
            .resize_to_fill(side, side, image::imageops::FilterType::Triangle)
            .to_rgb8()
            .into_raw(),
    )
}

fn local_clock() -> (i32, u8, u8, u8, u8) {
    let now =
        ak820_protocol::legacy::local_now().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    (
        now.year(),
        now.month() as u8,
        now.day(),
        now.weekday().number_days_from_monday(),
        now.hour(),
    )
}

/// Collects live data for a scene. Blocking: run off the async runtime.
/// `youtube` is the YouTube Music player state read by the native layer.
pub fn build_scene(
    name: &str,
    config: &AnimeConfig,
    youtube: Option<serde_json::Value>,
) -> Result<Scene, String> {
    let (year, month, day, weekday, hour) = local_clock();
    Ok(match name {
        "daily" => Scene::Daily {
            year,
            month,
            day,
            weekday,
            hour,
        },
        "weather" => {
            let (temp_c, weather, city) = fetch_weather(&config.city)?;
            Scene::Weather {
                temp_c,
                weather,
                city,
                hour,
            }
        }
        "now_playing" => {
            if let Some(playing) = crate::now_playing::fetch().filter(|p| p.is_playing) {
                Scene::NowPlaying {
                    title: playing.title.clone().unwrap_or_default(),
                    artist: playing.artist.clone().unwrap_or_default(),
                    cover: fetch_cover(&playing.source),
                }
            } else if let Some(yt) = youtube {
                let text = |key: &str| yt[key].as_str().unwrap_or_default().to_string();
                let cover = yt["cover_url"]
                    .as_str()
                    .filter(|url| url.starts_with("https://"))
                    .and_then(|url| curl(url).ok())
                    .and_then(|bytes| cover_from(&bytes));
                Scene::NowPlaying {
                    title: text("title"),
                    artist: text("artist"),
                    cover,
                }
            } else {
                return Err("Nothing is playing in YouTube Music, Music or Spotify. YouTube Music needs Accessibility for Modder".into());
            }
        }
        other => return Err(format!("Unknown anime scene: {other}")),
    })
}

pub fn animation(scene: &Scene, config: &AnimeConfig) -> Result<TftAnimation, String> {
    let frames = ak820_anime::render(scene, &config.character, config.animated)
        .into_iter()
        .map(|f| TftFrame::from_rgb888(&f.rgb, f.delay_ms).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TftAnimation { frames })
}

/// PNG data URLs of every frame, for the preview in the app.
pub fn preview(scene: &Scene, config: &AnimeConfig) -> Vec<String> {
    ak820_anime::render(scene, &config.character, config.animated)
        .iter()
        .map(|f| format!("data:image/png;base64,{}", base64(&ak820_anime::to_png(f))))
        .collect()
}

/// Uploads over USB. Blocking.
pub fn upload(scene: &Scene, config: &AnimeConfig) -> Result<(), String> {
    if !config.consent {
        return Err(
            "Allow anime uploads first: each one replaces the keyboard's stored animation".into(),
        );
    }
    let anim = animation(scene, config)?;
    // Battery reads and lighting briefly hold the keyboard's transaction lock.
    let mut attempt = 0;
    loop {
        let result = Connection::open_tft()
            .and_then(|tft| tft.upload_tft_animation_confirmed(&anim, true))
            .map_err(|e| e.to_string());
        match result {
            Err(error) if error.contains("Another keyboard operation") && attempt < 8 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(750));
            }
            // Nothing was saved when the first chunk is refused; try once more.
            Err(error) if error.contains("No acknowledgement for TFT chunk 1/") && attempt < 1 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
            other => return other,
        }
    }
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(
            percent_encode("Алматы ok"),
            "%D0%90%D0%BB%D0%BC%D0%B0%D1%82%D1%8B%20ok"
        );
        assert_eq!(
            parse_applescript_data("«data JPEGFFD8FF»"),
            Some(vec![0xFF, 0xD8, 0xFF])
        );
        assert_eq!(parse_applescript_data("missing value"), None);
        assert_eq!(weather_from_code(0), Weather::Clear);
        assert_eq!(weather_from_code(61), Weather::Rain);
        assert_eq!(weather_from_code(73), Weather::Snow);
        assert_eq!(weather_from_code(96), Weather::Storm);
        assert_eq!(
            city_from_zone("/var/db/timezone/zoneinfo/Asia/Almaty").as_deref(),
            Some("Almaty")
        );
        assert_eq!(
            city_from_zone("/usr/share/zoneinfo/America/New_York").as_deref(),
            Some("New York")
        );
        assert_eq!(city_from_zone("/usr/share/zoneinfo/UTC"), None);
    }

    #[test]
    fn config_limits_protect_flash() {
        let config = AnimeConfig::default();
        config.validate().unwrap();
        assert!(!config.consent && !config.auto, "uploads are opt-in");
        assert!(AnimeConfig {
            ambient_minutes: 5,
            ..config.clone()
        }
        .validate()
        .is_err());
        assert!(AnimeConfig {
            music_minutes: 0,
            ..config.clone()
        }
        .validate()
        .is_err());
        assert_eq!(config.music_minutes, 2);
        assert!(AnimeConfig {
            rotation: vec!["shell".into()],
            ..config.clone()
        }
        .validate()
        .is_err());
        assert!(upload(
            &Scene::Daily {
                year: 2026,
                month: 1,
                day: 1,
                weekday: 0,
                hour: 9
            },
            &config
        )
        .is_err());
    }

    #[test]
    fn rendered_cards_fit_the_legacy_upload() {
        let config = AnimeConfig::default();
        let scene = Scene::Daily {
            year: 2026,
            month: 10,
            day: 21,
            weekday: 2,
            hour: 20,
        };
        let anim = animation(&scene, &config).unwrap();
        assert!(!anim.frames.is_empty() && anim.frames.len() <= 8);
        ak820_protocol::legacy_tft::encode_animation(&anim).unwrap();
        assert!(preview(&scene, &config)[0].starts_with("data:image/png;base64,iVBOR"));
    }
}
