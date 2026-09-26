//! Per-empire ship designs, loosely after the classic Star Trek silhouettes.
//! Coordinates are in ship radii with the nose pointing up (-y).

use crate::consts::{Faction, ShipType, Team};
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

pub fn ship_parts(team: Team, ship: ShipType, faction: Option<Faction>) -> Vec<Part> {
    if let Some(f) = faction {
        return alien(f, ship);
    }
    if ship == ShipType::Freighter {
        return freighter();
    }
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
        _ => return federation(ShipType::Cruiser),
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
        _ => return klingon(ShipType::Cruiser),
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
        _ => return romulan(ShipType::Cruiser),
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

/// Supply freighter: a bridge on a long spine lined with cargo pods.
fn freighter() -> Vec<Part> {
    let mut v = vec![circle((0.0, -0.8), 0.2), capsule((0.0, -0.65), (0.0, 0.95), 0.16)];
    for k in 0..4 {
        let y = -0.45 + k as f32 * 0.35;
        v.extend(pair(|s| Poly { pts: vec![(s * 0.1, y), (s * 0.42, y), (s * 0.42, y + 0.27), (s * 0.1, y + 0.27)], fill: true }));
    }
    v
}

/// Alien vessels and monsters.
fn alien(f: Faction, ship: ShipType) -> Vec<Part> {
    let mut v = Vec::new();
    match f {
        // Reliant: a Miranda-class hull, as Khan stole it.
        Faction::Khan => return federation(ShipType::Destroyer),
        // The ISS fleet flies ships identical to Starfleet's.
        Faction::Mirror => return federation(ship),
        Faction::Gorn => {
            // Blunt, heavy hammerhead with swept engine pods.
            v.push(sym(&[(0.0, -0.95), (0.3, -0.85), (0.9, -0.75), (0.9, -0.45), (0.3, -0.35), (0.3, 0.45), (0.75, 0.95), (0.0, 0.75)]));
            v.push(Circle { c: (0.0, -0.62), r: 0.14, fill: false });
            v.push(line((0.0, -0.3), (0.0, 0.6)));
        }
        Faction::Tholian => {
            // A crystalline wedge.
            v.push(sym(&[(0.0, -1.0), (0.55, 0.8), (0.0, 0.45)]));
            v.push(line((0.0, -0.65), (0.0, 0.45)));
            v.extend(pair(|s| line((0.0, -0.2), (s * 0.3, 0.45))));
        }
        Faction::Fesarius => {
            // A vast globe made of many smaller modules.
            v.push(circle((0.0, 0.0), 1.0));
            for k in 0..12 {
                let a = k as f32 / 12.0 * TAU;
                v.push(Circle { c: (a.cos() * 0.72, a.sin() * 0.72), r: 0.16, fill: false });
            }
            for k in 0..6 {
                let a = k as f32 / 6.0 * TAU + 0.26;
                v.push(Circle { c: (a.cos() * 0.38, a.sin() * 0.38), r: 0.12, fill: false });
            }
            v.push(Circle { c: (0.0, 0.0), r: 0.14, fill: true });
        }
        Faction::Doomsday => {
            // The planet killer: a long neutronium cone, open maw forward.
            v.push(Poly { pts: vec![(-0.5, -1.0), (0.5, -1.0), (0.26, 1.0), (-0.26, 1.0)], fill: true });
            v.push(Hole { pts: (0..16).map(|k| { let a = k as f32 / 16.0 * TAU; (a.cos() * 0.4, -0.9 + a.sin() * 0.08) }).collect() });
            v.extend(pair(|s| line((s * 0.22, -0.8), (s * 0.12, 0.95))));
            v.push(line((-0.4, -0.25), (0.4, -0.25)));
            v.push(line((-0.33, 0.35), (0.33, 0.35)));
        }
        Faction::Amoeba => {
            // A wobbling single cell with a nucleus and vacuoles.
            v.push(Poly {
                pts: (0..28)
                    .map(|k| {
                        let a = k as f32 / 28.0 * TAU;
                        let r = 0.85 + 0.1 * (3.0 * a).sin() + 0.06 * (5.0 * a + 1.0).sin();
                        (a.cos() * r, a.sin() * r)
                    })
                    .collect(),
                fill: true,
            });
            v.push(Circle { c: (0.12, -0.1), r: 0.3, fill: false });
            v.push(Circle { c: (0.12, -0.1), r: 0.1, fill: false });
            v.push(Circle { c: (-0.4, 0.3), r: 0.1, fill: false });
            v.push(Circle { c: (0.35, 0.45), r: 0.07, fill: false });
        }
        Faction::Borg => {
            // The cube, with its maze of conduits.
            v.push(Poly { pts: vec![(-0.72, -0.72), (0.72, -0.72), (0.72, 0.72), (-0.72, 0.72)], fill: true });
            for k in 1..4 {
                let t = -0.72 + k as f32 * 0.36;
                v.push(line((t, -0.72), (t, 0.72)));
                v.push(line((-0.72, t), (0.72, t)));
            }
            v.push(Poly { pts: vec![(-0.25, -0.25), (0.25, -0.25), (0.25, 0.25), (-0.25, 0.25)], fill: false });
        }
        // V'Ger is drawn specially (a luminous cloud); this is its core.
        Faction::Vger => {
            v.push(circle((0.0, 0.0), 0.2));
            v.push(Circle { c: (0.0, 0.0), r: 0.5, fill: false });
        }
        Faction::Crystal => {
            // A snowflake of crystal spines.
            v.push(Poly {
                pts: (0..24)
                    .map(|k| {
                        let a = k as f32 / 24.0 * TAU;
                        let r = if k % 2 == 0 { if k % 4 == 0 { 1.0 } else { 0.7 } } else { 0.28 };
                        (a.cos() * r, a.sin() * r)
                    })
                    .collect(),
                fill: true,
            });
            for k in 0..6 {
                let a = k as f32 / 6.0 * TAU;
                v.push(line((0.0, 0.0), (a.cos() * 0.95, a.sin() * 0.95)));
            }
        }
        Faction::Probe => {
            // A long cylinder with a small sphere on one end.
            v.push(Poly { pts: vec![(-0.28, -0.75), (0.28, -0.75), (0.28, 1.0), (-0.28, 1.0)], fill: true });
            v.push(line((-0.28, -0.35), (0.28, -0.35)));
            v.push(line((-0.28, 0.35), (0.28, 0.35)));
            v.push(Circle { c: (0.0, -0.88), r: 0.14, fill: true });
        }
        Faction::Species8472 => {
            // Organic tripod: a central spine and two swept claws.
            v.push(sym(&[(0.0, -1.0), (0.18, -0.45), (0.85, 0.05), (0.95, 0.55), (0.55, 0.25), (0.3, 0.95), (0.0, 0.55)]));
            v.push(line((0.0, -0.6), (0.0, 0.4)));
        }
        Faction::JemHadar => {
            // Beetle-shaped attack ship with forward prongs.
            v.push(sym(&[(0.0, -0.7), (0.3, -0.65), (0.4, -0.1), (0.75, 0.35), (0.55, 0.9), (0.0, 0.7)]));
            v.extend(pair(|s| line((s * 0.3, -0.65), (s * 0.18, -1.0))));
            v.push(Circle { c: (0.0, -0.2), r: 0.14, fill: false });
        }
        Faction::Tribbles => {
            // Never flown, but just in case: a fuzzy ball.
            v.push(Poly {
                pts: (0..24).map(|k| { let a = k as f32 / 24.0 * TAU; let r = if k % 2 == 0 { 0.8 } else { 0.65 }; (a.cos() * r, a.sin() * r) }).collect(),
                fill: true,
            });
        }
        Faction::Chang => {
            // Klingon Bird-of-Prey: a head on a long neck, wings swept down.
            v.push(circle((0.0, -0.78), 0.18));
            v.push(capsule((0.0, -0.62), (0.0, 0.15), 0.14));
            v.push(sym(&[(0.0, -0.05), (0.35, 0.05), (1.0, 0.5), (0.95, 0.72), (0.3, 0.52), (0.0, 0.7)]));
            v.extend(pair(|s| line((s * 0.95, 0.5), (s * 0.95, 0.15))));
        }
        Faction::Hirogen => {
            // Long predatory arrowhead with a spine and rear fins.
            v.push(sym(&[(0.0, -1.0), (0.2, -0.45), (0.45, 0.55), (0.3, 0.95), (0.0, 0.75)]));
            v.push(line((0.0, -0.7), (0.0, 0.6)));
            v.extend(pair(|s| line((s * 0.3, 0.3), (s * 0.75, 0.95))));
        }
        Faction::Q if ship == ShipType::QChampion => {
            // Q's champion: a dark, angular warship with a glowing core.
            v.push(sym(&[(0.0, -1.0), (0.35, -0.25), (0.9, 0.15), (0.6, 0.9), (0.0, 0.6)]));
            v.push(Circle { c: (0.0, 0.05), r: 0.2, fill: false });
            v.extend(pair(|s| line((s * 0.35, -0.25), (s * 0.55, 0.55))));
        }
        Faction::Q => {
            // A flash of light.
            v.push(Poly {
                pts: (0..16).map(|k| { let a = k as f32 / 16.0 * TAU; let r = if k % 2 == 0 { 1.0 } else { 0.3 }; (a.cos() * r, a.sin() * r) }).collect(),
                fill: true,
            });
            v.push(Circle { c: (0.0, 0.0), r: 0.45, fill: false });
        }
        Faction::Ferengi => {
            // D'Kora marauder: a horseshoe with its prongs forward.
            let mut pts: Vec<(f32, f32)> = (0..=12).map(|k| { let a = (-25.0 + k as f32 * 230.0 / 12.0).to_radians(); (a.cos() * 0.95, a.sin() * 0.95) }).collect();
            pts.extend((0..=12).rev().map(|k| { let a = (-25.0 + k as f32 * 230.0 / 12.0).to_radians(); (a.cos() * 0.5, a.sin() * 0.5) }));
            v.push(Poly { pts, fill: true });
            v.push(circle((0.0, 0.05), 0.22));
        }
        Faction::Swarm => {
            // A tiny dart.
            v.push(sym(&[(0.0, -1.0), (0.6, 0.8), (0.0, 0.4)]));
        }
    }
    v
}

/// Where the engines are (for exhaust glows), in ship radii.
pub fn engine_points(team: Team, ship: ShipType, faction: Option<Faction>) -> Vec<(f32, f32)> {
    match faction {
        Some(Faction::Khan) => return engine_points(Team::Fed, ShipType::Destroyer, None),
        Some(Faction::Mirror) => return engine_points(Team::Fed, ship, None),
        Some(Faction::Gorn) => return vec![(0.6, 0.9), (-0.6, 0.9)],
        Some(Faction::Tholian) => return vec![(0.0, 0.55)],
        Some(Faction::JemHadar) => return vec![(0.45, 0.85), (-0.45, 0.85)],
        Some(Faction::Chang) => return vec![(0.0, 0.75)],
        Some(Faction::Hirogen) => return vec![(0.3, 0.95), (-0.3, 0.95)],
        Some(Faction::Ferengi) => return vec![(0.0, 0.3)],
        Some(_) => return vec![],
        None => {}
    }
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
    use crate::client::palette::team_rgb;
    use crate::client::vg::Canvas;

    /// Renders every empire's ships into a PPM contact sheet
    /// ($TMPDIR/netrek-ships.ppm) for eyeballing the designs.
    #[test]
    fn gallery() {
        let (cell, r) = (120.0f32, 44.0f32);
        let mut c = Canvas::new(8 * cell as i32, 8 * cell as i32, [0.0, 0.0, 0.0]);
        let mut rows: Vec<Vec<(Team, ShipType, Option<Faction>)>> = Team::PLAYABLE
            .iter()
            .map(|&t| ShipType::ALL.iter().map(|&s| (t, s, None)).collect())
            .collect();
        rows.push(vec![
            (Team::Ind, ShipType::Augment, Some(Faction::Khan)),
            (Team::Ind, ShipType::GornRaider, Some(Faction::Gorn)),
            (Team::Ind, ShipType::TholianVessel, Some(Faction::Tholian)),
            (Team::Ind, ShipType::Cruiser, Some(Faction::Mirror)),
            (Team::Ind, ShipType::Fesarius, Some(Faction::Fesarius)),
            (Team::Ind, ShipType::PlanetKiller, Some(Faction::Doomsday)),
            (Team::Ind, ShipType::Amoeba, Some(Faction::Amoeba)),
            (Team::Ind, ShipType::BorgCube, Some(Faction::Borg)),
        ]);
        rows.push(vec![
            (Team::Ind, ShipType::VgerCloud, Some(Faction::Vger)),
            (Team::Ind, ShipType::CrystalEntity, Some(Faction::Crystal)),
            (Team::Ind, ShipType::WhaleProbe, Some(Faction::Probe)),
            (Team::Ind, ShipType::Bioship, Some(Faction::Species8472)),
            (Team::Ind, ShipType::JemHadarFighter, Some(Faction::JemHadar)),
            (Team::Ind, ShipType::BirdOfPrey, Some(Faction::Chang)),
            (Team::Ind, ShipType::HirogenHunter, Some(Faction::Hirogen)),
            (Team::Ind, ShipType::QEntity, Some(Faction::Q)),
        ]);
        rows.push(vec![
            (Team::Ind, ShipType::QChampion, Some(Faction::Q)),
            (Team::Ind, ShipType::FerengiMarauder, Some(Faction::Ferengi)),
            (Team::Ind, ShipType::SwarmShip, Some(Faction::Swarm)),
        ]);
        for (row, ships) in rows.iter().enumerate() {
            for (col, &(team, ship, faction)) in ships.iter().enumerate() {
                let (x, y) = (col as f32 * cell + cell / 2.0, row as f32 * cell + cell / 2.0);
                let team_c = match faction {
                    Some(f) => crate::client::palette::faction_rgb(f),
                    None => team_rgb(team),
                };
                let fill = [team_c[0] * 0.42, team_c[1] * 0.42, team_c[2] * 0.42];
                let rot = |p: (f32, f32)| (x + p.0 * r, y + p.1 * r);
                for part in ship_parts(team, ship, faction) {
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
