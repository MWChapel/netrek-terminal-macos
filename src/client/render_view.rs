//! The viewscreen: what lies dead ahead of your ship, in perspective, as if
//! looking out of the bridge. The galaxy is flat, so the camera rides a
//! little above its plane: near things sit low, far things close to the
//! horizon, and the space dust streams past as you move.

use super::palette::{callsign, mix, planet_rgb, player_rgb, rgb, scale, ship_size_units, team_rgb, torp_rgb, Rgb};
use super::shipart::{ship_parts, Part};
use super::sixel::TextRenderer;
use super::vg::Canvas;
use super::App;
use crate::consts::*;
use crate::proto::*;
use std::f32::consts::{PI, TAU};

const WHITE: Rgb = [255.0, 255.0, 255.0];
const GREY: Rgb = rgb(0x8a8f99);
/// Horizontal field of view.
const FOV: f32 = 75.0 * PI / 180.0;
/// Height of the camera above the galaxy's plane, in galaxy units.
pub(super) const EYE: f32 = 600.0;
/// Nothing closer than this is drawn (it's beside or behind the camera).
pub(super) const NEAR: f32 = 150.0;
/// How far the viewscreen sees planets, ships and space dust.
const PLANET_RANGE: f32 = 45_000.0;
const SHIP_RANGE: f32 = 30_000.0;
const DUST_RANGE: f32 = 9_000.0;
/// Planets look this big (galaxy units of radius).
const PLANET_R: f32 = 600.0;

fn hash(x: i64, y: i64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 32)
}

/// The camera: where it is, which way it faces, and how it projects.
pub(super) struct Cam {
    pub(super) x: f32,
    pub(super) y: f32,
    /// Forward and right unit vectors in galaxy space.
    pub(super) fwd: (f32, f32),
    pub(super) right: (f32, f32),
    /// Screen centre, horizon line and focal length in pixels.
    pub(super) cx: f32,
    pub(super) horizon: f32,
    pub(super) focal: f32,
    pub(super) heading: f32,
}

impl Cam {
    /// Galaxy point -> (sideways offset, depth) from the camera.
    pub(super) fn local(&self, x: f32, y: f32) -> (f32, f32) {
        let (dx, dy) = (x - self.x, y - self.y);
        (dx * self.right.0 + dy * self.right.1, dx * self.fwd.0 + dy * self.fwd.1)
    }

    /// (sideways, depth) on the galaxy plane -> screen pixels.
    pub(super) fn project(&self, (lat, z): (f32, f32)) -> (f32, f32) {
        (self.cx + lat / z * self.focal, self.horizon + EYE / z * self.focal)
    }

    /// A point in camera space (sideways, up, depth) -> screen pixels.
    pub(super) fn screen3(&self, q: V3) -> (f32, f32) {
        (self.cx + q[0] / q[2] * self.focal, self.horizon + (EYE - q[1]) / q[2] * self.focal)
    }

    /// A galaxy point at height `alt` -> camera space.
    pub(super) fn at(&self, x: f32, y: f32, alt: f32) -> V3 {
        let (l, z) = self.local(x, y);
        [l, alt, z]
    }

    /// Clip a polygon in camera space to the near plane and project it;
    /// None if none of it is in front of us.
    pub(super) fn clip_poly(&self, pts: &[V3]) -> Option<Vec<(f32, f32)>> {
        let n = pts.len();
        let mut out = Vec::with_capacity(n + 2);
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            if a[2] >= NEAR {
                out.push(self.screen3(a));
            }
            if (a[2] >= NEAR) != (b[2] >= NEAR) {
                let t = (NEAR - a[2]) / (b[2] - a[2]);
                out.push(self.screen3([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, NEAR]));
            }
        }
        (out.len() >= 3).then_some(out)
    }

    /// Clip a segment in camera space (sideways, up, depth) to the near
    /// plane and project it.
    pub(super) fn segment3(&self, a: V3, b: V3) -> Option<((f32, f32), (f32, f32))> {
        if a[2] < NEAR && b[2] < NEAR {
            return None;
        }
        let cut = |p: V3, q: V3| {
            if p[2] >= NEAR {
                return p;
            }
            let t = (NEAR - p[2]) / (q[2] - p[2]);
            [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, NEAR]
        };
        Some((self.screen3(cut(a, b)), self.screen3(cut(b, a))))
    }

    /// Clip a segment (in camera space) to the near plane and project it.
    pub(super) fn segment(&self, a: (f32, f32), b: (f32, f32)) -> Option<((f32, f32), (f32, f32))> {
        if a.1 < NEAR && b.1 < NEAR {
            return None;
        }
        let cut = |p: (f32, f32), q: (f32, f32)| {
            if p.1 >= NEAR {
                return p;
            }
            let t = (NEAR - p.1) / (q.1 - p.1);
            (p.0 + (q.0 - p.0) * t, NEAR)
        };
        Some((self.project(cut(a, b)), self.project(cut(b, a))))
    }
}

/// Something to draw, kept so we can paint from far to near.
enum Thing<'a> {
    Planet(usize),
    Ship(&'a PlayerInfo),
    Torp(&'a TorpInfo),
    Terrain(&'a TerrainInfo),
    Ring(super::render_view_terrain::RingPiece),
    Zone(super::render_view_terrain::ZonePiece),
    Web(super::render_view_terrain::WebPiece),
}

impl App {
    /// The forward viewscreen for the ship you're flying (or following).
    pub(super) fn draw_viewscreen_vec(&self, pw: i32, ph: i32) -> Canvas {
        let fs = (self.cell_px.1 as f32 * 0.55).clamp(9.0, 14.0);
        // Observers on a free camera watch from the middle of the view.
        let watch = self.observer.then(|| (self.last_center.0 as f32, self.last_center.1 as f32));
        viewscreen(&self.text, self.frame.as_ref(), self.slot, watch, fs, pw, ph)
    }
}

/// The viewscreen from the ship in `slot`; failing that, from `watch` (an
/// observer's free camera), slowly turning to take in the whole scene.
pub(super) fn viewscreen(tr: &TextRenderer, f: Option<&Frame>, slot: u8, watch: Option<(f32, f32)>, fs: f32, pw: i32, ph: i32) -> Canvas {
    let mut c = Canvas::new(pw, ph, [2.0, 3.0, 8.0]);
    let (w, h) = (pw as f32, ph as f32);
    let Some(f) = f else {
        return no_signal(c, tr, w, h, fs);
    };
    let me = f.players.iter().find(|p| p.id == slot && matches!(p.state, PState::Alive | PState::Exploding));
    let (x, y, heading) = match (me, watch) {
        (Some(p), _) => (p.x as f32, p.y as f32, p.dir as f32 * TAU / 256.0),
        // A full turn about every two minutes.
        (None, Some((x, y))) => (x, y, (f.tick as f32 * TAU / 1200.0).rem_euclid(TAU)),
        (None, None) => return no_signal(c, tr, w, h, fs),
    };
    let (sa, ca) = heading.sin_cos();
    let cam = Cam {
        x,
        y,
        fwd: (sa, -ca),
        right: (ca, sa),
        cx: w / 2.0,
        horizon: h * 0.38,
        focal: (w / 2.0) / (FOV / 2.0).tan(),
        heading,
    };
    let on_screen = |x: f32, y: f32, m: f32| x > -m && x < w + m && y > -m && y < h + m;

    draw_stars(&mut c, &cam, w, h);
    draw_dust(&mut c, &cam, w, h);

    // The edge of the galaxy, as a red line across the plane.
    let g = GWIDTH as f32;
    let corners = [(0.0, 0.0), (g, 0.0), (g, g), (0.0, g)];
    for k in 0..4 {
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        if let Some((p, q)) = cam.segment(cam.local(a.0, a.1), cam.local(b.0, b.1)) {
            c.line(p.0, p.1, q.0, q.1, 1.0, rgb(0xe02020), 0.45, 0.0);
        }
    }

    // Everything ahead, sorted far to near.
    let mut things: Vec<(f32, Thing)> = Vec::new();
    for (k, def) in super::planet_defs(f).iter().enumerate() {
        let (_, z) = cam.local(def.x as f32, def.y as f32);
        if z > NEAR && z < PLANET_RANGE {
            things.push((z, Thing::Planet(k)));
        }
    }
    for p in &f.players {
        if Some(p.id) == me.map(|m| m.id) || p.fuzzy || !matches!(p.state, PState::Alive | PState::Exploding) {
            continue;
        }
        let (_, z) = cam.local(p.x as f32, p.y as f32);
        if z > NEAR && z < SHIP_RANGE {
            things.push((z, Thing::Ship(p)));
        }
    }
    for t in &f.torps {
        let (_, z) = cam.local(t.x as f32, t.y as f32);
        if z > NEAR && z < SHIP_RANGE {
            things.push((z, Thing::Torp(t)));
        }
    }
    for t in &f.terrain {
        if let Some(z) = super::render_view_terrain::depth(&cam, t) {
            things.push((z, Thing::Terrain(t)));
        }
    }
    things.extend(super::render_view_terrain::zone_pieces(&cam, f).into_iter().map(|(z, p)| (z, Thing::Zone(p))));
    things.extend(super::render_view_terrain::ring_pieces(&cam, f).into_iter().map(|(z, p)| (z, Thing::Ring(p))));
    things.extend(super::render_view_terrain::web_pieces(&cam, f).into_iter().map(|(z, p)| (z, Thing::Web(p))));
    things.sort_by(|a, b| b.0.total_cmp(&a.0));

    let defs = super::planet_defs(f);
    let ring_lift = super::render_view_terrain::ring_lift(&cam, f);
    for (z, thing) in things {
        // Far things fade into the dark.
        let fade = |range: f32| (1.0 - (z / range - 0.7).max(0.0) / 0.3).clamp(0.0, 1.0);
        match thing {
            Thing::Planet(k) => {
                let def = &defs[k];
                let info = &f.planets[k];
                // Ringworld sections are stretches of the ring itself (which may
                // be tipped toward us): no globe, just the name over the band.
                let section = k >= PLANETS.len();
                let alt = if section { ring_lift(def.x as f32, def.y as f32) + super::render_view_terrain::section_alt() } else { 0.0 };
                let (sx, sy) = cam.screen3(cam.at(def.x as f32, def.y as f32, alt));
                let r = (PLANET_R / z * cam.focal).max(1.5);
                if !on_screen(sx, sy, r + 40.0) {
                    continue;
                }
                let col = if info.known { planet_rgb(info) } else { GREY };
                if section {
                    let label = if info.known && r > 4.0 { format!("{} {}", def.name, info.armies) } else { def.name.to_string() };
                    c.text_centered(tr, sx, sy - fs * 0.6, &label, fs, scale(col, 0.4 + 0.6 * fade(PLANET_RANGE)));
                    continue;
                }
                draw_planet(&mut c, sx, sy, r, col, fade(PLANET_RANGE));
                if r > 2.5 {
                    let label = if info.known && r > 10.0 { format!("{} {}", def.name, info.armies) } else { def.name.to_string() };
                    c.text_centered(tr, sx, sy + r + fs * 1.1, &label, fs, scale(col, 0.4 + 0.6 * fade(PLANET_RANGE)));
                }
            }
            Thing::Ship(p) => {
                let (sx, sy) = cam.project(cam.local(p.x as f32, p.y as f32));
                let size = if ship_size_units(p.ship) > 0.0 { ship_size_units(p.ship) as f32 } else if p.ship == ShipType::Starbase { 800.0 } else { 450.0 };
                let s = (size / z * cam.focal).max(7.0);
                if !on_screen(sx, sy, s * 2.0) {
                    continue;
                }
                let col = player_rgb(p);
                if p.state == PState::Exploding {
                    let e = p.explode_frame as f32 / 10.0;
                    let k = (1.0 - e).max(0.0);
                    c.glow(sx, sy, s * (1.0 + 2.5 * e), rgb(0xffa040), 0.9 * k);
                    c.glow(sx, sy, s * (0.4 + e), rgb(0xfff0c0), k);
                    continue;
                }
                let top = draw_ship(&mut c, &cam, p, size, col, fade(SHIP_RANGE), f.tick);
                if p.flags & (pf::CLOAK | pf::PHASED) == 0 {
                    c.text_centered(tr, sx, top - fs * 0.5, &callsign(p), fs * 0.9, scale(col, 0.5 + 0.5 * fade(SHIP_RANGE)));
                }
            }
            Thing::Zone(piece) => super::render_view_terrain::draw_zone_piece(&mut c, &cam, f, piece, tr, fs),
            Thing::Ring(piece) => super::render_view_terrain::draw_ring_piece(&mut c, &cam, f, piece, tr, fs),
            Thing::Terrain(t) => super::render_view_terrain::draw(&mut c, &cam, t, f.tick, tr, fs),
            Thing::Web(piece) => super::render_view_terrain::draw_web_piece(&mut c, &cam, piece, f.tick),
            Thing::Torp(t) => {
                let (sx, sy) = cam.project(cam.local(t.x as f32, t.y as f32));
                if !on_screen(sx, sy, 20.0) {
                    continue;
                }
                let col = if t.owner == slot { mix(team_rgb(t.team), WHITE, 0.5) } else { torp_rgb(f, t) };
                let r = (60.0 / z * cam.focal).clamp(1.2, 8.0);
                if t.explode > 0 {
                    let e = t.explode as f32 / 7.0;
                    c.glow(sx, sy, r * (3.0 + 6.0 * e), rgb(0xffb050), 0.8 * (1.0 - e));
                } else {
                    c.glow(sx, sy, r * 3.0, col, 0.6);
                    c.disc(sx, sy, r * 0.6, mix(col, WHITE, 0.7), 1.0);
                }
            }
        }
    }

    // Phasers, drawn over everything: yours lance out from below the screen.
    for ph in &f.phasers {
        let a = cam.local(ph.x1 as f32, ph.y1 as f32);
        let b = cam.local(ph.x2 as f32, ph.y2 as f32);
        let Some((p, q)) = cam.segment(a, b) else { continue };
        let col = match f.players.iter().find(|p| p.id == ph.owner) {
            Some(p) if p.ship == ShipType::DeathStar => rgb(0x40ff60),
            Some(p) if p.ship == ShipType::CygnusShip => rgb(0xff3030),
            Some(p) => player_rgb(p),
            None => WHITE,
        };
        c.line(p.0, p.1, q.0, q.1, 6.0, col, 0.25, 0.0);
        c.line(p.0, p.1, q.0, q.1, 1.6, mix(col, WHITE, 0.7), 0.95, 0.0);
        if ph.hit && b.1 > NEAR {
            c.glow(q.0, q.1, 10.0, mix(col, WHITE, 0.4), 0.8);
        }
    }

    // Shields shimmer at the edge of the screen.
    if let Some(me) = me.filter(|m| m.flags & pf::SHIELD != 0) {
        let frac = (f.me_info.shield as f32 / me.ship.stats().max_shield as f32).clamp(0.0, 1.0);
        let sc = if frac > 0.6 { rgb(0x40a0ff) } else if frac > 0.3 { rgb(0xffd24a) } else { rgb(0xff4a3a) };
        for k in 0..4 {
            let i = k as f32 * 2.0 + 1.0;
            c.round_rect(i, i, w - 2.0 * i, h - 2.0 * i, 8.0, sc, 0.18 - k as f32 * 0.04, Some(1.5));
        }
    }

    super::render_view_terrain::draw_inside(&mut c, &cam, f, w, h, tr, fs);
    draw_hud(&mut c, tr, &cam, me, f, w, h, fs);
    c
}

fn no_signal(mut c: Canvas, tr: &TextRenderer, w: f32, h: f32, fs: f32) -> Canvas {
    // A little static.
    let mut s = 0x1234_5678u64;
    for _ in 0..((w * h) as usize / 60) {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let (x, y) = ((s >> 33) as f32 % w, (s >> 13) as f32 % h);
        let b = 40.0 + ((s >> 5) & 0x3f) as f32;
        c.dot(x, y, [b, b, b], 1.0);
    }
    c.text_centered(tr, w / 2.0, h / 2.0, "VIEWSCREEN — NO SIGNAL", fs, GREY);
    c
}

/// Distant stars on the sky, turning as you turn.
fn draw_stars(c: &mut Canvas, cam: &Cam, w: f32, h: f32) {
    let half = FOV / 2.0 + 0.1;
    for k in 0..420 {
        let hv = hash(k, 77);
        let az = (hv % 100_000) as f32 / 100_000.0 * TAU;
        let mut rel = az - cam.heading;
        rel = (rel + PI).rem_euclid(TAU) - PI;
        if rel.abs() > half {
            continue;
        }
        let el = ((hv >> 20) % 10_000) as f32 / 10_000.0 * 0.9 - 0.2;
        let x = cam.cx + rel.tan() * cam.focal;
        let y = cam.horizon - el.tan() * cam.focal;
        if x < 0.0 || x >= w || y < 0.0 || y >= h {
            continue;
        }
        let b = 70.0 + ((hv >> 40) % 150) as f32;
        let tint = match (hv >> 50) % 6 {
            0 => [b, b * 0.85, b * 0.7],
            1 => [b * 0.8, b * 0.9, b],
            _ => [b, b, b],
        };
        c.dot(x, y, tint, 1.0);
        if b > 205.0 {
            c.glow(x, y, 2.5, tint, 0.5);
        }
    }
}

/// Space dust fixed in the galaxy, close by: it streams past as you fly.
fn draw_dust(c: &mut Canvas, cam: &Cam, w: f32, h: f32) {
    let cell = 900.0;
    let n = (DUST_RANGE / cell).ceil() as i64;
    let (gx, gy) = ((cam.x / cell).floor() as i64, (cam.y / cell).floor() as i64);
    for j in gy - n..=gy + n {
        for i in gx - n..=gx + n {
            let hv = hash(i ^ 0x5a5a, j);
            if hv % 3 != 0 {
                continue;
            }
            let x = i as f32 * cell + ((hv >> 8) % 1000) as f32 / 1000.0 * cell;
            let y = j as f32 * cell + ((hv >> 24) % 1000) as f32 / 1000.0 * cell;
            let (lat, z) = cam.local(x, y);
            if z < NEAR * 2.0 || z > DUST_RANGE {
                continue;
            }
            // Each mote floats at its own height above or below the plane.
            let alt = ((hv >> 36) % 1000) as f32 / 1000.0 * 2.0 * EYE;
            let sx = cam.cx + lat / z * cam.focal;
            let sy = cam.horizon + (EYE - alt) / z * cam.focal;
            if sx < 0.0 || sx >= w || sy < 0.0 || sy >= h {
                continue;
            }
            let b = 160.0 * (1.0 - z / DUST_RANGE);
            c.disc(sx, sy, (60.0 / z * cam.focal).clamp(0.5, 1.6), [b, b, b * 1.1], 1.0);
        }
    }
}

/// A lit sphere with a thin atmosphere.
fn draw_planet(c: &mut Canvas, x: f32, y: f32, r: f32, col: Rgb, fade: f32) {
    c.glow(x, y, r * 1.35, col, 0.18 * fade);
    c.disc(x, y, r, scale(col, 0.18 * fade + 0.04), 1.0);
    if r > 3.0 {
        // Sunlit from the upper left.
        c.disc(x - r * 0.18, y - r * 0.18, r * 0.78, scale(col, 0.34 * fade), 0.8);
        c.disc(x - r * 0.32, y - r * 0.32, r * 0.45, scale(col, 0.55 * fade), 0.6);
        c.glow(x - r * 0.42, y - r * 0.42, r * 0.35, mix(col, WHITE, 0.5), 0.35 * fade);
    }
    c.ring(x, y, r, 1.0, col, 0.8 * fade);
}

/// A point or direction in ship space: (x, y) as in the top-down art
/// (ship radii, nose toward -y), h up out of the galaxy's plane.
pub(super) type V3 = [f32; 3];

pub(super) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub(super) fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}

pub(super) fn centroid(pts: &[V3]) -> V3 {
    let n = pts.len() as f32;
    pts.iter().fold([0.0; 3], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n, s[2] + p[2] / n])
}

/// One flat face of a ship's hull.
pub(super) struct Face {
    pub pts: Vec<V3>,
    pub n: V3,
    /// An opening in the hull: drawn dark, unlit.
    pub hole: bool,
    /// Outline it (hull plates do; the facets of a dome don't).
    pub edge: bool,
    /// Its own paint, instead of the hull's.
    pub paint: Option<Rgb>,
}

impl Face {
    pub fn new(pts: Vec<V3>, n: V3, edge: bool) -> Face {
        Face { pts, n: norm(n), hole: false, edge, paint: None }
    }

    /// The same face in its own paint.
    pub fn painted(mut self, c: Rgb) -> Face {
        self.paint = Some(c);
        self
    }

    /// A face whose normal is worked out from its corners, turned to point
    /// away from `inside`.
    pub fn facing_out(pts: Vec<V3>, inside: V3, edge: bool) -> Face {
        let c = centroid(&pts);
        let mut n = cross(sub(pts[1], pts[0]), sub(pts[pts.len() - 1], pts[0]));
        if dot(n, n) < 1e-12 && pts.len() > 2 {
            n = cross(sub(pts[2], pts[1]), sub(pts[0], pts[1]));
        }
        if dot(n, sub(c, inside)) < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        Face::new(pts, n, edge)
    }
}

/// A hull plate: the outline swelled into a rounded, pillowed solid,
/// `up` thick above `base` and `down` below.
pub(super) fn pillow(pts: &[(f32, f32)], base: f32, up: f32, down: f32, out: &mut Vec<Face>) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    let area: f32 = (0..n).map(|i| pts[i].0 * pts[(i + 1) % n].1 - pts[(i + 1) % n].0 * pts[i].1).sum();
    let s = area.signum();
    let (cx, cy) = (pts.iter().map(|p| p.0).sum::<f32>() / n as f32, pts.iter().map(|p| p.1).sum::<f32>() / n as f32);
    let inside = [cx, cy, base];
    // Rings from the waist up (and down), drawing in toward the middle.
    const STEPS: usize = 4;
    let ring = |k: usize, dir: f32, t: f32, start: f32| -> Vec<V3> {
        let a = k as f32 / STEPS as f32 * PI / 2.0 * 0.92;
        let (inset, h) = (0.35 + 0.65 * a.cos(), start + dir * t * 0.75 * a.sin());
        pts.iter().map(|&(x, y)| [cx + (x - cx) * inset, cy + (y - cy) * inset, h]).collect()
    };
    // The waist: a short vertical band all round.
    let (wt, wb) = (base + up * 0.25, base - down * 0.25);
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        if dx != 0.0 || dy != 0.0 {
            out.push(Face::new(vec![[a.0, a.1, wb], [b.0, b.1, wb], [b.0, b.1, wt], [a.0, a.1, wt]], [s * dy, -s * dx, 0.0], false));
        }
    }
    for (dir, t, start) in [(1.0, up, wt), (-1.0, down, wb)] {
        let rings: Vec<Vec<V3>> = (0..=STEPS).map(|k| ring(k, dir, t, start)).collect();
        for k in 0..STEPS {
            let (a, b) = (&rings[k], &rings[k + 1]);
            for i in 0..n {
                let i1 = (i + 1) % n;
                let mut f = Face::facing_out(vec![a[i], a[i1], b[i1], b[i]], inside, false);
                if f.n[2] * dir < 0.0 {
                    f.n = [-f.n[0], -f.n[1], -f.n[2]];
                }
                out.push(f);
            }
        }
        out.push(Face::new(rings[STEPS].clone(), [0.0, 0.0, dir], false));
    }
}

/// A round part: a domed saucer on a short band, or (`sphere`) a ball.
pub(super) fn dome(c: (f32, f32), r: f32, lo: f32, hi: f32, sphere: bool, fine: bool, out: &mut Vec<Face>) {
    let (seg, levels) = if fine { (36, 9) } else { (14, 3) };
    let ring = |k: usize, up: bool| -> Vec<V3> {
        let phi = k as f32 / levels as f32 * PI / 2.0;
        let (rr, dh) = (r * phi.cos(), r * if sphere { 1.0 } else { 0.35 } * phi.sin());
        let h = if up { hi + dh } else { lo - if sphere { dh } else { 0.0 } };
        (0..seg).map(|j| {
            let a = j as f32 / seg as f32 * TAU;
            [c.0 + a.cos() * rr, c.1 + a.sin() * rr, h]
        }).collect()
    };
    let mid = [c.0, c.1, (lo + hi) / 2.0];
    let caps: &[bool] = if sphere { &[true, false] } else { &[true] };
    for &up in caps {
        for k in 0..levels {
            let (a, b) = (ring(k, up), ring(k + 1, up));
            for j in 0..seg {
                let j1 = (j + 1) % seg;
                let pts = if k + 1 == levels { vec![a[j], a[j1], b[j]] } else { vec![a[j], a[j1], b[j1], b[j]] };
                out.push(Face::facing_out(pts, mid, false));
            }
        }
    }
    // The band round the rim (a sphere's equator), and for a saucer a flat
    // underside.
    let (top, bot) = (ring(0, true), ring(0, false));
    for j in 0..seg {
        let j1 = (j + 1) % seg;
        out.push(Face::facing_out(vec![bot[j], bot[j1], top[j1], top[j]], mid, false));
    }
    if !sphere {
        out.push(Face::new(bot, [0.0, 0.0, -1.0], true));
    }
}

/// The ship's art as a solid: each part a plate a little above the last,
/// so later parts sit on top of earlier ones as they do in the drawing.
pub(super) fn ship_mesh(parts: Vec<Part>, fine: bool) -> (Vec<Face>, Vec<(V3, V3)>) {
    let mut faces = Vec::new();
    // The hull's top surface, for draping the drawing's lines over it.
    let mut tops: Vec<Box<dyn Fn(f32, f32) -> Option<f32>>> = Vec::new();
    let mut flat = Vec::new();
    for (k, part) in parts.into_iter().enumerate() {
        let lift = k as f32 * 0.02;
        match part {
            Part::Poly { pts, .. } => {
                let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.0), m.1.max(p.0)));
                let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.1), m.1.max(p.1)));
                let t = ((x1 - x0).min(y1 - y0) * 0.4).clamp(0.05, 0.2);
                pillow(&pts, lift, t, t * 0.7, &mut faces);
                tops.push(Box::new(move |x, y| inside_poly(&pts, x, y).then_some(lift + t * 0.85)));
            }
            Part::Circle { c, r, .. } => {
                let t = (r * 0.25).clamp(0.02, 0.08);
                let sphere = r >= 0.7;
                dome(c, r, lift - t, lift + t, sphere, fine, &mut faces);
                tops.push(Box::new(move |x, y| {
                    let d2 = (x - c.0).powi(2) + (y - c.1).powi(2);
                    (d2 < r * r).then(|| lift + t + (r * r - d2).sqrt() * if sphere { 1.0 } else { 0.35 })
                }));
            }
            Part::Hole { pts } => {
                let h = lift + 0.2;
                faces.push(Face { pts: pts.iter().map(|&(x, y)| [x, y, h]).collect(), n: [0.0, 0.0, 1.0], hole: true, edge: true, paint: None });
            }
            Part::Line { a, b } => flat.push((a, b, lift)),
        }
    }
    // Lines are painted on the hull: drape each one over whatever is
    // beneath it, in short steps so it follows the curves.
    let mut lines = Vec::new();
    for (a, b, lift) in flat {
        const STEPS: usize = 10;
        let at = |s: f32| {
            let (x, y) = (a.0 + (b.0 - a.0) * s, a.1 + (b.1 - a.1) * s);
            let h = tops.iter().filter_map(|top| top(x, y)).fold(lift, f32::max);
            [x, y, h + 0.015]
        };
        for k in 0..STEPS {
            lines.push((at(k as f32 / STEPS as f32), at((k + 1) as f32 / STEPS as f32)));
        }
    }
    (faces, lines)
}

/// A capped cylinder along the ship's length (y) from `y0` to `y1`, its
/// axis at sideways offset and height `(x, h)`.
pub(super) fn cylinder((x, h): (f32, f32), y0: f32, y1: f32, r: f32, seg: usize, out: &mut Vec<Face>) {
    let at = |k: usize, y: f32| {
        let a = k as f32 / seg as f32 * TAU;
        [x + a.cos() * r, y, h + a.sin() * r]
    };
    for k in 0..seg {
        let pts = vec![at(k, y0), at(k + 1, y0), at(k + 1, y1), at(k, y1)];
        out.push(Face::facing_out(pts, [x, (y0 + y1) / 2.0, h], false));
    }
    out.push(Face::new((0..seg).map(|k| at(k, y0)).collect(), [0.0, -1.0, 0.0], true));
    out.push(Face::new((0..seg).map(|k| at(k, y1)).collect(), [0.0, 1.0, 0.0], true));
}

/// A ball centred at `c`.
pub(super) fn ball(c: V3, r: f32, seg: usize, levels: usize, out: &mut Vec<Face>) {
    let at = |i: usize, j: usize| {
        let lon = i as f32 / seg as f32 * TAU;
        let lat = -PI / 2.0 + j as f32 / levels as f32 * PI;
        [c[0] + lon.cos() * lat.cos() * r, c[1] + lon.sin() * lat.cos() * r, c[2] + lat.sin() * r]
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

/// Whether (x, y) is inside the polygon.
fn inside_poly(pts: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut inside = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a.1 > y) != (b.1 > y) && x < a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            inside = !inside;
        }
    }
    inside
}

/// Where a model sits and how it's turned, for drawing it in 3D.
pub(super) struct Pose<'a> {
    cam: &'a Cam,
    /// Its centre in camera space (sideways, depth), and its size in
    /// galaxy units.
    pub lat0: f32,
    pub z0: f32,
    pub s: f32,
    sb: f32,
    cb: f32,
    st: f32,
    ct: f32,
}

impl<'a> Pose<'a> {
    /// A model of radius `size` at (x, y), turned to `heading` (radians,
    /// 0 = nose north). Far away it's drawn bigger than life, at least
    /// `min_px` pixels, so it stays readable.
    pub fn new(cam: &'a Cam, x: f32, y: f32, size: f32, heading: f32, min_px: f32) -> Pose<'a> {
        let (lat0, z0) = cam.local(x, y);
        let s = size.max(min_px * z0 / cam.focal);
        let (sb, cb) = heading.sin_cos();
        // Tip the model toward us a little, so we always see some of its top.
        let tilt = (0.38 - (EYE / z0.max(1.0)).atan()).max(0.0);
        let (st, ct) = tilt.sin_cos();
        Pose { cam, lat0, z0, s, sb, cb, st, ct }
    }

    /// Model-space direction -> camera space (sideways, up, depth).
    pub fn dir(&self, v: V3) -> V3 {
        let (ox, oy) = (v[0] * self.cb - v[1] * self.sb, v[0] * self.sb + v[1] * self.cb);
        let (l, f) = (ox * self.cam.right.0 + oy * self.cam.right.1, ox * self.cam.fwd.0 + oy * self.cam.fwd.1);
        [l, v[2] * self.ct + f * self.st, f * self.ct - v[2] * self.st]
    }

    /// Model-space point -> camera space.
    pub fn to_cam(&self, v: V3) -> V3 {
        let q = self.dir(v);
        [self.lat0 + q[0] * self.s, q[1] * self.s, self.z0 + q[2] * self.s]
    }

    /// Rough size on screen, in pixels.
    pub fn px(&self) -> f32 {
        self.s / self.z0 * self.cam.focal
    }
}

/// A model as a lit solid in true perspective: `col` lights its rim and
/// outlines, `hull` is its paint. Returns the top of its picture on screen
/// (for a label), or None if none of it was in view.
pub(super) fn draw_mesh(c: &mut Canvas, pose: &Pose, parts: Vec<Part>, col: Rgb, hull: Rgb, alpha: f32, ghost: bool) -> Option<f32> {
    let model = Model::from_mesh(ship_mesh(parts, pose.px() > 25.0));
    draw_model(c, pose, &model, col, hull, alpha, ghost)
}

/// A ship's 3D model, in ship radii: x to starboard, y aft (the nose is
/// toward -y, as in the drawings), h up out of the galaxy's plane.
pub(super) struct Model {
    pub faces: Vec<Face>,
    /// Lines painted on the hull (or, `neon`, glowing in space).
    pub lines: Vec<(V3, V3)>,
    /// Paint, if not the usual metal tinted with the ship's colour.
    pub hull: Option<Rgb>,
    /// The colour of its lines, if not the ship's.
    pub line_col: Option<Rgb>,
    /// How solid it is (1 = opaque; less for things you can see into).
    pub alpha: f32,
    /// Lights and glows: where, how big (ship radii), colour and strength.
    pub glows: Vec<(V3, f32, Rgb, f32)>,
    /// Lines drawn as glowing neon, like an arcade vector screen.
    pub neon: bool,
    /// Always turned to face the viewer.
    pub billboard: bool,
    /// Has blinking running lights (ships do; creatures don't).
    pub lights: bool,
}

impl Model {
    pub fn from_mesh((faces, lines): (Vec<Face>, Vec<(V3, V3)>)) -> Model {
        Model { faces, lines, hull: None, line_col: None, alpha: 1.0, glows: Vec::new(), neon: false, billboard: false, lights: true }
    }
}

/// Draw a model: lit faces and the lines on them, far to near, then its
/// glows. Returns the top of its picture on screen, if any of it showed.
fn draw_model(c: &mut Canvas, pose: &Pose, model: &Model, col: Rgb, hull: Rgb, alpha: f32, ghost: bool) -> Option<f32> {
    let hull = model.hull.unwrap_or(hull);
    let alpha = alpha * model.alpha;
    let lcol = model.line_col.unwrap_or(col);
    let cam = pose.cam;
    let mut top_y = f32::MAX;
    if model.neon {
        // Glowing vector lines, with no hull to hide them.
        let k = (pose.px() / 40.0).clamp(0.6, 2.5);
        for &(a, b) in &model.lines {
            if let Some((p, q)) = cam.segment3(pose.to_cam(a), pose.to_cam(b)) {
                c.line(p.0, p.1, q.0, q.1, 5.0 * k, lcol, 0.22 * alpha, 0.0);
                c.line(p.0, p.1, q.0, q.1, 1.4 * k, mix(lcol, WHITE, 0.45), alpha, 0.0);
                top_y = top_y.min(p.1).min(q.1);
            }
        }
    }
    let lines: &[(V3, V3)] = if model.neon { &[] } else { &model.lines };
    if let Some(t) = draw_faces(c, pose, &model.faces, lines, col, lcol, hull, alpha, ghost) {
        top_y = top_y.min(t);
    }
    if !ghost {
        for &(at, r, gc, a) in &model.glows {
            let q = pose.to_cam(at);
            if q[2] > NEAR {
                let (gx, gy) = cam.screen3(q);
                let rp = (r * pose.s / q[2] * cam.focal).max(1.5);
                c.glow(gx, gy, rp, gc, a * alpha.max(0.5));
                top_y = top_y.min(gy - rp * 0.5);
            }
        }
    }
    (top_y != f32::MAX).then_some(top_y)
}

/// Draw faces and the lines painted on them.
fn draw_faces(c: &mut Canvas, pose: &Pose, faces: &[Face], lines: &[(V3, V3)], col: Rgb, lcol: Rgb, hull: Rgb, alpha: f32, ghost: bool) -> Option<f32> {
    let cam = pose.cam;
    let eye = [0.0, EYE, 0.0];
    // Light from the upper left, a little behind the viewer.
    let light = norm([-0.55, 0.75, -0.4]);
    let half = norm([light[0], light[1] + 0.3, light[2] - 1.0]);
    let px = pose.px();
    let mut shown: Vec<(f32, Vec<(f32, f32)>, Rgb, bool, bool)> = Vec::new();
    let mut top_y = f32::MAX;
    for face in faces {
        let pts: Vec<V3> = face.pts.iter().map(|&v| pose.to_cam(v)).collect();
        if pts.iter().any(|q| q[2] < NEAR) {
            continue;
        }
        let n = pose.dir(face.n);
        let ctr = centroid(&pts);
        if dot(n, sub(eye, ctr)) <= 0.0 {
            continue; // facing away
        }
        let shade = if face.hole {
            [6.0, 7.0, 10.0]
        } else {
            let d = dot(n, light).max(0.0);
            let spec = dot(n, half).max(0.0).powi(18) * 0.55;
            // Rim light in the model's colour picks out its silhouette.
            let rim = (1.0 - dot(n, norm(sub(eye, ctr))).abs()).powi(3) * 0.7;
            mix(mix(scale(face.paint.unwrap_or(hull), 0.24 + 0.76 * d), col, rim), WHITE, spec)
        };
        let sp: Vec<(f32, f32)> = pts.iter().map(|&q| cam.screen3(q)).collect();
        top_y = sp.iter().fold(top_y, |m, q| m.min(q.1));
        shown.push((dot(sub(ctr, eye), sub(ctr, eye)), sp, shade, face.edge, face.hole));
    }
    // Lines go in the same far-to-near order, so the far side of the hull
    // hides them; each sorts just in front of the surface it lies on.
    let edge = mix(col, WHITE, 0.35);
    let line_edge = mix(lcol, WHITE, 0.35);
    let line_w = (px * 0.04).clamp(0.8, 3.0);
    for &(a, b) in lines {
        let (a, b) = (pose.to_cam(a), pose.to_cam(b));
        if a[2] > NEAR && b[2] > NEAR {
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
            let d = dot(sub(mid, eye), sub(mid, eye)).sqrt() - pose.s * 0.03;
            let (a, b) = (cam.screen3(a), cam.screen3(b));
            shown.push((d.max(0.0).powi(2), vec![a, b], line_edge, false, true));
        }
    }
    shown.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, sp, shade, outline, hole) in &shown {
        if sp.len() == 2 {
            // A line drawn on the hull.
            c.line(sp[0].0, sp[0].1, sp[1].0, sp[1].1, line_w, *shade, alpha, if ghost { 2.0 } else { 0.0 });
            continue;
        }
        if ghost {
            if *outline {
                c.polyline(sp, true, 0.8, edge, alpha, 2.0);
            }
            continue;
        }
        c.fill_poly(sp, *shade, alpha);
        // Fill the hairline seams between facets (on solid hulls).
        if alpha > 0.95 {
            c.polyline(sp, true, 0.6, *shade, alpha * 0.9, 0.0);
        }
        if *outline && px > 14.0 && !*hole {
            c.polyline(sp, true, 0.7, edge, alpha * 0.35, 0.0);
        }
    }
    (top_y != f32::MAX).then_some(top_y)
}

/// A ship as a lit, solid 3D model. `size` is its radius in galaxy units.
/// Returns the top of its picture on screen, for the label.
fn draw_ship(c: &mut Canvas, cam: &Cam, p: &PlayerInfo, size: f32, col: Rgb, fade: f32, tick: u32) -> f32 {
    let cloaked = p.flags & (pf::CLOAK | pf::PHASED) != 0;
    let alpha = if cloaked { 0.25 } else { fade.max(0.3) };
    let mut pose = Pose::new(cam, p.x as f32, p.y as f32, size, p.dir as f32 * TAU / 256.0, 7.0);
    let fine = pose.px() > 25.0;
    let model = match super::render_models::alien_model(p.ship, p.faction, tick, fine) {
        Some(m) => m,
        None => Model::from_mesh(ship_mesh(ship_parts(p.team, p.ship, p.faction), fine)),
    };
    if model.billboard {
        pose = Pose::new(cam, p.x as f32, p.y as f32, size, cam.heading, 7.0);
    }
    let (lat0, z0, s, px) = (pose.lat0, pose.z0, pose.s, pose.px());

    // Engine glow behind the hull.
    if p.speed > 0 && !cloaked {
        let k = p.speed as f32 / p.ship.stats().max_speed as f32;
        for (ex, ey) in super::shipart::engine_points(p.team, p.ship, p.faction) {
            let q = pose.to_cam([ex, ey + 0.05, 0.0]);
            if q[2] > NEAR {
                let (gx, gy) = cam.screen3(q);
                let r = (s * (0.25 + 0.3 * k) / q[2] * cam.focal).max(2.0);
                c.glow(gx, gy, r, rgb(0xff9a40), (0.35 + 0.5 * k) * alpha);
            }
        }
    }

    // The shield: a ring round the ship, its far half behind the hull.
    let shielded = p.flags & pf::SHIELD != 0 && !cloaked;
    let shield = |c: &mut Canvas, near: bool| {
        let pts: Vec<(f32, f32)> = (0..=24)
            .map(|k| {
                let a = k as f32 / 24.0 * PI + if near { PI } else { 0.0 };
                cam.screen3([lat0 + a.cos() * s * 1.35, 0.0, (z0 + a.sin() * s * 1.35).max(NEAR)])
            })
            .collect();
        c.polyline(&pts, false, 1.0, col, 0.45 * alpha, 0.0);
    };
    if shielded {
        shield(c, false);
    }
    let parts = ship_parts(p.team, p.ship, p.faction);
    let all: Vec<(f32, f32)> = parts
        .iter()
        .flat_map(|q| match q {
            Part::Poly { pts, .. } => pts.clone(),
            Part::Circle { c, r, .. } => vec![(c.0 - r, c.1), (c.0 + r, c.1)],
            _ => Vec::new(),
        })
        .collect();
    let hull = mix(col, rgb(0xb4bcc8), 0.45);
    let top = draw_model(c, &pose, &model, col, hull, alpha, cloaked);

    if model.lights && !cloaked && px > 10.0 && (tick / 6 + p.id as u32) % 3 != 0 && !all.is_empty() {
        // Running lights at the wingtips, blinking.
        let left = all.iter().copied().fold((f32::MAX, 0.0), |m, q| if q.0 < m.0 { q } else { m });
        let right = all.iter().copied().fold((f32::MIN, 0.0), |m, q| if q.0 > m.0 { q } else { m });
        for (q, lc) in [(left, rgb(0xff3030)), (right, rgb(0x30ff60))] {
            let v = pose.to_cam([q.0, q.1, 0.12]);
            if v[2] > NEAR {
                let (lx, ly) = cam.screen3(v);
                c.glow(lx, ly, (px * 0.08).clamp(2.0, 6.0), lc, 0.9 * alpha);
            }
        }
    }
    if shielded {
        shield(c, true);
    }
    top.unwrap_or_else(|| cam.screen3([lat0, 0.0, z0]).1 - px)
}

/// Reticle, heading and speed, and corner brackets.
fn draw_hud(c: &mut Canvas, tr: &TextRenderer, cam: &Cam, me: Option<&PlayerInfo>, f: &Frame, w: f32, h: f32, fs: f32) {
    let hud = rgb(0x60d0ff);
    let (x, y) = (cam.cx, cam.horizon);
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        c.line(x + dx * 5.0, y + dy * 5.0, x + dx * 12.0, y + dy * 12.0, 1.0, hud, 0.55, 0.0);
    }
    let m = 6.0;
    let l = (w.min(h) * 0.08).max(8.0);
    for (cx, cy, sx, sy) in [(m, m, 1.0, 1.0), (w - m, m, -1.0, 1.0), (m, h - m, 1.0, -1.0), (w - m, h - m, -1.0, -1.0)] {
        c.polyline(&[(cx, cy + sy * l), (cx, cy), (cx + sx * l, cy)], false, 1.2, hud, 0.6, 0.0);
    }
    let deg = (cam.heading.to_degrees().round() as i32).rem_euclid(360);
    c.text(tr, m + 6.0, m + fs * 1.3, if me.is_some() { "VIEWSCREEN" } else { "WATCH CAMERA" }, fs, scale(hud, 0.85));
    c.text_right(tr, w - m - 6.0, m + fs * 1.3, &format!("HDG {:03}", deg), fs, scale(hud, 0.85));
    if me.is_some() {
        c.text_right(tr, w - m - 6.0, h - m - fs * 0.5, &format!("WARP {}", f.me_info.speed), fs, scale(hud, 0.85));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ship(id: u8, team: Team, ship: ShipType, x: i32, y: i32, dir: u8) -> PlayerInfo {
        PlayerInfo {
            id,
            name: format!("p{}", id),
            team,
            ship,
            state: PState::Alive,
            x,
            y,
            dir,
            speed: 4,
            flags: pf::SHIELD,
            kills: 0.0,
            armies: 0,
            tractor_target: None,
            fuzzy: false,
            explode_frame: 0,
            faction: None,
            rank: None,
        }
    }

    /// Renders the viewscreen looking at Earth with a few ships about into
    /// $TMPDIR/netrek-viewscreen.ppm for eyeballing.
    #[test]
    fn viewscreen_gallery() {
        let tr = TextRenderer::load();
        let mut f = crate::server::world::World::new().frame_for(0);
        let earth = PLANETS.iter().find(|p| p.name == "Earth").unwrap();
        let (ex, ey) = (earth.x as i32, earth.y as i32);
        for p in f.planets.iter_mut() {
            p.known = true;
        }
        // Us, south of Earth, heading north at it.
        f.players = vec![
            ship(0, Team::Fed, ShipType::Cruiser, ex, ey + 9000, 0),
            ship(1, Team::Rom, ShipType::Cruiser, ex - 1500, ey + 4000, 64),
            ship(2, Team::Kli, ShipType::Battleship, ex + 2500, ey + 6000, 200),
            ship(3, Team::Fed, ShipType::Starbase, ex + 300, ey + 1500, 0),
            ship(4, Team::Ori, ShipType::Destroyer, ex - 6000, ey - 5000, 128),
        ];
        f.torps.clear();
        f.torps.push(TorpInfo { owner: 0, team: Team::Fed, kind: TorpKind::Photon, x: ex - 400, y: ey + 6000, explode: 0, quantum: false });
        f.phasers.clear();
        f.phasers.push(PhaserInfo { owner: 0, x1: ex, y1: ey + 9000, x2: ex - 1500, y2: ey + 4000, hit: true });
        let (pw, ph) = (520, 260);
        let mut c = Canvas::new(pw, ph * 2 + 10, [30.0, 30.0, 30.0]);
        c.draw_canvas(&viewscreen(&tr, Some(&f), 0, None, 12.0, pw, ph), 0, 0);
        c.draw_canvas(&viewscreen(&tr, Some(&f), 99, Some((ex as f32, ey as f32 + 9000.0)), 12.0, pw, ph), 0, ph + 10);
        // Close-ups: a row of ships and aliens at various headings.
        let line = [
            (Team::Fed, ShipType::Cruiser, None, 40u8),
            (Team::Kli, ShipType::Battleship, None, 100),
            (Team::Rom, ShipType::Warbird, None, 170),
            (Team::Ori, ShipType::Starbase, None, 0),
            (Team::Ind, ShipType::BorgCube, Some(Faction::Borg), 20),
            (Team::Ind, ShipType::DelphicSphere, Some(Faction::SphereBuilders), 0),
            (Team::Ind, ShipType::WhaleProbe, Some(Faction::Probe), 70),
        ];
        let mut g = viewscreen(&tr, None, 0, None, 12.0, 0, 0);
        for (row, z) in [4000, 9000].into_iter().enumerate() {
            f.players = vec![ship(0, Team::Fed, ShipType::Cruiser, ex, ey + 30000, 0)];
            f.torps.clear();
            f.phasers.clear();
            for (k, &(team, kind, faction, dir)) in line.iter().enumerate() {
                let mut p = ship(k as u8 + 1, team, kind, ex + (k as i32 * 2 - 5) * z / 8, ey + 30000 - z, dir);
                p.faction = faction;
                f.players.push(p);
            }
            g = viewscreen(&tr, Some(&f), 0, None, 12.0, pw * 2, ph);
            let mut big = Canvas::new(c.w.max(pw * 2), c.h + (ph + 10) * (row as i32 + 1), [30.0, 30.0, 30.0]);
            big.draw_canvas(&c, 0, 0);
            big.draw_canvas(&g, 0, c.h + 10);
            c = big;
        }
        let _ = g;
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-viewscreen.ppm"), out).unwrap();
    }

    /// Renders the whale probe at a few headings into
    /// $TMPDIR/netrek-view-probe.ppm.
    #[test]
    fn probe_gallery() {
        let tr = TextRenderer::load();
        let mut f = crate::server::world::World::new().frame_for(0);
        f.planets.clear();
        f.terrain.clear();
        f.torps.clear();
        f.phasers.clear();
        let (pw, ph) = (300, 200);
        let mut c = Canvas::new((pw + 6) * 4, ph, [30.0, 30.0, 30.0]);
        for (k, dir) in [64u8, 0, 128, 200].into_iter().enumerate() {
            let mut probe = ship(1, Team::Ind, ShipType::WhaleProbe, 50_000, 50_000, dir);
            probe.faction = Some(Faction::Probe);
            probe.flags = 0;
            f.players = vec![ship(0, Team::Fed, ShipType::Cruiser, 50_000, 56_000, 0), probe];
            c.draw_canvas(&viewscreen(&tr, Some(&f), 0, None, 12.0, pw, ph), k as i32 * (pw + 6), 0);
        }
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-view-probe.ppm"), out).unwrap();
    }

    /// Every alien ship on the viewscreen, close up, in
    /// $TMPDIR/netrek-view-aliens.ppm.
    #[test]
    fn alien_gallery() {
        use Faction as F;
        use ShipType as S;
        let tr = TextRenderer::load();
        let mut f = crate::server::world::World::new().frame_for(0);
        f.planets.clear();
        f.terrain.clear();
        f.torps.clear();
        f.phasers.clear();
        f.tick = 40;
        let aliens = [
            (S::Augment, F::Khan), (S::GornRaider, F::Gorn), (S::TholianVessel, F::Tholian), (S::Cruiser, F::Mirror),
            (S::Fesarius, F::Fesarius), (S::PlanetKiller, F::Doomsday), (S::Amoeba, F::Amoeba), (S::BorgCube, F::Borg),
            (S::VgerCloud, F::Vger), (S::CrystalEntity, F::Crystal), (S::WhaleProbe, F::Probe), (S::Bioship, F::Species8472),
            (S::JemHadarFighter, F::JemHadar), (S::BirdOfPrey, F::Chang), (S::HirogenHunter, F::Hirogen), (S::QEntity, F::Q),
            (S::QChampion, F::Q), (S::FerengiMarauder, F::Ferengi), (S::SwarmShip, F::Swarm), (S::NomadProbe, F::Nomad),
            (S::ArmusSlick, F::Armus), (S::ChangelingShip, F::Changeling), (S::MetronPresence, F::Metrons), (S::PakledClunker, F::Pakleds),
            (S::DarkMatterAnomaly, F::TenC), (S::CaretakerArray, F::Caretaker), (S::Horta, F::Horta), (S::DelphicSphere, F::SphereBuilders),
            (S::DysonHatch, F::Dyson), (S::KzintiDreadnought, F::Kzinti), (S::KzintiCruiser, F::Kzinti), (S::KzintiStriker, F::Kzinti),
            (S::DeathStar, F::GalacticEmpire), (S::StarDestroyer, F::GalacticEmpire), (S::TieFighter, F::GalacticEmpire), (S::TieAdvanced, F::GalacticEmpire), (S::CygnusShip, F::Cygnus), (S::RogueMoon, F::Moonbase), (S::Eagle, F::Moonbase), (S::Mongo, F::Ming), (S::WarRocketAjax, F::Ming), (S::MingRocket, F::Ming),
            (S::Narada, F::Narada), (S::NaradaDrill, F::Narada), (S::KrenimTimeship, F::Krenim), (S::MalonFreighter, F::Malon), (S::Vaal, F::Vaal), (S::Landru, F::Landru),
            (S::BluegillMother, F::Bluegill), (S::DreadnoughtMissile, F::Dreadnought), (S::Scimitar, F::Scimitar), (S::Calamarain, F::Calamarain), (S::Nagilum, F::Nagilum), (S::KobayashiMaru, F::Maru), (S::Ktinga, F::Maru), (S::EventHorizon, F::EventHorizon),
            (S::TempestCore, F::Tempest), (S::Flipper, F::Tempest), (S::Tanker, F::Tempest), (S::Pulsar, F::Tempest), (S::Fuseball, F::Tempest),
        ];
        let (pw, ph, cols) = (240, 160, 7);
        let rows = (aliens.len() as i32 + cols - 1) / cols;
        let mut c = Canvas::new((pw + 4) * cols, (ph + 4) * rows, [30.0, 30.0, 30.0]);
        let focal = (pw as f32 / 2.0) / (FOV / 2.0).tan();
        for (k, &(kind, faction)) in aliens.iter().enumerate() {
            let size = if ship_size_units(kind) > 0.0 { ship_size_units(kind) as f32 } else { 450.0 };
            let dist = (size * focal / (pw as f32 * 0.2)) as i32;
            let mut a = ship(1, Team::Ind, kind, 50_000, 50_000 - dist, 40);
            a.faction = Some(faction);
            a.flags = 0;
            f.players = vec![ship(0, Team::Fed, ShipType::Cruiser, 50_000, 50_000, 0), a];
            let img = viewscreen(&tr, Some(&f), 0, None, 11.0, pw, ph);
            c.draw_canvas(&img, (k as i32 % cols) * (pw + 4), (k as i32 / cols) * (ph + 4));
        }
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-view-aliens.ppm"), out).unwrap();
    }
}
