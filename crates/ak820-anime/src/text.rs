//! Bitmap text via `embedded-graphics` mono fonts, always drawn with a dark
//! outline so it stays legible over busy backgrounds.

use crate::canvas::{Canvas, Rgb};
use embedded_graphics::{
    mono_font::{MonoFont, MonoTextStyle},
    pixelcolor::{Rgb888, RgbColor},
    prelude::*,
    text::{Baseline, Text},
};

pub use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_5X8, FONT_6X10, FONT_9X15_BOLD};

impl OriginDimensions for Canvas {
    fn size(&self) -> Size {
        Size::new(self.width() as u32, self.height() as u32)
    }
}

impl DrawTarget for Canvas {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels {
            self.set(
                p.x,
                p.y,
                Rgb {
                    r: c.r(),
                    g: c.g(),
                    b: c.b(),
                },
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
}

fn advance(font: &MonoFont) -> i32 {
    (font.character_size.width + font.character_spacing) as i32
}

pub fn text_width(s: &str, font: &MonoFont) -> i32 {
    let n = s.chars().count() as i32;
    if n == 0 {
        0
    } else {
        n * advance(font) - font.character_spacing as i32
    }
}

/// Map arbitrary input to the ASCII the fonts can draw: Cyrillic is
/// transliterated, everything else non-printable becomes '?'.
pub fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_graphic() || ch == ' ' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push(' ');
        } else if let Some(t) = translit(ch) {
            out.push_str(&t);
        } else {
            out.push('?');
        }
    }
    out
}

fn translit(ch: char) -> Option<String> {
    const MAP: &[(char, &str)] = &[
        ('а', "a"),
        ('б', "b"),
        ('в', "v"),
        ('г', "g"),
        ('д', "d"),
        ('е', "e"),
        ('ё', "e"),
        ('ж', "zh"),
        ('з', "z"),
        ('и', "i"),
        ('й', "y"),
        ('к', "k"),
        ('л', "l"),
        ('м', "m"),
        ('н', "n"),
        ('о', "o"),
        ('п', "p"),
        ('р', "r"),
        ('с', "s"),
        ('т', "t"),
        ('у', "u"),
        ('ф', "f"),
        ('х', "h"),
        ('ц', "ts"),
        ('ч', "ch"),
        ('ш', "sh"),
        ('щ', "sch"),
        ('ъ', ""),
        ('ы', "y"),
        ('ь', ""),
        ('э', "e"),
        ('ю', "yu"),
        ('я', "ya"),
        ('қ', "q"),
        ('ғ', "g"),
        ('ү', "u"),
        ('ұ', "u"),
        ('ң', "n"),
        ('ө', "o"),
        ('һ', "h"),
        ('і', "i"),
        ('ә', "a"),
        ('’', "'"),
        ('‘', "'"),
        ('“', "\""),
        ('”', "\""),
        ('–', "-"),
        ('—', "-"),
        ('…', "..."),
        ('é', "e"),
        ('è', "e"),
        ('ü', "u"),
        ('ö', "o"),
        ('ä', "a"),
        ('ß', "ss"),
        ('ñ', "n"),
        ('ç', "c"),
        ('á', "a"),
        ('í', "i"),
        ('ó', "o"),
        ('ú', "u"),
    ];
    let lower = ch.to_lowercase().next()?;
    let t = MAP.iter().find(|(k, _)| *k == lower)?.1;
    if ch.is_uppercase() {
        // Capitalise the first letter only ("Ж" → "Zh").
        let mut it = t.chars();
        return Some(match it.next() {
            Some(f) => f.to_ascii_uppercase().to_string() + it.as_str(),
            None => String::new(),
        });
    }
    Some(t.to_string())
}

/// Sanitise and truncate to `max_w` pixels, appending ".." when cut.
pub fn fit(s: &str, font: &MonoFont, max_w: i32) -> String {
    let s = sanitize(s.trim());
    if text_width(&s, font) <= max_w {
        return s;
    }
    let mut chars: Vec<char> = s.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cand: String = chars.iter().collect::<String>().trim_end().to_string() + "..";
        if text_width(&cand, font) <= max_w {
            return cand;
        }
    }
    String::new()
}

/// Draw `s` with a 1px dark outline (all 8 neighbours) and optional drop shadow.
/// Returns the drawn width.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    c: &mut Canvas,
    s: &str,
    x: i32,
    y: i32,
    font: &MonoFont,
    color: Rgb,
    outline: Rgb,
    align: Align,
) -> i32 {
    let s = sanitize(s);
    let w = text_width(&s, font);
    let x = match align {
        Align::Left => x,
        Align::Center => x - w / 2,
    };
    let rgb = |c: Rgb| Rgb888::new(c.r, c.g, c.b);
    let out_style = MonoTextStyle::new(font, rgb(outline));
    // Soft shadow one more pixel down-right, then the hard outline ring.
    let _ = Text::with_baseline(&s, Point::new(x + 1, y + 2), out_style, Baseline::Top).draw(c);
    for (dx, dy) in [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        let _ =
            Text::with_baseline(&s, Point::new(x + dx, y + dy), out_style, Baseline::Top).draw(c);
    }
    let style = MonoTextStyle::new(font, rgb(color));
    let _ = Text::with_baseline(&s, Point::new(x, y), style, Baseline::Top).draw(c);
    w
}

/// Plain text without outline (for text on solid props like a headband).
pub fn draw_plain(c: &mut Canvas, s: &str, x: i32, y: i32, font: &MonoFont, color: Rgb) {
    let s = sanitize(s);
    let style = MonoTextStyle::new(font, Rgb888::new(color.r, color.g, color.b));
    let _ = Text::with_baseline(&s, Point::new(x, y), style, Baseline::Top).draw(c);
}

/// Small hollow degree ring (font has no '°'), top-left at (x, y).
pub fn degree(c: &mut Canvas, x: i32, y: i32, color: Rgb, outline: Rgb) {
    let ring = [".###.", "#...#", "#...#", "#...#", ".###."];
    // Outline: dilate the ring by one pixel.
    for (ry, row) in ring.iter().enumerate() {
        for (rx, ch) in row.chars().enumerate() {
            if ch == '#' {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        c.set(x + rx as i32 + dx, y + ry as i32 + dy, outline);
                    }
                }
            }
        }
    }
    c.sprite(x, y, &ring, &[('#', color)]);
    // Re-open the hole so the ring reads as a ring.
    c.rect(x + 1, y + 1, 3, 3, outline);
    c.sprite(x, y, &ring, &[('#', color)]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_truncates() {
        let s = fit("A very very long song title indeed", &FONT_6X10, 60);
        assert!(text_width(&s, &FONT_6X10) <= 60);
        assert!(s.ends_with(".."));
        assert_eq!(fit("Hi", &FONT_6X10, 60), "Hi");
    }

    #[test]
    fn sanitize_cyrillic() {
        assert_eq!(sanitize("Алматы"), "Almaty");
        assert_eq!(sanitize("Жаз ♪"), "Zhaz ?");
    }
}
