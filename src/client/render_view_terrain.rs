//! Space terrain on the viewscreen: every kind of terrain in 3D, in the
//! same perspective as the ships and planets. Clouds are volumes of glowing
//! puffs you can fly into, fields are marked out on the galaxy's plane,
//! stars and planetoids are lit spheres, and wrecks are solid models.

use super::palette::{mix, rgb, scale, Rgb};
use super::render_terrain::*;
use super::render_view::{draw_mesh, Cam, Pose, V3, EYE, NEAR};
use super::shipart::Part;
use super::sixel::TextRenderer;
use super::vg::Canvas;
use crate::proto::{Frame, TerrainInfo, TerrainKind, ZoneInfo, ZoneKind};
use std::f32::consts::{PI, TAU};

/// How far the viewscreen sees terrain.
const RANGE: f32 = 45_000.0;

/// Whether `t` is a stretch between two points rather than a round patch.
fn is_segment(t: &TerrainInfo) -> bool {
    matches!(t.kind, TerrainKind::Corridor | TerrainKind::Comet | TerrainKind::GalacticBarrier)
}

/// Depth to sort `t` by among everything else in view, or None if it's
/// out of sight.
pub(super) fn depth(cam: &Cam, t: &TerrainInfo) -> Option<f32> {
    let r = t.r as f32;
    let tan = (cam.cx / cam.focal) * 1.3;
    if is_segment(t) {
        let (a, b) = (cam.local(t.x as f32, t.y as f32), cam.local(t.x2 as f32, t.y2 as f32));
        if a.1.max(b.1) < NEAR || a.1.min(b.1) > RANGE {
            return None;
        }
        return Some((a.1.max(NEAR) + b.1.max(NEAR)) / 2.0);
    }
    let mouths: &[(i32, i32)] = if t.kind == TerrainKind::Wormhole { &[(t.x, t.y), (t.x2, t.y2)] } else { &[(t.x, t.y)] };
    let reach = match t.kind {
        TerrainKind::Wormhole | TerrainKind::FluidicRift => 2000.0,
        TerrainKind::Derelict | TerrainKind::AbandonedStation => 1500.0,
        TerrainKind::BlackHole => r.min(3000.0),
        _ => r,
    };
    mouths
        .iter()
        .filter_map(|&(x, y)| {
            let (lat, z) = cam.local(x as f32, y as f32);
            let wide = z.max(0.0) * tan + reach;
            (z + reach > NEAR && z - reach < RANGE && lat.abs() < wide).then_some(z)
        })
        .reduce(f32::min)
}

/// A soft glow at a point in camera space, `r` galaxy units across.
fn glow3(c: &mut Canvas, cam: &Cam, p: V3, r: f32, col: Rgb, a: f32) {
    if p[2] > NEAR {
        let (x, y) = cam.screen3(p);
        let rp = (r / p[2] * cam.focal).clamp(1.0, cam.cx * 4.0);
        c.glow(x, y, rp, col, a);
    }
}

/// A line through points in camera space, clipped at the near plane;
/// `dash` > 0 leaves out every other segment.
fn poly3(c: &mut Canvas, cam: &Cam, pts: &[V3], closed: bool, width: f32, col: Rgb, a: f32, dash: bool) {
    let n = pts.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    for i in 0..segs {
        if dash && i % 2 == 1 {
            continue;
        }
        if let Some((p, q)) = cam.segment3(pts[i], pts[(i + 1) % n]) {
            c.line(p.0, p.1, q.0, q.1, width, col, a, 0.0);
        }
    }
}

/// A circle on the galaxy's plane (or at height `alt`).
fn ring3(c: &mut Canvas, cam: &Cam, x: f32, y: f32, r: f32, alt: f32, width: f32, col: Rgb, a: f32, dash: bool) {
    let n = if dash { 72 } else { 64 };
    let pts: Vec<V3> = (0..n).map(|k| {
        let t = k as f32 / n as f32 * TAU;
        cam.at(x + t.cos() * r, y + t.sin() * r, alt)
    }).collect();
    poly3(c, cam, &pts, true, width, col, a, dash);
}

/// A lit sphere of radius `r` (galaxy units) centred at `p` (camera space).
fn sphere(c: &mut Canvas, cam: &Cam, p: V3, r: f32, col: Rgb, glow: f32) -> Option<(f32, f32, f32)> {
    if p[2] < NEAR + r * 0.5 {
        return None;
    }
    let (x, y) = cam.screen3(p);
    let rp = (r / p[2] * cam.focal).max(1.5);
    if glow > 0.0 {
        c.glow(x, y, rp * 1.5, col, glow);
    }
    c.disc(x, y, rp, scale(col, 0.22), 1.0);
    if rp > 3.0 {
        // Sunlit from the upper left.
        c.disc(x - rp * 0.16, y - rp * 0.16, rp * 0.8, scale(col, 0.42), 0.85);
        c.disc(x - rp * 0.3, y - rp * 0.3, rp * 0.48, scale(col, 0.65), 0.7);
        c.glow(x - rp * 0.4, y - rp * 0.4, rp * 0.4, mix(col, WHITE, 0.5), 0.4);
    }
    c.ring(x, y, rp, 1.0, mix(col, WHITE, 0.2), 0.6);
    Some((x, y, rp))
}

/// A volume of glowing puffs: nebulae, storms and gas clouds.
fn cloud(c: &mut Canvas, cam: &Cam, t: &TerrainInfo, cols: (Rgb, Rgb), n: u64, a: f32, sd: u64) {
    let r = t.r as f32;
    let mut puffs: Vec<(V3, f32, Rgb)> = (0..n)
        .map(|k| {
            let ang = rnd(sd, k) * TAU;
            let d = rnd(sd, k + 50).sqrt() * r * 0.85;
            let alt = (rnd(sd, k + 300) - 0.5) * r * 0.45;
            let p = cam.at(t.x as f32 + ang.cos() * d, t.y as f32 + ang.sin() * d, alt);
            (p, r * (0.18 + 0.22 * rnd(sd, k + 20)), mix(cols.0, cols.1, rnd(sd, k + 90)))
        })
        .filter(|q| q.0[2] > NEAR)
        .collect();
    puffs.sort_by(|p, q| q.0[2].total_cmp(&p.0[2]));
    for (p, size, col) in puffs {
        // Puffs right on top of us would fill the screen: thin them out.
        let near = (p[2] / (size * 1.5)).min(1.0);
        glow3(c, cam, p, size, col, a * near);
    }
}

/// Name a piece of terrain, above it, when it's near enough to read.
fn label(c: &mut Canvas, tr: &TextRenderer, cam: &Cam, x: f32, y: f32, alt: f32, name: &str, col: Rgb, fs: f32) {
    let p = cam.at(x, y, alt);
    if name.is_empty() || p[2] < NEAR * 6.0 || p[2] > 25_000.0 {
        return;
    }
    let (sx, sy) = cam.screen3(p);
    c.text_centered(tr, sx, sy - fs * 0.4, name, fs * 0.85, scale(col, 0.85));
}

/// One piece of terrain, in 3D.
pub(super) fn draw(c: &mut Canvas, cam: &Cam, t: &TerrainInfo, tick: u32, tr: &TextRenderer, fs: f32) {
    let tickf = tick as f32;
    let sd = seed(t);
    let (x, y, r) = (t.x as f32, t.y as f32, t.r as f32);
    let ctr = cam.at(x, y, 0.0);
    match t.kind {
        TerrainKind::Nebula => {
            cloud(c, cam, t, (NEBULA, rgb(0xd060c0)), 40, 0.13, sd);
            label(c, tr, cam, x, y, r * 0.35, &t.name, NEBULA, fs);
        }
        TerrainKind::IonStorm => {
            cloud(c, cam, t, (STORM, rgb(0xa0c8ff)), 26, 0.10, sd);
            // Lightning, different every frame, forking through the storm.
            let flicker = (tick / 2) as u64;
            for k in 0..4u64 {
                if rnd(sd ^ flicker, k + 500) > 0.6 {
                    continue;
                }
                let ang = rnd(sd ^ flicker, k) * TAU;
                let d = rnd(sd ^ flicker, k + 40) * r * 0.6;
                let mut p = [x + ang.cos() * d, y + ang.sin() * d, (rnd(sd ^ flicker, k + 80) - 0.5) * r * 0.3];
                let mut dir = [rnd(sd ^ flicker, k + 7) - 0.5, rnd(sd ^ flicker, k + 8) - 0.5, -0.6];
                let mut pts = vec![cam.at(p[0], p[1], p[2])];
                for s in 0..8u64 {
                    for (j, v) in dir.iter_mut().enumerate() {
                        *v += (rnd(sd ^ flicker, k * 31 + s * 3 + j as u64) - 0.5) * 0.9;
                    }
                    let l = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt().max(0.01);
                    for j in 0..3 {
                        p[j] += dir[j] / l * r * 0.1;
                    }
                    pts.push(cam.at(p[0], p[1], p[2]));
                }
                poly3(c, cam, &pts, false, 3.5, STORM, 0.25, false);
                poly3(c, cam, &pts, false, 1.2, mix(STORM, WHITE, 0.7), 0.95, false);
                glow3(c, cam, pts[0], r * 0.25, mix(STORM, WHITE, 0.4), 0.35);
            }
            label(c, tr, cam, x, y, r * 0.35, &t.name, STORM, fs);
        }
        TerrainKind::MetreonCloud => {
            cloud(c, cam, t, (METREON, rgb(0xff5020)), 32, 0.12, sd);
            // Sparks of unstable gas.
            let flicker = (tick / 3) as u64;
            for k in 0..10u64 {
                let ang = rnd(sd ^ flicker, k) * TAU;
                let d = rnd(sd ^ flicker, k + 30).sqrt() * r * 0.8;
                let p = cam.at(x + ang.cos() * d, y + ang.sin() * d, (rnd(sd ^ flicker, k + 60) - 0.5) * r * 0.3);
                glow3(c, cam, p, 120.0, rgb(0xffe0a0), 0.8);
            }
            label(c, tr, cam, x, y, r * 0.35, &t.name, METREON, fs);
        }
        TerrainKind::Asteroids => {
            ring3(c, cam, x, y, r, 0.0, 1.0, ROCK, 0.25, true);
            let mut rocks: Vec<(V3, f32, u64)> = (0..70u64)
                .map(|k| {
                    let ang = rnd(sd, k) * TAU;
                    let d = rnd(sd, k + 100).sqrt() * r * 0.95;
                    let alt = (rnd(sd, k + 400) - 0.5) * r * 0.25;
                    (cam.at(x + ang.cos() * d, y + ang.sin() * d, alt), 150.0 + 300.0 * rnd(sd, k + 200), k)
                })
                .filter(|q| q.0[2] > NEAR && q.0[2] < 25_000.0)
                .collect();
            rocks.sort_by(|p, q| q.0[2].total_cmp(&p.0[2]));
            for (p, size, k) in rocks {
                let (sx, sy) = cam.screen3(p);
                let rr = size / p[2] * cam.focal;
                if rr < 1.0 {
                    c.dot(sx, sy, ROCK, 0.9);
                    continue;
                }
                // A tumbling, lumpy rock, lit from the upper left.
                let spin = tickf * 0.02 * (rnd(sd, k + 600) - 0.5) + rnd(sd, k + 700) * TAU;
                let shape = |kk: f32| -> Vec<(f32, f32)> {
                    (0..8)
                        .map(|j| {
                            let b = j as f32 / 8.0 * TAU + spin;
                            let m = rr * kk * (0.65 + 0.4 * rnd(sd, k * 13 + j));
                            (sx + b.cos() * m, sy + b.sin() * m * 0.85)
                        })
                        .collect()
                };
                c.fill_poly(&shape(1.0), scale(ROCK, 0.35), 1.0);
                if rr > 2.5 {
                    let hi: Vec<(f32, f32)> = shape(0.62).into_iter().map(|(px, py)| (px - rr * 0.2, py - rr * 0.2)).collect();
                    c.fill_poly(&hi, scale(ROCK, 0.7), 0.8);
                    c.polyline(&shape(1.0), true, 0.7, ROCK, 0.6, 0.0);
                }
            }
            label(c, tr, cam, x, y, r * 0.3, &t.name, ROCK, fs);
        }
        TerrainKind::BlackHole => {
            ring3(c, cam, x, y, r, 0.0, 1.0, ACCRETION, 0.15, true);
            glow3(c, cam, ctr, 2800.0, ACCRETION, 0.35);
            // The accretion disc on the plane: its far half behind the hole,
            // its near half in front, hot streaks whirling round it.
            let disc = |c: &mut Canvas, front: bool| {
                for k in 0..7 {
                    let rr = 950.0 + k as f32 * 260.0;
                    let col = mix(mix(ACCRETION, WHITE, 0.7), rgb(0xff5020), k as f32 / 6.0);
                    let n = 72;
                    let pts: Vec<V3> = (0..=n).map(|j| {
                        let a = j as f32 / n as f32 * TAU;
                        cam.at(x + a.cos() * rr, y + a.sin() * rr, 0.0)
                    }).collect();
                    for j in 0..n {
                        let mid = (pts[j][2] + pts[j + 1][2]) / 2.0;
                        if (mid < ctr[2]) != front {
                            continue;
                        }
                        let a = j as f32 / n as f32 * TAU;
                        let streak = 0.5 + 0.5 * (a * 3.0 - tickf * 0.25 * (1.6 - k as f32 * 0.15)).sin();
                        if let Some((p, q)) = cam.segment3(pts[j], pts[j + 1]) {
                            c.line(p.0, p.1, q.0, q.1, 2.2, col, 0.25 + 0.55 * streak, 0.0);
                        }
                    }
                }
            };
            disc(c, false);
            if let Some((sx, sy, rp)) = sphere(c, cam, ctr, 700.0, [0.0, 0.0, 0.0], 0.0) {
                c.disc(sx, sy, rp, [0.0, 0.0, 0.0], 1.0);
                // The photon ring, bent light round the event horizon.
                c.ring(sx, sy, rp * 1.08, 1.5, mix(ACCRETION, WHITE, 0.6), 0.8);
                c.ring(sx, sy, rp * 1.2, 1.0, rgb(0xb070ff), 0.4);
            }
            disc(c, true);
            label(c, tr, cam, x, y, 2200.0, &t.name, ACCRETION, fs);
        }
        TerrainKind::Pulsar => {
            ring3(c, cam, x, y, r, 0.0, 1.0, PULSAR, 0.2, true);
            glow3(c, cam, ctr, 1500.0, PULSAR, 0.6);
            glow3(c, cam, ctr, 400.0, WHITE, 1.0);
            // Two beams sweeping round, tipped out of the plane.
            let a = tickf * 0.15;
            let tilt: f32 = 0.45;
            for s in [1.0f32, -1.0] {
                let end = [x + a.cos() * r * tilt.cos() * s, y + a.sin() * r * tilt.cos() * s, r * tilt.sin() * s];
                let pts = [ctr, cam.at(end[0], end[1], end[2])];
                poly3(c, cam, &pts, false, 7.0, PULSAR, 0.12, false);
                poly3(c, cam, &pts, false, 1.5, mix(PULSAR, WHITE, 0.6), 0.7, false);
            }
            // The pulse: a shock ring racing outward.
            let since = PULSAR_PERIOD - t.phase as f32;
            if since < 12.0 {
                let k = since / 12.0;
                ring3(c, cam, x, y, r * k.max(0.05), 0.0, 3.0, mix(PULSAR, WHITE, 0.5), 1.0 - k, false);
            }
            label(c, tr, cam, x, y, 1800.0, &t.name, PULSAR, fs);
        }
        TerrainKind::Wormhole => {
            for (mx, my) in [(x, y), (t.x2 as f32, t.y2 as f32)] {
                let p = cam.at(mx, my, 500.0);
                if p[2] < NEAR * 2.0 {
                    continue;
                }
                let (sx, sy) = cam.screen3(p);
                let mr = (900.0 / p[2] * cam.focal).max(3.0);
                c.glow(sx, sy, mr * 2.0, WORMHOLE, 0.45);
                // A tunnel: rings shrinking away into the throat.
                c.disc(sx, sy, mr, scale(WORMHOLE, 0.12), 1.0);
                for k in 0..6 {
                    let f = 1.0 - k as f32 * 0.15;
                    let off = k as f32 * mr * 0.04;
                    let spin = tickf * 0.1 + k as f32;
                    c.ring(sx + off * spin.cos(), sy + off * spin.sin(), mr * f, 1.2, mix(WORMHOLE, WHITE, k as f32 * 0.1), 0.9 - k as f32 * 0.1);
                }
                let pts: Vec<(f32, f32)> = (0..60)
                    .map(|j| {
                        let s = j as f32 / 60.0;
                        let a = tickf * 0.25 + s * TAU * 2.5;
                        (sx + a.cos() * mr * s, sy + a.sin() * mr * s)
                    })
                    .collect();
                c.polyline(&pts, false, 1.3, mix(WORMHOLE, WHITE, 0.5), 0.8, 0.0);
                c.glow(sx, sy, mr * 0.3, WHITE, 0.6);
                label(c, tr, cam, mx, my, 1700.0, &t.name, WORMHOLE, fs);
            }
        }
        TerrainKind::Derelict => {
            // A broken hull, tumbling slowly, with debris round it.
            let heading = tickf * 0.01 + rnd(sd, 1) * TAU;
            let pose = Pose::new(cam, x, y, 450.0, heading, 6.0);
            if pose.z0 > NEAR * 2.0 {
                let parts = vec![
                    Part::Poly { pts: vec![(-1.0, -0.3), (-0.2, -0.38), (0.1, -0.35), (0.2, 0.3), (-0.3, 0.22), (-1.0, 0.25)], fill: true },
                    Part::Poly { pts: vec![(0.45, -0.25), (1.0, 0.1), (0.95, 0.2), (0.4, -0.12)], fill: true },
                    Part::Hole { pts: vec![(-0.6, -0.15), (-0.35, -0.2), (-0.4, 0.1)] },
                    Part::Line { a: (0.2, 0.0), b: (0.45, -0.2) },
                ];
                draw_mesh(c, &pose, parts, WRECK, scale(WRECK, 0.55), 1.0, false);
                for k in 0..8u64 {
                    let a = rnd(sd, k + 10) * TAU + tickf * 0.004;
                    let d = 500.0 + rnd(sd, k + 20) * 600.0;
                    let p = cam.at(x + a.cos() * d, y + a.sin() * d, (rnd(sd, k + 30) - 0.5) * 500.0);
                    if p[2] > NEAR {
                        let (dx, dy) = cam.screen3(p);
                        c.disc(dx, dy, (40.0 / p[2] * cam.focal).max(0.7), WRECK, 0.8);
                    }
                }
                // Sparks from a failing power conduit.
                if (tick / 4 + sd as u32) % 5 == 0 {
                    glow3(c, cam, pose.to_cam([-0.2, -0.3, 0.1]), 150.0, rgb(0x9ad0ff), 0.9);
                }
            }
            label(c, tr, cam, x, y, 1000.0, &t.name, WRECK, fs);
        }
        TerrainKind::AbandonedStation => {
            // A K-7 style station: a hub, three spokes and pods, turning slowly.
            let pose = Pose::new(cam, x, y, 500.0, tickf * 0.003 + rnd(sd, 1) * TAU, 6.0);
            ring3(c, cam, x, y, r, 0.0, 1.0, STATION, 0.2, true);
            if pose.z0 > NEAR * 2.0 {
                let mut parts = vec![Part::Circle { c: (0.0, 0.0), r: 0.55, fill: true }];
                for k in 0..3 {
                    let a = k as f32 / 3.0 * TAU - PI / 2.0;
                    let (ca, sa) = (a.cos(), a.sin());
                    let (nx, ny) = (-sa * 0.09, ca * 0.09);
                    parts.push(Part::Poly {
                        pts: vec![(ca * 0.5 + nx, sa * 0.5 + ny), (ca * 1.4 + nx, sa * 1.4 + ny), (ca * 1.4 - nx, sa * 1.4 - ny), (ca * 0.5 - nx, sa * 0.5 - ny)],
                        fill: true,
                    });
                    parts.push(Part::Circle { c: (ca * 1.6, sa * 1.6), r: 0.35, fill: true });
                }
                draw_mesh(c, &pose, parts, STATION, scale(STATION, 0.6), 1.0, false);
                if (tick / 8) % 2 == 0 {
                    glow3(c, cam, pose.to_cam([0.0, 0.0, 0.75]), 120.0, rgb(0xff4040), 0.9);
                }
            }
            label(c, tr, cam, x, y, 1600.0, &t.name, STATION, fs);
        }
        TerrainKind::Corridor => {
            let (x2, y2) = (t.x2 as f32, t.y2 as f32);
            let (dx, dy) = (x2 - x, y2 - y);
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            let (ux, uy) = (dx / len, dy / len);
            let (nx, ny) = (-uy, ux);
            // Dashed rails along both edges.
            let n = (len / 600.0).max(2.0) as usize;
            for s in [-1.0f32, 1.0] {
                let pts: Vec<V3> = (0..=n)
                    .map(|k| {
                        let f = k as f32 / n as f32;
                        cam.at(x + dx * f + nx * r * s, y + dy * f + ny * r * s, 0.0)
                    })
                    .collect();
                poly3(c, cam, &pts, false, 1.2, SLIPSTREAM, 0.55, true);
            }
            // Chevrons flowing down the stream, and streaks of current.
            let gap = 2500.0;
            let mut d = (tickf * 60.0) % gap;
            while d < len {
                let (cx, cy) = (x + ux * d, y + uy * d);
                let k = r * 0.6;
                let tip = cam.at(cx, cy, 0.0);
                for side in [1.0f32, -1.0] {
                    let back = cam.at(cx - ux * k + nx * k * side, cy - uy * k + ny * k * side, 0.0);
                    poly3(c, cam, &[back, tip], false, 1.4, SLIPSTREAM, 0.7, false);
                }
                d += gap;
            }
            for k in 0..30u64 {
                let f = (rnd(sd, k) + tickf * 0.01 * (0.5 + rnd(sd, k + 9))).fract();
                let off = (rnd(sd, k + 40) - 0.5) * 1.6 * r;
                let alt = (rnd(sd, k + 80) - 0.5) * r * 0.8;
                let (px, py) = (x + dx * f + nx * off, y + dy * f + ny * off);
                let a = cam.at(px, py, alt);
                let b = cam.at(px - ux * 500.0, py - uy * 500.0, alt);
                poly3(c, cam, &[b, a], false, 1.0, mix(SLIPSTREAM, WHITE, 0.4), 0.5, false);
            }
            let (mx, my) = (x + dx * 0.5, y + dy * 0.5);
            label(c, tr, cam, mx, my, r, &t.name, SLIPSTREAM, fs);
        }
        TerrainKind::Star => {
            glow3(c, cam, ctr, r, STAR, 0.22);
            glow3(c, cam, ctr, 2400.0, mix(STAR, WHITE, 0.4), 0.7);
            // Prominences licking off the surface.
            for k in 0..5u64 {
                let a = rnd(sd, k) * TAU + tickf * 0.01;
                let q = cam.at(x + a.cos() * 1250.0, y, a.sin() * 1250.0);
                glow3(c, cam, q, 450.0 + 200.0 * (tickf * 0.1 + k as f32).sin(), rgb(0xff8030), 0.5);
            }
            if let Some((sx, sy, rp)) = sphere(c, cam, ctr, 1200.0, mix(STAR, WHITE, 0.5), 0.0) {
                c.disc(sx, sy, rp, mix(STAR, WHITE, 0.55), 1.0);
                c.disc(sx, sy, rp * 0.8, mix(STAR, WHITE, 0.8), 0.8);
                c.glow(sx, sy, rp * 0.6, WHITE, 0.8);
            }
            label(c, tr, cam, x, y, 2600.0, &t.name, STAR, fs);
        }
        TerrainKind::Comet => {
            let (x2, y2) = (t.x2 as f32, t.y2 as f32);
            // A tail that widens and fades away from the head.
            let n = 36;
            for k in (0..n).rev() {
                let s = k as f32 / n as f32;
                let p = cam.at(x + (x2 - x) * s, y + (y2 - y) * s, s * 600.0);
                glow3(c, cam, p, 500.0 * (1.0 + 4.0 * s), COMET, 0.28 * (1.0 - s).powf(1.6));
            }
            // A straight blue ion tail.
            let ion = [cam.at(x, y, 0.0), cam.at(x + (x2 - x) * 0.8, y + (y2 - y) * 0.8, 900.0)];
            poly3(c, cam, &ion, false, 1.2, rgb(0x80b8ff), 0.5, false);
            glow3(c, cam, ctr, 900.0, COMET, 0.8);
            glow3(c, cam, ctr, 250.0, WHITE, 1.0);
            label(c, tr, cam, x, y, 1200.0, &t.name, COMET, fs);
        }
        TerrainKind::TachyonGrid => {
            ring3(c, cam, x, y, r, 0.0, 1.2, TACHYON, 0.5, false);
            // A glowing grid laid out on the plane.
            let step = 1500.0;
            let mut o = -r + (r % step);
            while o < r {
                let chord = (r * r - o * o).max(0.0).sqrt();
                let lines = [
                    [cam.at(x + o, y - chord, 0.0), cam.at(x + o, y + chord, 0.0)],
                    [cam.at(x - chord, y + o, 0.0), cam.at(x + chord, y + o, 0.0)],
                ];
                for l in lines {
                    poly3(c, cam, &l, false, 1.0, TACHYON, 0.22, false);
                }
                o += step;
            }
            // A pulse sweeping across it.
            let sweep = -r + (tickf * 150.0) % (2.0 * r);
            let chord = (r * r - sweep * sweep).max(0.0).sqrt();
            poly3(c, cam, &[cam.at(x + sweep, y - chord, 0.0), cam.at(x + sweep, y + chord, 0.0)], false, 2.5, mix(TACHYON, WHITE, 0.4), 0.5, false);
            label(c, tr, cam, x, y, r * 0.2, &t.name, TACHYON, fs);
        }
        TerrainKind::Minefield => {
            ring3(c, cam, x, y, r, 0.0, 1.3, MINES, 0.6, true);
            // Warning buoys round the edge (the mines themselves can't be seen).
            for k in 0..8u64 {
                let a = k as f32 / 8.0 * TAU + rnd(sd, 1);
                let (bx, by) = (x + a.cos() * r, y + a.sin() * r);
                let (lo, hi) = (cam.at(bx, by, 0.0), cam.at(bx, by, 450.0));
                poly3(c, cam, &[lo, hi], false, 1.0, scale(MINES, 0.7), 0.8, false);
                if (tick / 5 + k as u32) % 4 == 0 {
                    glow3(c, cam, hi, 250.0, MINES, 0.95);
                } else {
                    glow3(c, cam, hi, 80.0, MINES, 0.6);
                }
            }
            label(c, tr, cam, x, y, r * 0.15, &t.name, MINES, fs);
        }
        TerrainKind::ChronitonField => {
            cloud(c, cam, t, (CHRONITON, rgb(0x80ffe0)), 12, 0.05, sd);
            for k in [1.0f32, 0.66, 0.33] {
                ring3(c, cam, x, y, r * k, 0.0, 1.0, CHRONITON, 0.4, true);
            }
            // Clock marks, and one slowly turning hand of arc.
            for k in 0..12 {
                let a = k as f32 / 12.0 * TAU;
                let (ca, sa) = (a.cos(), a.sin());
                poly3(c, cam, &[cam.at(x + ca * r * 0.9, y + sa * r * 0.9, 0.0), cam.at(x + ca * r, y + sa * r, 0.0)], false, 1.5, CHRONITON, 0.7, false);
            }
            let a0 = tickf * 0.01;
            let arc: Vec<V3> = (0..20).map(|k| {
                let a = a0 + k as f32 * 0.05;
                cam.at(x + a.cos() * r * 0.8, y + a.sin() * r * 0.8, 0.0)
            }).collect();
            poly3(c, cam, &arc, false, 2.5, CHRONITON, 0.75, false);
            label(c, tr, cam, x, y, r * 0.3, &t.name, CHRONITON, fs);
        }
        TerrainKind::GravitonEddy => {
            glow3(c, cam, ctr, r * 0.5, EDDY, 0.15);
            // Spiral arms on the plane, dipping into a funnel at the centre.
            for arm in 0..5 {
                let pts: Vec<V3> = (0..40)
                    .map(|k| {
                        let s = k as f32 / 39.0;
                        let a = tickf * 0.04 + arm as f32 * TAU / 5.0 + s * 3.2;
                        cam.at(x + a.cos() * r * s, y + a.sin() * r * s, -(1.0 - s).powi(2) * r * 0.35)
                    })
                    .collect();
                poly3(c, cam, &pts, false, 1.6, EDDY, 0.6, false);
            }
            label(c, tr, cam, x, y, r * 0.25, &t.name, EDDY, fs);
        }
        TerrainKind::Magnetar => {
            ring3(c, cam, x, y, r, 0.0, 1.0, MAGNETAR, 0.25, true);
            // Field lines looping out of one pole and into the other.
            for j in 0..8 {
                let phi = j as f32 / 8.0 * TAU + tickf * 0.005;
                let (cp, sp) = (phi.cos(), phi.sin());
                for k in 1..4 {
                    let ry = r * 0.25 * k as f32;
                    let rx = ry * 0.6;
                    let pts: Vec<V3> = (0..=24).map(|i| {
                        let a = i as f32 / 24.0 * PI;
                        cam.at(x + cp * a.sin() * rx, y + sp * a.sin() * rx, a.cos() * ry)
                    }).collect();
                    poly3(c, cam, &pts, false, 1.0, MAGNETAR, 0.3, false);
                }
            }
            glow3(c, cam, ctr, 1600.0, MAGNETAR, 0.6);
            sphere(c, cam, ctr, 350.0, rgb(0xffd0f8), 0.0);
            label(c, tr, cam, x, y, r * 0.8, &t.name, MAGNETAR, fs);
        }
        TerrainKind::TetryonField => {
            ring3(c, cam, x, y, r, 0.0, 1.0, TETRYON, 0.4, true);
            // Crackling particles all through the field.
            let flicker = (tick / 3) as u64;
            for k in 0..80u64 {
                let a = rnd(sd ^ flicker, k) * TAU;
                let d = rnd(sd ^ flicker, k + 100).sqrt() * r;
                let p = cam.at(x + a.cos() * d, y + a.sin() * d, (rnd(sd ^ flicker, k + 200) - 0.5) * r * 0.4);
                glow3(c, cam, p, 90.0, TETRYON, 0.85);
            }
            label(c, tr, cam, x, y, r * 0.25, &t.name, TETRYON, fs);
        }
        TerrainKind::Planetoid => {
            if let Some((sx, sy, rp)) = sphere(c, cam, ctr, r, PLANETOID, 0.0) {
                // Craters on the side facing us.
                if rp > 6.0 {
                    for k in 0..9u64 {
                        let (u, v) = (rnd(sd, k) * 1.6 - 0.8, rnd(sd, k + 30) * 1.6 - 0.8);
                        if u * u + v * v > 0.6 {
                            continue;
                        }
                        let cr = rp * (0.06 + 0.1 * rnd(sd, k + 60));
                        let (cx, cy) = (sx + u * rp, sy + v * rp);
                        c.disc(cx, cy, cr, scale(PLANETOID, 0.25), 0.7);
                        c.ring(cx + cr * 0.15, cy + cr * 0.15, cr, 1.0, scale(PLANETOID, 0.85), 0.6);
                    }
                }
            }
            label(c, tr, cam, x, y, r * 1.2, &t.name, PLANETOID, fs);
        }
        TerrainKind::GalacticBarrier => {
            // A shimmering wall of energy standing across the plane.
            let (x2, y2) = (t.x2 as f32, t.y2 as f32);
            let hgt = 2500.0;
            let n = 48;
            let at = |s: f32, alt: f32| cam.at(x + (x2 - x) * s, y + (y2 - y) * s, alt);
            for k in 0..n {
                let (s0, s1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
                let quad = [at(s0, -hgt), at(s1, -hgt), at(s1, hgt), at(s0, hgt)];
                if quad.iter().all(|q| q[2] > NEAR) {
                    let sp: Vec<(f32, f32)> = quad.iter().map(|&q| cam.screen3(q)).collect();
                    let shimmer = 0.5 + 0.5 * (s0 * 40.0 + tickf * 0.2).sin();
                    c.fill_poly(&sp, BARRIER, 0.05 + 0.05 * shimmer);
                }
            }
            for edge in [-hgt, 0.0, hgt] {
                let pts: Vec<V3> = (0..=n * 2)
                    .map(|k| {
                        let s = k as f32 / (n * 2) as f32;
                        at(s, edge + (s * 80.0 + tickf * 0.3).sin() * 150.0)
                    })
                    .collect();
                let (w, a) = if edge == 0.0 { (1.5, 0.8) } else { (1.0, 0.45) };
                poly3(c, cam, &pts, false, w, mix(BARRIER, WHITE, 0.3), a, false);
            }
            // Bolts of energy running up and down it.
            for k in 0..6u64 {
                let s = (rnd(sd, k) + tickf * 0.003 * (1.0 + rnd(sd, k + 5))).fract();
                poly3(c, cam, &[at(s, -hgt), at(s, hgt)], false, 1.2, mix(BARRIER, WHITE, 0.5), 0.5, false);
            }
            label(c, tr, cam, (x + x2) / 2.0, (y + y2) / 2.0, hgt * 1.1, &t.name, BARRIER, fs);
        }
        TerrainKind::FluidicRift => {
            // A jagged tear in space, standing up out of the plane.
            let p = cam.at(x, y, 400.0);
            if p[2] > NEAR * 2.0 {
                let (sx, sy) = cam.screen3(p);
                let rr = (900.0 / p[2] * cam.focal).max(4.0);
                c.glow(sx, sy, rr * 2.4, RIFT, 0.4);
                let tear = |k: f32, spin: f32| -> Vec<(f32, f32)> {
                    (0..22)
                        .map(|j| {
                            let a = j as f32 / 22.0 * TAU + spin;
                            let jag = if j % 2 == 0 { 1.0 } else { 0.5 + 0.15 * (tickf * 0.3 + j as f32).sin() };
                            (sx + a.cos() * rr * jag * k * 0.6, sy + a.sin() * rr * jag * k)
                        })
                        .collect()
                };
                c.fill_poly(&tear(1.0, tickf * 0.05), scale(RIFT, 0.3), 0.9);
                c.fill_poly(&tear(0.55, -tickf * 0.07), rgb(0x200818), 1.0);
                c.polyline(&tear(1.0, tickf * 0.05), true, 1.4, mix(RIFT, WHITE, 0.4), 0.9, 0.0);
            }
            label(c, tr, cam, x, y, 1800.0, &t.name, RIFT, fs);
        }
    }
}

/// When you're inside a cloud or field, it tints the whole viewscreen.
pub(super) fn draw_inside(c: &mut Canvas, cam: &Cam, f: &Frame, w: f32, h: f32, tr: &TextRenderer, fs: f32) {
    let tick = f.tick;
    for t in &f.terrain {
        let (dx, dy) = (t.x as f32 - cam.x, t.y as f32 - cam.y);
        let d = (dx * dx + dy * dy).sqrt();
        let r = t.r as f32;
        if is_segment(t) || d > r {
            continue;
        }
        let deep = 1.0 - d / r;
        let (col, a) = match t.kind {
            TerrainKind::Nebula => (NEBULA, 0.14 + 0.12 * deep),
            TerrainKind::MetreonCloud => (METREON, 0.12 + 0.1 * deep),
            TerrainKind::IonStorm => {
                // Lightning flashes now and then.
                let flash = rnd(seed(t) ^ (tick / 2) as u64, 999) < 0.12;
                (if flash { mix(STORM, WHITE, 0.6) } else { STORM }, if flash { 0.3 } else { 0.1 + 0.08 * deep })
            }
            TerrainKind::ChronitonField => (CHRONITON, 0.07),
            TerrainKind::TetryonField => (TETRYON, 0.05 + 0.03 * ((tick as f32) * 0.7).sin().abs()),
            TerrainKind::TachyonGrid => (TACHYON, 0.04),
            TerrainKind::Star => (STAR, 0.1 + 0.25 * deep),
            TerrainKind::Magnetar => (MAGNETAR, 0.05 + 0.1 * deep),
            TerrainKind::GravitonEddy => (EDDY, 0.05),
            TerrainKind::Minefield => (MINES, 0.04),
            _ => continue,
        };
        c.fill_rect(0.0, 0.0, w, h, col, a);
        let name = if t.name.is_empty() { format!("{:?}", t.kind) } else { t.name.clone() };
        c.text(tr, 12.0, h - 12.0, &format!("INSIDE: {}", name.to_uppercase()), fs * 0.9, mix(col, WHITE, 0.4));
    }
}

fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The Ringworld is drawn in this many pieces round its circumference.
const RING_SEGS: usize = 144;
/// The bottom and top of its band (the floor faces the sun at its centre).
/// The top rim is just below a ship's eye, so from outside you look over
/// the near wall to the sunlit far side.
const RING_LO: f32 = -2200.0;
const RING_HI: f32 = 300.0;
const RING_HALF: f32 = (RING_HI - RING_LO) / 2.0;
const RING_MID: f32 = (RING_HI + RING_LO) / 2.0;
/// Where the shadow squares orbit (a fraction of the ring's radius), and
/// their half-width.
const SQUARE_ORBIT: f32 = 0.45;
const SQUARE_HALF: f32 = 0.12;

/// Seen from outside, the Ringworld is tipped toward the viewer (more the
/// farther out you are), so you look down into its sunlit floor rather
/// than edge on. Returns how much to raise a point at (x, y) so it sits on
/// the tipped ring; zero inside it.
pub(super) fn ring_lift<'a>(cam: &'a Cam, f: &Frame) -> impl Fn(f32, f32) -> f32 + 'a {
    let tip = f.ring.as_ref().map_or(0.0, |r| {
        let d = (cam.x - r.x as f32).hypot(cam.y - r.y as f32);
        let out = ((d / r.r as f32 - 1.0) / 0.6).clamp(0.0, 1.0);
        0.3 * out * out * (3.0 - 2.0 * out)
    });
    let cz = f.ring.as_ref().map_or(0.0, |r| cam.local(r.x as f32, r.y as f32).1);
    move |x, y| (cam.local(x, y).1 - cz) * tip
}

/// A piece of the Ringworld, sorted by depth with everything else.
#[derive(Clone, Copy)]
pub(super) enum RingPiece {
    Seg(usize),
    Square(usize),
    Sun,
}

fn ring_angle(k: usize) -> f32 {
    k as f32 / RING_SEGS as f32 * TAU
}

/// The angle of shadow square `j` (they turn slowly, as on the tactical).
fn square_angle(tick: u32, j: usize) -> f32 {
    tick as f32 * 0.002 + j as f32 * TAU / 5.0
}

/// The pieces of the Ringworld in view, with their depths.
pub(super) fn ring_pieces(cam: &Cam, f: &Frame) -> Vec<(f32, RingPiece)> {
    let Some(ring) = &f.ring else { return Vec::new() };
    let (cx, cy, rr) = (ring.x as f32, ring.y as f32, ring.r as f32);
    let tan = (cam.cx / cam.focal) * 1.3;
    let seg_len = rr * TAU / RING_SEGS as f32;
    let mut out = Vec::new();
    let mut add = |x: f32, y: f32, size: f32, piece: RingPiece, bias: f32| {
        let (lat, z) = cam.local(x, y);
        if z + size > NEAR && z - size < RANGE * 1.5 && lat.abs() < z.max(0.0) * tan + size {
            // Pieces of the ring sort a little behind what's on them.
            out.push((z.max(NEAR) + bias, piece));
        }
    };
    for k in 0..RING_SEGS {
        let a = ring_angle(k) + TAU / RING_SEGS as f32 / 2.0;
        add(cx + a.cos() * rr, cy + a.sin() * rr, seg_len + RING_HALF, RingPiece::Seg(k), 700.0);
    }
    for j in 0..5 {
        let a = square_angle(f.tick, j);
        add(cx + a.cos() * rr * SQUARE_ORBIT, cy + a.sin() * rr * SQUARE_ORBIT, rr * SQUARE_HALF, RingPiece::Square(j), 0.0);
    }
    add(cx, cy, 2000.0, RingPiece::Sun, 0.0);
    out
}

/// One piece of the Ringworld: a stretch of its band (land and sea on the
/// sunward face, bare structure outside), a shadow square, or its sun.
pub(super) fn draw_ring_piece(c: &mut Canvas, cam: &Cam, f: &Frame, piece: RingPiece, tr: &TextRenderer, fs: f32) {
    let Some(ring) = &f.ring else { return };
    let (cx, cy, rr) = (ring.x as f32, ring.y as f32, ring.r as f32);
    let lift = ring_lift(cam, f);
    let cam_at = |x: f32, y: f32, h: f32| cam.at(x, y, h + lift(x, y));
    match piece {
        RingPiece::Sun => {
            let p = cam_at(cx, cy, 0.0);
            glow3(c, cam, p, 3000.0, rgb(0xfff0c0), 0.45);
            sphere(c, cam, p, 450.0, rgb(0xfff4d0), 0.0);
            glow3(c, cam, p, 500.0, WHITE, 0.9);
            label(c, tr, cam, cx, cy, 1500.0 + lift(cx, cy), "Ringworld", RING_LIT, fs);
        }
        RingPiece::Square(j) => {
            let a = square_angle(f.tick, j);
            let (ca, sa) = (a.cos(), a.sin());
            let (mx, my) = (cx + ca * rr * SQUARE_ORBIT, cy + sa * rr * SQUARE_ORBIT);
            let half = rr * SQUARE_HALF;
            let h = RING_HALF * 0.9;
            let m = RING_MID;
            let quad = [
                cam_at(mx - sa * half, my + ca * half, m - h),
                cam_at(mx + sa * half, my - ca * half, m - h),
                cam_at(mx + sa * half, my - ca * half, m + h),
                cam_at(mx - sa * half, my + ca * half, m + h),
            ];
            if let Some(sp) = cam.clip_poly(&quad) {
                c.fill_poly(&sp, [8.0, 10.0, 16.0], 0.95);
                c.polyline(&sp, true, 1.0, scale(RING_LIT, 0.5), 0.7, 0.0);
            }
        }
        RingPiece::Seg(k) => {
            let (a0, a1) = (ring_angle(k), ring_angle(k + 1));
            let am = (a0 + a1) / 2.0;
            let at = |a: f32, h: f32| cam_at(cx + a.cos() * rr, cy + a.sin() * rr, h);
            // Which face we see: the sunward floor if we're inside the ring.
            let inside = {
                // Facing the sunward floor: we're on the same side of this
                // piece of band as the ring's centre.
                let (q0, q1, q3, mid) = (at(a0, RING_LO), at(a1, RING_LO), at(a0, RING_HI), cam_at(cx, cy, RING_MID));
                let (u, v) = (sub3(q1, q0), sub3(q3, q0));
                let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                let eye = [0.0, EYE, 0.0];
                dot3(n, sub3(mid, q0)).signum() == dot3(n, sub3(eye, q0)).signum()
            };
            let (_, zm) = cam.local(cx + am.cos() * rr, cy + am.sin() * rr);
            let haze = (1.0 - zm / (RANGE * 1.5)).clamp(0.25, 1.0);
            if inside {
                // Day or night under the shadow squares.
                let shade = (0..5)
                    .map(|j| {
                        let d = (am - square_angle(f.tick, j) + PI).rem_euclid(TAU) - PI;
                        d.abs()
                    })
                    .fold(f32::MAX, f32::min);
                let day = ((shade - 0.2) / 0.12).clamp(0.0, 1.0);
                let rows = 4;
                for row in 0..rows {
                    let (h0, h1) = (RING_LO + 2.0 * RING_HALF * row as f32 / rows as f32, RING_LO + 2.0 * RING_HALF * (row + 1) as f32 / rows as f32);
                    let Some(sp) = cam.clip_poly(&[at(a0, h0), at(a1, h0), at(a1, h1), at(a0, h1)]) else { continue };
                    // Land, sea, desert and cloud, fixed to the ring.
                    let hv = rnd(ring.x as u64 ^ 0x51, (k * 7 + row) as u64);
                    let ground = if hv < 0.38 {
                        rgb(0x3d7fc0)
                    } else if hv < 0.75 {
                        rgb(0x4f9a58)
                    } else if hv < 0.9 {
                        rgb(0xb8a070)
                    } else {
                        rgb(0xdfe8f0)
                    };
                    let lit = mix(scale(ground, 0.12), ground, day);
                    c.fill_poly(&sp, scale(lit, haze), 1.0);
                    c.polyline(&sp, true, 0.6, scale(lit, haze), 0.9, 0.0);
                    // City lights on the night side.
                    if day < 0.5 && (0.38..0.9).contains(&hv) {
                        let p = at(am, (h0 + h1) / 2.0);
                        if p[2] > NEAR {
                            let (lx, ly) = cam.screen3(p);
                            c.glow(lx, ly, 2.5, rgb(0xffd890), 0.8 * (1.0 - day * 2.0));
                        }
                    }
                }
            } else {
                // The outside: bare structure, with seams between sections.
                if let Some(sp) = cam.clip_poly(&[at(a0, RING_LO), at(a1, RING_LO), at(a1, RING_HI), at(a0, RING_HI)]) {
                    let ribs = if k % 2 == 0 { 1.0 } else { 0.85 };
                    c.fill_poly(&sp, scale(RING_DARK, haze * ribs), 1.0);
                    c.polyline(&sp, true, 0.6, scale(RING_DARK, haze * ribs), 0.9, 0.0);
                }
                let n = ring.sections.len().max(1);
                if k % (RING_SEGS / n).max(1) == 0 {
                    poly3(c, cam, &[at(a0, RING_LO), at(a0, RING_HI)], false, 1.0, scale(RING_LIT, 0.6), 0.6 * haze, false);
                }
            }
            // The rim walls along both edges.
            for h in [RING_LO, RING_HI] {
                poly3(c, cam, &[at(a0, h), at(a1, h)], false, 1.5, RING_LIT, 0.85 * haze, false);
            }
        }
    }
}

/// A Dyson sphere's shell is drawn in this many panels round and up.
const SHELL_LON: usize = 40;
const SHELL_LAT: usize = 20;

/// A piece of an alien zone, sorted by depth with everything else.
#[derive(Clone, Copy)]
pub(super) enum ZonePiece {
    /// Panel (lon, lat) of the Dyson shell in zone `z`.
    Panel(usize, usize, usize),
    /// The star inside a Dyson sphere.
    Core(usize),
    /// Any other zone, drawn whole.
    Whole(usize),
}

/// A point on a Dyson shell: longitude panel edge `i`, latitude edge `j`.
fn shell_point(z: &ZoneInfo, i: usize, j: usize) -> (f32, f32, f32) {
    let lon = i as f32 / SHELL_LON as f32 * TAU;
    let lat = -PI / 2.0 + j as f32 / SHELL_LAT as f32 * PI;
    let r = z.r as f32;
    (z.x as f32 + lon.cos() * lat.cos() * r, z.y as f32 + lon.sin() * lat.cos() * r, lat.sin() * r)
}

fn panel(cam: &Cam, z: &ZoneInfo, i: usize, j: usize) -> [V3; 4] {
    let p = |i: usize, j: usize| {
        let (x, y, h) = shell_point(z, i, j);
        cam.at(x, y, h)
    };
    [p(i, j), p(i + 1, j), p(i + 1, j + 1), p(i, j + 1)]
}

/// Whether the camera is inside a Dyson shell.
fn inside_shell(cam: &Cam, z: &ZoneInfo) -> bool {
    let (dx, dy) = (cam.x - z.x as f32, cam.y - z.y as f32);
    dx * dx + dy * dy + EYE * EYE < (z.r as f32).powi(2)
}

/// The pieces of the alien zones in view, with their depths.
pub(super) fn zone_pieces(cam: &Cam, f: &Frame) -> Vec<(f32, ZonePiece)> {
    let tan = (cam.cx / cam.focal) * 1.3;
    let mut out = Vec::new();
    for (k, z) in f.zones.iter().enumerate() {
        let r = z.r as f32;
        let (lat, d) = cam.local(z.x as f32, z.y as f32);
        if d + r < NEAR || d - r > RANGE * 1.5 || lat.abs() > d.max(0.0) * tan + r {
            continue;
        }
        if z.kind != ZoneKind::DysonShell {
            // Hatches sit on the shell's surface: draw them just in front of it.
            let bias = if matches!(z.kind, ZoneKind::DysonHatch | ZoneKind::DysonHatchOpen) { -400.0 } else { 0.0 };
            out.push(((d + bias).max(NEAR), ZonePiece::Whole(k)));
            continue;
        }
        out.push((d.max(NEAR), ZonePiece::Core(k)));
        let inside = inside_shell(cam, z);
        for i in 0..SHELL_LON {
            for j in 0..SHELL_LAT {
                let q = panel(cam, z, i, j);
                if q.iter().all(|p| p[2] < NEAR) {
                    continue;
                }
                // From outside only the near face shows (the shell hides the rest).
                let c = [(q[0][0] + q[2][0]) / 2.0, (q[0][1] + q[2][1]) / 2.0, (q[0][2] + q[2][2]) / 2.0];
                if !inside {
                    let ctr = cam.at(z.x as f32, z.y as f32, 0.0);
                    let out_n = sub3(c, ctr);
                    if dot3(out_n, sub3([0.0, EYE, 0.0], c)) <= 0.0 {
                        continue;
                    }
                }
                out.push((c[2].max(NEAR), ZonePiece::Panel(k, i, j)));
            }
        }
    }
    out
}

/// One piece of an alien zone.
pub(super) fn draw_zone_piece(c: &mut Canvas, cam: &Cam, f: &Frame, piece: ZonePiece, tr: &TextRenderer, fs: f32) {
    let tick = f.tick as f32;
    match piece {
        ZonePiece::Panel(k, i, j) => {
            let z = &f.zones[k];
            let q = panel(cam, z, i, j);
            let Some(sp) = cam.clip_poly(&q) else { return };
            let ctr = cam.at(z.x as f32, z.y as f32, 0.0);
            let mid = [(q[0][0] + q[2][0]) / 2.0, (q[0][1] + q[2][1]) / 2.0, (q[0][2] + q[2][2]) / 2.0];
            let n = {
                let v = sub3(mid, ctr);
                let l = dot3(v, v).sqrt().max(1.0);
                [v[0] / l, v[1] / l, v[2] / l]
            };
            let hv = rnd(z.x as u64 ^ 0xd5, (i * 97 + j) as u64);
            let plate = scale(DYSON, 0.85 + 0.3 * hv);
            if inside_shell(cam, z) {
                // The inner face, lit warm by the star at the centre; farther
                // panels fade into the haze.
                let haze = (1.0 - mid[2].max(0.0) / (z.r as f32 * 2.2)).clamp(0.35, 1.0);
                let col = scale(mix(plate, rgb(0xffe0a0), 0.25), 0.42 * haze);
                c.fill_poly(&sp, col, 1.0);
                c.polyline(&sp, true, 0.8, scale(DYSON, 0.6 * haze), 0.8, 0.0);
                // Habitats: lights scattered over the inside.
                if hv > 0.55 {
                    for s in 0..3u64 {
                        let (u, v) = (rnd(hv.to_bits() as u64, s), rnd(hv.to_bits() as u64, s + 9));
                        let p = [
                            q[0][0] + (q[1][0] - q[0][0]) * u + (q[3][0] - q[0][0]) * v,
                            q[0][1] + (q[1][1] - q[0][1]) * u + (q[3][1] - q[0][1]) * v,
                            q[0][2] + (q[1][2] - q[0][2]) * u + (q[3][2] - q[0][2]) * v,
                        ];
                        if p[2] > NEAR {
                            let (lx, ly) = cam.screen3(p);
                            c.glow(lx, ly, 2.5, rgb(0xffe8b0), 0.9 * haze);
                        }
                    }
                }
            } else {
                // Dark metal plating, lit from the upper left, with seams.
                let light = [-0.55f32, 0.75, -0.4];
                let l = dot3(light, light).sqrt();
                let d = (dot3(n, [light[0] / l, light[1] / l, light[2] / l])).max(0.0);
                let col = scale(plate, 0.12 + 0.45 * d);
                c.fill_poly(&sp, col, 1.0);
                c.polyline(&sp, true, 0.7, scale(DYSON, 0.25 + 0.4 * d), 0.9, 0.0);
            }
        }
        ZonePiece::Core(k) => {
            let z = &f.zones[k];
            let p = cam.at(z.x as f32, z.y as f32, 0.0);
            glow3(c, cam, p, z.r as f32 * 0.5, rgb(0xffd890), 0.3);
            glow3(c, cam, p, 1500.0, rgb(0xfff0d0), 0.8);
            if !inside_shell(cam, z) {
                label(c, tr, cam, z.x as f32, z.y as f32, z.r as f32 * 1.1, "Dyson sphere", DYSON, fs);
            }
        }
        ZonePiece::Whole(k) => {
            let z = &f.zones[k];
            let (x, y, r) = (z.x as f32, z.y as f32, z.r as f32);
            let ctr = cam.at(x, y, 0.0);
            match z.kind {
                ZoneKind::DysonHatch | ZoneKind::DysonHatchOpen => {
                    // The hatch, on the shell where it faces this spot.
                    let Some(shell) = f.zones.iter().find(|s| s.kind == ZoneKind::DysonShell && ((s.x - z.x) as f32).hypot((s.y - z.y) as f32) < s.r as f32 * 1.3) else {
                        return;
                    };
                    let (ox, oy) = ((x - shell.x as f32), (y - shell.y as f32));
                    let l = ox.hypot(oy).max(1.0);
                    let (nx, ny) = (ox / l, oy / l);
                    let (px, py) = (shell.x as f32 + nx * shell.r as f32, shell.y as f32 + ny * shell.r as f32);
                    let hr = (r * 0.6).max(600.0);
                    let rim: Vec<V3> = (0..32)
                        .map(|a| {
                            let a = a as f32 / 32.0 * TAU;
                            cam.at(px - ny * a.cos() * hr, py + nx * a.cos() * hr, a.sin() * hr)
                        })
                        .collect();
                    let Some(sp) = cam.clip_poly(&rim) else { return };
                    if z.kind == ZoneKind::DysonHatchOpen {
                        let pulse = 0.6 + 0.4 * (tick * 0.3).sin();
                        c.fill_poly(&sp, rgb(0xfff0c0), 0.85);
                        glow3(c, cam, cam.at(px, py, 0.0), hr * 2.0, rgb(0xfff0c0), 0.5 * pulse);
                    } else {
                        c.fill_poly(&sp, scale(DYSON, 0.25), 1.0);
                    }
                    c.polyline(&sp, true, 1.5, DYSON, 0.95, 0.0);
                    label(c, tr, cam, px, py, hr * 1.3, if z.kind == ZoneKind::DysonHatchOpen { "hatch (open)" } else { "hatch" }, DYSON, fs);
                }
                ZoneKind::Slick => {
                    // An oily slick spread on the plane, with a rainbow sheen.
                    let edge: Vec<V3> = slick_edge(z, tick, 64).into_iter().map(|(a, b)| cam.at(a as f32, b as f32, 0.0)).collect();
                    if let Some(sp) = cam.clip_poly(&edge) {
                        c.fill_poly(&sp, rgb(0x120e18), 0.85);
                        c.polyline(&sp, true, 3.0, mix(SLICK, rgb(0x40e0c0), 0.5 + 0.5 * (tick * 0.07).sin()), 0.3, 0.0);
                        c.polyline(&sp, true, 1.2, SLICK, 0.9, 0.0);
                    }
                    label(c, tr, cam, x, y, 300.0, "Armus", SLICK, fs);
                }
                ZoneKind::Arena => {
                    // A dome of shimmering force over the arena.
                    let pulse = 0.6 + 0.4 * (tick * 0.2).sin();
                    ring3(c, cam, x, y, r, 0.0, 2.0, ARENA, 0.5 + 0.3 * pulse, false);
                    for j in 1..5 {
                        let lat = j as f32 / 5.0 * PI / 2.0;
                        ring3(c, cam, x, y, r * lat.cos(), r * lat.sin(), 1.0, ARENA, 0.25 * pulse, false);
                    }
                    for i in 0..12 {
                        let lon = i as f32 / 12.0 * TAU;
                        let arc: Vec<V3> = (0..=12).map(|j| {
                            let lat = j as f32 / 12.0 * PI / 2.0;
                            cam.at(x + lon.cos() * lat.cos() * r, y + lon.sin() * lat.cos() * r, lat.sin() * r)
                        }).collect();
                        poly3(c, cam, &arc, false, 1.0, ARENA, 0.2 * pulse, false);
                    }
                    label(c, tr, cam, x, y, r * 1.05, "Metron arena", ARENA, fs);
                }
                ZoneKind::Beacon | ZoneKind::BeaconLit => {
                    let lit = z.kind == ZoneKind::BeaconLit;
                    ring3(c, cam, x, y, r, 0.0, 1.2, BEACON, 0.6, !lit);
                    // A pylon, and when held a pillar of light.
                    let (lo, hi) = (cam.at(x, y, -800.0), cam.at(x, y, 1600.0));
                    poly3(c, cam, &[lo, hi], false, 2.0, BEACON, 0.9, false);
                    if lit {
                        let top = cam.at(x, y, 12_000.0);
                        poly3(c, cam, &[ctr, top], false, 10.0, BEACON, 0.18, false);
                        poly3(c, cam, &[ctr, top], false, 2.0, mix(BEACON, WHITE, 0.6), 0.9, false);
                        glow3(c, cam, hi, r * 0.5, BEACON, 0.5);
                    }
                    glow3(c, cam, hi, 300.0, if lit { WHITE } else { BEACON }, 0.9);
                    label(c, tr, cam, x, y, 2200.0, "hyperfield beacon", BEACON, fs);
                }
                ZoneKind::Displacement => {
                    // The wave: a rippling wall of light racing outward.
                    for (k, h) in [(1.0f32, 0.0f32), (0.8, 0.0)] {
                        ring3(c, cam, x, y, r * k, h, 3.0 * k, DISPLACE, 0.6 * k, false);
                    }
                    let wall: Vec<V3> = (0..64).map(|i| {
                        let a = i as f32 / 64.0 * TAU;
                        cam.at(x + a.cos() * r, y + a.sin() * r, 900.0 + 300.0 * (a * 8.0 + tick * 0.3).sin())
                    }).collect();
                    poly3(c, cam, &wall, true, 1.2, mix(DISPLACE, WHITE, 0.4), 0.5, false);
                }
                ZoneKind::DysonShell => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{ShipType, Team};
    use crate::proto::{PState, PlayerInfo, TerrainKind};

    /// Renders every kind of terrain as seen on the viewscreen (and the
    /// view from inside a nebula) into $TMPDIR/netrek-view-terrain.ppm.
    #[test]
    fn view_terrain_gallery() {
        let tr = TextRenderer::load();
        let mut f = crate::server::world::World::new().frame_for(0);
        f.planets.clear();
        f.tick = 37;
        let (mx, my) = (50_000i32, 90_000i32);
        f.players = vec![PlayerInfo {
            id: 0, name: "me".into(), team: Team::Fed, ship: ShipType::Cruiser, state: PState::Alive, x: mx, y: my, dir: 0, speed: 0,
            flags: 0, kills: 0.0, armies: 0, tractor_target: None, fuzzy: false, explode_frame: 0, faction: None, rank: None,
        }];
        let mut kinds: Vec<(TerrainKind, bool)> = crate::server::terrain::ALL_KINDS.iter().map(|&k| (k, false)).collect();
        kinds.push((TerrainKind::Nebula, true));
        kinds.push((TerrainKind::IonStorm, true));
        kinds.push((TerrainKind::Star, true));
        let (pw, ph, cols) = (380, 190, 4);
        let rows = (kinds.len() as i32 + cols - 1) / cols;
        let mut c = Canvas::new(cols * (pw + 6), rows * (ph + 6), [30.0, 30.0, 30.0]);
        for (k, &(kind, inside)) in kinds.iter().enumerate() {
            let r = match kind {
                TerrainKind::Planetoid | TerrainKind::AbandonedStation => 1500,
                TerrainKind::FluidicRift => 700,
                TerrainKind::GalacticBarrier => 350,
                TerrainKind::Derelict => 900,
                TerrainKind::Wormhole => 600,
                TerrainKind::Corridor => 700,
                TerrainKind::Comet => 500,
                _ => 5500,
            };
            let ahead = |lat: i32, d: i32| (mx + lat, my - d);
            let ((x, y), (x2, y2)) = match kind {
                _ if inside => (ahead(800, 1500), ahead(800, 1500)),
                TerrainKind::Corridor => (ahead(-9000, 2500), ahead(9000, 16000)),
                TerrainKind::Comet => (ahead(-2500, 6000), ahead(7000, 14000)),
                TerrainKind::GalacticBarrier => (ahead(-15000, 6000), ahead(15000, 9000)),
                TerrainKind::Wormhole => (ahead(0, 5000), (10_000, 10_000)),
                TerrainKind::Derelict | TerrainKind::AbandonedStation => (ahead(0, 3000), ahead(0, 3000)),
                TerrainKind::Planetoid | TerrainKind::FluidicRift => (ahead(0, 7000), ahead(0, 7000)),
                _ => (ahead(0, 12_000), ahead(0, 12_000)),
            };
            f.terrain = vec![TerrainInfo { kind, x, y, r, x2, y2, phase: 95, name: format!("{:?}", kind) }];
            let img = super::super::render_view::viewscreen(&tr, Some(&f), 0, None, 12.0, pw, ph);
            c.draw_canvas(&img, (k as i32 % cols) * (pw + 6), (k as i32 / cols) * (ph + 6));
        }
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-view-terrain.ppm"), out).unwrap();
    }

    /// Renders the Ringworld on the viewscreen from outside it, from inside
    /// it, and side on, into $TMPDIR/netrek-view-ring.ppm.
    #[test]
    fn view_ring_gallery() {
        use crate::proto::{RingInfo, RingSectionInfo};
        let tr = TextRenderer::load();
        let mut f = crate::server::world::World::new().frame_for(0);
        f.tick = 300;
        f.terrain.clear();
        let (cx, cy, r) = (50_000i32, 50_000i32, 8500i32);
        let sections: Vec<RingSectionInfo> = (0..10)
            .map(|k| {
                let a = 0.3 + k as f64 * std::f64::consts::TAU / 10.0;
                RingSectionInfo { name: format!("Section {}", k), x: cx + (a.cos() * r as f64) as i32, y: cy + (a.sin() * r as f64) as i32 }
            })
            .collect();
        let base = f.planets[0].clone();
        for _ in 0..10 {
            f.planets.push(base.clone());
        }
        f.ring = Some(RingInfo { x: cx, y: cy, r, sections, kzin: 3 });
        let views = [(cx, cy + 22_000, 0u8), (cx - 2000, cy + 4000, 0), (cx + 15_000, cy + 6000, 210), (cx, cy, 64)];
        let (pw, ph) = (520, 230);
        let mut c = Canvas::new(pw * 2 + 6, (ph + 6) * 2, [30.0, 30.0, 30.0]);
        for (k, &(x, y, dir)) in views.iter().enumerate() {
            f.players = vec![PlayerInfo {
                id: 0, name: "me".into(), team: Team::Fed, ship: ShipType::Cruiser, state: PState::Alive, x, y, dir, speed: 0,
                flags: 0, kills: 0.0, armies: 0, tractor_target: None, fuzzy: false, explode_frame: 0, faction: None, rank: None,
            }];
            let img = super::super::render_view::viewscreen(&tr, Some(&f), 0, None, 12.0, pw, ph);
            c.draw_canvas(&img, (k as i32 % 2) * (pw + 6), (k as i32 / 2) * (ph + 6));
        }
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-view-ring.ppm"), out).unwrap();
    }

    /// Renders the Dyson sphere from outside and inside, and the other
    /// alien zones, into $TMPDIR/netrek-view-zones.ppm.
    #[test]
    fn view_zone_gallery() {
        use crate::proto::ZoneInfo;
        let tr = TextRenderer::load();
        let mut f = crate::server::world::World::new().frame_for(0);
        f.tick = 120;
        f.terrain.clear();
        f.planets.clear();
        let (cx, cy, r) = (50_000i32, 50_000i32, 9000i32);
        let shell = ZoneInfo { kind: ZoneKind::DysonShell, x: cx, y: cy, r };
        let hatch = |open: bool| ZoneInfo { kind: if open { ZoneKind::DysonHatchOpen } else { ZoneKind::DysonHatch }, x: cx, y: cy + r, r: 900 };
        let z = |kind, r| ZoneInfo { kind, x: cx, y: cy, r };
        // (zones, camera x, y, heading)
        let views: Vec<(Vec<ZoneInfo>, i32, i32, u8)> = vec![
            (vec![shell.clone(), hatch(false)], cx, cy + 26_000, 0),
            (vec![shell.clone(), hatch(true)], cx - 9000, cy + 20_000, 10),
            (vec![shell.clone(), hatch(true)], cx - 2000, cy + 3000, 0),
            (vec![shell.clone(), hatch(true)], cx + 1000, cy - 2000, 128),
            (vec![z(ZoneKind::Slick, 3000)], cx, cy + 9000, 0),
            (vec![z(ZoneKind::Arena, 5000)], cx, cy + 14_000, 0),
            (vec![z(ZoneKind::BeaconLit, 2500)], cx, cy + 9000, 0),
            (vec![z(ZoneKind::Displacement, 6000)], cx, cy + 14_000, 0),
        ];
        let (pw, ph) = (480, 220);
        let mut c = Canvas::new(pw * 2 + 6, (ph + 6) * 4, [30.0, 30.0, 30.0]);
        for (k, (zones, x, y, dir)) in views.into_iter().enumerate() {
            f.zones = zones;
            f.players = vec![PlayerInfo {
                id: 0, name: "me".into(), team: Team::Fed, ship: ShipType::Cruiser, state: PState::Alive, x, y, dir, speed: 0,
                flags: 0, kills: 0.0, armies: 0, tractor_target: None, fuzzy: false, explode_frame: 0, faction: None, rank: None,
            }];
            let img = super::super::render_view::viewscreen(&tr, Some(&f), 0, None, 12.0, pw, ph);
            c.draw_canvas(&img, (k as i32 % 2) * (pw + 6), (k as i32 / 2) * (ph + 6));
        }
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-view-zones.ppm"), out).unwrap();
    }
}
