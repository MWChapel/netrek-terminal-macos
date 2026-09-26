//! Real pixel graphics for terminals that speak SIXEL (iTerm2, WezTerm, foot,
//! and tmux 3.4+ when the outer terminal supports it), plus text rendering
//! into images using the system's monospace font.

use super::palette::Rgb;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Encode an image as a SIXEL escape sequence, with an adaptive palette of
/// up to 256 colors (exact when the image has fewer distinct colors).
pub fn encode_canvas(c: &super::vg::Canvas) -> Vec<u8> {
    encode_with(c.w, c.h, |x, y| c.get(x, y))
}

pub fn encode_with(w: i32, h: i32, get: impl Fn(i32, i32) -> Rgb) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let key = |c: Rgb| -> u16 {
        let q = |v: f32| (v.clamp(0.0, 255.0) as u16) >> 3;
        (q(c[0]) << 10) | (q(c[1]) << 5) | q(c[2])
    };
    let mut keys = vec![0u16; w * h];
    let mut hist: HashMap<u16, u32> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let k = key(get(x as i32, y as i32));
            keys[y * w + x] = k;
            *hist.entry(k).or_insert(0) += 1;
        }
    }
    let mut colors: Vec<(u16, u32)> = hist.into_iter().collect();
    colors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let palette: Vec<u16> = colors.iter().take(255).map(|c| c.0).collect();
    let unpack = |k: u16| [((k >> 10) & 31) as i32, ((k >> 5) & 31) as i32, (k & 31) as i32];
    let mut map: HashMap<u16, u8> = palette.iter().enumerate().map(|(i, &k)| (k, i as u8)).collect();
    for &(k, _) in colors.iter().skip(255) {
        let c = unpack(k);
        let best = palette
            .iter()
            .enumerate()
            .min_by_key(|(_, &p)| {
                let q = unpack(p);
                (c[0] - q[0]).pow(2) + (c[1] - q[1]).pow(2) + (c[2] - q[2]).pow(2)
            })
            .map(|(i, _)| i as u8)
            .unwrap_or(0);
        map.insert(k, best);
    }
    let idx: Vec<u8> = keys.iter().map(|k| map[k]).collect();

    let mut out = String::with_capacity(w * h / 4);
    // P1=9: square pixels, P2=1: unset pixels keep their color. Then raster size.
    let _ = write!(out, "\x1bP9;1;0q\"1;1;{};{}", w, h);
    for (i, &k) in palette.iter().enumerate() {
        let c = unpack(k);
        let pct = |v: i32| (v * 100 + 15) / 31;
        let _ = write!(out, "#{};2;{};{};{}", i, pct(c[0]), pct(c[1]), pct(c[2]));
    }
    let mut row = vec![0u8; w];
    let mut used = vec![false; palette.len().max(1)];
    for band in (0..h).step_by(6) {
        used.iter_mut().for_each(|u| *u = false);
        for y in band..(band + 6).min(h) {
            for x in 0..w {
                used[idx[y * w + x] as usize] = true;
            }
        }
        let mut first = true;
        for (c, _) in used.iter().enumerate().filter(|(_, &u)| u) {
            for x in 0..w {
                let mut bits = 0u8;
                for r in 0..6 {
                    let y = band + r;
                    if y < h && idx[y * w + x] as usize == c {
                        bits |= 1 << r;
                    }
                }
                row[x] = bits;
            }
            if !first {
                out.push('$');
            }
            first = false;
            let _ = write!(out, "#{}", c);
            let mut x = 0;
            while x < w {
                let b = row[x];
                let mut n = 1;
                while x + n < w && row[x + n] == b {
                    n += 1;
                }
                let ch = (63 + b) as char;
                if n > 3 {
                    let _ = write!(out, "!{}{}", n, ch);
                } else {
                    for _ in 0..n {
                        out.push(ch);
                    }
                }
                x += n;
            }
        }
        out.push('-');
    }
    out.push_str("\x1b\\");
    out.into_bytes()
}

type Glyph = (fontdue::Metrics, Vec<u8>);

/// Draws anti-aliased text into pixel buffers.
pub struct TextRenderer {
    font: Option<fontdue::Font>,
    cache: RefCell<HashMap<(char, u32), Glyph>>,
}

impl TextRenderer {
    pub fn load() -> TextRenderer {
        let paths = [
            ("/System/Library/Fonts/Menlo.ttc", 0),
            ("/System/Library/Fonts/Monaco.ttf", 0),
            ("/System/Library/Fonts/SFNSMono.ttf", 0),
            ("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 0),
        ];
        let font = paths.iter().find_map(|(p, idx)| {
            let bytes = std::fs::read(p).ok()?;
            let settings = fontdue::FontSettings { collection_index: *idx, scale: 24.0, ..Default::default() };
            fontdue::Font::from_bytes(bytes, settings).ok()
        });
        TextRenderer { font, cache: RefCell::new(HashMap::new()) }
    }

    fn with_glyph<R>(&self, c: char, size: f32, f: impl FnOnce(&Glyph) -> R) -> Option<R> {
        let font = self.font.as_ref()?;
        let key = (c, (size * 4.0) as u32);
        let mut cache = self.cache.borrow_mut();
        let g = cache.entry(key).or_insert_with(|| font.rasterize(c, size));
        Some(f(g))
    }

    pub fn width(&self, s: &str, size: f32) -> f32 {
        s.chars().map(|c| self.with_glyph(c, size, |g| g.0.advance_width).unwrap_or(size * 0.6)).sum()
    }

    /// Rasterize `s` and hand each covered pixel (x, y, coverage) to `plot`.
    pub fn render(&self, x: f32, baseline: f32, s: &str, size: f32, mut plot: impl FnMut(i32, i32, f32)) {
        let mut pen = x;
        for c in s.chars() {
            let adv = self
                .with_glyph(c, size, |(m, bmp)| {
                    let left = (pen + m.xmin as f32).round() as i32;
                    let top = (baseline - m.height as f32 - m.ymin as f32).round() as i32;
                    for gy in 0..m.height {
                        for gx in 0..m.width {
                            let cov = bmp[gy * m.width + gx] as f32 / 255.0;
                            if cov > 0.0 {
                                plot(left + gx as i32, top + gy as i32, cov);
                            }
                        }
                    }
                    m.advance_width
                })
                .unwrap_or(size * 0.6);
            pen += adv;
        }
    }
}
