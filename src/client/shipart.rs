//! Per-empire ship designs, loosely after the classic Star Trek silhouettes.
//! Coordinates are in ship radii with the nose pointing up (-y).

use crate::consts::{ShipType, Team};
use std::f32::consts::TAU;

pub enum Part {
    /// Closed outline, optionally filled with the hull color.
    Poly { pts: Vec<(f32, f32)>, fill: bool },
    /// A single stroke.
    Line { a: (f32, f32), b: (f32, f32) },
    Circle { c: (f32, f32), r: f32, fill: bool },
    /// An opening in the hull (drawn in the background color, then outlined).
    Hole { pts: Vec<(f32, f32)> },
}

use Part::*;

fn mirror(half: &[(f32, f32)]) -> Vec<(f32, f32)> {
    // `half` runs down the right side from the nose; mirror it for the left.
    let mut pts = half.to_vec();
    for &(x, y) in half.iter().rev() {
        if x != 0.0 {
            pts.push((-x, y));
        }
    }
    pts
}

fn sym(half: &[(f32, f32)]) -> Part {
    Poly { pts: mirror(half), fill: true }
}

/// A rounded rod from `a` to `b` (nacelles, pods, necks).
fn capsule(a: (f32, f32), b: (f32, f32), w: f32) -> Part {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (nx, ny) = (-dy / len * w / 2.0, dx / len * w / 2.0);
    let ang = ny.atan2(nx);
    let mut pts = Vec::new();
    for k in 0..=6 {
        let t = ang + TAU / 2.0 * k as f32 / 6.0;
        pts.push((a.0 + t.cos() * w / 2.0, a.1 + t.sin() * w / 2.0));
    }
    for k in 0..=6 {
        let t = ang + TAU / 2.0 + TAU / 2.0 * k as f32 / 6.0;
        pts.push((b.0 + t.cos() * w / 2.0, b.1 + t.sin() * w / 2.0));
    }
    Poly { pts, fill: true }
}

fn pair(f: impl Fn(f32) -> Part) -> [Part; 2] {
    [f(1.0), f(-1.0)]
}

fn circle(c: (f32, f32), r: f32) -> Part {
    Circle { c, r, fill: true }
}

fn line(a: (f32, f32), b: (f32, f32)) -> Part {
    Line { a, b }
}

pub fn ship_parts(team: Team, ship: ShipType) -> Vec<Part> {
    match team {
        Team::Fed | Team::Ind => federation(ship),
        Team::Kli => klingon(ship),
        Team::Rom => romulan(ship),
        Team::Ori => orion(ship),
    }
}

/// Saucer, engineering hull, twin nacelles on pylons.
fn federation(ship: ShipType) -> Vec<Part> {
    let mut v = Vec::new();
    match ship {
        ShipType::Scout => {
            // Small escort: saucer with nacelles slung right beneath it.
            v.push(circle((0.0, -0.35), 0.5));
            v.extend(pair(|s| capsule((s * 0.45, 0.0), (s * 0.45, 0.9), 0.18)));
            v.push(line((0.0, 0.1), (0.0, 0.5)));
        }
        ShipType::Destroyer => {
            // Miranda style: saucer with a roll bar, nacelles below.
            v.push(circle((0.0, -0.3), 0.55));
            v.extend(pair(|s| capsule((s * 0.6, 0.05), (s * 0.6, 0.95), 0.16)));
            v.push(line((-0.6, 0.35), (0.6, 0.35)));
        }
        ShipType::Cruiser => {
            // Constitution class.
            v.push(circle((0.0, -0.55), 0.42));
            v.push(capsule((0.0, -0.1), (0.0, 0.45), 0.24));
            v.extend(pair(|s| line((0.0, 0.3), (s * 0.5, 0.45))));
            v.extend(pair(|s| capsule((s * 0.5, 0.05), (s * 0.5, 1.0), 0.16)));
        }
        ShipType::Battleship => {
            // Galaxy class: wide oval saucer, big engineering hull, long nacelles.
            v.push(Poly {
                pts: (0..24)
                    .map(|k| {
                        let a = k as f32 / 24.0 * TAU;
                        (a.cos() * 0.62, -0.5 + a.sin() * 0.42)
                    })
                    .collect(),
                fill: true,
            });
            v.push(capsule((0.0, -0.1), (0.0, 0.6), 0.34));
            v.extend(pair(|s| line((s * 0.15, 0.35), (s * 0.62, 0.5))));
            v.extend(pair(|s| capsule((s * 0.62, 0.1), (s * 0.62, 1.0), 0.17)));
        }
        ShipType::Assault => {
            // Saucer on a bulky cargo hull.
            v.push(circle((0.0, -0.6), 0.35));
            v.push(sym(&[(0.0, -0.3), (0.35, -0.25), (0.4, 0.7), (0.0, 0.8)]));
            v.extend(pair(|s| capsule((s * 0.7, 0.0), (s * 0.7, 0.9), 0.16)));
            v.extend(pair(|s| line((s * 0.4, 0.3), (s * 0.7, 0.3))));
        }
        ShipType::Starbase => {
            // Spacedock: a big mushroom cap with docking ring.
            v.push(circle((0.0, 0.0), 0.95));
            v.push(Circle { c: (0.0, 0.0), r: 0.6, fill: false });
            v.push(circle((0.0, 0.0), 0.25));
            for k in 0..4 {
                let a = k as f32 * TAU / 4.0 + TAU / 8.0;
                v.push(line((a.cos() * 0.25, a.sin() * 0.25), (a.cos() * 0.6, a.sin() * 0.6)));
            }
        }
    }
    v
}

/// Command bulb on a long neck, swept wings with nacelles at the tips.
fn klingon(ship: ShipType) -> Vec<Part> {
    let mut v = Vec::new();
    match ship {
        ShipType::Scout | ShipType::Destroyer => {
            // Bird-of-Prey: wings angled down, disruptors at the tips.
            let span = if ship == ShipType::Scout { 0.85 } else { 1.0 };
            v.push(circle((0.0, -0.72), 0.17));
            v.push(capsule((0.0, -0.6), (0.0, -0.25), 0.14));
            v.push(sym(&[(0.0, -0.3), (0.28, -0.1), (span, 0.25), (span * 0.95, 0.45), (0.22, 0.35), (0.0, 0.65)]));
            v.extend(pair(|s| line((s * span, 0.25), (s * span, -0.25))));
        }
        ShipType::Cruiser | ShipType::Battleship | ShipType::Assault => {
            // D7 / K't'inga battlecruiser.
            let w = if ship == ShipType::Battleship { 1.0 } else { 0.9 };
            v.push(circle((0.0, -0.8), if ship == ShipType::Battleship { 0.2 } else { 0.17 }));
            v.push(capsule((0.0, -0.65), (0.0, 0.05), 0.1));
            v.push(sym(&[(0.0, -0.05), (0.35, 0.05), (w, 0.35), (w, 0.55), (0.45, 0.5), (0.0, 0.75)]));
            v.extend(pair(|s| capsule((s * w, 0.2), (s * w, 0.95), 0.16)));
            if ship == ShipType::Assault {
                v.push(capsule((0.0, 0.3), (0.0, 0.95), 0.3)); // troop pod
            }
        }
        ShipType::Starbase => {
            // Three-armed station.
            v.push(circle((0.0, 0.0), 0.35));
            for k in 0..3 {
                let a = k as f32 * TAU / 3.0 - TAU / 4.0;
                let tip = (a.cos() * 0.95, a.sin() * 0.95);
                v.push(capsule((a.cos() * 0.3, a.sin() * 0.3), tip, 0.2));
                v.push(circle(tip, 0.15));
            }
        }
    }
    v
}

/// Bird-of-Prey body with wide wings, or the double-hulled D'deridex warbird.
fn romulan(ship: ShipType) -> Vec<Part> {
    let mut v = Vec::new();
    match ship {
        ShipType::Scout | ShipType::Destroyer | ShipType::Cruiser => {
            // TOS Romulan Bird-of-Prey: rounded hull, swept wings, tip nacelles.
            let span = match ship {
                ShipType::Scout => 0.8,
                ShipType::Destroyer => 0.9,
                _ => 1.0,
            };
            v.push(sym(&[(0.0, -0.9), (0.3, -0.75), (0.42, -0.35), (span, 0.45), (span * 0.9, 0.6), (0.3, 0.45), (0.0, 0.55)]));
            v.push(Circle { c: (0.0, -0.45), r: 0.22, fill: false });
            v.extend(pair(|s| capsule((s * span * 0.95, 0.1), (s * span * 0.95, 0.9), 0.14)));
        }
        ShipType::Battleship | ShipType::Assault => {
            // D'deridex warbird: head on a neck, open double-hulled wing.
            v.push(circle((0.0, -0.85), 0.15));
            v.push(capsule((0.0, -0.75), (0.0, -0.35), 0.1));
            v.push(sym(&[(0.0, -0.4), (0.75, 0.0), (0.95, 0.55), (0.55, 0.95), (0.0, 0.7)]));
            v.push(Hole { pts: mirror(&[(0.0, -0.15), (0.5, 0.15), (0.5, 0.55), (0.0, 0.42)]) });
            if ship == ShipType::Assault {
                v.push(capsule((0.0, 0.4), (0.0, 0.95), 0.22));
            }
        }
        ShipType::Starbase => {
            // Ring station with three wing fins.
            v.push(circle((0.0, 0.0), 0.75));
            v.push(Hole { pts: (0..16).map(|k| { let a = k as f32 / 16.0 * TAU; (a.cos() * 0.4, a.sin() * 0.4) }).collect() });
            for k in 0..3 {
                let a = k as f32 * TAU / 3.0 + TAU / 4.0;
                let (c, s) = (a.cos(), a.sin());
                v.push(Poly { pts: vec![(c * 0.7 - s * 0.15, s * 0.7 + c * 0.15), (c * 1.0, s * 1.0), (c * 0.7 + s * 0.15, s * 0.7 - c * 0.15)], fill: true });
            }
            v.push(circle((0.0, 0.0), 0.18));
        }
    }
    v
}

/// Orion raiders: a narrow dagger hull with forked side pods.
fn orion(ship: ShipType) -> Vec<Part> {
    let mut v = Vec::new();
    match ship {
        ShipType::Starbase => {
            // Pirate outpost: a hexagon with four docking prongs.
            v.push(Poly { pts: (0..6).map(|k| { let a = k as f32 / 6.0 * TAU; (a.cos() * 0.7, a.sin() * 0.7) }).collect(), fill: true });
            v.push(Hole { pts: (0..6).map(|k| { let a = k as f32 / 6.0 * TAU; (a.cos() * 0.3, a.sin() * 0.3) }).collect() });
            for k in 0..4 {
                let a = k as f32 * TAU / 4.0 + TAU / 8.0;
                v.push(line((a.cos() * 0.7, a.sin() * 0.7), (a.cos() * 1.0, a.sin() * 1.0)));
            }
        }
        _ => {
            let (pod_x, hull_w) = match ship {
                ShipType::Scout => (0.4, 0.14),
                ShipType::Destroyer => (0.5, 0.18),
                ShipType::Cruiser => (0.58, 0.22),
                ShipType::Battleship => (0.7, 0.3),
                _ => (0.62, 0.34),
            };
            v.push(sym(&[(0.0, -1.0), (hull_w, -0.3), (hull_w, 0.75), (0.0, 0.95)]));
            v.extend(pair(|s| capsule((s * pod_x, -0.2), (s * pod_x, 0.85), 0.17)));
            v.extend(pair(|s| line((s * hull_w, 0.25), (s * pod_x, 0.25))));
            v.extend(pair(|s| line((s * pod_x, -0.2), (s * (pod_x - 0.08), -0.62))));
            if ship == ShipType::Battleship {
                v.extend(pair(|s| line((s * hull_w, -0.1), (s * pod_x, 0.05))));
            }
        }
    }
    v
}

/// Where the engines are (for exhaust glows), in ship radii.
pub fn engine_points(team: Team, ship: ShipType) -> Vec<(f32, f32)> {
    match (team, ship) {
        (_, ShipType::Starbase) => vec![],
        (Team::Fed | Team::Ind, ShipType::Scout) => vec![(0.45, 0.95), (-0.45, 0.95)],
        (Team::Fed | Team::Ind, ShipType::Assault) => vec![(0.7, 0.95), (-0.7, 0.95)],
        (Team::Fed | Team::Ind, ShipType::Battleship) => vec![(0.62, 1.05), (-0.62, 1.05)],
        (Team::Fed | Team::Ind, _) => vec![(0.55, 1.0), (-0.55, 1.0)],
        (Team::Kli, ShipType::Scout | ShipType::Destroyer) => vec![(0.0, 0.7)],
        (Team::Kli, _) => vec![(0.9, 1.0), (-0.9, 1.0)],
        (Team::Rom, ShipType::Battleship | ShipType::Assault) => vec![(0.55, 0.95), (-0.55, 0.95)],
        (Team::Rom, _) => vec![(0.85, 0.95), (-0.85, 0.95)],
        (Team::Ori, _) => vec![(0.55, 0.9), (-0.55, 0.9), (0.0, 1.0)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::render_px::team_rgb;
    use crate::client::vg::Canvas;

    /// Renders every empire's ships into a PPM contact sheet
    /// ($TMPDIR/netrek-ships.ppm) for eyeballing the designs.
    #[test]
    fn gallery() {
        let (cell, r) = (120.0f32, 44.0f32);
        let mut c = Canvas::new(6 * cell as i32, 4 * cell as i32, [0.0, 0.0, 0.0]);
        for (row, team) in Team::PLAYABLE.iter().enumerate() {
            for (col, ship) in ShipType::ALL.iter().enumerate() {
                let (x, y) = (col as f32 * cell + cell / 2.0, row as f32 * cell + cell / 2.0);
                let team_c = team_rgb(*team);
                let fill = [team_c[0] * 0.42, team_c[1] * 0.42, team_c[2] * 0.42];
                let rot = |p: (f32, f32)| (x + p.0 * r, y + p.1 * r);
                for part in ship_parts(*team, *ship) {
                    match part {
                        Part::Poly { pts, fill: f } => {
                            let pts: Vec<_> = pts.into_iter().map(rot).collect();
                            if f {
                                c.fill_poly(&pts, fill, 1.0);
                            }
                            c.polyline(&pts, true, 1.5, team_c, 1.0, 0.0);
                        }
                        Part::Hole { pts } => {
                            let pts: Vec<_> = pts.into_iter().map(rot).collect();
                            c.fill_poly(&pts, [0.0; 3], 1.0);
                            c.polyline(&pts, true, 1.5, team_c, 1.0, 0.0);
                        }
                        Part::Circle { c: p, r: rr, fill: f } => {
                            let (cx, cy) = rot(p);
                            if f {
                                c.disc(cx, cy, rr * r, fill, 1.0);
                            }
                            c.ring(cx, cy, rr * r, 1.5, team_c, 1.0);
                        }
                        Part::Line { a, b } => {
                            let (a, b) = (rot(a), rot(b));
                            c.line(a.0, a.1, b.0, b.1, 1.5, team_c, 1.0, 0.0);
                        }
                    }
                }
            }
        }
        let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
        for y in 0..c.h {
            for x in 0..c.w {
                let p = c.get(x, y);
                out.extend([p[0] as u8, p[1] as u8, p[2] as u8]);
            }
        }
        std::fs::write(std::env::temp_dir().join("netrek-ships.ppm"), out).unwrap();
    }
}
