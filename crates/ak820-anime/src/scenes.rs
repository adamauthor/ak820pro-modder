//! Scene compositions: backgrounds, icons, particles, text layout.

use crate::canvas::{Canvas, Rgb, Rng, WHITE};
use crate::character::{self, Expression, Pose, Prop};
use crate::text::{self, Align, FONT_10X20, FONT_6X10, FONT_9X15_BOLD};
use crate::{Character, Weather};
use std::f32::consts::TAU;

/// Per-frame animation context.
#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub frame: u32,
    pub frames: u32,
    pub blink: bool,
}

impl Ctx {
    /// 0..1 loop phase of this frame.
    fn phase(&self) -> f32 {
        self.frame as f32 / self.frames.max(1) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tod {
    Morning,
    Day,
    Evening,
    Night,
}

pub fn tod(hour: u8) -> Tod {
    match hour % 24 {
        5..=10 => Tod::Morning,
        11..=16 => Tod::Day,
        17..=20 => Tod::Evening,
        _ => Tod::Night,
    }
}

const INK: Rgb = Rgb::hex(0x1E1430);

// ---------------------------------------------------------------------------
// Backgrounds & decorations
// ---------------------------------------------------------------------------

fn sky(c: &mut Canvas, t: Tod) {
    match t {
        Tod::Morning => c.vgradient(&[
            (0.0, Rgb::hex(0xFFB8D6)),
            (0.55, Rgb::hex(0xFFDCC8)),
            (1.0, Rgb::hex(0xFFF1CF)),
        ]),
        Tod::Day => c.vgradient(&[
            (0.0, Rgb::hex(0x4FA8F0)),
            (0.7, Rgb::hex(0x9CD6FF)),
            (1.0, Rgb::hex(0xD2EEFF)),
        ]),
        Tod::Evening => c.vgradient(&[
            (0.0, Rgb::hex(0x4E3486)),
            (0.45, Rgb::hex(0xD9629A)),
            (0.8, Rgb::hex(0xFF9E6A)),
            (1.0, Rgb::hex(0xFFCB85)),
        ]),
        Tod::Night => c.vgradient(&[
            (0.0, Rgb::hex(0x0C1236)),
            (0.7, Rgb::hex(0x222E5C)),
            (1.0, Rgb::hex(0x3A3D76)),
        ]),
    }
}

fn sun(c: &mut Canvas, x: f32, y: f32, r: f32, color: Rgb) {
    c.glow(x, y, r * 2.6, color.lighten(0.3), 0.55);
    for i in 0..8 {
        let a = i as f32 * TAU / 8.0;
        let (x0, y0) = (x + a.cos() * (r + 2.0), y + a.sin() * (r + 2.0));
        let (x1, y1) = (x + a.cos() * (r + 5.0), y + a.sin() * (r + 5.0));
        c.line(x0 as i32, y0 as i32, x1 as i32, y1 as i32, color);
    }
    c.ellipse_outlined(x, y, r, r, color, color.darken(0.2));
    c.ellipse(
        x - r * 0.3,
        y - r * 0.3,
        r * 0.45,
        r * 0.45,
        color.lighten(0.5),
    );
}

fn moon(c: &mut Canvas, x: f32, y: f32, r: f32) {
    c.glow(x, y, r * 2.8, Rgb::hex(0xB8C4FF), 0.35);
    let mut m = c.mask();
    m.ellipse(x, y, r, r);
    m.erase_ellipse(x + r * 0.55, y - r * 0.35, r * 0.85, r * 0.85);
    c.paint(&m, |_, _| Rgb::hex(0xFFF3B8), Some(Rgb::hex(0xD9C46A)));
}

fn cloud(c: &mut Canvas, x: f32, y: f32, w: f32, fill: Rgb, line: Rgb) {
    let mut m = c.mask();
    m.ellipse(x, y, w * 0.5, w * 0.16);
    m.ellipse(x - w * 0.2, y - w * 0.1, w * 0.22, w * 0.18);
    m.ellipse(x + w * 0.1, y - w * 0.16, w * 0.26, w * 0.24);
    m.rect(x - w * 0.5, y - 1.0, x + w * 0.5, y + w * 0.12);
    let shade = fill.lerp(line, 0.25);
    let bottom = y + w * 0.04;
    c.paint(
        &m,
        |_, py| if py as f32 > bottom { shade } else { fill },
        Some(line),
    );
}

/// Seamlessly looping vertical fall: returns y for a particle.
fn fall_y(base: i32, ctx: &Ctx, min_span: i32) -> i32 {
    let n = ctx.frames.max(1) as i32;
    let step = (min_span + n - 1) / n;
    let span = step * n;
    (base + ctx.frame as i32 * step).rem_euclid(span) - (span - 128) / 2 - 4
}

fn petals(c: &mut Canvas, ctx: &Ctx, count: usize, seed: u64) {
    let mut rng = Rng::new(seed);
    let a = Rgb::hex(0xFFB3CF);
    let b = Rgb::hex(0xF57AA8);
    for i in 0..count {
        let bx = rng.range(0, 128);
        let by = rng.range(0, 140);
        let y = fall_y(by, ctx, 140);
        let sway = ((ctx.phase() + i as f32 * 0.37) * TAU).sin() * 3.0;
        let x = bx + sway.round() as i32 - ctx.frame as i32 * 2;
        let x = x.rem_euclid(132) - 2;
        let rows: &[&str] = if (i as u32 + ctx.frame) % 2 == 0 {
            &[".ab", "aab", "bb."]
        } else {
            &["ba.", "baa", ".bb"]
        };
        c.sprite(x, y, rows, &[('a', a), ('b', b)]);
    }
}

/// 4-point twinkle. `size` 0..=2.
fn sparkle(c: &mut Canvas, x: i32, y: i32, size: i32, color: Rgb) {
    c.set(x, y, WHITE);
    for d in 1..=size + 1 {
        let col = if d == 1 { color.lighten(0.5) } else { color };
        c.set(x + d, y, col);
        c.set(x - d, y, col);
        c.set(x, y + d, col);
        c.set(x, y - d, col);
    }
    if size >= 2 {
        for (dx, dy) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
            c.blend(x + dx, y + dy, color, 0.6);
        }
    }
}

fn stars(c: &mut Canvas, ctx: &Ctx, seed: u64, max_y: i32) {
    let mut rng = Rng::new(seed);
    for i in 0..34 {
        let x = rng.range(0, 128);
        let y = rng.range(0, max_y);
        let tw = (i as u32 + ctx.frame) % 4;
        if i % 6 == 0 {
            sparkle(c, x, y, if tw == 0 { 1 } else { 0 }, Rgb::hex(0xFFF2A8));
        } else {
            let a = [1.0, 0.75, 0.45, 0.75][tw as usize];
            c.blend(x, y, Rgb::hex(0xFFFFFF), a);
        }
    }
}

fn twinkles(c: &mut Canvas, ctx: &Ctx, seed: u64, n: usize, color: Rgb) {
    let mut rng = Rng::new(seed);
    for i in 0..n {
        // Keep sparkles in open sky: off the character and the text.
        let free = |x: i32, y: i32| {
            if y < 22 {
                false
            } else if x >= 56 {
                y < 40
            } else {
                !(46..=110).contains(&y)
            }
        };
        let (mut x, mut y) = (rng.range(4, 124), rng.range(4, 124));
        for _ in 0..24 {
            if free(x, y) {
                break;
            }
            x = rng.range(4, 124);
            y = rng.range(4, 124);
        }
        let s = ((ctx.frame as usize + i) % 4) as i32;
        let size = [0, 1, 2, 1][s as usize];
        sparkle(c, x, y, size, color);
    }
}

fn polka(c: &mut Canvas, color: Rgb, a: f32) {
    for row in 0..9 {
        for col in 0..9 {
            let x = col * 16 + if row % 2 == 0 { 0 } else { 8 };
            let y = row * 16;
            c.ellipse_blend(x as f32 + 0.5, y as f32 + 0.5, 2.5, 2.5, color, a);
        }
    }
}

// ---------------------------------------------------------------------------
// Daily
// ---------------------------------------------------------------------------

const WEEKDAYS: [&str; 7] = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

pub fn daily(c: &mut Canvas, ch: &Character, ctx: &Ctx, d: (i32, u8, u8, u8, u8)) {
    let (year, month, day, weekday, hour) = d;
    let t = tod(hour);
    sky(c, t);
    match t {
        Tod::Morning => {
            sun(c, 24.0, 38.0, 8.0, Rgb::hex(0xFFC75A));
            cloud(c, 110.0, 34.0, 26.0, WHITE, Rgb::hex(0xF2A9C4));
        }
        Tod::Day => {
            sun(c, 24.0, 38.0, 8.0, Rgb::hex(0xFFD84A));
            cloud(c, 108.0, 30.0, 30.0, WHITE, Rgb::hex(0x8DBDE6));
        }
        Tod::Evening => {
            c.glow(64.0, 128.0, 70.0, Rgb::hex(0xFFE08A), 0.5);
            sun(c, 24.0, 38.0, 8.0, Rgb::hex(0xFF8F5A));
            cloud(c, 110.0, 30.0, 24.0, Rgb::hex(0xF7A1C0), Rgb::hex(0x9C4A86));
        }
        Tod::Night => {
            stars(c, ctx, 7, 110);
            moon(c, 24.0, 38.0, 9.0);
        }
    }

    let expr = match t {
        Tod::Morning => Expression::Happy,
        Tod::Day => Expression::Excited,
        Tod::Evening => Expression::Normal,
        Tod::Night => Expression::Sleepy,
    };
    character::draw(
        c,
        ch,
        &Pose {
            x: 88,
            y: 74,
            expr,
            blink: ctx.blink,
            prop: Prop::None,
            frame: ctx.frame,
        },
    );

    match t {
        Tod::Morning | Tod::Day => petals(c, ctx, 9, 11),
        Tod::Evening => twinkles(c, ctx, 5, 6, Rgb::hex(0xFFE7A0)),
        Tod::Night => {
            // Sleepy "z" floating up.
            let y = 50 - (ctx.frame as i32 % 4) * 3;
            text::draw(c, "z", 112, y, &FONT_6X10, WHITE, INK, Align::Left);
            text::draw(c, "Z", 118, y - 9, &FONT_6X10, WHITE, INK, Align::Left);
        }
    }

    let greet = match t {
        Tod::Morning => "Ohayo!",
        Tod::Day => "Konnichiwa!",
        Tod::Evening => "Konbanwa~",
        Tod::Night => "Oyasumi~",
    };
    let outline = match t {
        Tod::Morning => Rgb::hex(0x9C3A6A),
        Tod::Day => Rgb::hex(0x1F4F8F),
        Tod::Evening => Rgb::hex(0x3E1F5C),
        Tod::Night => Rgb::hex(0x0A0E28),
    };
    text::draw(
        c,
        greet,
        64,
        3,
        &FONT_9X15_BOLD,
        WHITE,
        outline,
        Align::Center,
    );

    let wd = WEEKDAYS[(weekday % 7) as usize];
    let wd_col = match weekday % 7 {
        5 => Rgb::hex(0x9CCBFF),
        6 => Rgb::hex(0xFF9AAE),
        _ => Rgb::hex(0xFFF4B0),
    };
    text::draw(c, wd, 6, 54, &FONT_10X20, wd_col, outline, Align::Left);
    let date = format!("{:02}.{:02}", day.min(99), month.min(99));
    text::draw(
        c,
        &date,
        5,
        77,
        &FONT_9X15_BOLD,
        WHITE,
        outline,
        Align::Left,
    );
    let yr = format!("{}", year.clamp(-999, 9999));
    text::draw(
        c,
        &yr,
        8,
        95,
        &FONT_6X10,
        WHITE.lerp(wd_col, 0.5),
        outline,
        Align::Left,
    );
}

// ---------------------------------------------------------------------------
// Weather
// ---------------------------------------------------------------------------

fn weather_bg(c: &mut Canvas, w: &Weather, hour: u8) {
    let night = tod(hour) == Tod::Night;
    match w {
        Weather::Clear | Weather::Cloudy => {
            if night {
                sky(c, Tod::Night)
            } else if *w == Weather::Cloudy {
                c.vgradient(&[(0.0, Rgb::hex(0x7FA2C8)), (1.0, Rgb::hex(0xC9D8E8))])
            } else {
                sky(c, tod(hour))
            }
        }
        Weather::Rain => c.vgradient(&[(0.0, Rgb::hex(0x4A5A80)), (1.0, Rgb::hex(0x8A9CC0))]),
        Weather::Storm => c.vgradient(&[(0.0, Rgb::hex(0x231C42)), (1.0, Rgb::hex(0x5A4A82))]),
        Weather::Snow => c.vgradient(&[(0.0, Rgb::hex(0x9FB8E0)), (1.0, Rgb::hex(0xE8F0FF))]),
        Weather::Fog => c.vgradient(&[(0.0, Rgb::hex(0xA8A8BC)), (1.0, Rgb::hex(0xDCDCE6))]),
    }
}

fn weather_icon(c: &mut Canvas, w: &Weather, night: bool, x: f32, y: f32) {
    let grey = Rgb::hex(0x9AA4BC);
    let dark = Rgb::hex(0x5E6684);
    match w {
        Weather::Clear => {
            if night {
                moon(c, x, y, 10.0)
            } else {
                sun(c, x, y, 9.0, Rgb::hex(0xFFD23F))
            }
        }
        Weather::Cloudy => {
            if night {
                moon(c, x - 6.0, y - 5.0, 7.0)
            } else {
                sun(c, x - 6.0, y - 6.0, 6.0, Rgb::hex(0xFFD23F))
            }
            cloud(c, x + 2.0, y + 4.0, 30.0, WHITE, Rgb::hex(0x7C8AAE));
        }
        Weather::Rain => {
            cloud(c, x, y - 2.0, 32.0, Rgb::hex(0xDDE3F0), dark);
            for (dx, dy) in [(-9, 10), (0, 12), (9, 10)] {
                raindrop(c, x as i32 + dx, y as i32 + dy, Rgb::hex(0x6FB6FF));
            }
        }
        Weather::Storm => {
            cloud(c, x, y - 2.0, 32.0, grey, Rgb::hex(0x2E2A48));
            bolt(c, x as i32 - 2, y as i32 + 2);
        }
        Weather::Snow => {
            cloud(c, x, y - 2.0, 32.0, WHITE, Rgb::hex(0x7C8AAE));
            for (dx, dy) in [(-8, 13), (7, 15)] {
                flake(c, x as i32 + dx, y as i32 + dy);
            }
        }
        Weather::Fog => {
            for (i, dy) in [-8, -2, 4, 10].iter().enumerate() {
                let off = if i % 2 == 0 { -3 } else { 3 };
                c.round_rect_blend(x as i32 - 14 + off, y as i32 + dy, 26, 3, 1, WHITE, 0.95);
            }
        }
    }
}

fn raindrop(c: &mut Canvas, x: i32, y: i32, col: Rgb) {
    c.sprite(
        x - 1,
        y - 2,
        &[".o.", "oao", "aaa", ".a."],
        &[('o', col.darken(0.2)), ('a', col)],
    );
}

fn flake(c: &mut Canvas, x: i32, y: i32) {
    c.sprite(
        x - 3,
        y - 3,
        &[
            "...b...", ".b.b.b.", "..bbb..", "bbbwbbb", "..bbb..", ".b.b.b.", "...b...",
        ],
        &[('w', WHITE), ('b', Rgb::hex(0x5B95E6))],
    );
}

fn bolt(c: &mut Canvas, x: i32, y: i32) {
    let (x, y) = (x as f32, y as f32);
    let mut m = c.mask();
    m.poly(&[
        (x + 3.0, y),
        (x - 3.0, y + 9.0),
        (x + 1.0, y + 9.0),
        (x - 2.0, y + 18.0),
        (x + 7.0, y + 6.0),
        (x + 3.0, y + 6.0),
        (x + 7.0, y),
    ]);
    c.paint(&m, |_, _| Rgb::hex(0xFFE14A), Some(Rgb::hex(0xB07A10)));
}

pub fn weather(
    c: &mut Canvas,
    ch: &Character,
    ctx: &Ctx,
    temp_c: f32,
    w: &Weather,
    city: &str,
    hour: u8,
) {
    let night = tod(hour) == Tod::Night;
    weather_bg(c, w, hour);
    if night && matches!(w, Weather::Clear | Weather::Cloudy) {
        stars(c, ctx, 3, 90);
    }
    if *w == Weather::Storm && ctx.frame % 3 == 1 {
        c.rect_blend(0, 0, 128, 128, WHITE, 0.25);
    }

    // Precipitation behind the character (she's under the umbrella).
    let mut rng = Rng::new(99);
    match w {
        Weather::Rain | Weather::Storm => {
            for _ in 0..44 {
                let x = rng.range(0, 140);
                let y = fall_y(rng.range(0, 150), ctx, 150);
                let col = Rgb::hex(0xD6E6FF);
                for k in 0..5 {
                    c.blend(x - k / 2 - (y + k) / 6, y + k, col, 0.65);
                }
            }
        }
        Weather::Snow => {
            for i in 0..46 {
                let x0 = rng.range(0, 128);
                let y = fall_y(rng.range(0, 140), ctx, 140);
                let sway = ((ctx.phase() + i as f32 * 0.21) * TAU).sin() * 2.0;
                let x = x0 + sway.round() as i32;
                c.set(x, y, WHITE);
                if i % 2 == 0 {
                    c.set(x + 1, y, WHITE);
                    c.set(x, y + 1, WHITE);
                    c.set(x + 1, y + 1, Rgb::hex(0xE0ECFF));
                }
            }
        }
        Weather::Fog => {}
        Weather::Clear | Weather::Cloudy => {
            if !night && *w == Weather::Cloudy {
                let dx = (ctx.phase() * 16.0) as i32;
                cloud(c, (112 - dx) as f32, 44.0, 30.0, WHITE, Rgb::hex(0x7C8AAE));
            }
        }
    }

    let (prop, expr) = match w {
        Weather::Rain | Weather::Storm => (Prop::Umbrella, Expression::Normal),
        Weather::Snow => (Prop::Scarf, Expression::Happy),
        Weather::Clear if !night => (Prop::Sunglasses, Expression::Excited),
        Weather::Clear => (Prop::None, Expression::Normal),
        Weather::Fog => (Prop::None, Expression::Sleepy),
        Weather::Cloudy => (Prop::None, Expression::Normal),
    };
    let prop = if prop == Prop::None && temp_c < 0.0 {
        Prop::Scarf
    } else {
        prop
    };
    character::draw(
        c,
        ch,
        &Pose {
            x: 90,
            y: 76,
            expr,
            blink: ctx.blink,
            prop,
            frame: ctx.frame,
        },
    );

    if *w == Weather::Fog {
        // Soft drifting fog banks: vertical bell profile × horizontal wave.
        for i in 0..3 {
            let yc = 40 + i * 30;
            let off = ((ctx.phase() * 128.0) as i32 * if i % 2 == 0 { 1 } else { -1 } + i * 37)
                .rem_euclid(128);
            for dy in -7..=7 {
                let v = 1.0 - (dy as f32 / 8.0).powi(2);
                for x in 0..128 {
                    let xx = (x + off) % 128;
                    let h = 0.5 + 0.5 * ((xx as f32 / 128.0) * TAU * 2.0).sin();
                    c.blend(x, yc + dy, WHITE, 0.32 * v * (0.4 + 0.6 * h));
                }
            }
        }
    }
    if *w == Weather::Clear && !night {
        twinkles(c, ctx, 21, 4, Rgb::hex(0xFFF2A0));
    }
    if night && *w == Weather::Clear {
        twinkles(c, ctx, 23, 3, Rgb::hex(0xFFF2A0));
    }

    // Text column.
    let outline = INK;
    let city = text::fit(city, &FONT_6X10, 122);
    text::draw(c, &city, 64, 2, &FONT_6X10, WHITE, outline, Align::Center);
    weather_icon(c, w, night, 22.0, 30.0);

    let t = if temp_c.is_finite() {
        temp_c.round().clamp(-99.0, 999.0) as i32
    } else {
        0
    };
    let num = format!("{t}");
    let tcol = if t <= 0 {
        Rgb::hex(0xB8E0FF)
    } else if t >= 28 {
        Rgb::hex(0xFFC27A)
    } else {
        WHITE
    };
    let x = 4;
    let w_num = text::draw(c, &num, x, 50, &FONT_10X20, tcol, outline, Align::Left);
    text::degree(c, x + w_num + 2, 52, tcol, outline);
    text::draw(
        c,
        "C",
        x + w_num + 9,
        50,
        &FONT_10X20,
        tcol,
        outline,
        Align::Left,
    );
    let label = match w {
        Weather::Clear => {
            if night {
                "Clear"
            } else {
                "Sunny!"
            }
        }
        Weather::Cloudy => "Cloudy",
        Weather::Rain => "Rainy",
        Weather::Storm => "Storm!",
        Weather::Snow => "Snowy~",
        Weather::Fog => "Foggy",
    };
    text::draw(c, label, 5, 74, &FONT_6X10, WHITE, outline, Align::Left);
}

// ---------------------------------------------------------------------------
// Now playing
// ---------------------------------------------------------------------------

fn vinyl(c: &mut Canvas, x: f32, y: f32, r: f32, ctx: &Ctx) {
    c.ellipse_outlined(x, y, r, r, Rgb::hex(0x1B1822), Rgb::hex(0x0A090E));
    for k in 1..5 {
        let rr = r * (0.42 + k as f32 * 0.13);
        let mut m = c.mask();
        m.ellipse(x, y, rr, rr);
        m.erase_ellipse(x, y, rr - 1.0, rr - 1.0);
        c.paint(&m, |_, _| Rgb::hex(0x2C2836), None);
    }
    // Rotating shine.
    let a0 = ctx.phase() * TAU * 0.5 - 2.4;
    for (k, rr) in [0.88f32, 0.8, 0.7, 0.6].iter().enumerate() {
        let span = 0.7 - k as f32 * 0.1;
        let steps = 40;
        for i in 0..=steps {
            let a = a0 + span * (i as f32 / steps as f32 - 0.5);
            let (px, py) = (x + a.cos() * r * rr, y + a.sin() * r * rr);
            c.set(px as i32, py as i32, Rgb::hex(0x5A5470));
            let (qx, qy) = (x - a.cos() * r * rr, y - a.sin() * r * rr);
            c.set(qx as i32, qy as i32, Rgb::hex(0x3E3A50));
        }
    }
    c.ellipse_outlined(
        x,
        y,
        r * 0.33,
        r * 0.33,
        Rgb::hex(0xFF7EB3),
        Rgb::hex(0xB8407A),
    );
    let la = ctx.phase() * TAU;
    c.ellipse(
        x + la.cos() * r * 0.2,
        y + la.sin() * r * 0.2,
        1.5,
        1.5,
        WHITE,
    );
    c.ellipse(x, y, 1.8, 1.8, Rgb::hex(0x1B1822));
}

fn note(c: &mut Canvas, x: i32, y: i32, double: bool, col: Rgb) {
    let rows: &[&str] = if double {
        &[
            "..ooooo", "..o...o", "..o...o", "..o...o", "ooo.ooo", "ooo.ooo",
        ]
    } else {
        &["..oo.", "..o.o", "..o..", "..o..", "ooo..", "ooo.."]
    };
    // Outline by stamping dark first.
    for (dy, row) in rows.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            if ch == 'o' {
                for (ox, oy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    c.set(x + dx as i32 + ox, y + dy as i32 + oy, INK);
                }
            }
        }
    }
    c.sprite(x, y, rows, &[('o', col)]);
}

pub fn now_playing(
    c: &mut Canvas,
    ch: &Character,
    ctx: &Ctx,
    title: &str,
    artist: &str,
    cover: Option<&[u8]>,
) {
    c.vgradient(&[
        (0.0, Rgb::hex(0x2A1B52)),
        (0.6, Rgb::hex(0x5E2F86)),
        (1.0, Rgb::hex(0xA04C9C)),
    ]);
    polka(c, Rgb::hex(0xFFFFFF), 0.06);
    twinkles(c, ctx, 31, 5, Rgb::hex(0xFFD6F0));

    // Character peeking from the right, bobbing to the beat.
    let bob = if ctx.frames > 1 && ctx.frame % 2 == 1 {
        1
    } else {
        0
    };
    character::draw(
        c,
        ch,
        &Pose {
            x: 108,
            y: 62 + bob,
            expr: Expression::Happy,
            blink: false,
            prop: Prop::Headphones,
            frame: ctx.frame,
        },
    );

    // Framed cover.
    let (fx, fy, fs) = (6, 6, 78);
    c.rect_blend(fx + 3, fy + 3, fs, fs, Rgb::hex(0x000000), 0.35);
    c.rect(fx, fy, fs, fs, WHITE);
    c.rect(fx + 1, fy + 1, fs - 2, fs - 2, Rgb::hex(0xFFD6EA));
    match cover {
        Some(px) => c.blit_rgb(px, 128, 128, fx + 3, fy + 3, fs - 6, fs - 6),
        None => {
            c.rect(fx + 3, fy + 3, fs - 6, fs - 6, Rgb::hex(0x3A2A5C));
            vinyl(c, (fx + fs / 2) as f32, (fy + fs / 2) as f32, 33.0, ctx);
        }
    }
    // Corner sparkles on the frame.
    sparkle(c, fx + fs - 2, fy + 1, 2, Rgb::hex(0xFFE27A));
    sparkle(c, fx + 1, fy + fs - 3, 1, Rgb::hex(0xFFB3E0));

    // Bouncing notes.
    let notes = [
        (96, 10, false, 0.0),
        (114, 22, true, 0.33),
        (88, 30, false, 0.66),
    ];
    for (x, y, dbl, off) in notes {
        let b = ((ctx.phase() + off) * TAU).sin() * 3.0;
        let col = if dbl {
            Rgb::hex(0xFFE27A)
        } else {
            Rgb::hex(0xFFB3E0)
        };
        note(c, x, y + b.round() as i32, dbl, col);
    }

    // Translucent band with title / artist.
    c.rect_blend(0, 94, 128, 34, Rgb::hex(0x140C24), 0.62);
    c.rect(0, 94, 128, 1, Rgb::hex(0xFF7EB3));
    note(c, 4, 98, false, Rgb::hex(0xFF9ED0));
    let t = text::fit(title, &FONT_6X10, 114);
    let a = text::fit(artist, &FONT_6X10, 120);
    let t = if t.is_empty() {
        "Unknown".to_string()
    } else {
        t
    };
    text::draw(c, &t, 12, 97, &FONT_6X10, WHITE, INK, Align::Left);
    text::draw(
        c,
        &a,
        4,
        112,
        &FONT_6X10,
        Rgb::hex(0xFFB3D9),
        INK,
        Align::Left,
    );
}

// ---------------------------------------------------------------------------
// Pomodoro
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Focus,
    Break,
    LongBreak,
    Done,
}

fn tomato(c: &mut Canvas, x: f32, y: f32, r: f32, face: bool) {
    c.ellipse_outlined(x, y, r, r * 0.9, Rgb::hex(0xF0443A), Rgb::hex(0x8A1A1A));
    c.ellipse(
        x - r * 0.4,
        y - r * 0.35,
        r * 0.28,
        r * 0.2,
        Rgb::hex(0xFFA89A),
    );
    let leaf = Rgb::hex(0x4CB050);
    let mut m = c.mask();
    let ly = y - r * 0.85;
    for i in 0..5 {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * TAU / 5.0;
        m.ellipse(
            x + a.cos() * r * 0.3,
            ly + a.sin() * r * 0.18,
            r * 0.22,
            r * 0.14,
        );
    }
    m.rect(x - 0.5, ly - r * 0.45, x + 1.5, ly);
    c.paint(&m, |_, _| leaf, Some(Rgb::hex(0x22662A)));
    if face && r >= 8.0 {
        let (xi, yi) = (x as i32, y as i32);
        c.rect(xi - 4, yi, 2, 2, INK);
        c.rect(xi + 2, yi, 2, 2, INK);
        c.set(xi - 4, yi, WHITE);
        c.set(xi + 2, yi, WHITE);
        for (dx, dy) in [(-1, 3), (0, 4), (1, 3)] {
            c.set(xi + dx, yi + dy, INK);
        }
        c.blend(xi - 6, yi + 3, Rgb::hex(0xFF9AB0), 0.8);
        c.blend(xi + 5, yi + 3, Rgb::hex(0xFF9AB0), 0.8);
    }
}

pub fn pomodoro(c: &mut Canvas, ch: &Character, ctx: &Ctx, phase: &str, minutes: u32, round: u32) {
    let ph = match phase.trim().to_ascii_lowercase().as_str() {
        "break" | "short_break" => Phase::Break,
        "long_break" | "longbreak" => Phase::LongBreak,
        "done" | "finished" => Phase::Done,
        _ => Phase::Focus,
    };
    let (top, bottom, outline) = match ph {
        Phase::Focus => (0xFF8A72, 0xD5303A, 0x5A0E18),
        Phase::Break => (0xFFC7B8, 0xF07A78, 0x7A2232),
        Phase::LongBreak => (0xFFE0C4, 0xF29070, 0x7A3222),
        Phase::Done => (0xFFE27A, 0xFF6B5B, 0x7A2A10),
    };
    let outline = Rgb::hex(outline);
    c.vgradient(&[(0.0, Rgb::hex(top)), (1.0, Rgb::hex(bottom))]);
    polka(c, WHITE, 0.12);

    let (expr, prop) = match ph {
        Phase::Focus => (Expression::Determined, Prop::Headband),
        Phase::Break | Phase::LongBreak => (Expression::Sleepy, Prop::TeaCup),
        Phase::Done => (Expression::Happy, Prop::None),
    };
    if ph == Phase::Done {
        // Celebration burst behind her.
        for i in 0..12 {
            let a = i as f32 * TAU / 12.0 + ctx.phase() * 0.5;
            let (x0, y0) = (88.0 + a.cos() * 34.0, 66.0 + a.sin() * 34.0);
            let (x1, y1) = (88.0 + a.cos() * 60.0, 66.0 + a.sin() * 60.0);
            c.line_blend(x0 as i32, y0 as i32, x1 as i32, y1 as i32, WHITE, 0.45);
        }
    }
    character::draw(
        c,
        ch,
        &Pose {
            x: 88,
            y: 74,
            expr,
            blink: ctx.blink && ph == Phase::Focus,
            prop,
            frame: ctx.frame,
        },
    );

    match ph {
        Phase::Done => {
            let mut rng = Rng::new(5);
            let cols = [0xFFE27A, 0x7CC4FF, 0xFF7EB3, 0x8BF0A0, 0xFFFFFF];
            for i in 0..28 {
                let x = rng.range(0, 128);
                let y = fall_y(rng.range(0, 140), ctx, 140);
                let col = Rgb::hex(cols[i % cols.len()]);
                if (66..112).contains(&x) && (48..98).contains(&y) {
                    continue; // keep her face clear
                }
                if (i as u32 + ctx.frame) % 2 == 0 {
                    c.rect(x, y, 2, 3, col);
                } else {
                    c.rect(x, y, 3, 2, col);
                }
            }
            text::draw(
                c,
                "Yay~!",
                110,
                26,
                &FONT_6X10,
                WHITE,
                outline,
                Align::Center,
            );
        }
        Phase::Break | Phase::LongBreak => {
            let k = ctx.frame as i32 % 4;
            text::draw(
                c,
                "z",
                110,
                48 - k * 2,
                &FONT_6X10,
                WHITE,
                outline,
                Align::Left,
            );
            text::draw(
                c,
                "Z",
                116,
                38 - k * 2,
                &FONT_6X10,
                WHITE,
                outline,
                Align::Left,
            );
        }
        Phase::Focus => twinkles(c, ctx, 13, 4, Rgb::hex(0xFFE27A)),
    }

    let label = match ph {
        Phase::Focus => "Focus!",
        Phase::Break => "Break~",
        Phase::LongBreak => "Long Break",
        Phase::Done => "Done!",
    };
    text::draw(
        c,
        label,
        64,
        3,
        &FONT_9X15_BOLD,
        WHITE,
        outline,
        Align::Center,
    );
    tomato(c, 24.0, 38.0, 12.0, true);
    let time = format!("{:02}:00", minutes.min(999));
    let font = if time.len() > 5 {
        &FONT_9X15_BOLD
    } else {
        &FONT_10X20
    };
    text::draw(c, &time, 3, 56, font, WHITE, outline, Align::Left);
    let r = format!("Round {}", round.min(99));
    text::draw(
        c,
        &r,
        4,
        80,
        &FONT_6X10,
        Rgb::hex(0xFFF2C8),
        outline,
        Align::Left,
    );
    for i in 0..round.clamp(0, 4) {
        tomato(c, 8.0 + i as f32 * 11.0, 99.0, 4.0, false);
    }
}
