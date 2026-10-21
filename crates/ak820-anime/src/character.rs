//! Chibi anime girl bust, drawn procedurally around a head centre.
//!
//! Coordinates below are offsets (in pixels) from the head centre. The face is
//! ~38px wide, the whole head with hair ~50px, eyes 9×12. Layers are built as
//! masks first so outlines and cast shadows can reference each other.

use crate::canvas::{Canvas, Mask, Rgb, WHITE};
use crate::text::{self, FONT_5X8};
use crate::Character;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expression {
    Normal,
    Happy,
    Sleepy,
    Determined,
    Excited,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    None,
    Headphones,
    Umbrella,
    Scarf,
    Sunglasses,
    Headband,
    TeaCup,
}

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub x: i32,
    pub y: i32,
    pub expr: Expression,
    pub blink: bool,
    pub prop: Prop,
    pub frame: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Long,
    Bob,
    CatHime,
    TwinTails,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mouth {
    Smile,
    Cat,
    Grin,
}

struct Hair {
    base: Rgb,
    shade: Rgb,
    line: Rgb,
    hi: Rgb,
}

struct Look {
    hair: Hair,
    iris: [Rgb; 3],
    style: Style,
    mouth: Mouth,
    shirt: Rgb,
    collar: Rgb,
    stripe: Rgb,
    ribbon: Rgb,
    accent: Rgb,
}

const SKIN: Rgb = Rgb::hex(0xFFE7D9);
const SKIN_SHADE: Rgb = Rgb::hex(0xF6C2AE);
const SKIN_LINE: Rgb = Rgb::hex(0xC47B6E);
const LASH: Rgb = Rgb::hex(0x34182A);
const BLUSH: Rgb = Rgb::hex(0xFF6F91);
const MOUTH: Rgb = Rgb::hex(0x8E2F45);
const MOUTH_IN: Rgb = Rgb::hex(0xD9475F);
const TONGUE: Rgb = Rgb::hex(0xFF9BB0);

fn look(ch: &Character) -> Look {
    match ch {
        Character::Sakura => Look {
            hair: Hair {
                base: Rgb::hex(0xF79AC0),
                shade: Rgb::hex(0xDE6E9C),
                line: Rgb::hex(0x8F3463),
                hi: Rgb::hex(0xFFD9EA),
            },
            iris: [Rgb::hex(0x0E5A4E), Rgb::hex(0x22A383), Rgb::hex(0x8BF0CF)],
            style: Style::Long,
            mouth: Mouth::Smile,
            shirt: Rgb::hex(0xFAFAFF),
            collar: Rgb::hex(0x2E3F7A),
            stripe: WHITE,
            ribbon: Rgb::hex(0xE8384F),
            accent: Rgb::hex(0xFF7EB3),
        },
        Character::Yuki => Look {
            hair: Hair {
                base: Rgb::hex(0xCFE0F5),
                shade: Rgb::hex(0x94B0D8),
                line: Rgb::hex(0x4A6394),
                hi: Rgb::hex(0xFFFFFF),
            },
            iris: [Rgb::hex(0x173E8C), Rgb::hex(0x3B82DE), Rgb::hex(0xA6DBFF)],
            style: Style::Bob,
            mouth: Mouth::Smile,
            shirt: Rgb::hex(0xF4F8FF),
            collar: Rgb::hex(0x5B9BE0),
            stripe: WHITE,
            ribbon: Rgb::hex(0x24407E),
            accent: Rgb::hex(0x7CC4FF),
        },
        Character::Kuro => Look {
            hair: Hair {
                base: Rgb::hex(0x2E2C40),
                shade: Rgb::hex(0x1C1A2A),
                line: Rgb::hex(0x0C0A14),
                hi: Rgb::hex(0x6A6A92),
            },
            iris: [Rgb::hex(0x5C0612), Rgb::hex(0xD11F33), Rgb::hex(0xFF8A8A)],
            style: Style::CatHime,
            mouth: Mouth::Cat,
            shirt: Rgb::hex(0x3A3A4E),
            collar: Rgb::hex(0x1E1E2C),
            stripe: Rgb::hex(0xE8E8F0),
            ribbon: Rgb::hex(0xE02A3C),
            accent: Rgb::hex(0xE02A3C),
        },
        Character::Hikari => Look {
            hair: Hair {
                base: Rgb::hex(0xFFD86A),
                shade: Rgb::hex(0xE6A93A),
                line: Rgb::hex(0x94621A),
                hi: Rgb::hex(0xFFF6C8),
            },
            iris: [Rgb::hex(0x452283), Rgb::hex(0x7C52D6), Rgb::hex(0xCDB0FF)],
            style: Style::TwinTails,
            mouth: Mouth::Grin,
            shirt: Rgb::hex(0xFFFDF6),
            collar: Rgb::hex(0xE8504A),
            stripe: WHITE,
            ribbon: Rgb::hex(0xFFB830),
            accent: Rgb::hex(0xFF9A3C),
        },
    }
}

/// Helper converting head-relative offsets to canvas coordinates.
#[derive(Clone, Copy)]
struct Origin {
    cx: f32,
    cy: f32,
    hx: i32,
    hy: i32,
}

impl Origin {
    fn pts(&self, v: &[(f32, f32)]) -> Vec<(f32, f32)> {
        v.iter().map(|&(x, y)| (self.cx + x, self.cy + y)).collect()
    }
    fn mirror(&self, v: &[(f32, f32)]) -> Vec<(f32, f32)> {
        v.iter().map(|&(x, y)| (self.cx - x, self.cy + y)).collect()
    }
}

fn lerp_points(pts: &[(f32, f32)], x: f32) -> f32 {
    if x <= pts[0].0 {
        return pts[0].1;
    }
    for w in pts.windows(2) {
        if x <= w[1].0 {
            let t = (x - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + (w[1].1 - w[0].1) * t;
        }
    }
    pts[pts.len() - 1].1
}

/// Bottom edge of the bangs as (dx, dy) points, left to right.
fn bang_edge(style: Style) -> Vec<(f32, f32)> {
    match style {
        Style::CatHime => vec![
            (-26.0, 12.0),
            (-21.0, -3.0),
            (-16.0, -3.0),
            (-14.5, -7.0),
            (-13.0, -3.0),
            (-6.0, -3.0),
            (-4.5, -7.0),
            (-3.0, -3.0),
            (3.0, -3.0),
            (4.5, -7.0),
            (6.0, -3.0),
            (13.0, -3.0),
            (14.5, -7.0),
            (16.0, -3.0),
            (21.0, -3.0),
            (26.0, 12.0),
        ],
        _ => vec![
            (-26.0, 12.0),
            (-21.0, -6.0),
            (-18.0, 0.0),
            (-15.0, -10.0),
            (-11.0, -2.0),
            (-7.0, -11.0),
            (-3.5, -3.0),
            (0.0, -10.0),
            (3.5, -2.0),
            (7.5, -11.0),
            (11.5, -3.0),
            (15.0, -10.0),
            (18.0, 0.0),
            (21.0, -6.0),
            (26.0, 12.0),
        ],
    }
}

fn side_lock(style: Style) -> Vec<(f32, f32)> {
    // Left side lock (viewer's left); mirrored for the right.
    match style {
        Style::Long => vec![
            (-17.0, -6.0),
            (-23.5, -8.0),
            (-24.5, 6.0),
            (-24.0, 20.0),
            (-21.5, 34.0),
            (-19.0, 26.0),
            (-17.5, 14.0),
            (-17.0, 2.0),
        ],
        Style::Bob => vec![
            (-17.0, -6.0),
            (-23.5, -8.0),
            (-25.0, 6.0),
            (-23.5, 18.0),
            (-19.0, 23.0),
            (-16.0, 21.0),
            (-18.0, 16.0),
            (-17.0, 2.0),
        ],
        Style::CatHime => vec![
            (-17.0, -6.0),
            (-23.5, -8.0),
            (-24.0, 6.0),
            (-23.5, 22.0),
            (-17.0, 22.0),
            (-17.0, 2.0),
        ],
        Style::TwinTails => vec![
            (-17.0, -6.0),
            (-23.5, -8.0),
            (-24.0, 6.0),
            (-22.0, 18.0),
            (-19.0, 22.0),
            (-17.5, 12.0),
            (-17.0, 2.0),
        ],
    }
}

pub fn draw(c: &mut Canvas, ch: &Character, pose: &Pose) {
    let lk = look(ch);
    let o = Origin {
        cx: pose.x as f32 + 0.5,
        cy: pose.y as f32 + 0.5,
        hx: pose.x,
        hy: pose.y,
    };

    if pose.prop == Prop::Umbrella {
        umbrella_shaft(c, &o);
    }

    // ---- masks -----------------------------------------------------------
    let back = back_hair_mask(c, &o, lk.style);
    let front = front_hair_mask(c, &o, lk.style);
    let mut face = c.mask();
    {
        let (cx, cy) = (o.cx, o.cy);
        face.custom((cx - 21.0, cy - 20.0, cx + 21.0, cy + 22.0), |x, y| {
            let dx = (x - cx) / 19.0;
            let dy = y - cy - 1.0;
            let dy = if dy < 0.0 { dy / 19.0 } else { dy / 19.5 };
            // Slightly squarer cheeks than a plain ellipse.
            dx.abs().powf(2.3) + dy.abs().powf(2.0) <= 1.0
        });
    }

    // ---- back hair ---------------------------------------------------------
    let cy = o.cy;
    c.paint(
        &back,
        |_, y| {
            let t = ((y as f32 - cy) / 55.0).clamp(0.0, 1.0);
            lk.hair.shade.lerp(lk.hair.line, t * 0.35)
        },
        Some(lk.hair.line),
    );
    if lk.style == Style::CatHime {
        for side in [-1.0f32, 1.0] {
            let inner = [
                (side * 18.0, -21.0),
                (side * 15.5, -30.5),
                (side * 10.0, -24.5),
            ];
            let pts: Vec<_> = inner.iter().map(|&(x, y)| (o.cx + x, o.cy + y)).collect();
            c.poly(&pts, Rgb::hex(0xF7A3BC));
        }
    }
    if lk.style != Style::Bob {
        back_strands(c, &o, &back, &lk);
    }

    // ---- body ----------------------------------------------------------------
    draw_body(c, &o, &lk);
    if pose.prop == Prop::Scarf {
        scarf(c, &o);
    }

    // ---- face ----------------------------------------------------------------
    c.paint(
        &face,
        |x, y| {
            if front.get(x, y - 1) || front.get(x, y - 2) {
                SKIN_SHADE
            } else {
                SKIN
            }
        },
        Some(SKIN_LINE),
    );
    draw_face_features(c, &o, &lk, pose);

    // ---- front hair ----------------------------------------------------------
    paint_front_hair(c, &o, &front, &lk);
    hair_accessory(c, &o, &lk, ch);

    // ---- props on top -----------------------------------------------------
    match pose.prop {
        Prop::Headphones => headphones(c, &o),
        Prop::Headband => headband(c, &o, &front),
        Prop::Sunglasses => sunglasses(c, &o),
        Prop::TeaCup => tea_cup(c, &o, pose.frame),
        Prop::Umbrella => umbrella_canopy(c, &o, &lk),
        Prop::Scarf | Prop::None => {}
    }
    if pose.expr == Expression::Determined {
        brows(c, &o, &lk);
    }
}

fn back_hair_mask(c: &Canvas, o: &Origin, style: Style) -> Mask {
    let mut m = c.mask();
    m.ellipse(o.cx, o.cy - 5.0, 25.5, 23.5);
    match style {
        Style::Long => {
            m.poly(&o.pts(&[
                (-25.0, -6.0),
                (25.0, -6.0),
                (28.0, 16.0),
                (32.0, 40.0),
                (35.0, 56.0),
                (31.0, 52.0),
                (30.0, 62.0),
                (-30.0, 62.0),
                (-31.0, 52.0),
                (-35.0, 56.0),
                (-32.0, 40.0),
                (-28.0, 16.0),
            ]));
        }
        Style::Bob => {
            m.ellipse(o.cx, o.cy + 2.0, 26.0, 21.0);
        }
        Style::CatHime => {
            m.poly(&o.pts(&[
                (-25.0, -6.0),
                (25.0, -6.0),
                (27.0, 30.0),
                (28.0, 62.0),
                (-28.0, 62.0),
                (-27.0, 30.0),
            ]));
            for side in [-1.0f32, 1.0] {
                m.poly(&o.pts(&[
                    (side * 23.0, -16.0),
                    (side * 17.0, -34.0),
                    (side * 6.0, -24.0),
                ]));
            }
        }
        Style::TwinTails => {
            for pts in [o.pts(&TAIL), o.mirror(&TAIL)] {
                m.poly(&pts);
            }
        }
    }
    m
}

const TAIL: [(f32, f32); 9] = [
    (-20.0, -18.0),
    (-27.0, -22.0),
    (-35.0, -12.0),
    (-38.0, 6.0),
    (-37.0, 26.0),
    (-33.0, 44.0),
    (-30.0, 30.0),
    (-28.0, 8.0),
    (-23.0, -8.0),
];

/// Darker strand lines inside the back hair for texture.
fn back_strands(c: &mut Canvas, o: &Origin, back: &Mask, lk: &Look) {
    let lines: &[(i32, i32, i32, i32)] = match lk.style {
        Style::TwinTails => &[(-32, -8, -34, 20), (-29, 4, -31, 30)],
        _ => &[(-26, 14, -28, 44), (-22, 24, -23, 50)],
    };
    for &(x0, y0, x1, y1) in lines {
        for s in [-1, 1] {
            let (ax, bx) = (o.hx + s * x0, o.hx + s * x1);
            let (ay, by) = (o.hy + y0, o.hy + y1);
            // Only inside the hair, never over its outline.
            let mut tmp = Vec::new();
            line_points(ax, ay, bx, by, &mut tmp);
            for (x, y) in tmp {
                if back.get(x, y) && !back.is_edge(x, y) {
                    c.set(x, y, lk.hair.line.lerp(lk.hair.shade, 0.5));
                }
            }
        }
    }
}

fn line_points(x0: i32, y0: i32, x1: i32, y1: i32, out: &mut Vec<(i32, i32)>) {
    let (mut x, mut y) = (x0, y0);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        out.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn front_hair_mask(c: &Canvas, o: &Origin, style: Style) -> Mask {
    let mut m = c.mask();
    let edge = bang_edge(style);
    let (cx, cy) = (o.cx, o.cy);
    m.custom((cx - 26.0, cy - 30.0, cx + 26.0, cy + 14.0), |x, y| {
        let dx = x - cx;
        let dy = y - cy;
        let nx = dx / 23.5;
        let ny = (dy + 6.0) / 21.5;
        nx * nx + ny * ny <= 1.0 && dy < lerp_points(&edge, dx)
    });
    let lock = side_lock(style);
    m.poly(&o.pts(&lock));
    m.poly(&o.mirror(&lock));
    if style == Style::TwinTails {
        // Ahoge: a cheeky strand sticking up.
        m.poly(&o.pts(&[
            (-1.0, -26.0),
            (2.0, -33.0),
            (8.0, -36.0),
            (4.0, -31.0),
            (3.0, -26.0),
        ]));
    }
    m
}

fn paint_front_hair(c: &mut Canvas, o: &Origin, front: &Mask, lk: &Look) {
    let edge = bang_edge(lk.style);
    let (cx, cy) = (o.cx, o.cy);
    c.paint(
        front,
        |x, y| {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let nx = dx / 23.5;
            let ny = (dy + 6.0) / 21.5;
            let r = (nx * nx + ny * ny).sqrt();
            // Angel ring highlight with little teeth hanging down.
            let tooth = (x - o.hx).rem_euclid(5) < 2;
            let lo = if tooth { 0.56 } else { 0.62 };
            if dy < -9.0 && dx.abs() < 17.0 && r > lo && r < 0.71 {
                return if tooth || r > 0.66 {
                    lk.hair.hi
                } else {
                    lk.hair.hi.lerp(lk.hair.base, 0.4)
                };
            }
            let e = lerp_points(&edge, dx);
            if dy > e - 3.0 && dx.abs() < 20.0 {
                return lk.hair.base.lerp(lk.hair.shade, 0.55);
            }
            if dx.abs() > 20.0 {
                return lk.hair.base.lerp(lk.hair.shade, 0.35);
            }
            lk.hair.base
        },
        Some(lk.hair.line),
    );
    // Strand separation lines rising from each notch of the bangs.
    for w in edge.windows(3) {
        let (p, n) = (w[1], w[0]);
        let is_notch = p.1 < n.1 && p.1 < w[2].1 && p.0.abs() < 20.0;
        if is_notch {
            let x = o.hx + p.0.round() as i32;
            let y = o.hy + p.1.round() as i32;
            let lean = if p.0 < 0.0 { 1 } else { -1 };
            let mut pts = Vec::new();
            line_points(x, y - 1, x + lean, y - 5, &mut pts);
            for (px, py) in pts {
                if front.get(px, py) && !front.is_edge(px, py) {
                    c.set(px, py, lk.hair.shade);
                }
            }
        }
    }
}

fn draw_body(c: &mut Canvas, o: &Origin, lk: &Look) {
    let mut neck = c.mask();
    neck.rect(o.cx - 4.0, o.cy + 12.0, o.cx + 4.0, o.cy + 28.0);
    c.paint(&neck, |_, _| SKIN_SHADE, Some(SKIN_LINE));
    // Everything below the neck sits a little higher for a short chibi neck.
    let o = &Origin {
        cy: o.cy - 3.0,
        hy: o.hy - 3,
        ..*o
    };

    let mut body = c.mask();
    body.ellipse(o.cx, o.cy + 47.0, 31.0, 21.0);
    body.rect(o.cx - 31.0, o.cy + 47.0, o.cx + 31.0, o.cy + 90.0);
    let (cx, shirt) = (o.cx, lk.shirt);
    c.paint(
        &body,
        |x, _| {
            let d = (x as f32 + 0.5 - cx).abs();
            if d > 25.0 {
                shirt.darken(0.12)
            } else {
                shirt
            }
        },
        Some(shirt.darken(0.5)),
    );
    // Sleeve seams.
    for s in [-1, 1] {
        let x = o.hx + s * 22;
        c.line(x, o.hy + 34, x + s * 2, o.hy + 60, shirt.darken(0.3));
    }

    // Sailor collar.
    let mut col = c.mask();
    col.poly(&o.pts(&[
        (-22.0, 28.0),
        (-6.0, 24.0),
        (0.0, 35.0),
        (6.0, 24.0),
        (22.0, 28.0),
        (18.0, 37.0),
        (0.0, 48.0),
        (-18.0, 37.0),
    ]));
    col.intersect(&body);
    c.paint(&col, |_, _| lk.collar, Some(lk.collar.darken(0.45)));
    c.poly(
        &o.pts(&[(-6.0, 24.0), (6.0, 24.0), (0.0, 35.0)]),
        SKIN_SHADE,
    );
    for s in [-1, 1] {
        c.line(o.hx + s * 18, o.hy + 33, o.hx + s * 3, o.hy + 42, lk.stripe);
    }

    // Ribbon bow.
    let mut bow = c.mask();
    bow.poly(&o.pts(&[(0.0, 37.0), (-8.0, 33.0), (-8.0, 42.0)]));
    bow.poly(&o.pts(&[(0.0, 37.0), (8.0, 33.0), (8.0, 42.0)]));
    bow.poly(&o.pts(&[(-1.5, 38.0), (-5.0, 50.0), (-1.0, 48.0)]));
    bow.poly(&o.pts(&[(1.5, 38.0), (5.0, 50.0), (1.0, 48.0)]));
    c.paint(&bow, |_, _| lk.ribbon, Some(lk.ribbon.darken(0.45)));
    c.rect(o.hx - 1, o.hy + 36, 3, 3, lk.ribbon.darken(0.2));
    c.set(o.hx - 5, o.hy + 35, lk.ribbon.lighten(0.4));
    c.set(o.hx + 5, o.hy + 35, lk.ribbon.lighten(0.4));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EyeState {
    Open,
    Closed,
    Happy,
    Sleepy,
}

fn draw_face_features(c: &mut Canvas, o: &Origin, lk: &Look, pose: &Pose) {
    let state = if pose.blink {
        EyeState::Closed
    } else {
        match pose.expr {
            Expression::Happy => EyeState::Happy,
            Expression::Sleepy => EyeState::Sleepy,
            _ => EyeState::Open,
        }
    };
    let star = pose.expr == Expression::Excited;
    eye(c, o.hx - 13, o.hy - 4, false, lk, state, star);
    eye(c, o.hx + 5, o.hy - 4, true, lk, state, star);

    // Blush with little hatch marks.
    for s in [-1, 1] {
        let bx = o.hx + s * 13;
        let by = o.hy + 10;
        c.ellipse_blend(bx as f32 + 0.5, by as f32 + 0.5, 4.0, 2.0, BLUSH, 0.45);
        for i in -1..=1 {
            c.blend(bx + i * 2, by, BLUSH.darken(0.1), 0.8);
            c.blend(bx + i * 2 + 1, by - 1, BLUSH.darken(0.1), 0.8);
        }
    }
    // Nose hint.
    c.set(o.hx, o.hy + 9, SKIN_LINE.lerp(SKIN, 0.35));

    let (mx, my) = (o.hx, o.hy + 13);
    let mouth = match pose.expr {
        Expression::Happy | Expression::Excited => Mouth::Grin,
        Expression::Sleepy => {
            small_o(c, mx, my);
            return;
        }
        Expression::Determined => Mouth::Grin,
        Expression::Normal => lk.mouth,
    };
    match mouth {
        Mouth::Smile => {
            for (dx, dy) in [(-2, 0), (-1, 1), (0, 1), (1, 1), (2, 0)] {
                c.set(mx + dx, my + dy, MOUTH);
            }
        }
        Mouth::Cat => {
            for (dx, dy) in [(-3, 0), (-2, 1), (-1, 1), (0, 0), (1, 1), (2, 1), (3, 0)] {
                c.set(mx + dx, my + dy, MOUTH);
            }
        }
        Mouth::Grin => {
            c.sprite(
                mx - 3,
                my,
                &["MMMMMMM", "MIIIIIM", ".MITIM.", "..MMM.."],
                &[('M', MOUTH), ('I', MOUTH_IN), ('T', TONGUE)],
            );
        }
    }
}

fn small_o(c: &mut Canvas, x: i32, y: i32) {
    c.sprite(
        x - 1,
        y,
        &[".M.", "MIM", ".M."],
        &[('M', MOUTH), ('I', MOUTH_IN)],
    );
}

/// One 9×12 anime eye. `flip` mirrors the shape (right eye) but keeps the
/// highlights on the same side, as if lit by one light source.
fn eye(c: &mut Canvas, x0: i32, y0: i32, flip: bool, lk: &Look, st: EyeState, star: bool) {
    let put = |c: &mut Canvas, col: i32, row: i32, color: Rgb| {
        let x = if flip { x0 + 8 - col } else { x0 + col };
        c.set(x, y0 + row, color);
    };
    match st {
        EyeState::Closed => {
            // Relaxed closed lid: gentle downward curve with a lash flick.
            for (col, row) in [
                (0, 6),
                (1, 7),
                (2, 8),
                (3, 8),
                (4, 8),
                (5, 8),
                (6, 8),
                (7, 7),
                (8, 7),
            ] {
                put(c, col, row, LASH);
            }
            put(c, -1, 5, LASH);
            put(c, 1, 8, LASH.lerp(SKIN, 0.5));
            return;
        }
        EyeState::Happy => {
            // "^" shaped smiling eye, 2px thick.
            let rows = [9, 7, 6, 5, 5, 5, 6, 7, 9];
            for (col, &r) in rows.iter().enumerate() {
                put(c, col as i32, r, LASH);
                put(c, col as i32, r + 1, LASH);
            }
            return;
        }
        EyeState::Open | EyeState::Sleepy => {}
    }

    let top_cut = if st == EyeState::Sleepy { 5 } else { 0 };
    let inside = |col: i32, row: i32| {
        let fx = col as f32 + 0.5 - 4.5;
        let fy = row as f32 + 0.5 - 6.6;
        row > top_cut && (fx / 4.7).powi(2) + (fy / 5.7).powi(2) <= 1.0
    };
    // First row of the eye in each column → for a 2px lash.
    let top_row = |col: i32| (0..12).find(|&r| inside(col, r)).unwrap_or(12);
    let [dark, mid, light] = lk.iris;
    for row in 0..12 {
        for col in 0..9 {
            if !inside(col, row) {
                continue;
            }
            let tr = top_row(col);
            let color = if row <= tr + 1 {
                LASH
            } else if !inside(col, row + 1) {
                LASH.lerp(dark, 0.4)
            } else {
                let fx = col as f32 + 0.5 - 4.6;
                let fy = row as f32 + 0.5 - 7.0;
                let in_iris = (fx / 3.7).powi(2) + (fy / 5.2).powi(2) <= 1.0;
                if !in_iris {
                    Rgb::hex(0xFDF8FF)
                } else {
                    let px = col as f32 + 0.5 - 4.6;
                    let py = row as f32 + 0.5 - 6.2;
                    if (px / 1.7).powi(2) + (py / 2.4).powi(2) <= 1.0 {
                        dark.darken(0.45)
                    } else {
                        let t = ((row - 3) as f32 / 7.0).clamp(0.0, 1.0);
                        if t < 0.5 {
                            dark.lerp(mid, t * 2.0)
                        } else {
                            mid.lerp(light, (t - 0.5) * 2.0)
                        }
                    }
                }
            };
            put(c, col, row, color);
        }
    }
    // Outer lash flick + thicker top.
    let tr = top_row(0);
    put(c, -1, tr.min(12) - 1, LASH);
    for col in 2..8 {
        let r = top_row(col);
        if r > 0 && r < 12 {
            put(c, col, r - 1, LASH);
        }
    }
    // Highlights: same screen-side for both eyes.
    let hl = WHITE;
    if star {
        for (dx, dy) in [(2, 3), (1, 4), (2, 4), (3, 4), (2, 5)] {
            c.set(x0 + dx, y0 + dy + top_cut.min(2), hl);
        }
    } else if st == EyeState::Open {
        c.rect(x0 + 2, y0 + 3, 2, 3, hl);
        c.set(x0 + 4, y0 + 3, hl);
    } else {
        c.rect(x0 + 2, y0 + 7, 2, 1, hl);
    }
    c.set(x0 + 6, y0 + 9, hl);
    c.set(x0 + 5, y0 + 9, light.lighten(0.4));
}

fn brows(c: &mut Canvas, o: &Origin, lk: &Look) {
    for s in [-1, 1] {
        let (x0, x1) = (o.hx + s * 13, o.hx + s * 5);
        c.line(x0, o.hy - 10, x1, o.hy - 7, lk.hair.line);
        c.line(x0, o.hy - 9, x1, o.hy - 6, lk.hair.line);
    }
}

fn hair_accessory(c: &mut Canvas, o: &Origin, lk: &Look, ch: &Character) {
    match ch {
        Character::Sakura => flower(c, o.cx + 16.0, o.cy - 13.0),
        Character::Yuki => {
            // Two crossed hairpins + a tiny snowflake.
            let (x, y) = (o.hx - 17, o.hy - 14);
            c.line(x - 3, y - 2, x + 3, y + 4, Rgb::hex(0x3E6FB8));
            c.line(x - 3, y + 2, x + 3, y - 2, Rgb::hex(0x7CC4FF));
            c.sprite(
                o.hx + 13,
                o.hy - 19,
                &["..w..", "w.w.w", ".wbw.", "w.w.w", "..w.."],
                &[('w', WHITE), ('b', Rgb::hex(0x7CC4FF))],
            );
        }
        Character::Kuro => {
            // Small red clip.
            c.rect(o.hx + 13, o.hy - 15, 5, 2, lk.accent);
            c.rect(o.hx + 13, o.hy - 12, 5, 2, lk.accent);
        }
        Character::Hikari => {
            for side in [-1.0f32, 1.0] {
                let (bx, by) = (o.cx + side * 22.0, o.cy - 16.0);
                let mut m = c.mask();
                m.poly(&[(bx, by), (bx - 6.0, by - 5.0), (bx - 6.0, by + 4.0)]);
                m.poly(&[(bx, by), (bx + 6.0, by - 5.0), (bx + 6.0, by + 4.0)]);
                m.ellipse(bx, by, 2.0, 2.0);
                c.paint(&m, |_, _| Rgb::hex(0xF0484E), Some(Rgb::hex(0x8E1A28)));
                c.set(bx as i32 - 4, by as i32 - 2, Rgb::hex(0xFF9A9A));
                c.set(bx as i32 + 3, by as i32 - 2, Rgb::hex(0xFF9A9A));
            }
        }
    }
}

/// Five-petal sakura blossom.
pub fn flower(c: &mut Canvas, x: f32, y: f32) {
    let mut m = c.mask();
    for i in 0..5 {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
        m.ellipse(x + a.cos() * 2.6, y + a.sin() * 2.6, 2.1, 2.1);
    }
    c.paint(&m, |_, _| Rgb::hex(0xFFF0F6), Some(Rgb::hex(0xD9467F)));
    c.set(x as i32, y as i32, Rgb::hex(0xFFC93C));
}

fn headphones(c: &mut Canvas, o: &Origin) {
    let band_c = Rgb::hex(0x3C3550);
    let mut band = c.mask();
    let (cx, cy) = (o.cx, o.cy);
    band.custom((cx - 30.0, cy - 36.0, cx + 30.0, cy + 2.0), |x, y| {
        let dx = x - cx;
        let dy = y - cy + 6.0;
        let r_out = (dx / 27.5).powi(2) + (dy / 25.5).powi(2);
        let r_in = (dx / 25.0).powi(2) + (dy / 23.0).powi(2);
        r_out <= 1.0 && r_in > 1.0 && dy < 6.0
    });
    c.paint(&band, |_, _| band_c.lighten(0.15), Some(band_c.darken(0.4)));
    for s in [-1.0f32, 1.0] {
        let x = cx + s * 24.0;
        c.ellipse_outlined(
            x,
            cy + 2.0,
            5.5,
            8.5,
            Rgb::hex(0xF6F0FF),
            band_c.darken(0.3),
        );
        c.ellipse_outlined(
            x + s * 1.0,
            cy + 2.0,
            3.0,
            5.5,
            Rgb::hex(0xFF6FAE),
            Rgb::hex(0x9E2A62),
        );
        c.set((x - s * 3.0) as i32, (cy - 3.0) as i32, WHITE);
        c.set((x - s * 3.0) as i32, (cy - 2.0) as i32, Rgb::hex(0xD8D0EE));
        c.set((x - s * 3.0) as i32, (cy + 6.0) as i32, Rgb::hex(0xC8C0DE));
    }
}

fn headband(c: &mut Canvas, o: &Origin, front: &Mask) {
    let mut band = front.clone();
    let mut rows = c.mask();
    rows.rect(o.cx - 30.0, o.cy - 17.0, o.cx + 30.0, o.cy - 7.0);
    band.intersect(&rows);
    c.paint(&band, |_, _| WHITE, Some(Rgb::hex(0xB8B8CC)));
    // Knot tails flying to the right.
    let red = Rgb::hex(0xE23B3B);
    let mut tails = c.mask();
    tails.poly(&o.pts(&[(19.0, -16.0), (31.0, -21.0), (33.0, -15.0), (20.0, -11.0)]));
    tails.poly(&o.pts(&[(19.0, -12.0), (32.0, -9.0), (30.0, -3.0), (19.0, -8.0)]));
    c.paint(&tails, |_, _| WHITE, Some(Rgb::hex(0xB8B8CC)));
    c.ellipse_outlined(
        o.cx + 20.0,
        o.cy - 12.0,
        2.5,
        2.5,
        WHITE,
        Rgb::hex(0xB8B8CC),
    );
    text::draw_plain(c, "FOCUS", o.hx - 12, o.hy - 16, &FONT_5X8, red);
}

fn sunglasses(c: &mut Canvas, o: &Origin) {
    let frame = Rgb::hex(0x22202E);
    let lens = Rgb::hex(0x3A3654);
    for s in [-1, 1] {
        let x = if s < 0 { o.hx - 13 } else { o.hx + 3 };
        let y = o.hy - 21;
        c.rect(x, y, 11, 6, frame);
        c.rect(x + 1, y + 1, 9, 4, lens);
        c.set(x + 2, y + 1, Rgb::hex(0xB8B4E8));
        c.set(x + 3, y + 1, Rgb::hex(0xB8B4E8));
        c.set(x + 2, y + 2, Rgb::hex(0x8C88C8));
    }
    c.rect(o.hx - 2, o.hy - 20, 5, 1, frame);
}

fn scarf(c: &mut Canvas, o: &Origin) {
    let red = Rgb::hex(0xE5484D);
    let mut m = c.mask();
    m.ellipse(o.cx, o.cy + 23.0, 15.0, 6.0);
    m.poly(&o.pts(&[(3.0, 22.0), (12.0, 24.0), (13.0, 42.0), (6.0, 43.0)]));
    let hy = o.hy;
    c.paint(
        &m,
        |_, y| {
            if (y - hy).rem_euclid(5) < 2 {
                Rgb::hex(0xFFF2F2)
            } else {
                red
            }
        },
        Some(red.darken(0.45)),
    );
    for x in [7, 9, 11] {
        c.set(o.hx + x, o.hy + 44, red.darken(0.2));
    }
}

fn tea_cup(c: &mut Canvas, o: &Origin, frame: u32) {
    let (x, y) = (o.cx - 1.0, o.cy + 44.0);
    c.ellipse_outlined(x + 9.0, y, 3.5, 4.0, WHITE, Rgb::hex(0x9A7A8A));
    c.ellipse(x + 9.0, y, 1.5, 2.0, Rgb::hex(0xFFE9F0));
    let mut cup = c.mask();
    cup.rect(x - 8.0, y - 6.0, x + 8.0, y + 2.0);
    cup.ellipse(x, y + 2.0, 8.0, 5.0);
    c.paint(&cup, |_, _| WHITE, Some(Rgb::hex(0x9A7A8A)));
    c.ellipse(x, y - 6.0, 7.0, 1.6, Rgb::hex(0xB0703C));
    // Heart on the cup.
    c.sprite(
        x as i32 - 2,
        y as i32 - 2,
        &[".h.h.", "hhhhh", ".hhh.", "..h.."],
        &[('h', Rgb::hex(0xFF6F91))],
    );
    // Hands.
    for s in [-1.0f32, 1.0] {
        c.ellipse_outlined(x + s * 9.5, y + 2.0, 3.5, 3.0, SKIN, SKIN_LINE);
    }
    // Steam wisps sway with the frame.
    let ph = frame as f32 * 0.9;
    for k in 0..2 {
        let bx = x - 3.0 + k as f32 * 5.0;
        for i in 0..8 {
            let yy = y - 9.0 - i as f32;
            let xx = bx + ((i as f32 * 0.8 + ph + k as f32 * 2.0).sin() * 1.5);
            c.blend(xx as i32, yy as i32, WHITE, 0.6 - i as f32 * 0.06);
        }
    }
}

fn umbrella_shaft(c: &mut Canvas, o: &Origin) {
    let col = Rgb::hex(0x4A3A4A);
    for d in 0..2 {
        c.line(o.hx + 9 + d, o.hy - 46, o.hx + 23 + d, o.hy + 40, col);
    }
}

fn umbrella_canopy(c: &mut Canvas, o: &Origin, lk: &Look) {
    let (ux, uy) = (o.cx + 8.0, o.cy - 22.0);
    let (rx, ry) = (40.0, 24.0);
    let mut m = c.mask();
    m.custom(
        (ux - rx - 1.0, uy - ry - 1.0, ux + rx + 1.0, uy + 1.0),
        |x, y| {
            let dx = (x - ux) / rx;
            let dy = (y - uy) / ry;
            y <= uy && dx * dx + dy * dy <= 1.0
        },
    );
    // Scalloped rim between ribs.
    let ribs = 5;
    let seg = 2.0 * rx / ribs as f32;
    for i in 0..ribs {
        let xm = ux - rx + seg * (i as f32 + 0.5);
        m.erase_ellipse(xm, uy + 1.0, seg / 2.0 - 0.5, 3.0);
    }
    let a = lk.accent;
    let b = lk.accent.lighten(0.45);
    c.paint(
        &m,
        |x, y| {
            // Project the pixel from the apex onto the rim to find its panel.
            let dx = x as f32 + 0.5 - ux;
            let down = (y as f32 + 0.5 - (uy - ry)).max(0.5);
            let rim_x = dx * ry / down + rx;
            let k = (rim_x / seg).floor() as i32;
            if k.rem_euclid(2) == 0 {
                a
            } else {
                b
            }
        },
        Some(a.darken(0.5)),
    );
    // Shine + top knob.
    c.line(
        ux as i32 - 22,
        uy as i32 - 10,
        ux as i32 - 12,
        uy as i32 - 17,
        WHITE.lerp(b, 0.3),
    );
    c.ellipse_outlined(ux, uy - ry - 1.0, 2.0, 2.0, a.darken(0.2), a.darken(0.5));
    // Hand holding the shaft.
    c.ellipse_outlined(o.cx + 22.0, o.cy + 31.0, 3.5, 3.0, SKIN, SKIN_LINE);
}
