//! A small software rasterizer for the character-cell ("blocks") mode.
//!
//! Drawing happens in logical coordinates (2 logical pixels per cell across),
//! into a supersampled framebuffer with several square physical pixels per
//! cell (e.g. 6x15). Each cell is then converted to whichever block element
//! glyph (halves, quarters, eighths) plus foreground/background colors best
//! reproduces its pixels. Terminals draw these glyphs themselves, edge to edge.

use super::canvas::Screen;
use crossterm::style::Color;

pub type Rgb = [f32; 3];

pub const fn rgb(hex: u32) -> Rgb {
    [((hex >> 16) & 0xff) as f32, ((hex >> 8) & 0xff) as f32, (hex & 0xff) as f32]
}

pub fn scale(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

pub struct Pixels {
    /// Physical framebuffer size.
    pub w: i32,
    pub h: i32,
    buf: Vec<Rgb>,
    /// Physical pixels per logical pixel.
    k: f32,
    /// Physical pixels per character cell.
    sx: i32,
    sy: i32,
}

/// Block-element glyphs as foreground rectangles (x0, y0, x1, y1) in cell
/// fractions. Complements (▄ ▐ ▙ ▔ ...) come free by swapping fg and bg.
const GLYPHS: &[(char, &[(f32, f32, f32, f32)])] = &[
    ('▀', &[(0.0, 0.0, 1.0, 0.5)]),
    ('▌', &[(0.0, 0.0, 0.5, 1.0)]),
    ('▘', &[(0.0, 0.0, 0.5, 0.5)]),
    ('▝', &[(0.5, 0.0, 1.0, 0.5)]),
    ('▖', &[(0.0, 0.5, 0.5, 1.0)]),
    ('▗', &[(0.5, 0.5, 1.0, 1.0)]),
    ('▚', &[(0.0, 0.0, 0.5, 0.5), (0.5, 0.5, 1.0, 1.0)]),
    ('▁', &[(0.0, 0.875, 1.0, 1.0)]),
    ('▂', &[(0.0, 0.75, 1.0, 1.0)]),
    ('▃', &[(0.0, 0.625, 1.0, 1.0)]),
    ('▅', &[(0.0, 0.375, 1.0, 1.0)]),
    ('▆', &[(0.0, 0.25, 1.0, 1.0)]),
    ('▇', &[(0.0, 0.125, 1.0, 1.0)]),
    ('▏', &[(0.0, 0.0, 0.125, 1.0)]),
    ('▎', &[(0.0, 0.0, 0.25, 1.0)]),
    ('▍', &[(0.0, 0.0, 0.375, 1.0)]),
    ('▋', &[(0.0, 0.0, 0.625, 1.0)]),
    ('▊', &[(0.0, 0.0, 0.75, 1.0)]),
    ('▉', &[(0.0, 0.0, 0.875, 1.0)]),
];

/// For a sx-by-sy subpixel grid, which subpixels each glyph paints.
fn glyph_masks(sx: i32, sy: i32) -> Vec<(char, Vec<bool>)> {
    let mut out: Vec<(char, Vec<bool>)> = Vec::new();
    for &(ch, rects) in GLYPHS {
        let mask: Vec<bool> = (0..sx * sy)
            .map(|i| {
                let (fx, fy) = (((i % sx) as f32 + 0.5) / sx as f32, ((i / sx) as f32 + 0.5) / sy as f32);
                rects.iter().any(|&(x0, y0, x1, y1)| fx >= x0 && fx < x1 && fy >= y0 && fy < y1)
            })
            .collect();
        let n = mask.iter().filter(|&&b| b).count();
        if n == 0 || n == mask.len() || out.iter().any(|(_, m)| *m == mask) {
            continue;
        }
        out.push((ch, mask));
    }
    out
}

impl Pixels {
    /// A plain buffer: 2x4 pixels per cell, one pixel per logical pixel.
    pub fn new(w: i32, h: i32) -> Pixels {
        Pixels { w: w.max(0), h: h.max(0), buf: vec![[0.0; 3]; (w.max(0) * h.max(0)) as usize], k: 1.0, sx: 2, sy: 4 }
    }

    /// A supersampled buffer covering `cols` x `rows` cells with `sx` x `sy`
    /// physical pixels per cell; logical coordinates are 2 per cell across.
    pub fn new_ss(cols: i32, rows: i32, sx: i32, sy: i32) -> Pixels {
        let (w, h) = ((cols * sx).max(0), (rows * sy).max(0));
        Pixels { w, h, buf: vec![[0.0; 3]; (w * h) as usize], k: sx as f32 / 2.0, sx, sy }
    }

    /// Fill every pixel from a function of its logical center coordinates.
    pub fn fill_with(&mut self, f: impl Fn(f64, f64) -> Rgb) {
        let k = self.k as f64;
        for y in 0..self.h {
            for x in 0..self.w {
                self.buf[(y * self.w + x) as usize] = f((x as f64 + 0.5) / k, (y as f64 + 0.5) / k);
            }
        }
    }

    /// A small point (a star) at logical coordinates.
    pub fn dot(&mut self, x: f32, y: f32, c: Rgb, a: f32) {
        let n = (self.k * 0.7).round().max(1.0) as i32;
        let (px, py) = ((x * self.k) as i32, (y * self.k) as i32);
        for dy in 0..n {
            for dx in 0..n {
                self.blend(px + dx, py + dy, c, a);
            }
        }
    }

    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h || a <= 0.0 {
            return;
        }
        let a = a.min(1.0);
        let p = &mut self.buf[(y * self.w + x) as usize];
        for k in 0..3 {
            p[k] += (c[k] - p[k]) * a;
        }
    }

    #[inline]
    pub fn add(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h || a <= 0.0 {
            return;
        }
        let p = &mut self.buf[(y * self.w + x) as usize];
        for k in 0..3 {
            p[k] = (p[k] + c[k] * a).min(255.0);
        }
    }

    fn bbox(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Option<(i32, i32, i32, i32)> {
        let (ax, ay) = ((x0.floor() as i32).max(0), (y0.floor() as i32).max(0));
        let (bx, by) = ((x1.ceil() as i32).min(self.w - 1), (y1.ceil() as i32).min(self.h - 1));
        if ax > bx || ay > by {
            None
        } else {
            Some((ax, ay, bx, by))
        }
    }

    /// Anti-aliased filled circle.
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
        let (cx, cy, r) = (cx * self.k, cy * self.k, r * self.k);
        let Some((x0, y0, x1, y1)) = self.bbox(cx - r - 1.0, cy - r - 1.0, cx + r + 1.0, cy + r + 1.0) else { return };
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                self.blend(x, y, c, cov * a);
            }
        }
    }

    /// A lit, shaded planet.
    pub fn sphere(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        let (cx, cy, r) = (cx * self.k, cy * self.k, r * self.k);
        let Some((x0, y0, x1, y1)) = self.bbox(cx - r - 1.0, cy - r - 1.0, cx + r + 1.0, cy + r + 1.0) else { return };
        let l = [-0.55f32, -0.6, 0.58];
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt();
                let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                if cov <= 0.0 {
                    continue;
                }
                let (nx, ny) = (dx / r.max(0.5), dy / r.max(0.5));
                let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
                let lambert = (nx * l[0] + ny * l[1] + nz * l[2]).max(0.0);
                let shade = 0.18 + 0.95 * lambert;
                let spec = lambert.powi(12) * 0.5;
                let col = mix(scale(c, shade), [255.0, 255.0, 255.0], spec);
                self.blend(x, y, col, cov);
            }
        }
    }

    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, width: f32, c: Rgb, a: f32) {
        let (cx, cy, r, width) = (cx * self.k, cy * self.k, r * self.k, width * self.k);
        let o = r + width;
        let Some((x0, y0, x1, y1)) = self.bbox(cx - o - 1.0, cy - o - 1.0, cx + o + 1.0, cy + o + 1.0) else { return };
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let cov = (width / 2.0 + 0.5 - (d - r).abs()).clamp(0.0, 1.0);
                self.blend(x, y, c, cov * a);
            }
        }
    }

    /// Additive radial glow.
    pub fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, intensity: f32) {
        let (cx, cy, r) = (cx * self.k, cy * self.k, r * self.k);
        let Some((x0, y0, x1, y1)) = self.bbox(cx - r, cy - r, cx + r, cy + r) else { return };
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt() / r.max(0.3);
                if d < 1.0 {
                    self.add(x, y, c, (1.0 - d).powi(2) * intensity);
                }
            }
        }
    }

    /// Anti-aliased line; `dash` > 0 gives dashes of that many pixels.
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, c: Rgb, a: f32, dash: f32) {
        let k = self.k;
        let (x0, y0, x1, y1, width, dash) = (x0 * k, y0 * k, x1 * k, y1 * k, width * k, dash * k);
        let pad = width + 1.0;
        let (lx, hx) = (x0.min(x1), x0.max(x1));
        let (ly, hy) = (y0.min(y1), y0.max(y1));
        let Some((bx0, by0, bx1, by1)) = self.bbox(lx - pad, ly - pad, hx + pad, hy + pad) else { return };
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        let len = len2.sqrt();
        for y in by0..=by1 {
            for x in bx0..=bx1 {
                let (px, py) = (x as f32 + 0.5 - x0, y as f32 + 0.5 - y0);
                let t = ((px * dx + py * dy) / len2).clamp(0.0, 1.0);
                let (qx, qy) = (px - t * dx, py - t * dy);
                let d = (qx * qx + qy * qy).sqrt();
                let mut cov = (width / 2.0 + 0.5 - d).clamp(0.0, 1.0);
                if dash > 0.0 && ((t * len / dash) as i32) % 2 == 1 {
                    cov = 0.0;
                }
                self.blend(x, y, c, cov * a);
            }
        }
    }

    /// Filled polygon, 4x supersampled for smooth edges.
    pub fn polygon(&mut self, pts: &[(f32, f32)], c: Rgb, a: f32) {
        if pts.len() < 3 {
            return;
        }
        let pts: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| (x * self.k, y * self.k)).collect();
        let pts = &pts[..];
        let (mut lx, mut ly, mut hx, mut hy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in pts {
            lx = lx.min(x);
            ly = ly.min(y);
            hx = hx.max(x);
            hy = hy.max(y);
        }
        let Some((x0, y0, x1, y1)) = self.bbox(lx, ly, hx, hy) else { return };
        let inside = |px: f32, py: f32| {
            let mut ins = false;
            let mut j = pts.len() - 1;
            for i in 0..pts.len() {
                let (xi, yi) = pts[i];
                let (xj, yj) = pts[j];
                if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
                    ins = !ins;
                }
                j = i;
            }
            ins
        };
        for y in y0..=y1 {
            for x in x0..=x1 {
                let mut n = 0;
                for (sx, sy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    if inside(x as f32 + sx, y as f32 + sy) {
                        n += 1;
                    }
                }
                if n > 0 {
                    self.blend(x, y, c, a * n as f32 / 4.0);
                }
            }
        }
    }

    pub fn outline(&mut self, pts: &[(f32, f32)], width: f32, c: Rgb, a: f32) {
        for k in 0..pts.len() {
            let (p, q) = (pts[k], pts[(k + 1) % pts.len()]);
            self.line(p.0, p.1, q.0, q.1, width, c, a, 0.0);
        }
    }

    /// Convert to character cells at (cx, cy) on the screen.
    pub fn blit(&self, scr: &mut Screen, cx: i32, cy: i32, truecolor: bool) {
        let (sx, sy) = (self.sx, self.sy);
        let n = (sx * sy) as usize;
        let masks = glyph_masks(sx, sy);
        let cols = self.w / sx;
        let rows = self.h / sy;
        let mut px = vec![[0f32; 3]; n];
        let dot = |a: [f32; 3]| a[0] * a[0] + a[1] * a[1] + a[2] * a[2];
        for row in 0..rows {
            for col in 0..cols {
                let mut sum = [0f32; 3];
                let mut sumsq = 0f32;
                for i in 0..n {
                    let (x, y) = (col * sx + (i as i32 % sx), row * sy + (i as i32 / sx));
                    let p = self.buf[(y * self.w + x) as usize];
                    px[i] = p;
                    for k in 0..3 {
                        sum[k] += p[k];
                        sumsq += p[k] * p[k];
                    }
                }
                // Error of painting the whole cell one color, with a small bias toward it.
                let mut best_err = sumsq - dot(sum) / n as f32 - 40.0 * n as f32;
                let mut best: Option<(char, Rgb, Rgb)> = None;
                if best_err > 0.0 {
                    for (g, mask) in &masks {
                        let mut fs = [0f32; 3];
                        let mut cnt = 0f32;
                        for i in 0..n {
                            if mask[i] {
                                for k in 0..3 {
                                    fs[k] += px[i][k];
                                }
                                cnt += 1.0;
                            }
                        }
                        let bs = [sum[0] - fs[0], sum[1] - fs[1], sum[2] - fs[2]];
                        let err = sumsq - dot(fs) / cnt - dot(bs) / (n as f32 - cnt);
                        if err < best_err {
                            best_err = err;
                            best = Some((*g, scale(fs, 1.0 / cnt), scale(bs, 1.0 / (n as f32 - cnt))));
                        }
                    }
                }
                let (g, fg, bg) = best.unwrap_or((' ', [0.0; 3], scale(sum, 1.0 / n as f32)));
                scr.put_cell(cx + col, cy + row, g, to_color(fg, truecolor), to_color(bg, truecolor), false);
            }
        }
    }

    /// Average color of a character cell (used to give text a matching background).
    pub fn cell_color(&self, col: i32, row: i32) -> Rgb {
        let mut s = [0f32; 3];
        let mut n = 0.0;
        for y in row * self.sy..(row + 1) * self.sy {
            for x in col * self.sx..(col + 1) * self.sx {
                if x >= 0 && y >= 0 && x < self.w && y < self.h {
                    let p = self.buf[(y * self.w + x) as usize];
                    for k in 0..3 {
                        s[k] += p[k];
                    }
                    n += 1.0;
                }
            }
        }
        if n > 0.0 {
            scale(s, 1.0 / n)
        } else {
            s
        }
    }
}

pub fn to_color(c: Rgb, truecolor: bool) -> Color {
    let q = |v: f32| ((v.clamp(0.0, 255.0) / 4.0).round() * 4.0).min(255.0) as u8;
    let (r, g, b) = (q(c[0]), q(c[1]), q(c[2]));
    if truecolor {
        return Color::Rgb { r, g, b };
    }
    // Nearest xterm-256 color from the 6x6x6 cube or the grey ramp.
    const LV: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let near = |v: u8| (0..6).min_by_key(|&i| (LV[i] - v as i32).abs()).unwrap();
    let (ri, gi, bi) = (near(r), near(g), near(b));
    let cube = 16 + 36 * ri + 6 * gi + bi;
    let cube_err = (LV[ri] - r as i32).pow(2) + (LV[gi] - g as i32).pow(2) + (LV[bi] - b as i32).pow(2);
    let avg = (r as i32 + g as i32 + b as i32) / 3;
    let gi2 = ((avg - 8).max(0) / 10).min(23);
    let gv = 8 + gi2 * 10;
    let grey_err = (gv - r as i32).pow(2) + (gv - g as i32).pow(2) + (gv - b as i32).pow(2);
    if grey_err < cube_err {
        Color::AnsiValue(232 + gi2 as u8)
    } else {
        Color::AnsiValue(cube as u8)
    }
}

pub fn truecolor_supported() -> bool {
    let ct = std::env::var("COLORTERM").unwrap_or_default().to_lowercase();
    if ct.contains("truecolor") || ct.contains("24bit") {
        return true;
    }
    matches!(
        std::env::var("TERM_PROGRAM").unwrap_or_default().as_str(),
        "iTerm.app" | "WezTerm" | "ghostty" | "vscode"
    )
}
