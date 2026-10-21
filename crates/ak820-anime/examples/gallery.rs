//! Writes PNG previews of every scene × character into a directory:
//! `<name>.png` (128px), `<name>@4x.png` (nearest-neighbour 512px), plus a
//! `sheet_<scene>.png` per scene (all characters side by side, 3x) and
//! `anim_<scene>.png` strips showing every animation frame (2x).
//!
//! Usage: cargo run -p ak820-anime --example gallery -- <out-dir>

use ak820_anime::{characters, render, to_png, Frame, Scene, Weather, SIZE};
use image::{imageops, ImageFormat, RgbImage};
use std::path::Path;

fn scenes() -> Vec<(&'static str, Scene)> {
    let daily = |hour, weekday| Scene::Daily {
        year: 2026,
        month: 10,
        day: 21,
        weekday,
        hour,
    };
    let weather = |weather, temp_c, hour| Scene::Weather {
        temp_c,
        weather,
        city: "Almaty".into(),
        hour,
    };
    let cover: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|i| {
            let (x, y) = (i % SIZE, i / SIZE);
            let ring = (((x as f32 - 64.0).hypot(y as f32 - 64.0)) / 8.0) as u32 % 2;
            [
                (40 + x + ring * 60) as u8,
                (60 + y / 2) as u8,
                (200 - y / 2 + ring * 30) as u8,
            ]
        })
        .collect();
    let pomo = |phase: &str| Scene::Pomodoro {
        phase: phase.into(),
        minutes: if phase == "focus" { 25 } else { 5 },
        round: 2,
    };
    vec![
        ("daily_morning", daily(8, 0)),
        ("daily_day", daily(13, 5)),
        ("daily_evening", daily(19, 6)),
        ("daily_night", daily(23, 3)),
        ("weather_clear", weather(Weather::Clear, 24.0, 13)),
        ("weather_clear_night", weather(Weather::Clear, 12.0, 23)),
        ("weather_cloudy", weather(Weather::Cloudy, 15.0, 13)),
        ("weather_rain", weather(Weather::Rain, 9.0, 13)),
        ("weather_snow", weather(Weather::Snow, -3.0, 13)),
        ("weather_storm", weather(Weather::Storm, 18.0, 20)),
        ("weather_fog", weather(Weather::Fog, -40.0, 8)),
        (
            "music_vinyl",
            Scene::NowPlaying {
                title: "Suzume (feat. Toaka) - Extended Version".into(),
                artist: "RADWIMPS".into(),
                cover: None,
            },
        ),
        (
            "music_cover",
            Scene::NowPlaying {
                title: "Idol".into(),
                artist: "YOASOBI".into(),
                cover: Some(cover),
            },
        ),
        ("pomo_focus", pomo("focus")),
        ("pomo_break", pomo("break")),
        ("pomo_long_break", pomo("long_break")),
        ("pomo_done", pomo("done")),
    ]
}

fn image_of(f: &Frame) -> RgbImage {
    RgbImage::from_raw(SIZE, SIZE, f.rgb.clone()).expect("frame size")
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "gallery".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).expect("create out dir");
    let chars = characters();
    for (name, scene) in scenes() {
        let mut sheet = RgbImage::new(SIZE * 3 * chars.len() as u32, SIZE * 3);
        for (i, (ch, ch_name)) in chars.iter().enumerate() {
            let frames = render(&scene, ch, false);
            let base = format!("{name}_{}", ch_name.to_lowercase());
            std::fs::write(dir.join(format!("{base}.png")), to_png(&frames[0])).expect("write");
            let img = image_of(&frames[0]);
            let big = imageops::resize(&img, SIZE * 4, SIZE * 4, imageops::FilterType::Nearest);
            big.save_with_format(dir.join(format!("{base}@4x.png")), ImageFormat::Png)
                .expect("write 4x");
            let mid = imageops::resize(&img, SIZE * 3, SIZE * 3, imageops::FilterType::Nearest);
            imageops::replace(&mut sheet, &mid, (i as u32 * SIZE * 3) as i64, 0);
        }
        sheet
            .save_with_format(dir.join(format!("sheet_{name}.png")), ImageFormat::Png)
            .expect("write sheet");

        let anim = render(&scene, &chars[0].0, true);
        let mut strip = RgbImage::new(SIZE * 2 * anim.len() as u32, SIZE * 2);
        for (i, f) in anim.iter().enumerate() {
            let img = imageops::resize(
                &image_of(f),
                SIZE * 2,
                SIZE * 2,
                imageops::FilterType::Nearest,
            );
            imageops::replace(&mut strip, &img, (i as u32 * SIZE * 2) as i64, 0);
        }
        strip
            .save_with_format(dir.join(format!("anim_{name}.png")), ImageFormat::Png)
            .expect("write strip");
    }
    println!("wrote gallery to {}", dir.display());
}
