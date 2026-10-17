//! Drawing space terrain (the server's --terrain option): a detailed look
//! for the tactical view, a simple one for the galaxy map, in both vector
//! and braille graphics.

use super::canvas::Braille;
use super::palette::{mix, rgb, scale, Rgb};
use super::sixel::TextRenderer;
use super::vg::Canvas;
use crate::proto::{Frame, TerrainInfo, TerrainKind, ZoneInfo, ZoneKind};
use crossterm::style::Color;
use std::f32::consts::TAU;

pub(super) const WHITE: Rgb = [255.0, 255.0, 255.0];
pub(super) const NEBULA: Rgb = rgb(0x8a4fd0);
pub(super) const STORM: Rgb = rgb(0x4a8cff);
pub(super) const ROCK: Rgb = rgb(0x8a8070);
pub(super) const ACCRETION: Rgb = rgb(0xff9a40);
pub(super) const PULSAR: Rgb = rgb(0x9ae8ff);
pub(super) const WORMHOLE: Rgb = rgb(0xc070ff);
pub(super) const WRECK: Rgb = rgb(0x9aa0a8);
pub(super) const SLIPSTREAM: Rgb = rgb(0x40e0c0);
pub(super) const STAR: Rgb = rgb(0xffc040);
pub(super) const COMET: Rgb = rgb(0xc8e8ff);
pub(super) const TACHYON: Rgb = rgb(0x50d8e8);
pub(super) const MINES: Rgb = rgb(0xff4d4d);
pub(super) const CHRONITON: Rgb = rgb(0x40e0b0);
pub(super) const EDDY: Rgb = rgb(0x7fa8ff);
pub(super) const MAGNETAR: Rgb = rgb(0xff50e0);
pub(super) const METREON: Rgb = rgb(0xffa040);
pub(super) const TETRYON: Rgb = rgb(0xd8ff60);
pub(super) const PLANETOID: Rgb = rgb(0x9a9088);
pub(super) const BARRIER: Rgb = rgb(0xb070ff);
pub(super) const RIFT: Rgb = rgb(0xff90d0);
pub(super) const STATION: Rgb = rgb(0xc0c8d8);
/// The Briar Patch's metaphasic clouds: amber and rose.
pub(super) const BRIAR: Rgb = rgb(0xd08858);
pub(super) const BRIAR_ROSE: Rgb = rgb(0xff90a0);
/// The Badlands' plasma storms: red fire and magenta.
pub(super) const BADLANDS: Rgb = rgb(0xe04060);
pub(super) const PLASMA_FIRE: Rgb = rgb(0xffa040);
/// The Nexus ribbon: gold and flame.
pub(super) const NEXUS: Rgb = rgb(0xffc860);
pub(super) const NEXUS_HOT: Rgb = rgb(0xff6a40);
pub(super) const TYKEN: Rgb = rgb(0x8a60d0);
/// The Genesis Planet: green and blue, turning red as it breaks up.
pub(super) const GENESIS: Rgb = rgb(0x60c890);
pub(super) const GENESIS_DYING: Rgb = rgb(0xff4020);
pub(super) const FILAMENT: Rgb = rgb(0x90b8ff);
pub(super) const HEKARAS: Rgb = rgb(0x8090ff);
pub(super) const OMEGA: Rgb = rgb(0xe0c8ff);
pub(super) const NEUTRAL: Rgb = rgb(0x9fd0a0);
/// The Guardian of Forever: old stone, and the mists of time inside it.
pub(super) const GUARDIAN: Rgb = rgb(0xc8b890);
pub(super) const TIME_MIST: Rgb = rgb(0xa0d8ff);
pub(super) const THETA: Rgb = rgb(0x98ff30);
/// The Genesis Planet's reach (matches the server).
pub(super) const GENESIS_REACH: f32 = 3500.0;
/// The Black Hole's whirlpool: deep violet at the edge, ice blue-white at
/// the eye.
pub(super) const MAELSTROM: Rgb = rgb(0x6a4cff);
pub(super) const MAELSTROM_HOT: Rgb = rgb(0xb8e8ff);
/// Spiral arms in the Black Hole's whirlpool, and how many turns each makes.
pub(super) const ARMS: usize = 5;
pub(super) const ARM_TURNS: f32 = 0.85;
/// The Black Hole's event horizon (matches the server).
pub(super) const GREAT_HORIZON: f32 = 1500.0;

/// A point on spiral arm `k` of the Black Hole's whirlpool, `s` of the way
/// in from the edge (0) to the event horizon (1): (angle, distance from the
/// centre, as a fraction of `r`). The whirlpool turns anticlockwise on the
/// map, and the arms trail behind, winding tighter as they fall in.
pub(super) fn arm_point(k: usize, s: f32, r: f32, tick: f32, sd: u64) -> (f32, f32) {
    let h = GREAT_HORIZON / r;
    let d = 1.0 - s * (1.0 - h);
    let a = rnd(sd, k as u64) * 0.6 + k as f32 * TAU / ARMS as f32 - tick * 0.03 - ARM_TURNS * TAU * s * s.sqrt();
    (a, d)
}

/// Pulsar cycle length in ticks (matches the server).
pub(super) const PULSAR_PERIOD: f32 = 100.0;

fn hash(a: i64, b: i64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 32)
}

/// A pseudo-random number in 0..1 from a seed and an index.
pub(super) fn rnd(seed: u64, k: u64) -> f32 {
    (hash(seed as i64, k as i64) % 10_000) as f32 / 10_000.0
}

pub(super) fn seed(t: &TerrainInfo) -> u64 {
    hash(t.x as i64 * 7 + t.kind as i64, t.y as i64)
}

/// Terrain on the tactical view (detailed) or the galaxy map (simple).
/// `to` maps galaxy units to pixels and `u` scales a length.
pub fn draw_vec(
    px: &mut Canvas,
    f: &Frame,
    to: &dyn Fn(f64, f64) -> (f32, f32),
    u: &dyn Fn(f64) -> f32,
    detail: bool,
    tr: &TextRenderer,
    fs: f32,
) {
    let tick = f.tick as f32;
    let (w, h) = (px.w as f32, px.h as f32);
    let on_screen = |x: f32, y: f32, r: f32| x + r > 0.0 && y + r > 0.0 && x - r < w && y - r < h;
    for t in &f.terrain {
        let (x, y) = to(t.x as f64, t.y as f64);
        let (x2, y2) = to(t.x2 as f64, t.y2 as f64);
        let r = u(t.r as f64).max(1.0);
        let sd = seed(t);
        let segment = matches!(
            t.kind,
            TerrainKind::Corridor | TerrainKind::Comet | TerrainKind::Wormhole | TerrainKind::GalacticBarrier | TerrainKind::Nexus | TerrainKind::HekarasCorridor | TerrainKind::NeutralZone
        );
        if !segment && !on_screen(x, y, r * 1.2) {
            continue;
        }
        if segment && !on_screen(x, y, r * 1.2) && !on_screen(x2, y2, r * 1.2) && !on_screen((x + x2) / 2.0, (y + y2) / 2.0, (x - x2).abs().max((y - y2).abs())) {
            continue;
        }
        let label = |px: &mut Canvas, lx: f32, ly: f32, col: Rgb| {
            if detail {
                px.text_centered(tr, lx, ly, &t.name, fs * 0.8, scale(col, 0.8));
            }
        };
        match t.kind {
            TerrainKind::Nebula => {
                px.glow(x, y, r * 1.15, NEBULA, if detail { 0.30 } else { 0.35 });
                if detail {
                    for k in 0..7 {
                        let a = rnd(sd, k) * TAU;
                        let d = rnd(sd, k + 50) * r * 0.6;
                        let c = mix(NEBULA, rgb(0xd060c0), rnd(sd, k + 90));
                        px.glow(x + a.cos() * d, y + a.sin() * d, r * (0.35 + 0.3 * rnd(sd, k + 20)), c, 0.20);
                    }
                    label(px, x, y - r * 0.8, NEBULA);
                }
            }
            TerrainKind::IonStorm => {
                px.glow(x, y, r * 1.1, STORM, 0.28);
                px.ring_dashed(x, y, r, 1.0, STORM, 0.4, 6.0);
                if detail {
                    // Lightning, different every frame.
                    let flicker = (tick / 2.0) as u64;
                    for k in 0..3u64 {
                        let mut a = rnd(sd ^ flicker, k) * TAU;
                        let (mut lx, mut ly) = (x, y);
                        let mut pts = vec![(lx, ly)];
                        for s in 0..6u64 {
                            a += (rnd(sd ^ flicker, k * 10 + s) - 0.5) * 1.4;
                            let step = r * 0.14;
                            lx += a.cos() * step;
                            ly += a.sin() * step;
                            pts.push((lx, ly));
                        }
                        px.polyline(&pts, false, 1.2, mix(STORM, WHITE, 0.6), 0.85, 0.0);
                    }
                    label(px, x, y - r * 0.85, STORM);
                }
            }
            TerrainKind::Asteroids => {
                px.ring_dashed(x, y, r, 1.0, ROCK, 0.35, 2.0);
                let n = if detail { 45 } else { 10 };
                for k in 0..n {
                    let a = rnd(sd, k) * TAU;
                    let d = rnd(sd, k + 100).sqrt() * r * 0.95;
                    let (ax, ay) = (x + a.cos() * d, y + a.sin() * d);
                    if !detail {
                        px.dot(ax, ay, ROCK, 0.8);
                        continue;
                    }
                    let size = u(150.0 + 250.0 * rnd(sd, k + 200) as f64).max(1.2);
                    let pts: Vec<(f32, f32)> = (0..6)
                        .map(|j| {
                            let b = j as f32 / 6.0 * TAU + rnd(sd, k * 7 + j);
                            let rr = size * (0.6 + 0.5 * rnd(sd, k * 13 + j));
                            (ax + b.cos() * rr, ay + b.sin() * rr)
                        })
                        .collect();
                    px.fill_poly(&pts, scale(ROCK, 0.55), 1.0);
                    px.polyline(&pts, true, 0.8, ROCK, 0.9, 0.0);
                }
                label(px, x, y - r * 0.9, ROCK);
            }
            TerrainKind::BlackHole => {
                if detail {
                    px.ring_dashed(x, y, r, 1.0, ACCRETION, 0.25, 5.0);
                }
                px.glow(x, y, u(2600.0).max(3.0), ACCRETION, 0.45);
                // The accretion disc, spinning.
                let disc = u(1300.0).max(2.5);
                for k in 0..3 {
                    let a0 = tick * 0.08 + k as f32 * TAU / 3.0;
                    let pts: Vec<(f32, f32)> = (0..12)
                        .map(|j| {
                            let a = a0 + j as f32 * 0.12;
                            (x + a.cos() * disc, y + a.sin() * disc * 0.45)
                        })
                        .collect();
                    px.polyline(&pts, false, 2.0, mix(ACCRETION, WHITE, 0.4), 0.8, 0.0);
                }
                px.disc(x, y, u(700.0).max(2.0), [0.0, 0.0, 0.0], 1.0);
                px.ring(x, y, u(700.0).max(2.0), 1.0, rgb(0xb070ff), 0.9);
                label(px, x, y - r * 0.55, ACCRETION);
            }
            TerrainKind::GreatBlackHole => {
                let tk = tick;
                px.glow(x, y, r * 1.05, MAELSTROM, if detail { 0.16 } else { 0.22 });
                if detail {
                    px.ring_dashed(x, y, r, 1.0, MAELSTROM, 0.3, 8.0);
                }
                // The whirlpool: luminous arms spiralling into the eye.
                let n = if detail { 60 } else { 18 };
                let strands = if detail { 3 } else { 1 };
                for k in 0..ARMS {
                    for st in 0..strands {
                        let off = st as f32 * 0.07;
                        let pts: Vec<(f32, f32, f32)> = (0..=n)
                            .map(|j| {
                                let s = j as f32 / n as f32;
                                let (a, d) = arm_point(k, s, t.r as f32, tk, sd);
                                let a = a - off;
                                (x + a.cos() * d * r, y + a.sin() * d * r, s)
                            })
                            .collect();
                        for j in 0..n {
                            let (p, q) = (pts[j], pts[j + 1]);
                            let s = p.2;
                            let col = mix(MAELSTROM, MAELSTROM_HOT, s.powf(1.5));
                            let a = (0.15 + 0.75 * s) * if st == 0 { 1.0 } else { 0.45 };
                            let wdt = if detail { 1.0 + 3.0 * s * (1.0 - s) * 2.0 } else { 1.2 };
                            px.line(p.0, p.1, q.0, q.1, wdt * 3.0, col, a * 0.18, 0.0);
                            px.line(p.0, p.1, q.0, q.1, wdt, col, a, 0.0);
                        }
                    }
                }
                // Debris whirling in: sparks travelling down the arms.
                if detail {
                    for k in 0..24u64 {
                        let s = ((rnd(sd, k + 900) + tk * 0.004 * (1.0 + rnd(sd, k + 950))) % 1.0).powf(0.8);
                        let (a, d) = arm_point(k as usize % ARMS, s, t.r as f32, tk, sd);
                        let a = a - rnd(sd, k + 990) * 0.25;
                        px.glow(x + a.cos() * d * r, y + a.sin() * d * r, 2.0 + 3.0 * s, MAELSTROM_HOT, 0.7);
                    }
                }
                // The eye: a black event horizon in a blaze of light.
                let eye = u(GREAT_HORIZON as f64).max(2.5);
                px.glow(x, y, eye * 3.2, MAELSTROM_HOT, 0.55);
                px.glow(x, y, eye * 1.8, WHITE, 0.5);
                px.disc(x, y, eye, [0.0, 0.0, 0.0], 1.0);
                px.ring(x, y, eye, 1.6, mix(MAELSTROM_HOT, WHITE, 0.5), 0.95);
                label(px, x, y - r * 0.6, MAELSTROM_HOT);
                if !detail {
                    px.text_centered(tr, x, y + eye + fs * 1.2, "BLACK HOLE", fs * 0.75, scale(MAELSTROM_HOT, 0.8));
                }
            }
            TerrainKind::Pulsar => {
                let since = PULSAR_PERIOD - t.phase as f32;
                if detail {
                    px.ring_dashed(x, y, r, 1.0, PULSAR, 0.25, 4.0);
                    // Sweeping beams.
                    let a = tick * 0.15;
                    for s in [0.0, std::f32::consts::PI] {
                        let (ex, ey) = (x + (a + s).cos() * r, y + (a + s).sin() * r);
                        px.line(x, y, ex, ey, 6.0, PULSAR, 0.12, 0.0);
                        px.line(x, y, ex, ey, 1.2, mix(PULSAR, WHITE, 0.5), 0.6, 0.0);
                    }
                    // The pulse itself: a shock ring racing outward.
                    if since < 12.0 {
                        let k = since / 12.0;
                        px.ring(x, y, r * k.max(0.05), 3.0, mix(PULSAR, WHITE, 0.5), 1.0 - k);
                    } else if t.phase <= 20 {
                        px.glow(x, y, r * 0.3, PULSAR, 0.25);
                    }
                    label(px, x, y - r * 0.9, PULSAR);
                }
                px.glow(x, y, u(1200.0).max(3.0), PULSAR, 0.6);
                px.disc(x, y, u(300.0).max(1.5), WHITE, 1.0);
            }
            TerrainKind::Wormhole => {
                for (mx, my) in [(x, y), (x2, y2)] {
                    if !on_screen(mx, my, r * 2.0) {
                        continue;
                    }
                    let mr = u(900.0).max(3.0);
                    px.glow(mx, my, mr * 1.8, WORMHOLE, 0.45);
                    if detail {
                        let pts: Vec<(f32, f32)> = (0..40)
                            .map(|j| {
                                let s = j as f32 / 40.0;
                                let a = tick * 0.25 + s * TAU * 2.0;
                                (mx + a.cos() * mr * s, my + a.sin() * mr * s)
                            })
                            .collect();
                        px.polyline(&pts, false, 1.4, mix(WORMHOLE, WHITE, 0.4), 0.9, 0.0);
                        label(px, mx, my - mr * 1.6, WORMHOLE);
                    }
                    px.ring(mx, my, mr, 1.3, WORMHOLE, 0.9);
                }
                if !detail {
                    px.line(x, y, x2, y2, 1.0, WORMHOLE, 0.25, 3.0);
                }
            }
            TerrainKind::Derelict => {
                let s = u(450.0).max(3.0);
                if detail {
                    px.glow(x, y, s * 2.5, WRECK, 0.15);
                    // A broken hull, drifting.
                    let a = tick * 0.01 + rnd(sd, 1) * TAU;
                    let rot = |lx: f32, ly: f32| (x + lx * a.cos() - ly * a.sin(), y + lx * a.sin() + ly * a.cos());
                    let hull = [rot(-s, -0.3 * s), rot(0.1 * s, -0.35 * s), rot(0.2 * s, 0.3 * s), rot(-s, 0.25 * s)];
                    px.fill_poly(&hull, scale(WRECK, 0.35), 1.0);
                    px.polyline(&hull, true, 1.0, WRECK, 0.9, 0.0);
                    let (b0, b1) = (rot(0.45 * s, -0.25 * s), rot(s, 0.2 * s));
                    px.line(b0.0, b0.1, b1.0, b1.1, 1.2, WRECK, 0.8, 0.0);
                    px.ring_dashed(x, y, u(900.0), 1.0, WRECK, 0.3, 3.0);
                    label(px, x, y - s * 2.2, WRECK);
                } else {
                    px.line(x - 2.5, y - 2.5, x + 2.5, y + 2.5, 1.0, WRECK, 0.9, 0.0);
                    px.line(x - 2.5, y + 2.5, x + 2.5, y - 2.5, 1.0, WRECK, 0.9, 0.0);
                }
            }
            TerrainKind::Corridor => {
                let (dx, dy) = (x2 - x, y2 - y);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (ux, uy) = (dx / len, dy / len);
                let (nx, ny) = (-uy, ux);
                if !detail {
                    px.line(x, y, x2, y2, 1.5, SLIPSTREAM, 0.5, 0.0);
                    continue;
                }
                let half = r;
                for s in [-1.0, 1.0] {
                    px.line(x + nx * half * s, y + ny * half * s, x2 + nx * half * s, y2 + ny * half * s, 1.0, SLIPSTREAM, 0.55, 4.0);
                }
                px.line(x, y, x2, y2, half * 2.0, SLIPSTREAM, 0.06, 0.0);
                // Chevrons flowing along the stream.
                let gap = u(2500.0).max(12.0);
                let off = (tick * u(3.0 * 20.0)) % gap;
                let mut d = off;
                while d < len {
                    let (cx, cy) = (x + ux * d, y + uy * d);
                    let c = half * 0.6;
                    px.line(cx - ux * c + nx * c, cy - uy * c + ny * c, cx, cy, 1.2, SLIPSTREAM, 0.7, 0.0);
                    px.line(cx - ux * c - nx * c, cy - uy * c - ny * c, cx, cy, 1.2, SLIPSTREAM, 0.7, 0.0);
                    d += gap;
                }
                label(px, (x + x2) / 2.0 + nx * half * 2.5, (y + y2) / 2.0 + ny * half * 2.5, SLIPSTREAM);
            }
            TerrainKind::Star => {
                px.glow(x, y, r, STAR, 0.32);
                if detail {
                    px.ring_dashed(x, y, r, 1.0, STAR, 0.35, 5.0);
                    label(px, x, y - r * 0.85, STAR);
                }
                px.glow(x, y, u(2000.0).max(3.0), mix(STAR, WHITE, 0.4), 0.8);
                px.disc(x, y, u(1200.0).max(2.0), mix(STAR, WHITE, 0.6), 1.0);
            }
            TerrainKind::Comet => {
                // A long tail that widens and fades away from the head,
                // built from short slices so it has no hard edges.
                let (dx, dy) = (x2 - x, y2 - y);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (nx, ny) = (-dy / len, dx / len);
                let w0 = u(500.0).max(1.5);
                let n = 24;
                for layer in [1.0f32, 0.45] {
                    for k in 0..n {
                        let (s0, s1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
                        let width = |s: f32| w0 * layer * (1.0 + 4.0 * s);
                        let (ax, ay) = (x + dx * s0, y + dy * s0);
                        let (bx, by) = (x + dx * s1, y + dy * s1);
                        let (wa, wb) = (width(s0), width(s1));
                        let slice = [(ax + nx * wa, ay + ny * wa), (bx + nx * wb, by + ny * wb), (bx - nx * wb, by - ny * wb), (ax - nx * wa, ay - ny * wa)];
                        let fade = (1.0 - s0).powf(1.6);
                        px.fill_poly(&slice, COMET, (if layer < 1.0 { 0.22 } else { 0.12 }) * fade);
                    }
                }
                px.line(x, y, x + dx * 0.7, y + dy * 0.7, 1.0, mix(COMET, WHITE, 0.5), 0.5, 0.0);
                px.glow(x, y, u(900.0).max(3.0), COMET, 0.8);
                px.disc(x, y, u(250.0).max(1.5), WHITE, 1.0);
                label(px, x, y - u(900.0) - 4.0, COMET);
            }
            TerrainKind::TachyonGrid => {
                px.ring_dashed(x, y, r, 1.0, TACHYON, 0.45, 3.0);
                if detail {
                    let step = u(1500.0).max(6.0);
                    let mut o = -r + (r % step);
                    while o < r {
                        let c = (r * r - o * o).max(0.0).sqrt();
                        px.line(x + o, y - c, x + o, y + c, 1.0, TACHYON, 0.12, 0.0);
                        px.line(x - c, y + o, x + c, y + o, 1.0, TACHYON, 0.12, 0.0);
                        o += step;
                    }
                    label(px, x, y - r * 0.9, TACHYON);
                }
            }
            TerrainKind::Minefield => {
                px.ring_dashed(x, y, r, 1.2, MINES, 0.55, 5.0);
                if detail {
                    // Warning markers (the mines themselves can't be seen).
                    for k in 0..8 {
                        let a = k as f32 / 8.0 * TAU + rnd(sd, 1);
                        let (mx, my) = (x + a.cos() * r * 0.97, y + a.sin() * r * 0.97);
                        px.line(mx - 3.0, my - 3.0, mx + 3.0, my + 3.0, 1.2, MINES, 0.8, 0.0);
                        px.line(mx - 3.0, my + 3.0, mx + 3.0, my - 3.0, 1.2, MINES, 0.8, 0.0);
                    }
                    label(px, x, y - r * 0.85, MINES);
                }
            }
            TerrainKind::ChronitonField => {
                px.glow(x, y, r, CHRONITON, 0.12);
                for k in [1.0f32, 0.66, 0.33] {
                    px.ring_dashed(x, y, r * k, 1.0, CHRONITON, 0.35, 6.0);
                }
                if detail {
                    // Clock marks, and one slowly turning hand of arc.
                    for k in 0..12 {
                        let a = k as f32 / 12.0 * TAU;
                        px.line(x + a.cos() * r * 0.9, y + a.sin() * r * 0.9, x + a.cos() * r, y + a.sin() * r, 1.2, CHRONITON, 0.6, 0.0);
                    }
                    let a0 = tick * 0.01;
                    let arc: Vec<(f32, f32)> = (0..16).map(|k| { let a = a0 + k as f32 * 0.06; (x + a.cos() * r * 0.8, y + a.sin() * r * 0.8) }).collect();
                    px.polyline(&arc, false, 2.0, CHRONITON, 0.7, 0.0);
                    label(px, x, y - r * 0.85, CHRONITON);
                }
            }
            TerrainKind::GravitonEddy => {
                px.glow(x, y, r, EDDY, 0.12);
                for arm in 0..4 {
                    let pts: Vec<(f32, f32)> = (0..30)
                        .map(|k| {
                            let s = k as f32 / 29.0;
                            let a = tick * 0.04 + arm as f32 * TAU / 4.0 + s * 3.0;
                            (x + a.cos() * r * s, y + a.sin() * r * s)
                        })
                        .collect();
                    px.polyline(&pts, false, if detail { 1.5 } else { 1.0 }, EDDY, 0.55, 0.0);
                }
                if detail {
                    label(px, x, y - r * 0.9, EDDY);
                }
            }
            TerrainKind::Magnetar => {
                px.ring_dashed(x, y, r, 1.0, MAGNETAR, 0.3, 5.0);
                if detail {
                    // Magnetic field loops above and below the poles.
                    for k in 1..4 {
                        let ry = r * 0.28 * k as f32;
                        let rx = ry * 0.55;
                        for side in [-1.0f32, 1.0] {
                            let pts: Vec<(f32, f32)> = (0..24).map(|j| { let a = j as f32 / 23.0 * std::f32::consts::PI; (x + side * a.sin() * rx, y - a.cos() * ry) }).collect();
                            px.polyline(&pts, false, 1.0, MAGNETAR, 0.35, 0.0);
                        }
                    }
                    label(px, x, y - r * 0.9, MAGNETAR);
                }
                px.glow(x, y, u(1500.0).max(3.0), MAGNETAR, 0.6);
                px.disc(x, y, u(350.0).max(1.5), rgb(0xffd0f8), 1.0);
            }
            TerrainKind::MetreonCloud => {
                px.glow(x, y, r * 1.1, METREON, 0.28);
                if detail {
                    for k in 0..5 {
                        let a = rnd(sd, k) * TAU;
                        let d = rnd(sd, k + 50) * r * 0.5;
                        px.glow(x + a.cos() * d, y + a.sin() * d, r * 0.45, mix(METREON, rgb(0xff5020), rnd(sd, k + 9)), 0.2);
                    }
                    label(px, x, y - r * 0.8, METREON);
                }
            }
            TerrainKind::TetryonField => {
                px.ring_dashed(x, y, r, 1.0, TETRYON, 0.4, 2.0);
                let n = if detail { 40 } else { 8 };
                let flicker = (tick / 3.0) as u64;
                for k in 0..n {
                    let a = rnd(sd ^ flicker, k) * TAU;
                    let d = rnd(sd ^ flicker, k + 100).sqrt() * r;
                    px.dot(x + a.cos() * d, y + a.sin() * d, TETRYON, 0.8);
                }
                if detail {
                    label(px, x, y - r * 0.9, TETRYON);
                }
            }
            TerrainKind::Planetoid => {
                px.disc(x, y, r, scale(PLANETOID, 0.45), 1.0);
                px.ring(x, y, r, 1.2, PLANETOID, 1.0);
                if detail {
                    for k in 0..6 {
                        let a = rnd(sd, k) * TAU;
                        let d = rnd(sd, k + 30) * r * 0.65;
                        px.ring(x + a.cos() * d, y + a.sin() * d, r * (0.08 + 0.1 * rnd(sd, k + 60)), 1.0, scale(PLANETOID, 0.8), 0.9);
                    }
                    label(px, x, y - r - 6.0, PLANETOID);
                }
            }
            TerrainKind::GalacticBarrier => {
                let (dx, dy) = (x2 - x, y2 - y);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (nx, ny) = (-dy / len, dx / len);
                px.line(x, y, x2, y2, u(700.0).max(4.0), BARRIER, 0.15, 0.0);
                px.line(x, y, x2, y2, 1.5, mix(BARRIER, WHITE, 0.4), 0.9, 0.0);
                if detail {
                    // A shimmering, wavering edge.
                    let steps = (len / 6.0).max(8.0) as usize;
                    let pts: Vec<(f32, f32)> = (0..=steps)
                        .map(|k| {
                            let s = k as f32 / steps as f32;
                            let off = (s * 60.0 + tick * 0.3).sin() * u(180.0);
                            (x + dx * s + nx * off, y + dy * s + ny * off)
                        })
                        .collect();
                    px.polyline(&pts, false, 1.0, BARRIER, 0.6, 0.0);
                    label(px, (x + x2) / 2.0 + nx * 14.0, (y + y2) / 2.0 + ny * 14.0, BARRIER);
                }
            }
            TerrainKind::FluidicRift => {
                let rr = u(900.0).max(4.0);
                px.glow(x, y, rr * 2.2, RIFT, 0.4);
                let pts: Vec<(f32, f32)> = (0..20)
                    .map(|k| {
                        let a = k as f32 / 20.0 * TAU + tick * 0.08;
                        let jag = if k % 2 == 0 { 1.0 } else { 0.55 };
                        (x + a.cos() * rr * jag, y + a.sin() * rr * jag * 0.6)
                    })
                    .collect();
                px.polyline(&pts, true, 1.3, mix(RIFT, WHITE, 0.3), 0.9, 0.0);
                if detail {
                    label(px, x, y - rr * 1.8, RIFT);
                }
            }
            TerrainKind::AbandonedStation => {
                let s = u(500.0).max(3.0);
                if detail {
                    // A K-7 style station: a hub and three spokes to pods.
                    px.ring_dashed(x, y, r, 1.0, STATION, 0.3, 4.0);
                    for k in 0..3 {
                        let a = k as f32 / 3.0 * TAU - std::f32::consts::FRAC_PI_2;
                        let (px2, py2) = (x + a.cos() * s * 1.6, y + a.sin() * s * 1.6);
                        px.line(x, y, px2, py2, 1.5, STATION, 0.9, 0.0);
                        px.disc(px2, py2, s * 0.35, scale(STATION, 0.4), 1.0);
                        px.ring(px2, py2, s * 0.35, 1.0, STATION, 1.0);
                    }
                    label(px, x, y - s * 2.6, STATION);
                }
                px.disc(x, y, s * 0.55, scale(STATION, 0.5), 1.0);
                px.ring(x, y, s * 0.55, 1.2, STATION, 1.0);
            }
            TerrainKind::BriarPatch => {
                px.glow(x, y, r * 1.15, BRIAR, if detail { 0.28 } else { 0.35 });
                if detail {
                    for k in 0..9 {
                        let a = rnd(sd, k) * TAU;
                        let d = rnd(sd, k + 50) * r * 0.65;
                        let c = mix(BRIAR, BRIAR_ROSE, rnd(sd, k + 90));
                        px.glow(x + a.cos() * d, y + a.sin() * d, r * (0.3 + 0.3 * rnd(sd, k + 20)), c, 0.2);
                    }
                    label(px, x, y - r * 0.8, BRIAR);
                }
            }
            TerrainKind::Badlands => {
                px.glow(x, y, r * 1.1, BADLANDS, 0.24);
                px.ring_dashed(x, y, r, 1.0, BADLANDS, 0.35, 7.0);
                if detail {
                    // Swirling plasma currents, and fire flaring here and there.
                    for arm in 0..4 {
                        let a0 = rnd(sd, arm) * TAU + tick * 0.01;
                        let pts: Vec<(f32, f32)> = (0..=14)
                            .map(|k| {
                                let s = k as f32 / 14.0;
                                let a = a0 + s * 2.2;
                                (x + a.cos() * r * (0.2 + 0.75 * s), y + a.sin() * r * (0.2 + 0.75 * s))
                            })
                            .collect();
                        px.polyline(&pts, false, 2.0, mix(BADLANDS, PLASMA_FIRE, 0.4), 0.35, 0.0);
                    }
                    let flicker = (tick / 4.0) as u64;
                    for k in 0..4 {
                        let a = rnd(sd ^ flicker, k) * TAU;
                        let d = rnd(sd ^ flicker, k + 9).sqrt() * r * 0.9;
                        px.glow(x + a.cos() * d, y + a.sin() * d, r * 0.12, PLASMA_FIRE, 0.5);
                    }
                    label(px, x, y - r * 0.85, BADLANDS);
                }
            }
            TerrainKind::Nexus => {
                let (dx, dy) = (x2 - x, y2 - y);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (nx, ny) = (-dy / len, dx / len);
                px.line(x, y, x2, y2, u(1400.0).max(5.0), NEXUS, 0.14, 0.0);
                // The ribbon twists as it goes.
                let steps = (len / 5.0).max(10.0) as usize;
                for (k, col) in [(0.0f32, NEXUS), (1.7, NEXUS_HOT)] {
                    let pts: Vec<(f32, f32)> = (0..=steps)
                        .map(|j| {
                            let s = j as f32 / steps as f32;
                            let off = (s * 18.0 + tick * 0.12 + k).sin() * u(500.0);
                            (x + dx * s + nx * off, y + dy * s + ny * off)
                        })
                        .collect();
                    px.polyline(&pts, false, 1.5, mix(col, WHITE, 0.3), 0.85, 0.0);
                }
                label(px, (x + x2) / 2.0 + nx * 16.0, (y + y2) / 2.0 + ny * 16.0, NEXUS);
            }
            TerrainKind::TykensRift => {
                px.disc(x, y, r, scale(TYKEN, 0.18), 0.6);
                px.ring_dashed(x, y, r, 1.2, TYKEN, 0.7, 5.0);
                if detail {
                    // Energy spiralling in toward the heart of the rift.
                    for arm in 0..5 {
                        let a0 = arm as f32 / 5.0 * TAU - tick * 0.03;
                        let pts: Vec<(f32, f32)> = (0..=12)
                            .map(|k| {
                                let s = k as f32 / 12.0;
                                let a = a0 + s * 2.5;
                                (x + a.cos() * r * (1.0 - s * 0.85), y + a.sin() * r * (1.0 - s * 0.85))
                            })
                            .collect();
                        px.polyline(&pts, false, 1.0, TYKEN, 0.6, 0.0);
                    }
                    label(px, x, y - r - 6.0, TYKEN);
                }
                px.glow(x, y, u(800.0).max(2.0), mix(TYKEN, WHITE, 0.4), 0.6);
            }
            TerrainKind::GenesisPlanet => {
                let k = t.phase as f32 / 255.0;
                let col = mix(GENESIS, GENESIS_DYING, k * k);
                if k > 0.6 {
                    px.glow(x, y, r * (1.6 + 0.4 * (tick * 0.5).sin()), GENESIS_DYING, 0.3 * k);
                }
                px.disc(x, y, r, scale(col, 0.5), 1.0);
                px.ring(x, y, r, 1.3, col, 1.0);
                if detail {
                    px.disc(x - r * 0.25, y - r * 0.2, r * 0.35, scale(mix(col, rgb(0x4080ff), 0.5), 0.8), 0.8);
                    px.ring_dashed(x, y, u(GENESIS_REACH as f64), 1.0, GENESIS, 0.35, 4.0);
                    label(px, x, y - r - 6.0, col);
                }
            }
            TerrainKind::QuantumFilaments => {
                px.ring_dashed(x, y, r, 1.0, FILAMENT, 0.25, 3.0);
                if detail {
                    // The strands themselves don't show: now and then one glints.
                    let flicker = (tick / 6.0) as u64;
                    for k in 0..3 {
                        let a = rnd(sd ^ flicker, k) * TAU;
                        let d = rnd(sd ^ flicker, k + 50).sqrt() * r;
                        let (gx, gy) = (x + a.cos() * d, y + a.sin() * d);
                        let b = rnd(sd ^ flicker, k + 90) * TAU;
                        let h = u(500.0);
                        px.line(gx - b.cos() * h, gy - b.sin() * h, gx + b.cos() * h, gy + b.sin() * h, 1.0, FILAMENT, 0.6, 0.0);
                    }
                    label(px, x, y - r * 0.9, FILAMENT);
                }
            }
            TerrainKind::HekarasCorridor => {
                let (dx, dy) = (x2 - x, y2 - y);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (nx, ny) = (-dy / len * r, dx / len * r);
                px.line(x, y, x2, y2, r * 2.0, HEKARAS, 0.06, 0.0);
                for side in [1.0f32, -1.0] {
                    px.line(x + nx * side, y + ny * side, x2 + nx * side, y2 + ny * side, 1.2, HEKARAS, 0.7, 0.0);
                }
                if detail {
                    // Warp-5 markers down the lane.
                    let n = (len / u(3000.0).max(1.0)) as usize;
                    for k in 1..n.max(1) {
                        let s = k as f32 / n as f32;
                        let (mx, my) = (x + dx * s, y + dy * s);
                        px.line(mx - nx * 0.3, my - ny * 0.3, mx + nx * 0.3, my + ny * 0.3, 1.0, HEKARAS, 0.5, 0.0);
                    }
                    let l = (nx * nx + ny * ny).sqrt().max(1.0);
                    label(px, (x + x2) / 2.0 + nx / l * (r + 12.0), (y + y2) / 2.0 + ny / l * (r + 12.0), HEKARAS);
                }
            }
            TerrainKind::OmegaZone => {
                px.glow(x, y, r, OMEGA, 0.12);
                px.ring(x, y, r, 1.5, OMEGA, 0.8);
                if detail {
                    // Subspace shattered: cracks radiating from the molecule.
                    for k in 0..8 {
                        let a = rnd(sd, k) * TAU;
                        let pts: Vec<(f32, f32)> = (0..=5)
                            .map(|j| {
                                let s = j as f32 / 5.0;
                                let wob = (rnd(sd, k * 10 + j) - 0.5) * 0.25;
                                (x + (a + wob).cos() * r * s, y + (a + wob).sin() * r * s)
                            })
                            .collect();
                        px.polyline(&pts, false, 1.0, OMEGA, 0.45, 0.0);
                    }
                    label(px, x, y - r - 6.0, OMEGA);
                }
                px.glow(x, y, u(700.0).max(2.0), WHITE, 0.6 + 0.3 * (tick * 0.4).sin());
            }
            TerrainKind::NeutralZone => {
                let (dx, dy) = (x2 - x, y2 - y);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (nx, ny) = (-dy / len * r, dx / len * r);
                px.line(x, y, x2, y2, r * 2.0, NEUTRAL, 0.07, 0.0);
                for side in [1.0f32, -1.0] {
                    let (ax, ay, bx, by) = (x + nx * side, y + ny * side, x2 + nx * side, y2 + ny * side);
                    let n = (len / 8.0).max(2.0) as usize;
                    for k in (0..n).step_by(2) {
                        let (s0, s1) = (k as f32 / n as f32, ((k + 1) as f32 / n as f32).min(1.0));
                        px.line(ax + (bx - ax) * s0, ay + (by - ay) * s0, ax + (bx - ax) * s1, ay + (by - ay) * s1, 1.2, NEUTRAL, 0.75, 0.0);
                    }
                }
                if detail {
                    let l = (nx * nx + ny * ny).sqrt().max(1.0);
                    label(px, (x + x2) / 2.0 + nx / l * (r + 12.0), (y + y2) / 2.0 + ny / l * (r + 12.0), NEUTRAL);
                }
            }
            TerrainKind::GuardianOfForever => {
                let rr = r.max(3.0);
                px.glow(x, y, rr * 2.0, TIME_MIST, 0.25 + 0.15 * (tick * 0.2).sin());
                px.ring(x, y, rr, (rr * 0.3).max(2.0), scale(GUARDIAN, 0.6), 1.0);
                px.ring(x, y, rr * 1.15, 1.2, GUARDIAN, 1.0);
                px.ring(x, y, rr * 0.85, 1.0, GUARDIAN, 0.8);
                if detail {
                    label(px, x, y - rr * 2.0, GUARDIAN);
                }
            }
            TerrainKind::ThetaRadiation => {
                let left = t.phase as f32 / 255.0;
                px.glow(x, y, r * 1.1, THETA, 0.25 * left.max(0.2));
                if detail {
                    let flicker = (tick / 3.0) as u64;
                    for k in 0..16 {
                        let a = rnd(sd ^ flicker, k) * TAU;
                        let d = rnd(sd ^ flicker, k + 100).sqrt() * r;
                        px.dot(x + a.cos() * d, y + a.sin() * d, THETA, 0.7 * left.max(0.3));
                    }
                }
            }
        }
    }
}

/// Points of the Tempest's web in galaxy units: for each lane edge
/// (spoke), where it meets the rim and the core ring, plus lane centres.
fn tempest_points(t: &crate::proto::TempestInfo) -> (Vec<(f64, f64)>, Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let (cx, cy, r) = (t.x as f64, t.y as f64, t.r_out as f64);
    let k_in = t.r_in as f64 / r;
    let n = t.lanes.max(3) as usize;
    let rim = |s: f64| t.shape.rim(cx, cy, r, t.lanes, s);
    let inner = |(x, y): (f64, f64)| (cx + (x - cx) * k_in, cy + (y - cy) * k_in);
    let outer: Vec<(f64, f64)> = (0..n).map(|k| rim(k as f64 + 0.5)).collect();
    let inner_pts: Vec<(f64, f64)> = outer.iter().map(|&p| inner(p)).collect();
    let centres: Vec<(f64, f64)> = (0..n).map(|k| rim(k as f64)).collect();
    (outer, inner_pts, centres)
}

/// The Tempest's web: a neon tube of lanes from the core out to the rim,
/// as in Atari's Tempest (circle, square or triangle). It flashes when the
/// core is exposed.
pub fn draw_tempest_vec(px: &mut Canvas, f: &Frame, to: &dyn Fn(f64, f64) -> (f32, f32), u: &dyn Fn(f64) -> f32, detail: bool) {
    let Some(t) = &f.tempest else { return };
    let (cx, cy) = to(t.x as f64, t.y as f64);
    let ri = u(t.r_in as f64).max(2.0);
    let flash = t.exposed && (f.tick / 3) % 2 == 0;
    let col = if flash { rgb(0xffe040) } else { super::palette::TEMPEST_BLUE };
    let (outer, inner, centres) = tempest_points(t);
    let outer: Vec<(f32, f32)> = outer.iter().map(|&(x, y)| to(x, y)).collect();
    let inner: Vec<(f32, f32)> = inner.iter().map(|&(x, y)| to(x, y)).collect();
    if detail {
        px.polyline(&outer, true, 5.0, col, 0.15, 0.0);
    }
    px.polyline(&outer, true, 1.5, col, 1.0, 0.0);
    px.polyline(&inner, true, 1.2, col, 0.9, 0.0);
    for k in 0..outer.len() {
        let (a, b) = (inner[k], outer[k]);
        px.line(a.0, a.1, b.0, b.1, 1.0, col, if detail { 0.8 } else { 0.5 }, 0.0);
    }
    if detail {
        // Faint lane centre lines, and the core pulsing within.
        let k_in = t.r_in as f64 / t.r_out as f64;
        for &(x, y) in &centres {
            let (ax, ay) = to(t.x as f64 + (x - t.x as f64) * k_in, t.y as f64 + (y - t.y as f64) * k_in);
            let (bx, by) = to(x, y);
            px.line(ax, ay, bx, by, 0.8, col, 0.12, 3.0);
        }
        let pulse = 0.5 + 0.5 * ((f.tick as f32) * 0.4).sin();
        let core = if t.exposed { rgb(0xff4040) } else { rgb(0xffe040) };
        px.glow(cx, cy, ri * (1.2 + 0.3 * pulse), core, if t.exposed { 0.8 } else { 0.35 });
    }
}

/// The Tempest's web in braille.
pub fn draw_tempest_braille(b: &mut Braille, f: &Frame, to_dot: &dyn Fn(f64, f64) -> (f64, f64), _per_dot: f64) {
    let Some(t) = &f.tempest else { return };
    let col = if t.exposed && (f.tick / 3) % 2 == 0 { Color::Yellow } else { Color::Blue };
    let (outer, inner, _) = tempest_points(t);
    let n = outer.len();
    for k in 0..n {
        let (o0, o1) = (to_dot(outer[k].0, outer[k].1), to_dot(outer[(k + 1) % n].0, outer[(k + 1) % n].1));
        let (i0, i1) = (to_dot(inner[k].0, inner[k].1), to_dot(inner[(k + 1) % n].0, inner[(k + 1) % n].1));
        b.line(o0.0, o0.1, o1.0, o1.1, col, 1);
        b.line(i0.0, i0.1, i1.0, i1.1, col, 1);
        b.line(i0.0, i0.1, o0.0, o0.1, col, 1);
    }
}

pub(super) const SLICK: Rgb = rgb(0x9a80b8);
pub(super) const ARENA: Rgb = rgb(0xf4f0ff);
pub(super) const BEACON: Rgb = rgb(0x9060ff);
/// A tear in subspace: a hot magenta crack.
pub(super) const TEAR: Rgb = rgb(0xff50b0);
/// The Tamarian tether's ring: the sand and sun of El-Adrel.
pub(super) const TANAGRA: Rgb = rgb(0xe8b860);
/// The Scimitar's thalaron weapon charging: a sickly green-white.
pub(super) const THALARON: Rgb = rgb(0xa0ff90);
/// Vaal's power-draining field.
pub(super) const VAAL_FIELD: Rgb = rgb(0xd84a30);

/// A jagged crack through (0, 0), from -1 to 1 along x: points (x, y),
/// jittering every few frames.
pub(super) fn crack(seed: u64, frame: u64, n: usize) -> Vec<(f32, f32)> {
    let tilt = rnd(seed, 3) * TAU;
    let (c, s) = (tilt.cos(), tilt.sin());
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32 * 2.0 - 1.0;
            let off = if k == 0 || k == n { 0.0 } else { (rnd(seed ^ frame, k as u64) - 0.5) * 0.35 };
            (t * c - off * s, t * s + off * c)
        })
        .collect()
}
pub(super) const DISPLACE: Rgb = rgb(0x80e0ff);
pub(super) const DYSON: Rgb = rgb(0xd0a860);
/// Mingo City's lightning field: red bolts, gold-white at the core.
pub(super) const LIGHTNING: Rgb = rgb(0xff4030);
pub(super) const LIGHTNING_HOT: Rgb = rgb(0xffe080);

/// A crackling bolt of lightning round a circle, from angle `a0` to `a1`:
/// points (angle, radius as a fraction of the circle's), jittering every
/// frame.
pub(super) fn bolt(a0: f32, a1: f32, n: usize, seed: u64, frame: u64) -> Vec<(f32, f32)> {
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            let jit = if k == 0 || k == n { 0.0 } else { (rnd(seed ^ frame, k as u64) - 0.5) * 0.12 };
            (a0 + (a1 - a0) * t, 1.0 + jit)
        })
        .collect()
}

/// The edge of Armus's slick: a wobbling blob, `n` points round.
pub(super) fn slick_edge(z: &ZoneInfo, tick: f32, n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|k| {
            let a = k as f32 / n as f32 * TAU;
            let r = 0.9 + 0.07 * (5.0 * a + tick * 0.05).sin() + 0.05 * (9.0 * a - tick * 0.03).sin();
            (z.x as f64 + (a.cos() * r) as f64 * z.r as f64, z.y as f64 + (a.sin() * r) as f64 * z.r as f64)
        })
        .collect()
}

/// Patches of space marked out by alien incursions: Armus's slick, the
/// Metrons' arena, Species 10-C's beacons, the Caretaker's wave.
pub fn draw_zones_vec(px: &mut Canvas, f: &Frame, to: &dyn Fn(f64, f64) -> (f32, f32), u: &dyn Fn(f64) -> f32, detail: bool) {
    let tick = f.tick as f32;
    for z in &f.zones {
        let (x, y) = to(z.x as f64, z.y as f64);
        let r = u(z.r as f64).max(2.0);
        match z.kind {
            ZoneKind::Slick => {
                let pts: Vec<(f32, f32)> = slick_edge(z, tick, 48).into_iter().map(|(a, b)| to(a, b)).collect();
                px.fill_poly(&pts, rgb(0x120e18), 0.85);
                if detail {
                    // An oily rainbow sheen.
                    px.polyline(&pts, true, 4.0, mix(SLICK, rgb(0x40e0c0), 0.5 + 0.5 * (tick * 0.07).sin()), 0.25, 0.0);
                }
                px.polyline(&pts, true, 1.5, SLICK, 0.9, 0.0);
            }
            ZoneKind::Arena => {
                let pulse = 0.6 + 0.4 * (tick * 0.2).sin();
                if detail {
                    px.ring(x, y, r, 6.0, ARENA, 0.12 * pulse);
                    px.ring_dashed(x, y, r * 1.04, 1.0, ARENA, 0.5, 8.0);
                }
                px.ring(x, y, r, 2.0, ARENA, 0.6 + 0.3 * pulse);
            }
            ZoneKind::Beacon => {
                px.ring_dashed(x, y, r, 1.2, BEACON, 0.6, 5.0);
                px.disc(x, y, (r * 0.12).max(1.5), BEACON, 0.8);
            }
            ZoneKind::BeaconLit => {
                px.glow(x, y, r * 1.2, BEACON, 0.35);
                px.ring(x, y, r, 2.0, mix(BEACON, WHITE, 0.4), 1.0);
                px.disc(x, y, (r * 0.15).max(2.0), WHITE, 1.0);
            }
            ZoneKind::LightningField => {
                // A crackling ring of red lightning round Mingo City.
                let frame = (tick / 2.0) as u64;
                px.glow(x, y, r * 1.1, LIGHTNING, if detail { 0.12 } else { 0.18 });
                let arcs = if detail { 10 } else { 5 };
                for k in 0..arcs {
                    if rnd(frame, k as u64 + 77) < 0.25 {
                        continue;
                    }
                    let a0 = k as f32 / arcs as f32 * TAU + rnd(frame, k as u64) * 0.3;
                    let pts: Vec<(f32, f32)> = bolt(a0, a0 + TAU / arcs as f32 * 0.9, if detail { 10 } else { 4 }, k as u64, frame)
                        .into_iter()
                        .map(|(a, d)| (x + a.cos() * d * r, y + a.sin() * d * r))
                        .collect();
                    px.polyline(&pts, false, 3.0, LIGHTNING, 0.25, 0.0);
                    px.polyline(&pts, false, 1.1, mix(LIGHTNING, LIGHTNING_HOT, 0.5), 0.9, 0.0);
                }
            }
            ZoneKind::Displacement => {
                px.ring(x, y, r, 3.0, DISPLACE, 0.6);
                px.ring(x, y, r * 0.8, 1.5, DISPLACE, 0.4);
            }
            ZoneKind::DysonShell => {
                // A dark metal band with panel seams.
                let band = (r * 0.06).max(2.0);
                px.ring(x, y, r, band, scale(DYSON, 0.35), 0.9);
                px.ring(x, y, r + band / 2.0, 1.2, DYSON, 0.9);
                px.ring(x, y, r - band / 2.0, 1.0, DYSON, 0.6);
                if detail {
                    for k in 0..48 {
                        let a = k as f32 / 48.0 * TAU;
                        let (c, s) = (a.cos(), a.sin());
                        px.line(x + c * (r - band / 2.0), y + s * (r - band / 2.0), x + c * (r + band / 2.0), y + s * (r + band / 2.0), 0.8, DYSON, 0.5, 0.0);
                    }
                }
            }
            ZoneKind::DysonHatch => {
                px.ring(x, y, (r * 0.4).max(2.0), 1.5, DYSON, 0.9);
            }
            ZoneKind::DysonHatchOpen => {
                let pulse = 0.6 + 0.4 * (tick * 0.3).sin();
                px.glow(x, y, r * 0.9, rgb(0xfff0c0), 0.4 * pulse);
                px.ring_dashed(x, y, r * 0.6, 1.5, rgb(0xfff0c0), 0.9, 4.0);
            }
            ZoneKind::SubspaceTear => {
                let pts: Vec<(f32, f32)> = crack(z.x as u64 ^ (z.y as u64) << 20, (tick / 3.0) as u64, if detail { 9 } else { 4 })
                    .into_iter()
                    .map(|(a, b)| (x + a * r, y + b * r))
                    .collect();
                px.glow(x, y, r, TEAR, 0.25);
                px.polyline(&pts, false, 3.0, TEAR, 0.3, 0.0);
                px.polyline(&pts, false, 1.2, mix(TEAR, WHITE, 0.5), 0.95, 0.0);
            }
            ZoneKind::Tanagra => {
                px.ring_dashed(x, y, r, 1.5, TANAGRA, 0.8, 6.0);
                if detail {
                    px.ring(x, y, r, 5.0, TANAGRA, 0.1);
                }
            }
            ZoneKind::Thalaron => {
                // Rings of radiation pulsing inward as it charges.
                let pulse = (tick * 0.15).fract();
                px.glow(x, y, r, THALARON, 0.08);
                px.ring(x, y, r, 2.0, THALARON, 0.8);
                px.ring(x, y, r * (1.0 - pulse), 1.5, THALARON, 0.6 * pulse);
            }
            ZoneKind::EnergyDrain => {
                px.ring_dashed(x, y, r, 1.2, VAAL_FIELD, 0.6, 5.0);
                if detail {
                    let pulse = 0.5 + 0.5 * (tick * 0.2).sin();
                    px.ring(x, y, r * 0.97, 4.0, VAAL_FIELD, 0.08 * pulse);
                }
            }
        }
    }
}

pub(super) const RING_LIT: Rgb = rgb(0x8fd0ff);
pub(super) const RING_DARK: Rgb = rgb(0x26405c);

/// The Ringworld: a band around its planet, lit blue on its inner face, with
/// seams between its ten sections and the shadow squares circling inside.
pub fn draw_ring_vec(px: &mut Canvas, f: &Frame, to: &dyn Fn(f64, f64) -> (f32, f32), u: &dyn Fn(f64) -> f32, detail: bool) {
    let Some(r) = &f.ring else { return };
    let (x, y) = to(r.x as f64, r.y as f64);
    let rr = u(r.r as f64);
    let band = u(1400.0).max(2.0);
    px.ring(x, y, rr, band, RING_DARK, 0.85);
    px.ring(x, y, rr - band / 2.0, 1.4, RING_LIT, 0.9);
    px.ring(x, y, rr + band / 2.0, 1.0, scale(RING_LIT, 0.55), 0.8);
    if !detail {
        return;
    }
    px.glow(x, y, rr - band, RING_LIT, 0.06);
    let n = r.sections.len().max(1);
    for s in &r.sections {
        // A seam halfway to the next section.
        let a = ((s.y - r.y) as f32).atan2((s.x - r.x) as f32) + TAU / (2.0 * n as f32);
        let (c, sn) = (a.cos(), a.sin());
        px.line(x + c * (rr - band / 2.0), y + sn * (rr - band / 2.0), x + c * (rr + band / 2.0), y + sn * (rr + band / 2.0), 1.0, RING_LIT, 0.6, 0.0);
    }
    // The shadow squares, turning slowly inside the ring.
    for k in 0..5 {
        let a = f.tick as f32 * 0.002 + k as f32 * TAU / 5.0;
        let (c, sn) = (a.cos(), a.sin());
        let (sx, sy) = (x + c * rr * 0.45, y + sn * rr * 0.45);
        let half = rr * 0.12;
        px.line(sx - sn * half, sy + c * half, sx + sn * half, sy - c * half, (band * 0.6).max(2.0), RING_DARK, 0.9, 0.0);
        px.line(sx - sn * half, sy + c * half, sx + sn * half, sy - c * half, 0.8, scale(RING_LIT, 0.5), 0.6, 0.0);
    }
}

/// The Ringworld in braille.
pub fn draw_ring_braille(b: &mut Braille, f: &Frame, to_dot: &dyn Fn(f64, f64) -> (f64, f64), per_dot: f64) {
    let Some(r) = &f.ring else { return };
    let (x, y) = to_dot(r.x as f64, r.y as f64);
    let rr = r.r as f64 / per_dot;
    b.circle(x, y, rr, Color::Blue, 1);
    b.arc(x, y, rr - (700.0 / per_dot).max(1.0), Color::DarkBlue, 1, 2);
}

/// The alien zones in braille.
pub fn draw_zones_braille(b: &mut Braille, f: &Frame, to_dot: &dyn Fn(f64, f64) -> (f64, f64), per_dot: f64) {
    for z in &f.zones {
        let (x, y) = to_dot(z.x as f64, z.y as f64);
        let r = z.r as f64 / per_dot;
        match z.kind {
            ZoneKind::Slick => {
                let pts: Vec<(f64, f64)> = slick_edge(z, f.tick as f32, 32).into_iter().map(|(a, c)| to_dot(a, c)).collect();
                for k in 0..pts.len() {
                    let (p, q) = (pts[k], pts[(k + 1) % pts.len()]);
                    b.line(p.0, p.1, q.0, q.1, Color::DarkMagenta, 1);
                }
            }
            ZoneKind::Arena => b.circle(x, y, r, Color::White, 1),
            ZoneKind::Beacon => {
                b.arc(x, y, r, Color::DarkMagenta, 1, 3);
                b.dotf(x, y, Color::Magenta, 2);
            }
            ZoneKind::BeaconLit => {
                b.circle(x, y, r, Color::Magenta, 2);
                b.disc(x, y, (r * 0.2).max(1.0), Color::White, 2);
            }
            ZoneKind::Displacement => b.arc(x, y, r, Color::Cyan, 1, 2),
            ZoneKind::LightningField => b.arc(x, y, r, if f.tick / 2 % 2 == 0 { Color::Red } else { Color::Yellow }, 1, 2),
            ZoneKind::DysonShell => {
                b.circle(x, y, r, Color::DarkYellow, 1);
                b.arc(x, y, r + 1.0, Color::DarkYellow, 1, 3);
            }
            ZoneKind::DysonHatch => b.circle(x, y, (r * 0.4).max(1.0), Color::Yellow, 2),
            ZoneKind::DysonHatchOpen => b.disc(x, y, (r * 0.4).max(1.0), Color::White, 2),
            ZoneKind::SubspaceTear => {
                let pts = crack(z.x as u64 ^ (z.y as u64) << 20, f.tick as u64 / 3, 4);
                for k in 0..pts.len() - 1 {
                    let (p, q) = (pts[k], pts[k + 1]);
                    b.line(x + p.0 as f64 * r, y + p.1 as f64 * r, x + q.0 as f64 * r, y + q.1 as f64 * r, Color::Magenta, 2);
                }
            }
            ZoneKind::Tanagra => b.arc(x, y, r, Color::DarkYellow, 1, 3),
            ZoneKind::Thalaron => b.circle(x, y, r, if f.tick / 3 % 2 == 0 { Color::Green } else { Color::White }, 1),
            ZoneKind::EnergyDrain => b.arc(x, y, r, Color::DarkRed, 1, 3),
        }
    }
}

/// Terrain in braille graphics. `per_dot` is galaxy units per braille dot.
pub fn draw_braille(
    b: &mut Braille,
    f: &Frame,
    to_dot: &dyn Fn(f64, f64) -> (f64, f64),
    per_dot: f64,
    detail: bool,
    labels: &mut Vec<(i32, i32, String, Color, bool)>,
) {
    let tick = f.tick as f64;
    let (dw, dh) = b.dots();
    let on = |x: f64, y: f64, r: f64| x + r > 0.0 && y + r > 0.0 && x - r < dw as f64 && y - r < dh as f64;
    let mut name = |x: f64, y: f64, t: &TerrainInfo, col: Color| {
        if detail {
            labels.push(((x / 2.0) as i32 - t.name.len() as i32 / 2, (y / 4.0) as i32, t.name.clone(), col, false));
        }
    };
    for t in &f.terrain {
        let (x, y) = to_dot(t.x as f64, t.y as f64);
        let (x2, y2) = to_dot(t.x2 as f64, t.y2 as f64);
        let r = t.r as f64 / per_dot;
        let sd = seed(t);
        let segment = matches!(
            t.kind,
            TerrainKind::Corridor | TerrainKind::Comet | TerrainKind::Wormhole | TerrainKind::GalacticBarrier | TerrainKind::Nexus | TerrainKind::HekarasCorridor | TerrainKind::NeutralZone
        );
        if !segment && !on(x, y, r) {
            continue;
        }
        match t.kind {
            TerrainKind::Nebula => {
                b.arc(x, y, r, Color::DarkMagenta, 0, 2);
                if detail {
                    for k in 0..60 {
                        let a = rnd(sd, k) as f64 * std::f64::consts::TAU;
                        let d = (rnd(sd, k + 300) as f64).sqrt() * r;
                        b.dotf(x + a.cos() * d, y + a.sin() * d, Color::DarkMagenta, 0);
                    }
                }
                name(x, y - r * 0.8, t, Color::Magenta);
            }
            TerrainKind::IonStorm => {
                b.arc(x, y, r, Color::Blue, 0, 3);
                if detail {
                    let flicker = (tick / 2.0) as u64;
                    let a = rnd(sd ^ flicker, 1) as f64 * std::f64::consts::TAU;
                    b.line(x, y, x + a.cos() * r * 0.8, y + a.sin() * r * 0.8, Color::Cyan, 1);
                }
                name(x, y - r * 0.85, t, Color::Blue);
            }
            TerrainKind::Asteroids => {
                let n = if detail { 50 } else { 8 };
                for k in 0..n {
                    let a = rnd(sd, k) as f64 * std::f64::consts::TAU;
                    let d = (rnd(sd, k + 100) as f64).sqrt() * r;
                    b.dotf(x + a.cos() * d, y + a.sin() * d, Color::DarkYellow, 1);
                }
                name(x, y - r * 0.9, t, Color::DarkYellow);
            }
            TerrainKind::BlackHole => {
                if detail {
                    b.arc(x, y, r, Color::DarkRed, 0, 4);
                }
                b.circle(x, y, (1300.0 / per_dot).max(1.5), Color::DarkYellow, 1);
                b.circle(x, y, (700.0 / per_dot).max(1.0), Color::Magenta, 1);
                name(x, y - r * 0.5, t, Color::DarkYellow);
            }
            TerrainKind::GreatBlackHole => {
                b.arc(x, y, r, Color::DarkBlue, 0, 3);
                let n = if detail { 40 } else { 12 };
                for k in 0..ARMS {
                    let pts: Vec<(f64, f64)> = (0..=n)
                        .map(|j| {
                            let (a, d) = arm_point(k, j as f32 / n as f32, t.r as f32, tick as f32, sd);
                            (x + a.cos() as f64 * d as f64 * r, y + a.sin() as f64 * d as f64 * r)
                        })
                        .collect();
                    for j in 0..n {
                        let col = if j > n * 2 / 3 { Color::Cyan } else if j > n / 3 { Color::Blue } else { Color::DarkMagenta };
                        b.line(pts[j].0, pts[j].1, pts[j + 1].0, pts[j + 1].1, col, 1);
                    }
                }
                b.circle(x, y, (GREAT_HORIZON as f64 / per_dot).max(1.5), Color::White, 2);
                name(x, y - r * 0.6, t, Color::Cyan);
            }
            TerrainKind::Pulsar => {
                b.disc(x, y, (300.0 / per_dot).max(0.8), Color::White, 2);
                if detail {
                    let a = tick * 0.15;
                    b.line(x - a.cos() * r, y - a.sin() * r, x + a.cos() * r, y + a.sin() * r, Color::Cyan, 1);
                    b.arc(x, y, r, Color::DarkCyan, 0, 4);
                    let since = PULSAR_PERIOD as f64 - t.phase as f64;
                    if since < 12.0 {
                        b.circle(x, y, r * since / 12.0, Color::White, 1);
                    }
                }
                name(x, y - r * 0.9, t, Color::Cyan);
            }
            TerrainKind::Wormhole => {
                for (mx, my) in [(x, y), (x2, y2)] {
                    if on(mx, my, r * 2.0) {
                        let mr = (900.0 / per_dot).max(1.5);
                        b.circle(mx, my, mr, Color::Magenta, 1);
                        b.circle(mx, my, mr * 0.5, Color::Magenta, 1);
                        name(mx, my - mr * 1.8, t, Color::Magenta);
                    }
                }
            }
            TerrainKind::Derelict => {
                let s = (450.0 / per_dot).max(1.0);
                b.line(x - s, y - s, x + s, y + s, Color::Grey, 1);
                b.line(x - s, y + s, x + s, y - s, Color::Grey, 1);
                name(x, y - s * 3.0, t, Color::Grey);
            }
            TerrainKind::Corridor => {
                if detail {
                    let (dx, dy) = (x2 - x, y2 - y);
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    let (nx, ny) = (-dy / len * r, dx / len * r);
                    b.line_pattern(x + nx, y + ny, x2 + nx, y2 + ny, Color::DarkCyan, 0, 3);
                    b.line_pattern(x - nx, y - ny, x2 - nx, y2 - ny, Color::DarkCyan, 0, 3);
                    name((x + x2) / 2.0 + nx * 3.0, (y + y2) / 2.0 + ny * 3.0, t, Color::DarkCyan);
                } else {
                    b.line_pattern(x, y, x2, y2, Color::DarkCyan, 0, 2);
                }
            }
            TerrainKind::Star => {
                b.disc(x, y, (1200.0 / per_dot).max(1.0), Color::Yellow, 2);
                if detail {
                    b.arc(x, y, r, Color::DarkYellow, 0, 3);
                }
                name(x, y - r * 0.85, t, Color::Yellow);
            }
            TerrainKind::Comet => {
                b.line(x, y, x2, y2, Color::DarkCyan, 0);
                b.disc(x, y, (400.0 / per_dot).max(0.8), Color::White, 2);
                name(x, y - 3.0, t, Color::White);
            }
            TerrainKind::TachyonGrid => {
                b.arc(x, y, r, Color::DarkCyan, 0, 2);
                name(x, y - r * 0.9, t, Color::Cyan);
            }
            TerrainKind::Minefield => {
                b.arc(x, y, r, Color::Red, 0, 3);
                name(x, y - r * 0.85, t, Color::Red);
            }
            TerrainKind::ChronitonField => {
                b.arc(x, y, r, Color::DarkGreen, 0, 2);
                b.arc(x, y, r * 0.5, Color::DarkGreen, 0, 3);
                name(x, y - r * 0.85, t, Color::Green);
            }
            TerrainKind::GravitonEddy => {
                for arm in 0..3 {
                    let a0 = tick * 0.04 + arm as f64 * std::f64::consts::TAU / 3.0;
                    let mut prev = (x, y);
                    for k in 1..12 {
                        let s = k as f64 / 11.0;
                        let a = a0 + s * 3.0;
                        let p = (x + a.cos() * r * s, y + a.sin() * r * s);
                        b.line(prev.0, prev.1, p.0, p.1, Color::Blue, 0);
                        prev = p;
                    }
                }
                name(x, y - r * 0.9, t, Color::Blue);
            }
            TerrainKind::Magnetar => {
                b.disc(x, y, (350.0 / per_dot).max(0.8), Color::Magenta, 2);
                b.arc(x, y, r, Color::DarkMagenta, 0, 4);
                name(x, y - r * 0.9, t, Color::Magenta);
            }
            TerrainKind::MetreonCloud => {
                b.arc(x, y, r, Color::DarkYellow, 0, 2);
                if detail {
                    for k in 0..40 {
                        let a = rnd(sd, k) as f64 * std::f64::consts::TAU;
                        let d = (rnd(sd, k + 300) as f64).sqrt() * r;
                        b.dotf(x + a.cos() * d, y + a.sin() * d, Color::DarkYellow, 0);
                    }
                }
                name(x, y - r * 0.8, t, Color::Yellow);
            }
            TerrainKind::TetryonField => {
                b.arc(x, y, r, Color::Yellow, 0, 4);
                name(x, y - r * 0.9, t, Color::Yellow);
            }
            TerrainKind::Planetoid => {
                b.disc(x, y, r, Color::DarkGrey, 1);
                b.circle(x, y, r, Color::Grey, 1);
                name(x, y - r - 2.0, t, Color::Grey);
            }
            TerrainKind::GalacticBarrier => {
                b.line(x, y, x2, y2, Color::Magenta, 1);
                if detail {
                    name((x + x2) / 2.0, (y + y2) / 2.0 - 3.0, t, Color::Magenta);
                }
            }
            TerrainKind::FluidicRift => {
                let rr = (900.0 / per_dot).max(1.5);
                b.circle(x, y, rr, Color::Magenta, 1);
                b.line(x - rr, y, x + rr, y, Color::White, 1);
                name(x, y - rr * 2.0, t, Color::Magenta);
            }
            TerrainKind::AbandonedStation => {
                let s = (500.0 / per_dot).max(1.0);
                b.circle(x, y, s, Color::Grey, 1);
                b.line(x, y - s * 1.8, x, y + s * 1.8, Color::Grey, 1);
                name(x, y - s * 3.0, t, Color::Grey);
            }
            TerrainKind::BriarPatch => {
                b.arc(x, y, r, Color::DarkYellow, 0, 2);
                if detail {
                    for k in 0..50 {
                        let a = rnd(sd, k) as f64 * std::f64::consts::TAU;
                        let d = (rnd(sd, k + 300) as f64).sqrt() * r;
                        b.dotf(x + a.cos() * d, y + a.sin() * d, Color::DarkRed, 0);
                    }
                }
                name(x, y - r * 0.8, t, Color::DarkYellow);
            }
            TerrainKind::Badlands => {
                b.arc(x, y, r, Color::Red, 0, 3);
                if detail {
                    let flicker = (tick / 4.0) as u64;
                    for k in 0..4 {
                        let a = rnd(sd ^ flicker, k) as f64 * std::f64::consts::TAU;
                        let d = (rnd(sd ^ flicker, k + 9) as f64).sqrt() * r * 0.9;
                        b.disc(x + a.cos() * d, y + a.sin() * d, 1.0, Color::Yellow, 2);
                    }
                }
                name(x, y - r * 0.85, t, Color::Red);
            }
            TerrainKind::Nexus => {
                b.line(x, y, x2, y2, Color::Yellow, 1);
                if detail {
                    name((x + x2) / 2.0, (y + y2) / 2.0 - 3.0, t, Color::Yellow);
                }
            }
            TerrainKind::TykensRift => {
                b.arc(x, y, r, Color::DarkMagenta, 0, 3);
                b.circle(x, y, (800.0 / per_dot).max(1.0), Color::Magenta, 1);
                name(x, y - r - 2.0, t, Color::Magenta);
            }
            TerrainKind::GenesisPlanet => {
                let dying = t.phase > 200 && (f.tick / 3) % 2 == 0;
                b.disc(x, y, r, if dying { Color::Red } else { Color::DarkGreen }, 1);
                b.circle(x, y, r, Color::Green, 1);
                if detail {
                    b.arc(x, y, 3500.0 / per_dot, Color::DarkGreen, 0, 3);
                }
                name(x, y - r - 2.0, t, Color::Green);
            }
            TerrainKind::QuantumFilaments => {
                b.arc(x, y, r, Color::DarkBlue, 0, 4);
                name(x, y - r * 0.9, t, Color::Blue);
            }
            TerrainKind::HekarasCorridor => {
                let (dx, dy) = (x2 - x, y2 - y);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (nx, ny) = (-dy / len * r, dx / len * r);
                b.line(x + nx, y + ny, x2 + nx, y2 + ny, Color::Blue, 1);
                b.line(x - nx, y - ny, x2 - nx, y2 - ny, Color::Blue, 1);
                if detail {
                    name((x + x2) / 2.0 + nx * 1.5, (y + y2) / 2.0 + ny * 1.5, t, Color::Blue);
                }
            }
            TerrainKind::OmegaZone => {
                b.circle(x, y, r, Color::White, 1);
                b.disc(x, y, (700.0 / per_dot).max(1.0), Color::White, 2);
                name(x, y - r - 2.0, t, Color::White);
            }
            TerrainKind::NeutralZone => {
                let (dx, dy) = (x2 - x, y2 - y);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (nx, ny) = (-dy / len * r, dx / len * r);
                b.line_pattern(x + nx, y + ny, x2 + nx, y2 + ny, Color::DarkGreen, 0, 3);
                b.line_pattern(x - nx, y - ny, x2 - nx, y2 - ny, Color::DarkGreen, 0, 3);
                if detail {
                    name((x + x2) / 2.0 + nx * 1.5, (y + y2) / 2.0 + ny * 1.5, t, Color::Green);
                }
            }
            TerrainKind::GuardianOfForever => {
                let rr = r.max(1.5);
                b.circle(x, y, rr, Color::Grey, 2);
                b.circle(x, y, rr * 0.6, Color::Cyan, 1);
                name(x, y - rr * 2.0, t, Color::Grey);
            }
            TerrainKind::ThetaRadiation => {
                if detail {
                    for k in 0..20 {
                        let a = rnd(sd, k) as f64 * std::f64::consts::TAU;
                        let d = (rnd(sd, k + 300) as f64).sqrt() * r;
                        b.dotf(x + a.cos() * d, y + a.sin() * d, Color::Green, 0);
                    }
                } else {
                    b.arc(x, y, r, Color::DarkGreen, 0, 3);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::SelfInfo;

    /// Renders every kind of terrain, tactical style, into
    /// $TMPDIR/netrek-terrain.ppm for eyeballing.
    #[test]
    fn terrain_gallery() {
        let kinds = crate::server::terrain::ALL_KINDS;
        let cell = 16_000.0;
        let terrain: Vec<TerrainInfo> = kinds
            .iter()
            .enumerate()
            .map(|(k, &kind)| {
                let (cx, cy) = ((k % 6) as f64 * cell + cell / 2.0, (k / 6) as f64 * cell + cell / 2.0);
                let r = match kind {
                    TerrainKind::Planetoid => 1500.0,
                    TerrainKind::AbandonedStation => 1500.0,
                    TerrainKind::FluidicRift => 700.0,
                    TerrainKind::GreatBlackHole => 7500.0,
                    TerrainKind::GalacticBarrier => 350.0,
                    TerrainKind::Derelict => 900.0,
                    TerrainKind::Wormhole => 600.0,
                    TerrainKind::Corridor => 700.0,
                    TerrainKind::Comet => 500.0,
                    TerrainKind::Nexus => 600.0,
                    TerrainKind::HekarasCorridor => 1300.0,
                    TerrainKind::NeutralZone => 1500.0,
                    TerrainKind::GenesisPlanet => 1500.0,
                    TerrainKind::GuardianOfForever => 800.0,
                    TerrainKind::TykensRift => 3500.0,
                    TerrainKind::ThetaRadiation => 2500.0,
                    _ => 5500.0,
                };
                let (x2, y2) = match kind {
                    TerrainKind::Wormhole => (cx + 4000.0, cy + 3000.0),
                    TerrainKind::Corridor => (cx + 6000.0, cy - 4000.0),
                    TerrainKind::Comet => (cx + 6000.0, cy + 4000.0),
                    TerrainKind::GalacticBarrier | TerrainKind::Nexus | TerrainKind::NeutralZone => (cx + 5000.0, cy + 5000.0),
                    TerrainKind::HekarasCorridor => (cx + 6000.0, cy - 4000.0),
                    _ => (cx, cy),
                };
                let (x, y) = match kind {
                    TerrainKind::Wormhole | TerrainKind::Corridor | TerrainKind::Comet | TerrainKind::HekarasCorridor => (cx - 5000.0, cy),
                    TerrainKind::GalacticBarrier | TerrainKind::Nexus | TerrainKind::NeutralZone => (cx - 5000.0, cy - 5000.0),
                    _ => (cx, cy),
                };
                TerrainInfo { kind, x: x as i32, y: y as i32, r: r as i32, x2: x2 as i32, y2: y2 as i32, phase: 95, name: format!("{:?}", kind) }
            })
            .collect();
        let f = Frame {
            tick: 37,
            me: 0,
            me_info: SelfInfo::default(),
            players: vec![],
            torps: vec![],
            phasers: vec![],
            planets: vec![],
            webs: vec![],
            loot: vec![],
            terrain,
            treaties: vec![],
            leaders: vec![],
            tempest: None,
            zones: Vec::new(),
            ring: None,
            observers: vec![],
            prizes: vec![],
            open_teams: vec![],
            team_planets: [0; 4],
            starbase_teams: vec![],
            banner: None,
        };
        let upd = 40.0;
        let rows = kinds.len().div_ceil(6) as f64;
        let mut c = Canvas::new((6.0 * cell / upd) as i32, (rows * cell / upd) as i32, [0.0, 0.0, 0.0]);
        let tr = TextRenderer::load();
        let to = |x: f64, y: f64| ((x / upd) as f32, (y / upd) as f32);
        let u = |d: f64| (d / upd) as f32;
        draw_vec(&mut c, &f, &to, &u, true, &tr, 12.0);
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-terrain.ppm"), out).unwrap();
    }

    /// Renders the Ringworld to $TMPDIR/netrek-ring.ppm, for eyeballing.
    #[test]
    fn ringworld_gallery() {
        use crate::proto::{RingInfo, RingSectionInfo};
        let (cx, cy, r) = (15_000.0f64, 15_000.0f64, 8500.0f64);
        let sections = (0..10)
            .map(|k| {
                let a = 0.3 + k as f64 * std::f64::consts::TAU / 10.0;
                RingSectionInfo { name: format!("S{}", k), x: (cx + a.cos() * r) as i32, y: (cy + a.sin() * r) as i32 }
            })
            .collect();
        let f = Frame {
            tick: 500,
            me: 0,
            me_info: SelfInfo::default(),
            players: vec![],
            torps: vec![],
            phasers: vec![],
            planets: vec![],
            webs: vec![],
            loot: vec![],
            terrain: vec![],
            treaties: vec![],
            leaders: vec![],
            tempest: None,
            zones: Vec::new(),
            ring: Some(RingInfo { x: cx as i32, y: cy as i32, r: r as i32, sections, kzin: 3 }),
            observers: vec![],
            prizes: vec![],
            open_teams: vec![],
            team_planets: [0; 4],
            starbase_teams: vec![],
            banner: None,
        };
        let upd = 40.0;
        let mut c = Canvas::new((30_000.0 / upd) as i32, (30_000.0 / upd) as i32, [0.0, 0.0, 0.0]);
        let to = |x: f64, y: f64| ((x / upd) as f32, (y / upd) as f32);
        let u = |d: f64| (d / upd) as f32;
        draw_ring_vec(&mut c, &f, &to, &u, true);
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-ring.ppm"), out).unwrap();
    }
}
