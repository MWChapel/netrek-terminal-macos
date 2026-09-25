//! "Blocks" graphics mode: the tactical and galactic views rendered as real
//! color pixels (shaded planets, filled ships, glows, fireballs).

use super::canvas::Screen;
use super::pixels::{mix, rgb, scale, to_color, Pixels, Rgb};
use super::shipart::{engine_points, ship_parts, Part};
use super::{App, Rect};
use crate::consts::*;
use crate::proto::*;
use crossterm::style::Color;
use std::f32::consts::TAU;

const SPACE: Rgb = [0.0, 0.0, 0.0];
const VOID: Rgb = rgb(0x0c0306);
const UNKNOWN: Rgb = rgb(0x4a5060);
const WHITE: Rgb = [255.0, 255.0, 255.0];
const FIRE: Rgb = rgb(0xff8a2a);

pub fn team_rgb(t: Team) -> Rgb {
    match t {
        Team::Fed => rgb(0xf2c94c),
        Team::Rom => rgb(0xff5a4f),
        Team::Kli => rgb(0x4fd46a),
        Team::Ori => rgb(0x3fc5f0),
        Team::Ind => rgb(0xa0a4ab),
    }
}

fn hash(x: i64, y: i64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 32)
}

/// Text labels placed over the pixels without overlapping each other.
struct Labels {
    w: i32,
    h: i32,
    occ: Vec<bool>,
    items: Vec<(i32, i32, String, Color, bool)>,
}

impl Labels {
    fn new(w: i32, h: i32) -> Labels {
        Labels { w, h, occ: vec![false; (w.max(0) * h.max(0)) as usize], items: Vec::new() }
    }

    fn place(&mut self, x: i32, y: i32, text: &str, col: Color, bold: bool) -> bool {
        let n = text.chars().count() as i32;
        let x = x.clamp(0, (self.w - n).max(0));
        if y < 0 || y >= self.h || n > self.w {
            return false;
        }
        let idx = |xx: i32| (y * self.w + xx) as usize;
        // Keep one blank cell between neighbouring labels.
        let (a, b) = ((x - 1).max(0), (x + n).min(self.w - 1));
        if (a..=b).any(|xx| self.occ[idx(xx)]) {
            return false;
        }
        for xx in x..x + n {
            self.occ[idx(xx)] = true;
        }
        self.items.push((x, y, text.to_string(), col, bold));
        true
    }

    fn draw(self, scr: &mut Screen, r: Rect, px: &Pixels, tc: bool) {
        for (x, y, text, col, bold) in self.items {
            for (k, ch) in text.chars().enumerate() {
                let cx = x + k as i32;
                let bg = to_color(scale(px.cell_color(cx, y), 0.85), tc);
                scr.put_cell(r.x + cx, r.y + y, ch, col, bg, bold);
            }
        }
    }
}

impl App {
    /// Logical pixels per character row in blocks mode (2 per column across,
    /// so this reflects the real shape of a cell).
    pub(super) fn logical_per_row(&self) -> f64 {
        2.0 * self.ss.1 as f64 / self.ss.0 as f64
    }

    fn label_color(&self, c: Rgb) -> Color {
        to_color(mix(c, WHITE, 0.15), self.truecolor)
    }

    pub(super) fn draw_tactical_px(&self, scr: &mut Screen, r: Rect, center: (f64, f64), upd: f64) {
        let f = self.frame.as_ref().unwrap();
        let tc = self.truecolor;
        let lpy = self.logical_per_row();
        let (pw, ph) = (r.w * 2, (r.h as f64 * lpy) as i32);
        let mut px = Pixels::new_ss(r.w, r.h, self.ss.0, self.ss.1);
        let (hx, hy) = (pw as f64 / 2.0, ph as f64 / 2.0);
        let to = |x: f64, y: f64| (((x - center.0) / upd + hx) as f32, ((y - center.1) / upd + hy) as f32);
        let (half_w, half_h) = (hx * upd, hy * upd);
        let visible = |x: f64, y: f64, m: f64| (x - center.0).abs() < half_w + m && (y - center.1).abs() < half_h + m;
        let zoom = self.zoom as f32;
        let mut labels = Labels::new(r.w, r.h);

        // Space, with the void beyond the galaxy's edge tinted red.
        px.fill_with(|x, y| {
            let wx = center.0 + (x - hx) * upd;
            let wy = center.1 + (y - hy) * upd;
            if wx < 0.0 || wy < 0.0 || wx > GWIDTH || wy > GWIDTH {
                VOID
            } else {
                SPACE
            }
        });

        // Two layers of stars; the far layer drifts slower (parallax).
        for (layer, cell, par, base) in [(1i64, 18.0 * upd, 0.4, 50.0f32), (2, 13.0 * upd, 1.0, 90.0)] {
            let (lx, ly) = (center.0 * par, center.1 * par);
            let (gx0, gx1) = (((lx - half_w) / cell).floor() as i64, ((lx + half_w) / cell).ceil() as i64);
            let (gy0, gy1) = (((ly - half_h) / cell).floor() as i64, ((ly + half_h) / cell).ceil() as i64);
            for gy in gy0..=gy1 {
                for gx in gx0..=gx1 {
                    let hv = hash(gx * 7 + layer * 1_000_003, gy);
                    if hv % 4 != 0 {
                        continue;
                    }
                    let sx = gx as f64 * cell + ((hv >> 8) % 10_000) as f64 / 10_000.0 * cell;
                    let sy = gy as f64 * cell + ((hv >> 24) % 10_000) as f64 / 10_000.0 * cell;
                    // Only the near layer is anchored in galaxy space; keep it inside the galaxy.
                    if layer == 2 && !(0.0..=GWIDTH).contains(&sx) || layer == 2 && !(0.0..=GWIDTH).contains(&sy) {
                        continue;
                    }
                    let x = ((sx - lx) / upd + hx) as f32;
                    let y = ((sy - ly) / upd + hy) as f32;
                    let b = base * (0.5 + ((hv >> 40) % 100) as f32 / 100.0);
                    let tint = match (hv >> 50) % 5 {
                        0 => [b * 0.8, b * 0.9, b * 1.25],
                        1 => [b * 1.2, b * 1.05, b * 0.8],
                        _ => [b, b, b * 1.05],
                    };
                    let wx = center.0 + (x as f64 - hx) * upd;
                    let wy = center.1 + (y as f64 - hy) * upd;
                    if wx < 0.0 || wy < 0.0 || wx > GWIDTH || wy > GWIDTH {
                        continue;
                    }
                    px.dot(x, y, tint, 1.0);
                    if layer == 2 && hv % 29 == 0 {
                        px.glow(x + 0.5, y + 0.5, 1.8, tint, 0.5);
                    }
                }
            }
        }

        // Edge of the galaxy.
        let corners = [(0.0, 0.0), (GWIDTH, 0.0), (GWIDTH, GWIDTH), (0.0, GWIDTH)];
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            let (p, q) = (to(a.0, a.1), to(b.0, b.1));
            let c = |v: f32, m: i32| v.clamp(-10.0, m as f32 + 10.0);
            px.line(c(p.0, pw), c(p.1, ph), c(q.0, pw), c(q.1, ph), 1.0, rgb(0xd02020), 1.0, 0.0);
        }

        // Planets.
        let pr = ((750.0 / upd) as f32).max(2.6);
        let full_names = zoom <= 1.3;
        let mut planet_labels = Vec::new();
        for (k, def) in PLANETS.iter().enumerate() {
            if !visible(def.x, def.y, 3000.0) {
                continue;
            }
            let info = &f.planets[k];
            let col = if info.known { team_rgb(info.owner) } else { UNKNOWN };
            let (x, y) = to(def.x, def.y);
            px.glow(x, y, pr * 2.1, col, 0.22);
            px.sphere(x, y, pr.max(1.2), col);
            if info.flags & PL_HOME != 0 {
                px.ring(x, y, pr * 1.55, 0.7, col, 0.55);
            }
            let (cx, cy) = ((x / 2.0) as i32, ((y + pr) as f64 / lpy) as i32 + 1);
            let name: String = if full_names { def.name.to_string() } else { def.name.chars().take(3).collect() };
            let lc = if info.known { self.label_color(col) } else { to_color(UNKNOWN, tc) };
            planet_labels.push((cx - name.chars().count() as i32 / 2, cy, name, lc, info.known && info.armies > 4));
            if info.known && full_names {
                let mut tag = info.armies.to_string();
                for (flag, ch) in [(PL_REPAIR, 'R'), (PL_FUEL, 'F'), (PL_AGRI, 'A')] {
                    if info.flags & flag != 0 {
                        tag.push(ch);
                    }
                }
                planet_labels.push((cx - tag.len() as i32 / 2, cy + 1, tag, to_color(rgb(0x8a90a0), tc), false));
            }
        }

        // Tractor / pressor beams.
        for p in f.players.iter().filter(|p| p.state == PState::Alive) {
            if let Some(t) = p.tractor_target.and_then(|t| f.players.iter().find(|q| q.id == t)) {
                let (a, b) = (to(p.x as f64, p.y as f64), to(t.x as f64, t.y as f64));
                let col = if p.flags & pf::PRESSOR != 0 { rgb(0xd070ff) } else { rgb(0x60ff90) };
                px.line(a.0, a.1, b.0, b.1, 1.0, col, 0.75, 2.0);
            }
        }

        // Phasers: a white-hot core with a colored halo.
        for ph in &f.phasers {
            let owner = f.players.iter().find(|p| p.id == ph.owner);
            let col = owner.map_or(WHITE, |p| team_rgb(p.team));
            let (a, b) = (to(ph.x1 as f64, ph.y1 as f64), to(ph.x2 as f64, ph.y2 as f64));
            let alpha = if ph.hit { 1.0 } else { 0.6 };
            px.line(a.0, a.1, b.0, b.1, 3.0, col, 0.35 * alpha, 0.0);
            px.line(a.0, a.1, b.0, b.1, 1.0, mix(col, WHITE, 0.6), alpha, 0.0);
            if ph.hit {
                px.glow(b.0, b.1, 4.0, mix(col, WHITE, 0.5), 0.9);
            }
        }

        // Torpedoes and plasma.
        for t in &f.torps {
            if !visible(t.x as f64, t.y as f64, 2000.0) {
                continue;
            }
            let (x, y) = to(t.x as f64, t.y as f64);
            let col = if t.owner == self.slot { rgb(0x9fe8ff) } else { team_rgb(t.team) };
            if t.explode > 0 {
                let e = t.explode as f32;
                let rad = (DAMDIST as f32 / upd as f32) * (0.25 + e * 0.12);
                px.glow(x, y, rad, FIRE, 1.1 - e * 0.17);
                px.glow(x, y, rad * 0.45, rgb(0xfff0c0), 0.9 - e * 0.15);
            } else if t.kind == TorpKind::Plasma {
                let pulse = 0.8 + 0.2 * ((f.tick as f32) * 1.3).sin();
                px.glow(x, y, 5.0, col, 0.9 * pulse);
                px.disc(x, y, 1.3, mix(col, WHITE, 0.6), 1.0);
            } else {
                px.glow(x, y, 3.5, col, 0.8);
                px.disc(x, y, 0.9, mix(col, WHITE, 0.5), 1.0);
            }
        }

        // Ships.
        let sr = (8.0 / zoom.sqrt()).clamp(5.0, 11.0);
        // Draw (and label) our own ship last-on-top but first in label priority.
        let mut order: Vec<&PlayerInfo> = f.players.iter().collect();
        order.sort_by_key(|p| p.id != self.slot);
        for p in order {
            if matches!(p.state, PState::Outfit | PState::Dead) || p.fuzzy || !visible(p.x as f64, p.y as f64, 2000.0) {
                continue;
            }
            let (x, y) = to(p.x as f64, p.y as f64);
            let is_me = p.id == self.slot;
            if p.state == PState::Exploding {
                let fr = p.explode_frame as f32;
                let fade = (1.0 - fr / 11.0).max(0.0);
                let rad = 3.0 + fr * 1.6;
                px.glow(x, y, rad * 1.6, rgb(0xff4020), fade);
                px.glow(x, y, rad, FIRE, 1.2 * fade);
                px.glow(x, y, rad * 0.5, rgb(0xfff6d0), 1.4 * fade);
                px.ring(x, y, rad * 1.2, 0.8, rgb(0xffc060), 0.6 * fade);
                for k in 0..14 {
                    let a = k as f32 / 14.0 * TAU + (p.id as f32);
                    let d = fr * (2.0 + (k % 3) as f32 * 0.7);
                    px.glow(x + a.cos() * d, y + a.sin() * d, 1.4, rgb(0xffb050), fade);
                }
                continue;
            }
            let team = team_rgb(p.team);
            let cloaked = p.flags & pf::CLOAK != 0;
            let a = p.dir as f32 * TAU / 256.0;
            let (sa, ca) = (a.sin(), a.cos());
            let alpha = if cloaked { 0.3 } else { 1.0 };
            let sr = if p.ship == ShipType::Starbase { sr * 1.5 } else { sr };
            let rot = |lx: f32, ly: f32| (x + (lx * ca - ly * sa) * sr, y + (lx * sa + ly * ca) * sr);
            // Engine glow grows with speed.
            if p.speed > 0 && !cloaked {
                let k = p.speed as f32 / p.ship.stats().max_speed as f32;
                for (ex, ey) in engine_points(p.team, p.ship) {
                    let (gx, gy) = rot(ex, ey + 0.05);
                    px.glow(gx, gy, 1.2 + 1.8 * k, rgb(0xff9a40), 0.35 + 0.6 * k);
                }
            }
            let body = if is_me { mix(team, WHITE, 0.45) } else { team };
            let edge = if is_me { WHITE } else { mix(team, WHITE, 0.3) };
            for part in ship_parts(p.team, p.ship) {
                match part {
                    Part::Poly { pts, fill } => {
                        let pts: Vec<(f32, f32)> = pts.iter().map(|&(lx, ly)| rot(lx, ly)).collect();
                        if fill {
                            px.polygon(&pts, body, alpha);
                        }
                        px.outline(&pts, 0.35, edge, alpha * 0.8);
                    }
                    Part::Hole { pts } => {
                        let pts: Vec<(f32, f32)> = pts.iter().map(|&(lx, ly)| rot(lx, ly)).collect();
                        px.polygon(&pts, SPACE, 1.0);
                    }
                    Part::Circle { c, r, fill } => {
                        let (cx, cy) = rot(c.0, c.1);
                        if fill {
                            px.disc(cx, cy, r * sr, body, alpha);
                        } else {
                            px.ring(cx, cy, r * sr, 0.35, edge, alpha);
                        }
                    }
                    Part::Line { a, b } => {
                        let (a, b) = (rot(a.0, a.1), rot(b.0, b.1));
                        px.line(a.0, a.1, b.0, b.1, 0.45, body, alpha, 0.0);
                    }
                }
            }
            if p.flags & pf::SHIELD != 0 && !cloaked {
                let scol = if is_me {
                    let frac = (f.me_info.shield as f32 / p.ship.stats().max_shield as f32).clamp(0.0, 1.0);
                    if frac > 0.6 {
                        rgb(0x5fb4ff)
                    } else if frac > 0.3 {
                        rgb(0xffd24a)
                    } else {
                        rgb(0xff4a3a)
                    }
                } else {
                    team
                };
                px.ring(x, y, sr * 1.3 + 1.0, 0.8, scol, 0.6);
            }
            let tag = slot_char(p.id).to_string();
            let lc = if cloaked { to_color(UNKNOWN, tc) } else if is_me { Color::White } else { self.label_color(team) };
            let (lx, ly) = (((x + sr * 1.3 + 2.0) / 2.0) as i32, ((y - sr) as f64 / lpy) as i32);
            labels.place(lx, ly, &tag, lc, is_me);
        }

        // Mouse pointer.
        if let Some((cx, cy)) = self.pointer {
            if r.contains(cx, cy) {
                let (x, y) = (((cx - r.x) * 2 + 1) as f32, (((cy - r.y) as f64 + 0.5) * lpy) as f32);
                let col = rgb(0xff60d0);
                px.line(x - 5.0, y, x - 2.0, y, 1.0, col, 0.9, 0.0);
                px.line(x + 2.0, y, x + 5.0, y, 1.0, col, 0.9, 0.0);
                px.line(x, y - 5.0, x, y - 2.0, 1.0, col, 0.9, 0.0);
                px.line(x, y + 2.0, x, y + 5.0, 1.0, col, 0.9, 0.0);
            }
        }

        px.blit(scr, r.x, r.y, tc);
        for (x, y, text, col, bold) in planet_labels {
            labels.place(x, y, &text, col, bold);
        }
        labels.draw(scr, r, &px, tc);
    }

    pub(super) fn draw_galactic_px(&self, scr: &mut Screen, r: Rect, center: (f64, f64), view_w: f64, view_h: f64) {
        let f = self.frame.as_ref().unwrap();
        let tc = self.truecolor;
        let lpy = self.logical_per_row();
        let (pw, ph) = (r.w * 2, (r.h as f64 * lpy) as i32);
        let mut px = Pixels::new_ss(r.w, r.h, self.ss.0, self.ss.1);
        let (sx, sy) = (pw as f64 / GWIDTH, ph as f64 / GWIDTH);
        let to = |x: f64, y: f64| ((x * sx) as f32, (y * sy) as f32);

        // Black space with a fixed star field.
        px.fill_with(|x, y| {
            let hv = hash((x * 2.0) as i64 * 31 + 7, (y * 2.0) as i64 * 17 + 3);
            if hv % 300 == 0 {
                let b = 50.0 + (hv >> 20) as f32 % 110.0;
                [b, b, b * 1.05]
            } else {
                [0.0, 0.0, 0.0]
            }
        });

        // Tactical window outline.
        let (a, b) = (to(center.0 - view_w / 2.0, center.1 - view_h / 2.0), to(center.0 + view_w / 2.0, center.1 + view_h / 2.0));
        let wc = rgb(0x606878);
        px.line(a.0, a.1, b.0, a.1, 1.0, wc, 0.45, 2.0);
        px.line(b.0, a.1, b.0, b.1, 1.0, wc, 0.45, 2.0);
        px.line(b.0, b.1, a.0, b.1, 1.0, wc, 0.45, 2.0);
        px.line(a.0, b.1, a.0, a.1, 1.0, wc, 0.45, 2.0);

        let mut labels = Labels::new(r.w, r.h);
        let mut planet_labels = Vec::new();
        let gr = (pw as f32 / 55.0).clamp(1.3, 4.5);
        for (k, def) in PLANETS.iter().enumerate() {
            let info = &f.planets[k];
            let col = if info.known { team_rgb(info.owner) } else { UNKNOWN };
            let (x, y) = to(def.x, def.y);
            px.sphere(x, y, gr, col);
            if info.flags & PL_HOME != 0 {
                px.ring(x, y, gr + 1.2, 0.6, col, 0.6);
            }
            let abbr: String = def.name.chars().take(3).collect();
            let lc = if info.known { self.label_color(col) } else { to_color(UNKNOWN, tc) };
            planet_labels.push(((x / 2.0) as i32 - 1, ((y + gr) as f64 / lpy) as i32 + 1, abbr, lc, info.known && info.armies > 4));
        }
        for p in f.players.iter().filter(|p| p.state == PState::Alive) {
            let (x, y) = to(p.x as f64, p.y as f64);
            let is_me = p.id == self.slot;
            if p.fuzzy {
                labels.place((x / 2.0) as i32, (y as f64 / lpy) as i32, "??", to_color(UNKNOWN, tc), false);
                continue;
            }
            let col = team_rgb(p.team);
            px.glow(x, y, 2.5, col, 0.8);
            if is_me {
                px.ring(x, y, 2.6, 0.8, WHITE, 0.9);
            }
            let tag = format!("{}{}", p.team.letter(), slot_char(p.id));
            let lc = if is_me { Color::White } else { self.label_color(col) };
            labels.place((x / 2.0) as i32 + 1, (y as f64 / lpy) as i32, &tag, lc, true);
        }
        if let Some((cx, cy)) = self.pointer {
            if r.contains(cx, cy) {
                let (x, y) = (((cx - r.x) * 2 + 1) as f32, (((cy - r.y) as f64 + 0.5) * lpy) as f32);
                px.ring(x, y, 2.0, 0.8, rgb(0xff60d0), 0.9);
            }
        }
        px.blit(scr, r.x, r.y, tc);
        for (x, y, text, col, bold) in planet_labels {
            labels.place(x, y, &text, col, bold);
        }
        labels.draw(scr, r, &px, tc);
    }
}
