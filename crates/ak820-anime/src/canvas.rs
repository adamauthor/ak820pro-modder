//! Tiny RGB888 raster canvas with pixel-art drawing primitives.
//!
//! Shapes are rasterised by testing pixel centres, which keeps edges crisp
//! (no anti-aliasing) — exactly what a 128×128 pixel-art display wants.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub const WHITE: Rgb = Rgb::hex(0xFFFFFF);
pub const BLACK: Rgb = Rgb::hex(0x000000);

impl Rgb {
    pub const fn hex(v: u32) -> Self {
        Self {
            r: ((v >> 16) & 0xFF) as u8,
            g: ((v >> 8) & 0xFF) as u8,
            b: (v & 0xFF) as u8,
        }
    }

    pub fn lerp(self, o: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb {
            r: m(self.r, o.r),
            g: m(self.g, o.g),
            b: m(self.b, o.b),
        }
    }

    pub fn darken(self, t: f32) -> Rgb {
        self.lerp(BLACK, t)
    }

    pub fn lighten(self, t: f32) -> Rgb {
        self.lerp(WHITE, t)
    }
}

/// Boolean coverage grid with the same dimensions as the canvas. Shapes are
/// unioned / subtracted into it and then painted in one go, which lets us draw
/// a clean 1px outline around arbitrary unions (hair, clouds, bodies…).
#[derive(Clone)]
pub struct Mask {
    w: i32,
    h: i32,
    bits: Vec<bool>,
}

impl Mask {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            bits: vec![false; (w * h) as usize],
        }
    }

    pub fn get(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && self.bits[(y * self.w + x) as usize]
    }

    fn apply(&mut self, bbox: (f32, f32, f32, f32), val: bool, inside: impl Fn(f32, f32) -> bool) {
        let x0 = (bbox.0.floor() as i32).max(0);
        let y0 = (bbox.1.floor() as i32).max(0);
        let x1 = (bbox.2.ceil() as i32).min(self.w - 1);
        let y1 = (bbox.3.ceil() as i32).min(self.h - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if inside(x as f32 + 0.5, y as f32 + 0.5) {
                    self.bits[(y * self.w + x) as usize] = val;
                }
            }
        }
    }

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        self.apply(ell_bbox(cx, cy, rx, ry), true, |x, y| {
            in_ellipse(x, y, cx, cy, rx, ry)
        });
    }

    pub fn erase_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        self.apply(ell_bbox(cx, cy, rx, ry), false, |x, y| {
            in_ellipse(x, y, cx, cy, rx, ry)
        });
    }

    pub fn poly(&mut self, pts: &[(f32, f32)]) {
        self.apply(poly_bbox(pts), true, |x, y| in_poly(x, y, pts));
    }

    pub fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.apply((x0, y0, x1, y1), true, |x, y| {
            x >= x0 && x < x1 && y >= y0 && y < y1
        });
    }

    /// Arbitrary predicate over pixel centres inside `bbox`.
    pub fn custom(&mut self, bbox: (f32, f32, f32, f32), inside: impl Fn(f32, f32) -> bool) {
        self.apply(bbox, true, inside);
    }

    /// Keep only pixels also covered by `other`.
    pub fn intersect(&mut self, other: &Mask) {
        for (a, b) in self.bits.iter_mut().zip(&other.bits) {
            *a = *a && *b;
        }
    }

    pub fn is_edge(&self, x: i32, y: i32) -> bool {
        self.get(x, y)
            && (!self.get(x - 1, y)
                || !self.get(x + 1, y)
                || !self.get(x, y - 1)
                || !self.get(x, y + 1))
    }
}

fn ell_bbox(cx: f32, cy: f32, rx: f32, ry: f32) -> (f32, f32, f32, f32) {
    (cx - rx - 1.0, cy - ry - 1.0, cx + rx + 1.0, cy + ry + 1.0)
}

fn in_ellipse(x: f32, y: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> bool {
    let dx = (x - cx) / rx.max(0.01);
    let dy = (y - cy) / ry.max(0.01);
    dx * dx + dy * dy <= 1.0
}

fn poly_bbox(pts: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    pts.iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(a, b, c, d), &(x, y)| (a.min(x), b.min(y), c.max(x), d.max(y)),
    )
}

/// Even-odd point-in-polygon test.
pub fn in_poly(x: f32, y: f32, pts: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let n = pts.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub struct Canvas {
    w: i32,
    h: i32,
    px: Vec<Rgb>,
}

impl Canvas {
    pub fn new(w: u32, h: u32, bg: Rgb) -> Self {
        Self {
            w: w as i32,
            h: h as i32,
            px: vec![bg; (w * h) as usize],
        }
    }

    pub fn width(&self) -> i32 {
        self.w
    }

    pub fn height(&self) -> i32 {
        self.h
    }

    pub fn mask(&self) -> Mask {
        Mask::new(self.w, self.h)
    }

    pub fn get(&self, x: i32, y: i32) -> Rgb {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return BLACK;
        }
        self.px[(y * self.w + x) as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    /// Alpha-blend `c` over the existing pixel.
    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if a <= 0.0 {
            return;
        }
        let cur = self.get(x, y);
        self.set(x, y, cur.lerp(c, a));
    }

    /// Vertical gradient through `(position 0..1, colour)` stops.
    pub fn vgradient(&mut self, stops: &[(f32, Rgb)]) {
        for y in 0..self.h {
            let t = y as f32 / (self.h - 1).max(1) as f32;
            let c = gradient_at(stops, t);
            for x in 0..self.w {
                self.set(x, y, c);
            }
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    pub fn rect_blend(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb, a: f32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.blend(xx, yy, c, a);
            }
        }
    }

    /// Rounded rectangle (corner radius `r`) blended with alpha `a`.
    #[allow(clippy::too_many_arguments)]
    pub fn round_rect_blend(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Rgb, a: f32) {
        for yy in 0..h {
            for xx in 0..w {
                let dx = if xx < r {
                    r - xx
                } else if xx >= w - r {
                    xx - (w - r - 1)
                } else {
                    0
                };
                let dy = if yy < r {
                    r - yy
                } else if yy >= h - r {
                    yy - (h - r - 1)
                } else {
                    0
                };
                if dx * dx + dy * dy <= r * r {
                    self.blend(x + xx, y + yy, c, a);
                }
            }
        }
    }

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgb) {
        self.ellipse_blend(cx, cy, rx, ry, c, 1.0);
    }

    pub fn ellipse_blend(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgb, a: f32) {
        let (x0, y0, x1, y1) = ell_bbox(cx, cy, rx, ry);
        for y in y0 as i32..=y1 as i32 {
            for x in x0 as i32..=x1 as i32 {
                if in_ellipse(x as f32 + 0.5, y as f32 + 0.5, cx, cy, rx, ry) {
                    self.blend(x, y, c, a);
                }
            }
        }
    }

    /// Filled ellipse with a 1px inner outline.
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse_outlined(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        fill: Rgb,
        outline: Rgb,
    ) {
        let mut m = self.mask();
        m.ellipse(cx, cy, rx, ry);
        self.paint(&m, |_, _| fill, Some(outline));
    }

    pub fn poly(&mut self, pts: &[(f32, f32)], c: Rgb) {
        let mut m = self.mask();
        m.poly(pts);
        self.paint(&m, |_, _| c, None);
    }

    /// Bresenham line.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        self.line_blend(x0, y0, x1, y1, c, 1.0);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn line_blend(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb, a: f32) {
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.blend(x, y, c, a);
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

    /// Soft radial glow (quadratic falloff).
    pub fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
        let (x0, y0, x1, y1) = ell_bbox(cx, cy, r, r);
        for y in y0 as i32..=y1 as i32 {
            for x in x0 as i32..=x1 as i32 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let d = (dx * dx + dy * dy).sqrt() / r;
                if d < 1.0 {
                    self.blend(x, y, c, a * (1.0 - d) * (1.0 - d));
                }
            }
        }
    }

    /// Paint every masked pixel with `fill(x, y)`; edge pixels get `outline`.
    pub fn paint(&mut self, m: &Mask, fill: impl Fn(i32, i32) -> Rgb, outline: Option<Rgb>) {
        for y in 0..self.h {
            for x in 0..self.w {
                if !m.get(x, y) {
                    continue;
                }
                let c = match outline {
                    Some(o) if m.is_edge(x, y) => o,
                    _ => fill(x, y),
                };
                self.set(x, y, c);
            }
        }
    }

    /// Draw a small pixel-art sprite. Each char maps to a colour via `pal`;
    /// unmapped chars (e.g. '.') are transparent.
    pub fn sprite(&mut self, x: i32, y: i32, rows: &[&str], pal: &[(char, Rgb)]) {
        for (dy, row) in rows.iter().enumerate() {
            for (dx, ch) in row.chars().enumerate() {
                if let Some(&(_, c)) = pal.iter().find(|(k, _)| *k == ch) {
                    self.set(x + dx as i32, y + dy as i32, c);
                }
            }
        }
    }

    /// Area-averaged scaled copy of an RGB888 source image into `(dx, dy, dw, dh)`.
    #[allow(clippy::too_many_arguments)]
    pub fn blit_rgb(&mut self, src: &[u8], sw: i32, sh: i32, dx: i32, dy: i32, dw: i32, dh: i32) {
        if dw <= 0 || dh <= 0 || sw <= 0 || sh <= 0 || src.len() < (sw * sh * 3) as usize {
            return;
        }
        for y in 0..dh {
            let sy0 = y * sh / dh;
            let sy1 = ((y + 1) * sh / dh).max(sy0 + 1);
            for x in 0..dw {
                let sx0 = x * sw / dw;
                let sx1 = ((x + 1) * sw / dw).max(sx0 + 1);
                let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
                for sy in sy0..sy1 {
                    for sx in sx0..sx1 {
                        let i = ((sy * sw + sx) * 3) as usize;
                        r += src[i] as u32;
                        g += src[i + 1] as u32;
                        b += src[i + 2] as u32;
                        n += 1;
                    }
                }
                let c = Rgb {
                    r: (r / n) as u8,
                    g: (g / n) as u8,
                    b: (b / n) as u8,
                };
                self.set(dx + x, dy + y, c);
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 3);
        for p in &self.px {
            out.extend_from_slice(&[p.r, p.g, p.b]);
        }
        out
    }
}

pub fn gradient_at(stops: &[(f32, Rgb)], t: f32) -> Rgb {
    match stops {
        [] => BLACK,
        [only] => only.1,
        _ => {
            if t <= stops[0].0 {
                return stops[0].1;
            }
            for w in stops.windows(2) {
                let (a, b) = (w[0], w[1]);
                if t <= b.0 {
                    let span = (b.0 - a.0).max(1e-6);
                    return a.1.lerp(b.1, (t - a.0) / span);
                }
            }
            stops[stops.len() - 1].1
        }
    }
}

/// Deterministic hash → pseudo-random stream (splitmix64).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 32) as u32
    }

    /// Uniform integer in `lo..hi`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u32() % (hi - lo) as u32) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_outline_and_poly() {
        let mut c = Canvas::new(16, 16, BLACK);
        let mut m = c.mask();
        m.poly(&[(2.0, 2.0), (14.0, 2.0), (14.0, 14.0), (2.0, 14.0)]);
        c.paint(&m, |_, _| WHITE, Some(Rgb::hex(0xFF0000)));
        assert_eq!(c.get(2, 2), Rgb::hex(0xFF0000));
        assert_eq!(c.get(8, 8), WHITE);
        assert_eq!(c.get(0, 0), BLACK);
    }

    #[test]
    fn gradient_endpoints() {
        let s = [(0.0, BLACK), (1.0, WHITE)];
        assert_eq!(gradient_at(&s, 0.0), BLACK);
        assert_eq!(gradient_at(&s, 1.0), WHITE);
    }
}
