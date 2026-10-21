//! Procedural anime-style pixel-art scenes for the AK820 Pro's 128×128 TFT.
//!
//! Everything is drawn in code (no assets, no network) and fully
//! deterministic: the same inputs always produce the same bytes.

mod canvas;
mod character;
mod scenes;
mod text;

use canvas::{Canvas, BLACK};
use scenes::Ctx;

pub const SIZE: u32 = 128;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Character {
    /// Pink hair, long, emerald eyes.
    Sakura,
    /// Silver-blue bob, ice-blue eyes.
    Yuki,
    /// Black hime cut with cat ears, red eyes.
    Kuro,
    /// Blonde twin tails, violet eyes.
    Hikari,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Weather {
    Clear,
    Cloudy,
    Rain,
    Snow,
    Storm,
    Fog,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Scene {
    /// Time-of-day greeting card (weekday 0 = Monday, hour 0..=23).
    Daily {
        year: i32,
        month: u8,
        day: u8,
        weekday: u8,
        hour: u8,
    },
    /// Weather card with temperature, icon and city.
    Weather {
        temp_c: f32,
        weather: Weather,
        city: String,
        hour: u8,
    },
    /// Now playing card; `cover` must be exactly 128*128*3 RGB888 bytes,
    /// anything else falls back to a vinyl record.
    NowPlaying {
        title: String,
        artist: String,
        cover: Option<Vec<u8>>,
    },
    /// Pomodoro card; phase "focus" | "break" | "long_break" | "done".
    Pomodoro {
        phase: String,
        minutes: u32,
        round: u32,
    },
}

pub struct Frame {
    /// 128*128*3 row-major RGB888.
    pub rgb: Vec<u8>,
    pub delay_ms: u16,
}

/// Delay used for the single frame of a static render.
const STATIC_DELAY_MS: u16 = 1000;

fn anim_spec(scene: &Scene) -> (u32, u16) {
    match scene {
        Scene::Daily { .. } => (8, 220),
        Scene::Weather { weather, .. } => match weather {
            Weather::Rain | Weather::Storm => (6, 140),
            Weather::Snow => (8, 240),
            _ => (6, 260),
        },
        Scene::NowPlaying { .. } => (6, 200),
        Scene::Pomodoro { .. } => (6, 260),
    }
}

/// Render a scene. `animated = false` → exactly one frame; otherwise a short
/// seamless loop of 4..=8 frames (one of which blinks).
pub fn render(scene: &Scene, character: &Character, animated: bool) -> Vec<Frame> {
    let (n, delay) = if animated {
        anim_spec(scene)
    } else {
        (1, STATIC_DELAY_MS)
    };
    let n = n.clamp(1, 8);
    (0..n)
        .map(|frame| {
            let ctx = Ctx {
                frame,
                frames: n,
                blink: animated && frame == n / 2,
            };
            Frame {
                rgb: render_one(scene, character, &ctx),
                delay_ms: delay,
            }
        })
        .collect()
}

fn render_one(scene: &Scene, ch: &Character, ctx: &Ctx) -> Vec<u8> {
    let mut c = Canvas::new(SIZE, SIZE, BLACK);
    match scene {
        Scene::Daily {
            year,
            month,
            day,
            weekday,
            hour,
        } => scenes::daily(&mut c, ch, ctx, (*year, *month, *day, *weekday, *hour)),
        Scene::Weather {
            temp_c,
            weather,
            city,
            hour,
        } => scenes::weather(&mut c, ch, ctx, *temp_c, weather, city, *hour),
        Scene::NowPlaying {
            title,
            artist,
            cover,
        } => {
            let cover = cover
                .as_deref()
                .filter(|px| px.len() == (SIZE * SIZE * 3) as usize);
            scenes::now_playing(&mut c, ch, ctx, title, artist, cover)
        }
        Scene::Pomodoro {
            phase,
            minutes,
            round,
        } => scenes::pomodoro(&mut c, ch, ctx, phase, *minutes, *round),
    }
    c.to_bytes()
}

/// PNG bytes of a frame (for UI preview). Returns an empty Vec if the frame
/// buffer has the wrong size.
pub fn to_png(frame: &Frame) -> Vec<u8> {
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
    let mut out = Vec::new();
    if frame.rgb.len() != (SIZE * SIZE * 3) as usize {
        return out;
    }
    let enc = PngEncoder::new(&mut out);
    if enc
        .write_image(&frame.rgb, SIZE, SIZE, ExtendedColorType::Rgb8)
        .is_err()
    {
        out.clear();
    }
    out
}

/// Character list for UI: (id, display name).
pub fn characters() -> Vec<(Character, &'static str)> {
    vec![
        (Character::Sakura, "Sakura"),
        (Character::Yuki, "Yuki"),
        (Character::Kuro, "Kuro"),
        (Character::Hikari, "Hikari"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_LEN: usize = (SIZE * SIZE * 3) as usize;

    fn all_scenes() -> Vec<Scene> {
        let mut v = Vec::new();
        for hour in [7u8, 13, 19, 23, 3] {
            v.push(Scene::Daily {
                year: 2026,
                month: 10,
                day: 21,
                weekday: (hour % 7),
                hour,
            });
        }
        for (i, w) in [
            Weather::Clear,
            Weather::Cloudy,
            Weather::Rain,
            Weather::Snow,
            Weather::Storm,
            Weather::Fog,
        ]
        .into_iter()
        .enumerate()
        {
            v.push(Scene::Weather {
                temp_c: -3.0 + i as f32 * 7.0,
                weather: w.clone(),
                city: "Almaty".into(),
                hour: 14,
            });
            v.push(Scene::Weather {
                temp_c: 12.0,
                weather: w,
                city: "Astana".into(),
                hour: 23,
            });
        }
        v.push(Scene::NowPlaying {
            title: "Sparkle".into(),
            artist: "RADWIMPS".into(),
            cover: None,
        });
        v.push(Scene::NowPlaying {
            title: "Gradient".into(),
            artist: "Test".into(),
            cover: Some((0..FRAME_LEN).map(|i| (i * 7 % 256) as u8).collect()),
        });
        for phase in ["focus", "break", "long_break", "done", "weird"] {
            v.push(Scene::Pomodoro {
                phase: phase.into(),
                minutes: 25,
                round: 2,
            });
        }
        v
    }

    #[test]
    fn every_scene_and_character_renders() {
        for scene in all_scenes() {
            for (ch, _) in characters() {
                let st = render(&scene, &ch, false);
                assert_eq!(st.len(), 1);
                assert_eq!(st[0].rgb.len(), FRAME_LEN);
                let an = render(&scene, &ch, true);
                assert!((4..=8).contains(&an.len()), "{scene:?}: {}", an.len());
                for f in &an {
                    assert_eq!(f.rgb.len(), FRAME_LEN);
                    assert!((120..=400).contains(&f.delay_ms));
                }
                // Animation actually animates.
                assert!(an.iter().any(|f| f.rgb != an[0].rgb), "{scene:?} static");
            }
        }
    }

    #[test]
    fn png_signature() {
        let f = &render(&all_scenes()[0], &Character::Sakura, false)[0];
        let png = to_png(f);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let bad = Frame {
            rgb: vec![0; 3],
            delay_ms: 100,
        };
        assert!(to_png(&bad).is_empty());
    }

    #[test]
    fn deterministic() {
        for scene in all_scenes() {
            let a = render(&scene, &Character::Kuro, true);
            let b = render(&scene, &Character::Kuro, true);
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(&b) {
                assert_eq!(x.rgb, y.rgb);
                assert_eq!(x.delay_ms, y.delay_ms);
            }
        }
    }

    #[test]
    fn cover_variants() {
        let mk = |cover| Scene::NowPlaying {
            title: "T".into(),
            artist: "A".into(),
            cover,
        };
        let none = render(&mk(None), &Character::Yuki, false);
        let some = render(&mk(Some(vec![200; FRAME_LEN])), &Character::Yuki, false);
        let wrong = render(&mk(Some(vec![1, 2, 3])), &Character::Yuki, false);
        assert_ne!(none[0].rgb, some[0].rgb);
        assert_eq!(none[0].rgb, wrong[0].rgb);
    }

    #[test]
    fn long_and_weird_text_does_not_panic() {
        let long = "A ridiculously long song title that goes on and on ".repeat(20);
        for ch in [Character::Sakura, Character::Hikari] {
            render(
                &Scene::NowPlaying {
                    title: long.clone(),
                    artist: "Исполнитель 🎵 日本語".into(),
                    cover: None,
                },
                &ch,
                true,
            );
            render(
                &Scene::Weather {
                    temp_c: 21.0,
                    weather: Weather::Cloudy,
                    city: long.clone(),
                    hour: 12,
                },
                &ch,
                true,
            );
            render(
                &Scene::NowPlaying {
                    title: String::new(),
                    artist: String::new(),
                    cover: None,
                },
                &ch,
                false,
            );
        }
    }

    #[test]
    fn extreme_values_do_not_panic() {
        for t in [-40.0, 45.5, f32::NAN, f32::INFINITY, -1e9, 1e9] {
            render(
                &Scene::Weather {
                    temp_c: t,
                    weather: Weather::Snow,
                    city: "X".into(),
                    hour: 99,
                },
                &Character::Kuro,
                true,
            );
        }
        render(
            &Scene::Daily {
                year: i32::MAX,
                month: 255,
                day: 255,
                weekday: 255,
                hour: 255,
            },
            &Character::Hikari,
            true,
        );
        render(
            &Scene::Pomodoro {
                phase: "focus".into(),
                minutes: u32::MAX,
                round: u32::MAX,
            },
            &Character::Yuki,
            true,
        );
    }

    #[test]
    fn serde_ids() {
        let ids: Vec<String> = characters()
            .into_iter()
            .map(|(c, _)| serde_json::to_string(&c).unwrap())
            .collect();
        assert_eq!(ids, ["\"sakura\"", "\"yuki\"", "\"kuro\"", "\"hikari\""]);
        let c: Character = serde_json::from_str("\"kuro\"").unwrap();
        assert_eq!(c, Character::Kuro);
        let w: Weather = serde_json::from_str("\"storm\"").unwrap();
        assert_eq!(w, Weather::Storm);
    }
}
