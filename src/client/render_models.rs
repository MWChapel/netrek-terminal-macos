//! 3D models of the aliens for the viewscreen. Each is built to suit what
//! it is (a crystal, a living cell, a cloud of light, an arcade sprite)
//! rather than raised from its map icon. Coordinates are in ship radii: x
//! to starboard, y aft (the nose is toward -y), h up.

use super::palette::rgb;
use super::render_view::{ball, centroid, cross, cylinder, norm, pillow, ship_mesh, sub, Face, Model, V3};
use super::shipart::{ship_parts, Part};
use crate::consts::{Faction, ShipType, Team};
use std::f32::consts::{PI, TAU};

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mul(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// Two directions square to `d` and to each other.
fn basis(d: V3) -> (V3, V3) {
    let u = norm(if d[2].abs() < 0.9 { cross(d, [0.0, 0.0, 1.0]) } else { cross(d, [1.0, 0.0, 0.0]) });
    (u, cross(d, u))
}

/// A tube (or a cone, when the radii differ) from `a` to `b`, capped at
/// either end as asked.
fn tube(a: V3, b: V3, r0: f32, r1: f32, seg: usize, caps: (bool, bool), out: &mut Vec<Face>) {
    let d = norm(sub(b, a));
    let (u, v) = basis(d);
    let ring = |c: V3, r: f32| -> Vec<V3> {
        (0..seg)
            .map(|k| {
                let t = k as f32 / seg as f32 * TAU;
                add(c, add(mul(u, t.cos() * r), mul(v, t.sin() * r)))
            })
            .collect()
    };
    let (ra, rb) = (ring(a, r0), ring(b, r1));
    let mid = mul(add(a, b), 0.5);
    for k in 0..seg {
        let k1 = (k + 1) % seg;
        out.push(Face::facing_out(vec![ra[k], ra[k1], rb[k1], rb[k]], mid, false));
    }
    if caps.0 {
        out.push(Face::new(ra, mul(d, -1.0), true));
    }
    if caps.1 {
        out.push(Face::new(rb, d, true));
    }
}

/// A box centred at `c` with half-sizes `h`.
fn boxy(c: V3, h: V3, out: &mut Vec<Face>) {
    let p = |sx: f32, sy: f32, sh: f32| [c[0] + sx * h[0], c[1] + sy * h[1], c[2] + sh * h[2]];
    let quads = [
        ([p(-1.0, -1.0, 1.0), p(1.0, -1.0, 1.0), p(1.0, 1.0, 1.0), p(-1.0, 1.0, 1.0)], [0.0, 0.0, 1.0]),
        ([p(-1.0, -1.0, -1.0), p(1.0, -1.0, -1.0), p(1.0, 1.0, -1.0), p(-1.0, 1.0, -1.0)], [0.0, 0.0, -1.0]),
        ([p(-1.0, -1.0, -1.0), p(1.0, -1.0, -1.0), p(1.0, -1.0, 1.0), p(-1.0, -1.0, 1.0)], [0.0, -1.0, 0.0]),
        ([p(-1.0, 1.0, -1.0), p(1.0, 1.0, -1.0), p(1.0, 1.0, 1.0), p(-1.0, 1.0, 1.0)], [0.0, 1.0, 0.0]),
        ([p(-1.0, -1.0, -1.0), p(-1.0, 1.0, -1.0), p(-1.0, 1.0, 1.0), p(-1.0, -1.0, 1.0)], [-1.0, 0.0, 0.0]),
        ([p(1.0, -1.0, -1.0), p(1.0, 1.0, -1.0), p(1.0, 1.0, 1.0), p(1.0, -1.0, 1.0)], [1.0, 0.0, 0.0]),
    ];
    for (pts, n) in quads {
        out.push(Face::new(pts.to_vec(), n, true));
    }
}

/// A rounded body round `c`: `shape` turns each direction from the centre
/// into the point of the surface that way (so it can squash, stretch,
/// taper and wobble).
fn blob(c: V3, seg: usize, levels: usize, shape: &dyn Fn(V3) -> V3, out: &mut Vec<Face>) {
    let at = |i: usize, j: usize| {
        let lon = i as f32 / seg as f32 * TAU;
        let lat = -PI / 2.0 + j as f32 / levels as f32 * PI;
        add(c, shape([lon.cos() * lat.cos(), lon.sin() * lat.cos(), lat.sin()]))
    };
    for i in 0..seg {
        for j in 0..levels {
            let mut pts = vec![at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            if j == 0 {
                pts.remove(0);
            } else if j + 1 == levels {
                pts.remove(2);
            }
            out.push(Face::facing_out(pts, c, false));
        }
    }
}

/// A cut gem: an outline drawn up to a point above it and down to one below.
fn gem(outline: &[V3], up: V3, down: V3, out: &mut Vec<Face>) {
    let c = centroid(outline);
    let n = outline.len();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        out.push(Face::facing_out(vec![a, b, up], c, true));
        out.push(Face::facing_out(vec![a, b, down], c, true));
    }
}

/// A flat plate: an outline given thickness `t` either side, along `n`.
fn plate(outline: &[V3], n: V3, t: f32, out: &mut Vec<Face>) {
    let front: Vec<V3> = outline.iter().map(|&p| add(p, mul(n, t))).collect();
    let back: Vec<V3> = outline.iter().map(|&p| add(p, mul(n, -t))).collect();
    let c = centroid(outline);
    for i in 0..outline.len() {
        let j = (i + 1) % outline.len();
        out.push(Face::facing_out(vec![back[i], back[j], front[j], front[i]], c, true));
    }
    out.push(Face::new(front, n, true));
    out.push(Face::new(back, mul(n, -1.0), true));
}

/// A circle of line segments round `axis` through `c`.
fn circle_lines(c: V3, axis: V3, r: f32, seg: usize) -> Vec<(V3, V3)> {
    let (u, v) = basis(norm(axis));
    let at = |k: usize| {
        let t = k as f32 / seg as f32 * TAU;
        add(c, add(mul(u, t.cos() * r), mul(v, t.sin() * r)))
    };
    (0..seg).map(|k| (at(k), at(k + 1))).collect()
}

/// A model with no hull of its own, only light.
fn light(glows: Vec<(V3, f32, [f32; 3], f32)>, lines: Vec<(V3, V3)>, line_col: [f32; 3]) -> Model {
    Model {
        faces: Vec::new(),
        lines,
        hull: None,
        line_col: Some(line_col),
        alpha: 1.0,
        glows,
        neon: true,
        billboard: false,
        lights: false,
    }
}

/// The alien's own model, or None to raise it from its drawing as usual.
pub(super) fn alien_model(ship: ShipType, faction: Option<Faction>, tick: u32, fine: bool) -> Option<Model> {
    let f = faction?;
    let seg = if fine { 28 } else { 12 };
    let lv = seg / 2;
    let t = tick as f32;
    let mut faces = Vec::new();
    let mut lines = Vec::new();
    let m = match (f, ship) {
        (Faction::Tholian, _) => {
            // A cut crystal of amber, lit from within.
            let outline = [[0.0, -1.0, 0.0], [0.55, 0.8, 0.0], [0.0, 0.45, 0.0], [-0.55, 0.8, 0.0]];
            gem(&outline, [0.0, 0.25, 0.3], [0.0, 0.25, -0.22], &mut faces);
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0xffb050));
            m.alpha = 0.85;
            m.glows = vec![([0.0, 0.2, 0.0], 0.7, rgb(0xffc060), 0.55)];
            m.lights = false;
            m
        }
        (Faction::Fesarius, _) => {
            // The First Federation's flagship: a vast globe studded with
            // modules, lights glowing among them.
            ball([0.0; 3], 1.0, seg, lv, &mut faces);
            let mut glows = Vec::new();
            for (n, lat, r) in [(12, 0.0f32, 0.17f32), (8, 0.7, 0.15), (8, -0.7, 0.15), (1, PI / 2.0, 0.18)] {
                for k in 0..n {
                    let lon = k as f32 / n as f32 * TAU + lat;
                    let d = [lon.cos() * lat.cos(), lon.sin() * lat.cos(), lat.sin()];
                    ball(mul(d, 0.97), r, 10, 5, &mut faces);
                    if (k + tick as usize / 8) % 3 == 0 {
                        glows.push((mul(d, 1.0 + r), 0.12, rgb(0x9ad0ff), 0.8));
                    }
                }
            }
            let mut m = Model::from_mesh((faces, lines));
            m.glows = glows;
            m.lights = false;
            m
        }
        (Faction::Doomsday, _) => {
            // The planet killer: a long neutronium cone, its maw open and
            // burning inside.
            tube([0.0, -1.0, 0.0], [0.0, 1.0, 0.0], 0.5, 0.26, seg, (false, true), &mut faces);
            let maw: Vec<V3> = (0..seg).map(|k| { let a = k as f32 / seg as f32 * TAU; [a.cos() * 0.47, -0.9, a.sin() * 0.47] }).collect();
            faces.push(Face::new(maw, [0.0, -1.0, 0.0], false).painted(rgb(0x5a1408)));
            for (y, r) in [(-0.25f32, 0.41f32), (0.35, 0.33)] {
                lines.extend(circle_lines([0.0, y, 0.0], [0.0, 1.0, 0.0], r + 0.01, seg));
            }
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0x8c9298));
            let pulse = 0.55 + 0.25 * (t * 0.2).sin();
            m.glows = vec![([0.0, -1.0, 0.0], 0.55, rgb(0xff6020), pulse), ([0.0, -1.0, 0.0], 0.25, rgb(0xffd080), pulse)];
            m.lights = false;
            m
        }
        (Faction::Amoeba, _) => {
            // A giant single cell: a wobbling, see-through membrane round a
            // glowing nucleus.
            let shape = |d: V3| {
                let lon = d[1].atan2(d[0]);
                let r = 0.88 + 0.08 * (3.0 * lon + t * 0.08).sin() + 0.05 * (5.0 * d[2] + lon * 2.0 - t * 0.05).sin();
                [d[0] * r, d[1] * r, d[2] * r * 0.55]
            };
            blob([0.0; 3], seg, lv, &shape, &mut faces);
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0x80f0c0));
            m.alpha = 0.5;
            m.glows = vec![
                ([0.0; 3], 1.0, rgb(0x60e0a0), 0.18),
                ([0.12, -0.1, 0.0], 0.45, rgb(0xd0ffe0), 0.7),
                ([0.12, -0.1, 0.0], 0.15, rgb(0xffffff), 0.8),
                ([-0.4, 0.3, 0.05], 0.15, rgb(0xa0ffd0), 0.5),
                ([0.35, 0.45, -0.05], 0.1, rgb(0xa0ffd0), 0.5),
            ];
            m.lights = false;
            m
        }
        (Faction::Borg, _) => {
            // The cube: dark plating laced with green-lit conduits.
            let h = 0.72;
            boxy([0.0; 3], [h, h, h], &mut faces);
            let e = h + 0.006;
            for k in 1..4 {
                let s = -h + k as f32 * 0.36;
                // Top, front, back and both sides.
                lines.push(([s, -h, e], [s, h, e]));
                lines.push(([-h, s, e], [h, s, e]));
                for side in [-e, e] {
                    lines.push(([s, side, -h], [s, side, h]));
                    lines.push(([-h, side, s], [h, side, s]));
                    lines.push(([side, s, -h], [side, s, h]));
                    lines.push(([side, -h, s], [side, h, s]));
                }
            }
            let mut glows = Vec::new();
            for k in 0..10u32 {
                let hv = (k.wrapping_mul(2654435761) >> 7) as f32;
                let (a, b) = ((hv % 100.0) / 50.0 - 1.0, ((hv / 100.0) % 100.0) / 50.0 - 1.0);
                let p = match k % 3 {
                    0 => [a * h, b * h, e],
                    1 => [a * h, -e, b * h],
                    _ => [e, a * h, b * h],
                };
                if (tick / 5 + k) % 4 != 0 {
                    glows.push((p, 0.08, rgb(0x40ff60), 0.9));
                }
            }
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0x30352f));
            m.line_col = Some(rgb(0x30d050));
            m.glows = glows;
            m.lights = false;
            m
        }
        (Faction::Vger, _) => {
            // A vast luminous cloud, energy rings turning round a bright core.
            let mut glows = Vec::new();
            for k in 0..12u32 {
                let a = k as f32 / 12.0 * TAU + t * 0.004;
                let d = 0.35 + 0.5 * ((k * 7 % 5) as f32 / 4.0);
                glows.push(([a.cos() * d, a.sin() * d, ((k % 3) as f32 - 1.0) * 0.15], 0.55, rgb(0x3a70d0), 0.22));
            }
            glows.push(([0.0; 3], 0.35, rgb(0x90c0ff), 0.6));
            glows.push(([0.0; 3], 0.12, rgb(0xffffff), 1.0));
            let spin = t * 0.01;
            let mut lines = circle_lines([0.0; 3], [0.0, 0.0, 1.0], 0.5, 40);
            lines.extend(circle_lines([0.0; 3], [spin.cos(), spin.sin(), 2.5], 0.32, 32));
            let mut m = light(glows, lines, rgb(0x9ad0ff));
            ball([0.0; 3], 0.08, 10, 5, &mut m.faces);
            m.hull = Some(rgb(0xe0f0ff));
            m
        }
        (Faction::Crystal, _) => {
            // The Crystalline Entity: a slowly turning snowflake of crystal,
            // spines in two planes.
            let spin = t * 0.015;
            let star = |k: usize| {
                let r = if k % 2 == 0 { if k % 4 == 0 { 1.0 } else { 0.7 } } else { 0.25 };
                (k as f32 / 24.0 * TAU + spin, r)
            };
            let flat: Vec<V3> = (0..24).map(|k| { let (a, r) = star(k); [a.cos() * r, a.sin() * r, 0.0] }).collect();
            gem(&flat, [0.0, 0.0, 0.12], [0.0, 0.0, -0.12], &mut faces);
            let upright: Vec<V3> = (0..24).map(|k| { let (a, r) = star(k); [0.0, a.cos() * r * 0.7, a.sin() * r * 0.7] }).collect();
            gem(&upright, [0.1, 0.0, 0.0], [-0.1, 0.0, 0.0], &mut faces);
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0xd8f0ff));
            m.alpha = 0.8;
            m.glows = vec![([0.0; 3], 0.5, rgb(0xc0e8ff), 0.6)];
            m.lights = false;
            m
        }
        (Faction::Probe, _) => {
            // The whale probe: a long cylinder lying along its course,
            // banded twice, with a sphere coming out of its underside.
            let r = 0.28;
            cylinder((0.0, 0.0), -0.75, 1.0, r, seg, &mut faces);
            ball([0.0, -0.5, -0.4], 0.2, seg, lv, &mut faces);
            for y in [-0.35f32, 0.35] {
                lines.extend(circle_lines([0.0, y, 0.0], [0.0, 1.0, 0.0], r + 0.008, seg));
            }
            let mut m = Model::from_mesh((faces, lines));
            m.lights = false;
            m
        }
        (Faction::Species8472, _) => {
            // A living bioship: organic, pale and veined.
            let mut m = Model::from_mesh(ship_mesh(ship_parts(Team::Ind, ship, faction), fine));
            m.hull = Some(rgb(0xd8c08a));
            m.glows = vec![([0.0, -0.3, 0.2], 0.25, rgb(0xfff0a0), 0.5)];
            m.lights = false;
            m
        }
        (Faction::Q, ShipType::QChampion) => {
            let mut m = Model::from_mesh(ship_mesh(ship_parts(Team::Ind, ship, faction), fine));
            m.glows = vec![([0.0, 0.05, 0.25], 0.35, rgb(0xfff4c0), 0.8)];
            m
        }
        (Faction::Q, _) => {
            // Q himself: a flash of white light throwing out rays.
            let mut lines = Vec::new();
            for k in 0..10 {
                let a = k as f32 / 10.0 * TAU + t * 0.05;
                let len = 0.6 + 0.4 * ((t * 0.3 + k as f32 * 1.7).sin() * 0.5 + 0.5);
                lines.push(([0.0; 3], [a.cos() * len, 0.0, a.sin() * len]));
            }
            let mut m = light(
                vec![([0.0; 3], 0.9, rgb(0xfff4c0), 0.45), ([0.0; 3], 0.35, rgb(0xffffff), 1.0)],
                lines,
                rgb(0xfff4c0),
            );
            m.billboard = true;
            m
        }
        (Faction::Nomad, _) => {
            // Nomad: a squat cylinder with a sensor head and two side panels.
            cylinder((0.0, 0.0), -0.55, 0.9, 0.3, seg, &mut faces);
            boxy([0.0, -0.75, 0.0], [0.38, 0.2, 0.26], &mut faces);
            ball([0.0, -0.98, 0.0], 0.08, seg / 2, lv / 2 + 1, &mut faces);
            let n = faces.len();
            for s in [-1.0f32, 1.0] {
                boxy([s * 0.58, 0.0, 0.0], [0.25, 0.33, 0.025], &mut faces);
            }
            for face in &mut faces[n..] {
                face.paint = Some(rgb(0x3a4458));
            }
            for y in [0.2f32, 0.55] {
                lines.extend(circle_lines([0.0, y, 0.0], [0.0, 1.0, 0.0], 0.31, seg));
            }
            let mut m = Model::from_mesh((faces, lines));
            m.glows = vec![([0.0, -1.06, 0.0], 0.12, rgb(0x80c0ff), 0.6 + 0.3 * (t * 0.3).sin())];
            m
        }
        (Faction::Armus, _) => {
            // Armus: a pool of black tar on the plane, with a face in it.
            let outline: Vec<(f32, f32)> = (0..32)
                .map(|k| {
                    let a = k as f32 / 32.0 * TAU;
                    let r = 0.8 + 0.12 * (4.0 * a + t * 0.05).sin() + 0.08 * (7.0 * a + 2.0 - t * 0.03).sin();
                    (a.cos() * r, a.sin() * r)
                })
                .collect();
            pillow(&outline, 0.0, 0.06, 0.02, &mut faces);
            // An oily rainbow sheen round its edge.
            for k in 0..outline.len() {
                let (a, b) = (outline[k], outline[(k + 1) % outline.len()]);
                lines.push(([a.0, a.1, 0.02], [b.0, b.1, 0.02]));
            }
            lines.push(([-0.3, 0.28, 0.07], [0.0, 0.3, 0.07]));
            lines.push(([0.0, 0.3, 0.07], [0.3, 0.22, 0.07]));
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0x0c0a10));
            let sheen = 0.5 + 0.5 * (t * 0.07).sin();
            m.line_col = Some(super::palette::mix(rgb(0x9a80c8), rgb(0x40d0b0), sheen));
            m.glows = vec![([-0.25, -0.15, 0.07], 0.08, rgb(0xe8e0ff), 0.9), ([0.22, -0.2, 0.07], 0.08, rgb(0xe8e0ff), 0.9)];
            m.lights = false;
            m
        }
        (Faction::Changeling, _) => {
            // A drop of molten gold, rippling as it holds its shape.
            let shape = |d: V3| {
                let taper = if d[1] < 0.0 { 1.0 + d[1] * 0.75 } else { 1.0 };
                let r = 1.0 + 0.05 * (3.0 * d[1].atan2(d[0]) + t * 0.15).sin();
                [d[0] * 0.55 * taper * r, d[1] * 0.95 * r, d[2] * 0.45 * taper * r]
            };
            blob([0.0; 3], seg, lv, &shape, &mut faces);
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0xe8a830));
            m.glows = vec![([0.0, 0.3, 0.1], 0.35, rgb(0xffd060), 0.3)];
            m.lights = false;
            m
        }
        (Faction::Metrons, _) => {
            // A radiant presence: a calm glow ringed by haloes of light.
            let mut lines = circle_lines([0.0; 3], [0.0, 1.0, 0.0], 0.7, 40);
            let tilt = t * 0.01;
            lines.extend(circle_lines([0.0; 3], [tilt.sin(), 1.0, tilt.cos()], 0.85, 40));
            for k in 0..12 {
                let a = k as f32 / 12.0 * TAU;
                lines.push(([a.cos() * 0.4, 0.0, a.sin() * 0.4], [a.cos() * 0.6, 0.0, a.sin() * 0.6]));
            }
            let mut m = light(
                vec![([0.0; 3], 1.2, rgb(0xfff0c8), 0.35), ([0.0; 3], 0.4, rgb(0xffffff), 0.95)],
                lines,
                rgb(0xfff0c0),
            );
            m.billboard = true;
            m
        }
        (Faction::TenC, _) => {
            // Species 10-C: a black core in a dark swirl of glowing arms.
            ball([0.0; 3], 0.3, seg, lv, &mut faces);
            let spin = t * 0.02;
            let mut lines = Vec::new();
            for arm in 0..3 {
                let base = arm as f32 / 3.0 * TAU + spin;
                let pts: Vec<V3> = (0..16)
                    .map(|k| {
                        let s = k as f32 / 15.0;
                        let a = base + s * 3.2;
                        let r = 0.3 + s * 0.7;
                        [a.cos() * r, a.sin() * r, 0.0]
                    })
                    .collect();
                lines.extend(pts.windows(2).map(|w| (w[0], w[1])));
            }
            lines.extend(circle_lines([0.0; 3], [0.0, 0.0, 1.0], 0.6, 40));
            let mut m = light(vec![([0.0; 3], 1.0, rgb(0x6030a0), 0.3)], lines, rgb(0x9050ff));
            m.faces = faces;
            m.hull = Some(rgb(0x060408));
            m
        }
        (Faction::Caretaker, _) => {
            // The Caretaker's array: a vast flat lattice of panels round a
            // pulsing emitter, two antennae forward.
            boxy([0.0; 3], [0.95, 0.45, 0.04], &mut faces);
            let dome = |d: V3| [d[0] * 0.3, d[1] * 0.3, d[2].max(0.0) * 0.14];
            blob([0.0, 0.0, 0.04], seg, lv, &dome, &mut faces);
            for s in [-1.0f32, 1.0] {
                tube([s * 0.3, -0.45, 0.02], [s * 0.55, -0.85, 0.12], 0.03, 0.02, seg / 3 + 3, (false, true), &mut faces);
            }
            for k in 1..6 {
                let x = -0.95 + k as f32 * 0.317;
                lines.push(([x, -0.45, 0.045], [x, 0.45, 0.045]));
            }
            lines.push(([-0.95, 0.0, 0.045], [-0.3, 0.0, 0.045]));
            lines.push(([0.3, 0.0, 0.045], [0.95, 0.0, 0.045]));
            let mut m = Model::from_mesh((faces, lines));
            m.glows = vec![([0.0, 0.0, 0.2], 0.4, rgb(0xc0e8ff), 0.5 + 0.4 * (t * 0.25).sin().abs())];
            m
        }
        (Faction::Horta, _) => {
            // The Horta: a lumpy creature of living rock, flecked with mineral.
            let shape = |d: V3| {
                let lon = d[1].atan2(d[0]);
                let r = 1.0 + 0.14 * (5.0 * lon + 0.7).sin() + 0.08 * (3.0 * d[2] + 2.0 * lon).sin();
                [d[0] * 0.8 * r, d[1] * 0.7 * r, d[2] * 0.42 * r]
            };
            blob([0.0; 3], seg, lv, &shape, &mut faces);
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0x7a5a3c));
            m.glows = [(-0.3, -0.2), (0.25, -0.3), (0.1, 0.25), (-0.35, 0.35), (0.45, 0.15)]
                .iter()
                .map(|&(x, y)| ([x, y, 0.38], 0.06, rgb(0xffa040), 0.8))
                .collect();
            m.lights = false;
            m
        }
        (Faction::GalacticEmpire, ShipType::DeathStar) => {
            // The battle station: a grey moon with an equatorial trench and a
            // superlaser dish.
            ball([0.0; 3], 1.0, seg, lv, &mut faces);
            let dn = norm([0.32, -0.42, 0.45]);
            let (u, v) = basis(dn);
            let dish: Vec<V3> = (0..20)
                .map(|k| {
                    let a = k as f32 / 20.0 * TAU;
                    mul(norm(add(mul(dn, 0.3f32.cos()), mul(add(mul(u, a.cos()), mul(v, a.sin())), 0.3f32.sin()))), 1.006)
                })
                .collect();
            faces.push(Face::new(dish.clone(), dn, true).painted(rgb(0x4a5258)));
            lines.extend(dish.iter().zip(dish.iter().cycle().skip(1)).map(|(&a, &b)| (a, b)));
            for lat in [0.0f32, -0.07, 0.6, -0.55] {
                lines.extend(circle_lines([0.0, 0.0, lat.sin()], [0.0, 0.0, 1.0], lat.cos() * 1.004, seg * 2));
            }
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0x98a0a8));
            m.line_col = Some(rgb(0x50585e));
            m.glows = vec![(mul(dn, 1.01), 0.07, rgb(0x40ff60), 0.8)];
            m.lights = false;
            m
        }
        (Faction::GalacticEmpire, ShipType::StarDestroyer) => {
            // The Star Destroyer: a dagger with a raised spine, the bridge
            // tower and its two shield domes at the stern.
            let ridge = |y: f32| 0.03 + 0.2 * (y + 1.0) / 1.95;
            let outline = [(0.0, -1.0), (0.62, 0.8), (0.62, 0.95), (-0.62, 0.95), (-0.62, 0.8)];
            let inside = [0.0, 0.3, 0.0];
            let n = outline.len();
            for i in 0..n {
                let (a, b) = (outline[i], outline[(i + 1) % n]);
                // The sloping upper hull, up to the spine.
                let top = if a.1 == b.1 { vec![[a.0, a.1, 0.0], [b.0, b.1, 0.0], [0.0, a.1, ridge(a.1)]] } else { vec![[a.0, a.1, 0.0], [b.0, b.1, 0.0], [0.0, b.1, ridge(b.1)], [0.0, a.1, ridge(a.1)]] };
                faces.push(Face::facing_out(top, inside, true));
                faces.push(Face::facing_out(vec![[a.0, a.1, 0.0], [b.0, b.1, 0.0], [b.0, b.1, -0.07], [a.0, a.1, -0.07]], inside, true));
            }
            faces.push(Face::new(outline.iter().map(|&(x, y)| [x, y, -0.07]).collect(), [0.0, 0.0, -1.0], true));
            boxy([0.0, 0.66, 0.28], [0.2, 0.1, 0.08], &mut faces);
            boxy([0.0, 0.7, 0.18], [0.07, 0.05, 0.06], &mut faces);
            for s in [-1.0f32, 1.0] {
                ball([s * 0.13, 0.62, 0.4], 0.05, seg / 2, lv / 2 + 1, &mut faces);
            }
            let half = |y: f32| 0.62 * ((y + 1.0) / 1.8).min(1.0);
            let on_top = |x: f32, y: f32| [x, y, ridge(y) * (1.0 - x.abs() / half(y).max(0.01)) + 0.01];
            for s in [-1.0f32, 1.0] {
                for k in 0..8 {
                    let (s0, s1) = (k as f32 / 8.0, (k + 1) as f32 / 8.0);
                    let p = |q: f32| on_top(s * (0.1 + 0.3 * q), -0.5 + 1.1 * q);
                    lines.push((p(s0), p(s1)));
                }
            }
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0xc4c8cc));
            m.line_col = Some(rgb(0x707880));
            m
        }
        (Faction::GalacticEmpire, ShipType::TieFighter | ShipType::TieAdvanced) => {
            // A ball cockpit between two upright hexagonal wings (Vader's
            // TIE has its wings bent in and a longer cockpit).
            let advanced = ship == ShipType::TieAdvanced;
            let cockpit = |d: V3| [d[0] * 0.3, d[1] * if advanced { 0.42 } else { 0.3 }, d[2] * 0.3];
            blob([0.0; 3], seg, lv, &cockpit, &mut faces);
            let window: Vec<V3> = (0..12).map(|k| { let a = k as f32 / 12.0 * TAU; [a.cos() * 0.12, -0.3 * if advanced { 1.38 } else { 0.98 }, a.sin() * 0.12] }).collect();
            faces.push(Face::new(window, [0.0, -1.0, 0.0], true).painted(rgb(0x14181e)));
            let n0 = faces.len();
            for s in [-1.0f32, 1.0] {
                tube([s * 0.25, 0.0, 0.0], [s * 0.78, 0.0, 0.0], 0.07, 0.07, seg / 3 + 3, (false, false), &mut faces);
                let hex = [(-0.4, 0.95), (0.4, 0.95), (0.75, 0.0), (0.4, -0.95), (-0.4, -0.95), (-0.75, 0.0)];
                let wing_x = |h: f32| if advanced { s * (0.82 - 0.28 * h.abs()) } else { s * 0.82 };
                if advanced {
                    for half in [1.0f32, -1.0] {
                        let pts: Vec<V3> = [(-0.75, 0.0), (-0.4, 0.95 * half), (0.4, 0.95 * half), (0.75, 0.0)].iter().map(|&(y, h)| [wing_x(h), y, h]).collect();
                        let n = norm(cross(sub(pts[1], pts[0]), sub(pts[3], pts[0])));
                        plate(&pts, n, 0.02, &mut faces);
                    }
                } else {
                    let pts: Vec<V3> = hex.iter().map(|&(y, h)| [wing_x(h), y, h]).collect();
                    plate(&pts, [1.0, 0.0, 0.0], 0.02, &mut faces);
                }
                // The wing's frame: spokes out to its corners.
                let face_x = |h: f32| wing_x(h) + s * 0.025;
                for &(y, h) in &hex {
                    lines.push(([face_x(0.0), 0.0, 0.0], [face_x(h), y, h]));
                }
                for k in 0..6 {
                    let (a, b) = (hex[k], hex[(k + 1) % 6]);
                    lines.push(([face_x(a.1), a.0, a.1], [face_x(b.1), b.0, b.1]));
                }
            }
            for face in &mut faces[n0..] {
                if face.edge {
                    face.paint = Some(rgb(0x5a6270));
                }
            }
            let mut m = Model::from_mesh((faces, lines));
            m.hull = Some(rgb(0x8a9098));
            m.line_col = Some(rgb(0x9aa0a8));
            m.lights = false;
            m
        }
        (Faction::Tempest, _) => {
            // The Tempest's creatures stay arcade sprites: glowing vector
            // shapes, standing upright to face you.
            let spin = match ship {
                ShipType::Flipper => t * 0.25,
                ShipType::Fuseball => t * 0.1,
                ShipType::TempestCore => t * 0.04,
                _ => 0.0,
            };
            let (sn, cs) = spin.sin_cos();
            let up = |(x, y): (f32, f32)| [x * cs - y * sn, 0.0, -(x * sn + y * cs)];
            let mut lines = Vec::new();
            for part in ship_parts(Team::Ind, ship, faction) {
                match part {
                    Part::Poly { pts, .. } | Part::Hole { pts } => {
                        for k in 0..pts.len() {
                            lines.push((up(pts[k]), up(pts[(k + 1) % pts.len()])));
                        }
                    }
                    Part::Line { a, b } => lines.push((up(a), up(b))),
                    Part::Circle { c, r, .. } => {
                        let c = up(c);
                        lines.extend(circle_lines(c, [0.0, 1.0, 0.0], r, 16));
                    }
                }
            }
            let mut m = light(vec![([0.0; 3], 0.5, rgb(0xffffff), 0.12)], lines, [0.0; 3]);
            m.line_col = None;
            m.billboard = true;
            if ship == ShipType::Pulsar {
                m.alpha = 0.45 + 0.55 * (t * 0.6).sin().abs();
            }
            m
        }
        _ => return None,
    };
    if m.glows.is_empty() && m.faces.is_empty() && m.lines.is_empty() {
        return None;
    }
    Some(m)
}
