//! "Vector" graphics mode: the tactical and galactic maps drawn as real
//! pixel images with thin anti-aliased lines, outlined planets and small
//! text, like the classic X11 client. Sent to the terminal as SIXEL.

use super::palette::{mix, rgb, scale, Rgb};
use super::vg::Canvas;
use super::palette::{callsign, faction_rgb, planet_rgb, player_rgb, ship_size_units, team_rgb, torp_rgb, LOOT};
use super::shipart::{engine_points, ship_parts, Part};
use super::render::HELP;
use super::{App, Popup};
use crate::consts::*;
use crate::proto::*;
use std::f32::consts::TAU;

const WHITE: Rgb = [255.0, 255.0, 255.0];
const GREY: Rgb = rgb(0x8a8f99);
const DARK_GREY: Rgb = rgb(0x50545c);

/// A phaser beam: a white-hot core in a coloured halo with a shimmer wound
/// around it, a muzzle flash, energy pulses racing down the beam, and a
/// flare of sparks where it hits. It flashes on its first frame, then
/// narrows and fades over its life (`age` counts frames, about six).
fn draw_phaser(px: &mut Canvas, a: (f32, f32), b: (f32, f32), col: Rgb, hit: bool, age: u8, k: f32, seed: u64) {
    let t = (age as f32 / 5.0).min(1.0);
    let fade = 1.0 - t * 0.85;
    let flash = if age == 0 { 1.0 } else { 0.0 };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let (nx, ny) = (-dy / len, dx / len);
    let halo = mix(col, WHITE, 0.15);
    let w = k * (1.0 - t * 0.6);

    // Halo, glow and core.
    px.line(a.0, a.1, b.0, b.1, 12.0 * w, halo, 0.14 * fade + 0.12 * flash, 0.0);
    px.line(a.0, a.1, b.0, b.1, 5.0 * w, halo, 0.40 * fade, 0.0);
    px.line(a.0, a.1, b.0, b.1, 2.0 * w, mix(col, WHITE, 0.6), 0.8 * fade, 0.0);
    px.line(a.0, a.1, b.0, b.1, 0.9 * k, WHITE, fade, 0.0);

    // A shimmering wave wound around the core, shifting every frame.
    let waves = (len / (12.0 * k)).max(2.0) as usize;
    let steps = waves * 10;
    let pts: Vec<(f32, f32)> = (0..=steps)
        .map(|i| {
            let s = i as f32 / steps as f32;
            let taper = (s * std::f32::consts::PI).sin().max(0.25);
            let off = 3.2 * w * taper * (s * waves as f32 * TAU + age as f32 * 1.9).sin();
            (a.0 + dx * s + nx * off, a.1 + dy * s + ny * off)
        })
        .collect();
    px.polyline(&pts, false, 1.0 * k, mix(col, WHITE, 0.4), 0.75 * fade, 0.0);

    // Energy pulses racing from the emitter to the target.
    for j in 0..3 {
        let s = (age as f32 * 0.31 + j as f32 / 3.0).fract();
        px.glow(a.0 + dx * s, a.1 + dy * s, 5.0 * k, WHITE, 0.55 * fade);
    }

    // Muzzle flash.
    px.glow(a.0, a.1, (9.0 + 9.0 * flash) * k, mix(col, WHITE, 0.5), 0.7 * fade);

    if hit {
        // Impact flare, a shock ring, and sparks flying outward.
        let r = (8.0 + 10.0 * t + 8.0 * flash) * k;
        px.glow(b.0, b.1, r * 1.7, mix(col, WHITE, 0.3), 0.75 * fade);
        px.glow(b.0, b.1, r * 0.6, WHITE, 0.95 * fade);
        px.ring(b.0, b.1, r * (0.5 + t), k, mix(col, WHITE, 0.5), 0.7 * (1.0 - t));
        let mut h = seed | 1;
        for _ in 0..9 {
            h = h.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let ang = (h >> 33) as f32 / (1u64 << 31) as f32 * TAU;
            let spd = 0.5 + ((h >> 13) & 0xff) as f32 / 255.0;
            let r0 = r * 0.3 + age as f32 * 4.5 * k * spd;
            let r1 = r0 + 5.0 * k * spd * (1.0 - t * 0.5);
            let (c, s) = (ang.cos(), ang.sin());
            px.line(b.0 + c * r0, b.1 + s * r0, b.0 + c * r1, b.1 + s * r1, k, mix(col, WHITE, 0.7), 0.9 * fade, 0.0);
        }
    } else {
        // A miss dissipates into space.
        px.glow(b.0, b.1, 7.0 * k, halo, 0.4 * fade);
    }
}

fn hash(x: i64, y: i64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 32)
}

impl App {
    /// Label size: follows the map size, but never below what's legible.
    fn font_px(&self, map_px: i32) -> f32 {
        (map_px as f32 / 55.0).max(self.cell_px.1 as f32 * 0.6).clamp(9.0, 18.0)
    }

    pub(super) fn draw_tactical_vec(&self, pw: i32, ph: i32, center: (f64, f64), upd: f64) -> Canvas {
        let f = self.frame.as_ref().unwrap();
        let tr = &self.text;
        let fs = self.font_px(pw);
        let mut px = Canvas::new(pw, ph, [0.0, 0.0, 0.0]);
        let (hx, hy) = (pw as f64 / 2.0, ph as f64 / 2.0);
        let to = |x: f64, y: f64| (((x - center.0) / upd + hx) as f32, ((y - center.1) / upd + hy) as f32);
        let (half_w, half_h) = (hx * upd, hy * upd);
        let visible = |x: f64, y: f64, m: f64| (x - center.0).abs() < half_w + m && (y - center.1).abs() < half_h + m;
        let u = |units: f64| (units / upd) as f32; // galaxy units -> pixels

        // Sparse stars, fixed in galaxy space.
        let cell = 2600.0;
        let (gx0, gx1) = (((center.0 - half_w) / cell).floor() as i64, ((center.0 + half_w) / cell).ceil() as i64);
        let (gy0, gy1) = (((center.1 - half_h) / cell).floor() as i64, ((center.1 + half_h) / cell).ceil() as i64);
        for gy in gy0..=gy1 {
            for gx in gx0..=gx1 {
                let hv = hash(gx, gy);
                if hv % 2 != 0 {
                    continue;
                }
                let sx = gx as f64 * cell + ((hv >> 8) % 10_000) as f64 / 10_000.0 * cell;
                let sy = gy as f64 * cell + ((hv >> 24) % 10_000) as f64 / 10_000.0 * cell;
                if !(0.0..=GWIDTH).contains(&sx) || !(0.0..=GWIDTH).contains(&sy) {
                    continue;
                }
                let (x, y) = to(sx, sy);
                let b = 90.0 + ((hv >> 40) % 120) as f32;
                px.dot(x, y, [b, b, b], 1.0);
            }
        }

        // Edge of the galaxy.
        let corners = [(0.0, 0.0), (GWIDTH, 0.0), (GWIDTH, GWIDTH), (0.0, GWIDTH)];
        for k in 0..4 {
            let (a, b) = (to(corners[k].0, corners[k].1), to(corners[(k + 1) % 4].0, corners[(k + 1) % 4].1));
            let c = |v: f32, m: i32| v.clamp(-10.0, m as f32 + 10.0);
            px.line(c(a.0, pw), c(a.1, ph), c(b.0, pw), c(b.1, ph), 1.2, rgb(0xe02020), 1.0, 0.0);
        }

        // Planets: outlined circles with names underneath.
        let pr = u(ORBDIST * 0.75).clamp(5.0, 60.0);
        for (k, def) in PLANETS.iter().enumerate() {
            if !visible(def.x, def.y, 4000.0) {
                continue;
            }
            let info = &f.planets[k];
            let col = if info.known { planet_rgb(info) } else { GREY };
            let (x, y) = to(def.x, def.y);
            px.disc(x, y, pr, scale(col, 0.12), 1.0);
            px.ring(x, y, pr, 1.3, col, 1.0);
            if info.flags & PL_HOME != 0 {
                px.ring(x, y, pr - 3.0, 1.0, col, 0.7);
            }
            if info.known {
                let mut tags = String::new();
                for (flag, ch) in [(PL_REPAIR, 'R'), (PL_FUEL, 'F'), (PL_AGRI, 'A')] {
                    if info.flags & flag != 0 {
                        tags.push(ch);
                    }
                }
                if info.tribbles {
                    tags.push('T');
                    px.ring_dashed(x, y, pr + 4.0, 1.5, faction_rgb(Faction::Tribbles), 0.9, 3.0);
                }
                px.text_centered(tr, x, y + fs * 0.35, &info.armies.to_string(), fs * 0.9, col);
                if !tags.is_empty() && pr > fs {
                    px.text_centered(tr, x, y + fs * 1.2, &tags, fs * 0.7, scale(col, 0.8));
                }
            } else {
                px.text_centered(tr, x, y + fs * 0.35, "?", fs, col);
            }
            px.text_centered(tr, x, y + pr + fs * 1.1, def.name, fs, col);
        }

        // Tholian webs: glowing crystalline strands.
        for w in &f.webs {
            let (a, b) = (to(w.x1 as f64, w.y1 as f64), to(w.x2 as f64, w.y2 as f64));
            px.line(a.0, a.1, b.0, b.1, 3.0, rgb(0xffa040), 0.18, 0.0);
            px.line(a.0, a.1, b.0, b.1, 1.0, rgb(0xffd090), 0.9, 0.0);
        }

        // Armies spilled from Ferengi wrecks.
        for l in &f.loot {
            let (x, y) = to(l.x as f64, l.y as f64);
            let r = u(300.0).clamp(3.0, 14.0);
            px.glow(x, y, r * 2.5, LOOT, 0.4);
            px.disc(x, y, r, scale(LOOT, 0.35), 1.0);
            px.ring(x, y, r, 1.3, LOOT, 1.0);
            px.text_centered(tr, x, y + fs * 0.35, &l.armies.to_string(), fs * 0.9, LOOT);
        }

        // Tractor / pressor beams.
        for p in f.players.iter().filter(|p| p.state == PState::Alive) {
            if let Some(t) = p.tractor_target.and_then(|t| f.players.iter().find(|q| q.id == t)) {
                let (a, b) = (to(p.x as f64, p.y as f64), to(t.x as f64, t.y as f64));
                let col = if p.flags & pf::PRESSOR != 0 { rgb(0xd070ff) } else { rgb(0x60ff90) };
                px.line(a.0, a.1, b.0, b.1, 1.0, col, 0.9, 3.0);
            }
        }

        // Phasers.
        let k = (fs / 13.0).clamp(0.8, 2.2);
        for ph in &f.phasers {
            let owner = f.players.iter().find(|p| p.id == ph.owner);
            let col = owner.map_or(WHITE, player_rgb);
            let (a, b) = (to(ph.x1 as f64, ph.y1 as f64), to(ph.x2 as f64, ph.y2 as f64));
            let age = self.phaser_age.get(&super::phaser_key(ph)).copied().unwrap_or(0);
            let seed = hash(ph.x2 as i64 ^ ph.owner as i64, ph.y2 as i64);
            draw_phaser(&mut px, a, b, col, ph.hit, age, k, seed);
        }

        // Torpedoes.
        for t in &f.torps {
            if !visible(t.x as f64, t.y as f64, 3000.0) {
                continue;
            }
            let (x, y) = to(t.x as f64, t.y as f64);
            let col = if t.owner == self.slot { mix(team_rgb(t.team), WHITE, 0.5) } else { torp_rgb(f, t) };
            if t.explode > 0 {
                let e = t.explode as f32;
                let fade = 1.0 - e / 7.0;
                px.ring(x, y, u(EXPDIST) + e * u(DAMDIST) * 0.12, 1.0, rgb(0xffd060), fade);
                px.ring(x, y, (u(EXPDIST) + e * u(DAMDIST) * 0.12) * 0.6, 1.0, rgb(0xff6030), fade);
            } else if t.kind == TorpKind::Plasma {
                px.ring(x, y, 4.0, 1.2, col, 1.0);
                px.disc(x, y, 1.8, mix(col, WHITE, 0.5), 1.0);
            } else {
                // The classic little torp cross.
                px.line(x - 2.0, y, x + 2.0, y, 1.0, col, 1.0, 0.0);
                px.line(x, y - 2.0, x, y + 2.0, 1.0, col, 1.0, 0.0);
            }
        }

        // Ships.
        let sr = u(520.0).clamp(7.0, 26.0);
        let mut order: Vec<&PlayerInfo> = f.players.iter().collect();
        order.sort_by_key(|p| p.id == self.slot); // draw ourselves on top
        for p in order {
            if matches!(p.state, PState::Outfit | PState::Dead) || p.fuzzy || !visible(p.x as f64, p.y as f64, 3000.0) {
                continue;
            }
            let (x, y) = to(p.x as f64, p.y as f64);
            let is_me = p.id == self.slot;
            let team = player_rgb(p);
            if p.state == PState::Exploding {
                let fr = p.explode_frame as f32;
                let fade = (1.0 - fr / 11.0).max(0.0);
                for (k, c) in [(1.0, rgb(0xfff0a0)), (1.7, rgb(0xffa040)), (2.4, rgb(0xff4020))] {
                    px.ring(x, y, sr * 0.4 * fr * k * 0.5 + 2.0, 1.0, c, fade);
                }
                for k in 0..12 {
                    let a = k as f32 / 12.0 * TAU + p.id as f32;
                    let (r0, r1) = (sr * 0.3 * fr, sr * 0.3 * fr + sr * 0.5);
                    px.line(x + a.cos() * r0, y + a.sin() * r0, x + a.cos() * r1, y + a.sin() * r1, 1.0, rgb(0xffc060), fade, 0.0);
                }
                continue;
            }
            let cloaked = p.flags & pf::CLOAK != 0;
            let col = if is_me { WHITE } else { team };
            let alpha = if cloaked { 0.35 } else { 1.0 };
            let dash = if cloaked { 2.0 } else { 0.0 };
            let a = p.dir as f32 * TAU / 256.0;
            let (sa, ca) = (a.sin(), a.cos());
            let sr = if ship_size_units(p.ship) > 0.0 {
                u(ship_size_units(p.ship))
            } else if p.ship == ShipType::Starbase {
                sr * 1.5
            } else {
                sr
            };
            let rot = |lx: f32, ly: f32| (x + (lx * ca - ly * sa) * sr, y + (lx * sa + ly * ca) * sr);
            if p.speed > 0 && !cloaked {
                let k = p.speed as f32 / p.ship.stats().max_speed as f32;
                for (ex, ey) in engine_points(p.team, p.ship, p.faction) {
                    let (gx, gy) = rot(ex, ey + 0.05);
                    px.glow(gx, gy, sr * (0.25 + 0.35 * k), rgb(0xff9a40), 0.35 + 0.5 * k);
                }
            }
            if p.ship == ShipType::VgerCloud {
                // A luminous cloud around a bright core.
                px.glow(x, y, sr, rgb(0x3060c0), 0.35);
                px.glow(x, y, sr * 0.5, rgb(0x70a8ff), 0.35);
                for (k, r) in [0.95f32, 0.75, 0.55].iter().enumerate() {
                    px.ring_dashed(x, y, sr * r, 1.0, rgb(0x80b8ff), 0.35, 3.0 + k as f32 * 2.0);
                }
                px.glow(x, y, u(1200.0), rgb(0xeaf4ff), 0.9);
            }
            let fill = scale(team, if is_me { 0.6 } else { 0.42 });
            let edge = if is_me { WHITE } else { mix(team, WHITE, 0.25) };
            for part in ship_parts(p.team, p.ship, p.faction) {
                match part {
                    Part::Poly { pts, fill: filled } => {
                        let pts: Vec<(f32, f32)> = pts.iter().map(|&(lx, ly)| rot(lx, ly)).collect();
                        if filled && !cloaked {
                            px.fill_poly(&pts, fill, alpha);
                        }
                        px.polyline(&pts, true, 1.1, edge, alpha, dash);
                    }
                    Part::Hole { pts } => {
                        let pts: Vec<(f32, f32)> = pts.iter().map(|&(lx, ly)| rot(lx, ly)).collect();
                        if !cloaked {
                            px.fill_poly(&pts, [0.0, 0.0, 0.0], 1.0);
                        }
                        px.polyline(&pts, true, 1.0, edge, alpha * 0.8, dash);
                    }
                    Part::Circle { c, r, fill: filled } => {
                        let (cx, cy) = rot(c.0, c.1);
                        if filled && !cloaked {
                            px.disc(cx, cy, r * sr, fill, alpha);
                        }
                        px.ring_dashed(cx, cy, r * sr, 1.1, edge, alpha, dash);
                    }
                    Part::Line { a, b } => {
                        let (a, b) = (rot(a.0, a.1), rot(b.0, b.1));
                        px.line(a.0, a.1, b.0, b.1, 1.1, edge, alpha, dash);
                    }
                }
            }
            let invulnerable = matches!(p.ship, ShipType::VgerCloud | ShipType::WhaleProbe);
            if p.flags & pf::SHIELD != 0 && !cloaked && !invulnerable {
                let scol = if is_me {
                    let frac = (f.me_info.shield as f32 / p.ship.stats().max_shield as f32).clamp(0.0, 1.0);
                    if frac > 0.6 {
                        rgb(0x40a0ff)
                    } else if frac > 0.3 {
                        rgb(0xffd24a)
                    } else {
                        rgb(0xff4a3a)
                    }
                } else {
                    team
                };
                px.ring(x, y, sr * 1.45, 1.0, scol, 0.9);
            }
            let tag = p.faction.map_or(slot_char(p.id).to_string(), |f| f.tag().to_string());
            if ship_size_units(p.ship) > 0.0 {
                // Monsters: name centred above them.
                px.text_centered(tr, x, y - sr * if p.ship == ShipType::VgerCloud { 0.25 } else { 1.05 } - 3.0, &tag, fs, col);
            } else {
                px.text(tr, x + sr * 1.5 + 1.0, y - sr * 0.6, &tag, fs, if cloaked { GREY } else { col });
            }
        }

        // Mouse pointer.
        if let Some((mx, my)) = self.pointer_px(true) {
            let c = rgb(0xff60d0);
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                px.line(mx + dx * 3.0, my + dy * 3.0, mx + dx * 8.0, my + dy * 8.0, 1.0, c, 0.9, 0.0);
            }
        }
        px
    }

    pub(super) fn draw_galactic_vec(&self, pw: i32, ph: i32, center: (f64, f64), view_w: f64, view_h: f64) -> Canvas {
        let f = self.frame.as_ref().unwrap();
        let tr = &self.text;
        let fs = (self.font_px(pw) * 0.9).max(9.0);
        let mut px = Canvas::new(pw, ph, [0.0, 0.0, 0.0]);
        let (sx, sy) = (pw as f64 / GWIDTH, ph as f64 / GWIDTH);
        let to = |x: f64, y: f64| ((x * sx) as f32, (y * sy) as f32);

        for k in 0..(pw * ph / 1500) as i64 {
            let hv = hash(k, 99);
            let (x, y) = ((hv % pw as u64) as i32, ((hv >> 20) % ph as u64) as i32);
            let b = if hv >> 40 & 3 == 0 { 150.0 } else { 80.0 };
            px.dot(x as f32, y as f32, [b, b, b], 1.0);
        }

        // Where the tactical window is looking.
        let (a, b) = (to(center.0 - view_w / 2.0, center.1 - view_h / 2.0), to(center.0 + view_w / 2.0, center.1 + view_h / 2.0));
        for (p, q) in [((a.0, a.1), (b.0, a.1)), ((b.0, a.1), (b.0, b.1)), ((b.0, b.1), (a.0, b.1)), ((a.0, b.1), (a.0, a.1))] {
            px.line(p.0, p.1, q.0, q.1, 1.0, DARK_GREY, 0.9, 3.0);
        }

        for w in &f.webs {
            let (a, b) = (to(w.x1 as f64, w.y1 as f64), to(w.x2 as f64, w.y2 as f64));
            px.line(a.0, a.1, b.0, b.1, 1.0, rgb(0xffa040), 0.6, 0.0);
        }

        let gr = (pw as f32 / 70.0).clamp(3.0, 9.0);
        for (k, def) in PLANETS.iter().enumerate() {
            let info = &f.planets[k];
            let col = if info.known { planet_rgb(info) } else { GREY };
            let (x, y) = to(def.x, def.y);
            px.disc(x, y, gr, scale(col, 0.15), 1.0);
            px.ring(x, y, gr, 1.1, col, 1.0);
            if info.flags & PL_HOME != 0 {
                px.ring(x, y, gr + 2.5, 1.0, col, 0.7);
            }
            if !info.known {
                px.text_centered(tr, x, y + fs * 0.35, "?", fs * 0.8, col);
            } else if info.armies > 4 {
                px.disc(x, y, gr * 0.35, col, 1.0);
            }
            let name: String = def.name.chars().take(3).collect();
            px.text_centered(tr, x, y + gr + fs * 1.05, &name, fs, col);
        }

        // V'Ger's cloud is big enough to see from anywhere.
        for p in f.players.iter().filter(|p| p.state == PState::Alive && p.ship == ShipType::VgerCloud) {
            let (x, y) = to(p.x as f64, p.y as f64);
            let r = (ship_size_units(p.ship) * sx) as f32;
            px.glow(x, y, r, rgb(0x3060c0), 0.45);
            px.ring_dashed(x, y, r, 1.0, rgb(0x80b8ff), 0.6, 3.0);
        }
        let mut order: Vec<&PlayerInfo> = f.players.iter().filter(|p| p.state == PState::Alive).collect();
        order.sort_by_key(|p| p.id == self.slot);
        for p in order {
            let (x, y) = to(p.x as f64, p.y as f64);
            if p.fuzzy {
                px.text_centered(tr, x, y + fs * 0.35, "??", fs, GREY);
                continue;
            }
            let is_me = p.id == self.slot;
            let col = if is_me { WHITE } else { mix(player_rgb(p), WHITE, 0.1) };
            let tag = callsign(p);
            if is_me {
                px.ring(x, y, fs * 0.9, 1.0, WHITE, 0.8);
            }
            px.text_centered(tr, x, y + fs * 0.35, &tag, fs, col);
        }

        if let Some((mx, my)) = self.pointer_px(false) {
            px.ring(mx, my, 4.0, 1.0, rgb(0xff60d0), 0.9);
        }
        px
    }

    /// The ship's controls: status lamps, gauge bars and a status line.
    pub(super) fn draw_dashboard_vec(&self, pw: i32, ph: i32) -> Canvas {
        let f = self.frame.as_ref().unwrap();
        let tr = &self.text;
        let mut c = Canvas::new(pw, ph, [0.0, 0.0, 0.0]);
        let Some(me) = self.me() else { return c };
        let s = me.ship.stats();
        let mi = &f.me_info;
        let pad = 6.0;
        let (w, h) = (pw as f32, ph as f32);
        // Scale text and rows to fill the panel: lamps + bar rows + status line.
        let mut cols = if w > 900.0 { 3 } else if w > 380.0 { 2 } else { 1 };
        let mut fs;
        loop {
            let per_col = (6 + cols - 1) / cols;
            fs = ((h - pad) / ((per_col as f32 + 2.0) * 1.75)).clamp(8.0, 22.0);
            if fs >= 10.0 || cols == 3 {
                break;
            }
            cols += 1;
        }
        let rh = (fs * 1.75).round();
        let (green, yellow, red) = (rgb(0x30d050), rgb(0xf0d030), rgb(0xf03030));
        let label_col = rgb(0xa0a8b8);

        // Row 1: status lamps, lit when active.
        let lamps: [(&str, bool, Rgb); 12] = [
            ("SHLD", me.flags & pf::SHIELD != 0, rgb(0x40a0ff)),
            ("CLOAK", me.flags & pf::CLOAK != 0, rgb(0xc070ff)),
            ("REPAIR", me.flags & pf::REPAIR != 0, green),
            ("ORBIT", mi.orbiting.is_some(), rgb(0x40d0d0)),
            ("BOMB", me.flags & pf::BOMB != 0, red),
            ("BEAM\u{2191}", me.flags & pf::BEAMUP != 0, yellow),
            ("BEAM\u{2193}", me.flags & pf::BEAMDOWN != 0, yellow),
            ("TRAC", me.flags & pf::TRACTOR != 0, green),
            ("PRES", me.flags & pf::PRESSOR != 0, rgb(0xd070ff)),
            ("LOCK", mi.lock.is_some(), WHITE),
            ("PREY", me.flags & pf::HUNTED != 0, red),
            ("TRIB", me.flags & pf::TRIBBLES != 0, faction_rgb(Faction::Tribbles)),
        ];
        let lfs = fs * 0.78;
        let mut x = pad;
        let ly = pad;
        let lh = rh - 6.0;
        for (name, on, col) in lamps {
            let lw = tr.width(name, lfs) + 12.0;
            if x + lw > w - pad {
                break;
            }
            if on {
                c.glow(x + lw / 2.0, ly + lh / 2.0, lw * 0.7, col, 0.35);
                c.round_rect(x, ly, lw, lh, lh / 2.0, col, 1.0, None);
                c.text_centered(tr, x + lw / 2.0, ly + lh / 2.0 + lfs * 0.35, name, lfs, [0.0, 0.0, 0.0]);
            } else {
                c.round_rect(x + 0.5, ly + 0.5, lw - 1.0, lh - 1.0, lh / 2.0, DARK_GREY, 1.0, Some(1.0));
                c.text_centered(tr, x + lw / 2.0, ly + lh / 2.0 + lfs * 0.35, name, lfs, DARK_GREY);
            }
            x += lw + 5.0;
        }

        // Gauge bars in as many columns as fit.
        let temp_frac = |v: u32| v as f64 / 100.0;
        let whot = me.flags & pf::WEAPON_HOT != 0;
        let ehot = me.flags & pf::ENGINE_HOT != 0;
        let up = [(0.0, red), (0.5, yellow), (1.0, green)];
        let down = [(0.0, green), (0.5, yellow), (1.0, red)];
        let bars: [(&str, f64, String, &[(f32, Rgb)], bool); 6] = [
            ("Spd", mi.speed as f64 / s.max_speed as f64, format!("{} ({})/{}", mi.speed, mi.desired_speed, mi.max_speed_now), &down, false),
            ("Shl", mi.shield as f64 / s.max_shield, format!("{}/{}", mi.shield, s.max_shield), &up, me.flags & pf::SHIELD == 0),
            ("Hul", mi.damage as f64 / s.max_damage, format!("{}/{}", mi.damage, s.max_damage), &down, false),
            ("Ful", mi.fuel as f64 / s.max_fuel, format!("{}/{}", mi.fuel, s.max_fuel), &up, false),
            ("Wtp", temp_frac(mi.wtemp), if whot { "OVERHEAT".into() } else { format!("{}%", mi.wtemp) }, &down, false),
            ("Etp", temp_frac(mi.etemp), if ehot { "OVERHEAT".into() } else { format!("{}%", mi.etemp) }, &down, false),
        ];
        let top = ly + rh + 2.0;
        let per_col = (6 + cols - 1) / cols;
        let col_w = (w - pad * 2.0 - (cols - 1) as f32 * 14.0) / cols as f32;
        let label_w = tr.width("Spd ", fs);

        for (k, (label, frac, value, stops, dim)) in bars.iter().enumerate() {
            let (ci, ri) = (k / per_col, k % per_col);
            let bx = pad + ci as f32 * (col_w + 14.0);
            let by = top + ri as f32 * rh;
            if by + rh > h {
                continue;
            }
            let hot = value == "OVERHEAT";
            c.text(tr, bx, by + rh * 0.62, label, fs, if hot { red } else { label_col });
            let tx = bx + label_w;
            let tw = (col_w - label_w).max(20.0);
            let th = (rh * 0.66).round();
            let ty = by + (rh - th) / 2.0;
            c.round_rect(tx, ty, tw, th, 3.0, rgb(0x1c2028), 1.0, None);
            let fw = (tw * frac.clamp(0.0, 1.0) as f32).max(0.0);
            if *dim {
                c.gradient_bar(tx, ty, fw, th, tw, &[(0.0, rgb(0x404650)), (1.0, rgb(0x606670))]);
            } else {
                c.gradient_bar(tx, ty, fw, th, tw, stops);
            }
            for t in 1..4 {
                let xx = tx + tw * t as f32 / 4.0;
                c.line(xx, ty + th - 3.0, xx, ty + th, 1.0, rgb(0x707888), 0.8, 0.0);
            }
            c.round_rect(tx + 0.5, ty + 0.5, tw - 1.0, th - 1.0, 3.0, if hot { red } else { rgb(0x586070) }, 1.0, Some(1.0));
            // Value printed inside the bar, with a shadow so it reads on any color.
            let vfs = th * 0.78;
            // In tight space drop the "/max" part.
            let short = value.split(['/', ' ']).next().unwrap_or(value).to_string();
            let value = if tr.width(value, vfs) > tw - 10.0 { &short } else { value };
            let vb = ty + th / 2.0 + vfs * 0.36;
            c.text_right(tr, tx + tw - 5.0, vb + 1.0, value, vfs, [0.0, 0.0, 0.0]);
            c.text_right(tr, tx + tw - 6.0, vb, value, vfs, if hot { red } else { WHITE });
        }

        // Status line.
        let sy = top + per_col as f32 * rh + rh * 0.62;
        if sy < h {
            let mut status = format!(
                "{} {}  kills {:.2}  armies {}/{}  torps {}/8",
                me.ship.stats().abbr,
                me.name,
                mi.kills,
                mi.armies,
                mi.max_armies_now,
                mi.torps_out
            );
            if let Some(k) = mi.orbiting {
                status += &format!("  orbiting {}", PLANETS[k as usize].name);
            }
            if let Some(l) = &mi.lock {
                status += &format!("  lock {}", l);
            }
            c.text(tr, pad, sy, &status, fs * 0.9, team_rgb(me.team));
            let secs = f.tick / UPS as u32;
            let clock = format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
            if tr.width(&status, fs * 0.9) + tr.width(&clock, fs * 0.9) + pad * 3.0 < w {
                c.text_right(tr, w - pad, sy, &clock, fs * 0.9, label_col);
            }
        }
        c
    }

    /// Text size and row height for the text-heavy panels.
    fn panel_font(&self) -> (f32, f32) {
        let fs = (self.cell_px.1 as f32 * 0.62).clamp(10.0, 16.0);
        (fs, (fs * 1.55).max(fs + 4.0).round())
    }

    /// A small nose-up silhouette of a ship, for lists.
    fn ship_icon(&self, c: &mut Canvas, x: f32, y: f32, r: f32, p: &PlayerInfo, col: Rgb) {
        let fill = scale(col, 0.42);
        let at = |q: (f32, f32)| (x + q.0 * r, y + q.1 * r);
        for part in ship_parts(p.team, p.ship, p.faction) {
            match part {
                Part::Poly { pts, fill: filled } => {
                    let pts: Vec<(f32, f32)> = pts.into_iter().map(at).collect();
                    if filled {
                        c.fill_poly(&pts, fill, 1.0);
                    }
                    c.polyline(&pts, true, 1.0, col, 1.0, 0.0);
                }
                Part::Hole { pts } => {
                    let pts: Vec<(f32, f32)> = pts.into_iter().map(at).collect();
                    c.fill_poly(&pts, [0.0, 0.0, 0.0], 1.0);
                    c.polyline(&pts, true, 1.0, col, 1.0, 0.0);
                }
                Part::Circle { c: q, r: rr, fill: filled } => {
                    let (cx, cy) = at(q);
                    if filled {
                        c.disc(cx, cy, rr * r, fill, 1.0);
                    }
                    c.ring(cx, cy, rr * r, 1.0, col, 1.0);
                }
                Part::Line { a, b } => {
                    let (a, b) = (at(a), at(b));
                    c.line(a.0, a.1, b.0, b.1, 1.0, col, 1.0, 0.0);
                }
            }
        }
    }

    /// The message window: warning line, talk line, and the message log.
    pub(super) fn draw_comms_vec(&self, pw: i32, ph: i32) -> Canvas {
        let tr = &self.text;
        let (fs, lh) = self.panel_font();
        let mut c = Canvas::new(pw, ph, [0.0, 0.0, 0.0]);
        let (w, h) = (pw as f32, ph as f32);
        let pad = 6.0;
        let base = |row: f32| pad + row * lh + lh * 0.7;
        if let Some((text, col)) = self.warning_text() {
            c.text(tr, pad, base(0.0), &text, fs, col);
        }
        let (line, col) = self.input_line_text();
        c.text(tr, pad, base(1.0), &line, fs, col);
        let sep = pad + 2.0 * lh + lh * 0.25;
        c.line(pad, sep, w - pad, sep, 1.0, rgb(0x3a404c), 1.0, 0.0);
        let my_team = self.me().map(|p| p.team).unwrap_or(Team::Ind);
        let rows = (((h - sep - pad) / lh).floor() as usize).max(1);
        let start = self.msgs.len().saturating_sub(rows);
        let from_w = tr.width("MMMMMMMMM", fs);
        for (k, m) in self.msgs.iter().skip(start).enumerate() {
            let y = sep + lh * 0.2 + k as f32 * lh + lh * 0.7;
            let col = match m.kind {
                MsgKind::System if m.from == "ALERT" => rgb(0xff5cf0),
                MsgKind::System => rgb(0xa0a6b0),
                MsgKind::All => WHITE,
                MsgKind::Team => team_rgb(my_team),
                MsgKind::Indiv => rgb(0x5fd7ff),
            };
            c.text(tr, pad, y, &m.from, fs, if m.kind == MsgKind::System && m.from != "ALERT" { rgb(0x7a808a) } else { col });
            c.text(tr, pad + from_w, y, &m.text, fs, col);
        }
        c
    }

    /// The player list, with a little silhouette of each ship.
    /// A popup (help, players or planets) drawn as an image, so vector mode
    /// keeps its maps on screen underneath it.
    pub(super) fn draw_popup_vec(&self, pw: i32, ph: i32, title: &str) -> Canvas {
        let tr = &self.text;
        let (fs, lh) = self.panel_font();
        let mut c = Canvas::new(pw, ph, [0.0, 0.0, 0.0]);
        let (w, h) = (pw as f32, ph as f32);
        let pad = 14.0;
        c.round_rect(1.5, 1.5, w - 3.0, h - 3.0, 6.0, rgb(0x9aa0ac), 1.0, Some(1.5));
        c.text(tr, pad, 4.0 + lh * 0.75, title, fs, WHITE);
        let top = 4.0 + lh * 1.15;
        c.line(pad * 0.6, top, w - pad * 0.6, top, 1.0, rgb(0x3a404c), 1.0, 0.0);
        match self.popup {
            Popup::Help => {
                let key_w = HELP.iter().map(|(k, _)| tr.width(k, fs)).fold(0.0, f32::max) + fs * 1.5;
                for (k, (keys, what)) in HELP.iter().enumerate() {
                    let y = top + 4.0 + (k as f32 + 0.75) * lh;
                    if y > h - 4.0 {
                        break;
                    }
                    c.text(tr, pad, y, keys, fs, rgb(0xf0d030));
                    c.text(tr, pad + key_w, y, what, fs, rgb(0xc8ccd4));
                }
            }
            Popup::Players => {
                let list = self.draw_players_vec((w - 2.0 * pad) as i32, (h - top - 8.0) as i32);
                c.draw_canvas(&list, pad as i32, (top + 2.0) as i32);
            }
            Popup::Planets => {
                let f = self.frame.as_ref().unwrap();
                let cw = tr.width("M", fs);
                let col_w = (w - 2.0 * pad) / 2.0;
                for (k, def) in PLANETS.iter().enumerate() {
                    let (col, row) = (k / 20, k % 20);
                    let x = pad + col as f32 * col_w;
                    let y = top + 4.0 + (row as f32 + 0.75) * lh;
                    if y > h - 4.0 {
                        continue;
                    }
                    let info = &f.planets[k];
                    if !info.known {
                        c.text(tr, x, y, def.name, fs, GREY);
                        c.text(tr, x + cw * 15.0, y, "?", fs, GREY);
                        continue;
                    }
                    let pc = planet_rgb(info);
                    c.text(tr, x, y, def.name, fs, pc);
                    c.text(tr, x + cw * 15.0, y, &info.owner.letter().to_string(), fs, pc);
                    c.text_right(tr, x + cw * 20.0, y, &info.armies.to_string(), fs, pc);
                    let mut tags = String::new();
                    for (flag, ch) in [(PL_REPAIR, 'R'), (PL_FUEL, 'F'), (PL_AGRI, 'A')] {
                        tags.push(if info.flags & flag != 0 { ch } else { ' ' });
                    }
                    if info.tribbles {
                        tags.push('T');
                    }
                    c.text(tr, x + cw * 21.5, y, &tags, fs, scale(pc, 0.8));
                }
            }
            Popup::None => {}
        }
        c
    }

    pub(super) fn draw_players_vec(&self, pw: i32, ph: i32) -> Canvas {
        let f = self.frame.as_ref().unwrap();
        let tr = &self.text;
        let (fs, lh) = self.panel_font();
        let mut c = Canvas::new(pw, ph, [0.0, 0.0, 0.0]);
        let (w, h) = (pw as f32, ph as f32);
        let pad = 6.0;
        let cw = tr.width("M", fs);
        // Column positions.
        let x_icon = pad + lh * 0.5;
        let x_tag = pad + lh * 1.1;
        let x_ty = x_tag + cw * 5.0;
        let x_name = x_ty + cw * 3.5;
        let x_kills = (x_name + cw * 18.0).min(w - cw * 14.0);
        let x_arm = x_kills + cw * 4.0;
        let x_status = x_arm + cw * 1.5;
        let head = rgb(0xe8ecf2);
        let yb = pad + lh * 0.7;
        c.text(tr, x_tag, yb, "No", fs, head);
        c.text(tr, x_ty, yb, "Ty", fs, head);
        c.text(tr, x_name, yb, "Name", fs, head);
        c.text_right(tr, x_kills, yb, "Kills", fs, head);
        c.text_right(tr, x_arm, yb, "Arm", fs, head);
        let line_y = pad + lh + 1.0;
        c.line(pad, line_y, w - pad, line_y, 1.0, rgb(0x3a404c), 1.0, 0.0);

        let mut ps: Vec<&PlayerInfo> = f.players.iter().filter(|p| p.state != PState::Outfit).collect();
        ps.sort_by_key(|p| (p.faction.is_some(), p.team.idx(), p.id));
        for (k, p) in ps.iter().enumerate() {
            let top = line_y + 2.0 + k as f32 * lh;
            if top + lh > h {
                break;
            }
            let mid = top + lh * 0.5;
            let y = top + lh * 0.7;
            let alive = p.state == PState::Alive;
            let base_col = player_rgb(p);
            let col = if alive { base_col } else { rgb(0x5a606a) };
            if p.id == self.slot {
                c.round_rect(pad - 2.0, top, w - 2.0 * pad + 4.0, lh, 3.0, base_col, 0.14, None);
            }
            self.ship_icon(&mut c, x_icon, mid, lh * 0.4, p, col);
            let name: String = p.name.chars().take(16).collect();
            c.text(tr, x_tag, y, &callsign(p), fs, col);
            c.text(tr, x_ty, y, p.ship.stats().abbr, fs, col);
            c.text(tr, x_name, y, &name, fs, if p.id == self.slot { mix(col, WHITE, 0.4) } else { col });
            c.text_right(tr, x_kills, y, &format!("{:.2}", p.kills), fs, col);
            if p.armies > 0 {
                c.text_right(tr, x_arm, y, &p.armies.to_string(), fs, col);
            }
            let status = if !alive {
                "dead"
            } else if p.faction.is_some() {
                "alien"
            } else if p.flags & pf::ROBOT != 0 {
                "robot"
            } else {
                ""
            };
            if x_status + cw * 5.0 < w {
                c.text(tr, x_status, y, status, fs * 0.9, scale(col, 0.75));
            }
        }
        c
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Renders a phaser hit and a miss at every age of their animation into
    /// $TMPDIR/netrek-phasers.ppm for eyeballing.
    #[test]
    fn phaser_gallery() {
        let (cw, ch) = (260.0f32, 70.0f32);
        let mut c = Canvas::new((cw * 2.0) as i32, (ch * 6.0) as i32, [0.0, 0.0, 0.0]);
        for age in 0..6u8 {
            let y = age as f32 * ch + ch / 2.0;
            draw_phaser(&mut c, (20.0, y), (cw - 30.0, y - 12.0), rgb(0xf2c94c), true, age, 1.2, 7);
            draw_phaser(&mut c, (cw + 20.0, y), (2.0 * cw - 30.0, y + 12.0), rgb(0xff5a4f), false, age, 1.2, 9);
        }
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-phasers.ppm"), out).unwrap();
    }
}
