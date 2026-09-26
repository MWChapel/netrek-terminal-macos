//! Space terrain (the --terrain option): nebulae, ion storms, asteroid
//! fields, a black hole, a pulsar, a wormhole, derelicts to salvage,
//! subspace slipstreams, a star, a comet and a tachyon detection grid.
//! Generated fresh with each galaxy, clear of the planets.

use super::world::{GameEvent, World};
use crate::consts::*;
use crate::proto::{TerrainInfo, TerrainKind};
use rand::seq::SliceRandom;
use rand::Rng;

pub struct Terrain {
    pub kind: TerrainKind,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub r: f64,
    /// The other wormhole mouth, the slipstream's far end, the comet's tail.
    pub x2: f64,
    pub y2: f64,
    /// Drift per tick (ion storm, comet).
    pub vx: f64,
    pub vy: f64,
    /// Pulsar: ticks into the current cycle.
    pub timer: i32,
    /// Derelict: ticks until another wreck turns up (0 = here now).
    pub respawn: i32,
}

pub const NEBULA_SPEED: i32 = 6;
pub const HORIZON: f64 = 600.0;
pub const PULSAR_PERIOD: i32 = 100;
pub const MOUTH: f64 = 600.0;
pub const SALVAGE_REACH: f64 = 900.0;
pub const SALVAGE_TICKS: i32 = 50;
pub const CORRIDOR_WIDTH: f64 = 1400.0;
pub const STAR_CORE: f64 = 1200.0;
pub const COMET_HEAD: f64 = 500.0;
pub const COMET_TAIL: f64 = 9000.0;
/// Close enough to see a ship hidden by a nebula or ion storm.
pub const SENSOR_RANGE: f64 = 3000.0;

impl Terrain {
    fn new(kind: TerrainKind, name: &str, (x, y): (f64, f64), r: f64) -> Terrain {
        Terrain { kind, name: name.into(), x, y, r, x2: x, y2: y, vx: 0.0, vy: 0.0, timer: 0, respawn: 0 }
    }

    pub fn visible(&self) -> bool {
        self.respawn == 0
    }

    pub fn info(&self) -> TerrainInfo {
        let phase = match self.kind {
            TerrainKind::Pulsar => (PULSAR_PERIOD - self.timer).clamp(0, 255) as u8,
            _ => 0,
        };
        TerrainInfo {
            kind: self.kind,
            x: self.x as i32,
            y: self.y as i32,
            r: self.r as i32,
            x2: self.x2 as i32,
            y2: self.y2 as i32,
            phase,
            name: self.name.clone(),
        }
    }
}

fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

/// Distance from (px, py) to the segment (ax, ay)-(bx, by).
pub fn seg_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = (dx * dx + dy * dy).max(1.0);
    let t = (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0);
    dist(px, py, ax + t * dx, ay + t * dy)
}

/// A random spot for something of radius `r`, at least `clear` from every
/// planet's edge and clear of the terrain already placed.
fn place(world: &World, r: f64, clear: f64) -> Option<(f64, f64)> {
    let mut rng = rand::thread_rng();
    let margin = r + 2000.0;
    for _ in 0..400 {
        let (x, y) = (rng.gen_range(margin..GWIDTH - margin), rng.gen_range(margin..GWIDTH - margin));
        let planets_ok = world.planets.iter().all(|pl| dist(x, y, pl.x, pl.y) > r + clear);
        let terrain_ok = world.terrain.iter().all(|t| match t.kind {
            TerrainKind::Corridor | TerrainKind::Comet | TerrainKind::IonStorm => true,
            TerrainKind::Wormhole => dist(x, y, t.x, t.y) > r + t.r + 2000.0 && dist(x, y, t.x2, t.y2) > r + t.r + 2000.0,
            _ => dist(x, y, t.x, t.y) > r + t.r + 1500.0,
        });
        if planets_ok && terrain_ok {
            return Some((x, y));
        }
    }
    None
}

/// A comet enters at a random edge and heads across the galaxy.
fn launch_comet(t: &mut Terrain) {
    let mut rng = rand::thread_rng();
    let (x, y) = match rng.gen_range(0..4) {
        0 => (rng.gen_range(10_000.0..90_000.0), -3000.0),
        1 => (rng.gen_range(10_000.0..90_000.0), GWIDTH + 3000.0),
        2 => (-3000.0, rng.gen_range(10_000.0..90_000.0)),
        _ => (GWIDTH + 3000.0, rng.gen_range(10_000.0..90_000.0)),
    };
    let (tx, ty) = (rng.gen_range(30_000.0..70_000.0), rng.gen_range(30_000.0..70_000.0));
    let d = dist(x, y, tx, ty);
    let speed = 2.0 * WARP1;
    (t.x, t.y, t.vx, t.vy) = (x, y, (tx - x) / d * speed, (ty - y) / d * speed);
    update_tail(t);
}

fn update_tail(t: &mut Terrain) {
    let v = (t.vx * t.vx + t.vy * t.vy).sqrt().max(1e-6);
    t.x2 = t.x - t.vx / v * COMET_TAIL;
    t.y2 = t.y - t.vy / v * COMET_TAIL;
}

pub fn generate(world: &mut World) {
    let mut rng = rand::thread_rng();
    world.terrain.clear();
    let add = |world: &mut World, t: Option<Terrain>| {
        if let Some(t) = t {
            world.terrain.push(t);
        }
    };
    let mut nebulae = vec!["Mutara Nebula", "Briar Patch", "Paulson Nebula", "Mar Oscura"];
    nebulae.shuffle(&mut rng);
    for name in nebulae.into_iter().take(3) {
        let r = rng.gen_range(5000.0..7000.0);
        let t = place(world, r, 1500.0).map(|p| Terrain::new(TerrainKind::Nebula, name, p, r));
        add(world, t);
    }
    for name in ["Maluria asteroid belt", "Hanoran asteroid field"] {
        let r = rng.gen_range(4000.0..5000.0);
        let t = place(world, r, 1500.0).map(|p| Terrain::new(TerrainKind::Asteroids, name, p, r));
        add(world, t);
    }
    let t = place(world, 7000.0, 3000.0).map(|p| Terrain::new(TerrainKind::BlackHole, "Tarsus singularity", p, 7000.0));
    add(world, t);
    let t = place(world, 6500.0, 1500.0).map(|p| Terrain::new(TerrainKind::Pulsar, "Zeta Lantis pulsar", p, 6500.0));
    add(world, t);
    let t = place(world, 5000.0, 2000.0).map(|p| Terrain::new(TerrainKind::Star, "Amargosa", p, 5000.0));
    add(world, t);
    let t = place(world, 6000.0, -3000.0).map(|p| Terrain::new(TerrainKind::TachyonGrid, "tachyon detection grid", p, 6000.0));
    add(world, t);
    // Wormhole: two mouths far apart.
    if let Some(a) = place(world, MOUTH, 3000.0) {
        for _ in 0..50 {
            if let Some(b) = place(world, MOUTH, 3000.0) {
                if dist(a.0, a.1, b.0, b.1) > 40_000.0 {
                    let mut t = Terrain::new(TerrainKind::Wormhole, "Barzan wormhole", a, MOUTH);
                    (t.x2, t.y2) = b;
                    world.terrain.push(t);
                    break;
                }
            }
        }
    }
    let mut wrecks = vec!["derelict freighter", "derelict USS Valiant", "derelict Klingon cruiser", "derelict Romulan scout"];
    wrecks.shuffle(&mut rng);
    for name in wrecks.into_iter().take(3) {
        let t = place(world, SALVAGE_REACH, 3000.0).map(|p| Terrain::new(TerrainKind::Derelict, name, p, SALVAGE_REACH));
        add(world, t);
    }
    // Slipstreams: long straight corridors.
    for name in ["Delta slipstream", "Theta slipstream"] {
        for _ in 0..100 {
            let (x, y) = (rng.gen_range(8000.0..92_000.0), rng.gen_range(8000.0..92_000.0));
            let a = rng.gen_range(0.0..std::f64::consts::TAU);
            let len = rng.gen_range(22_000.0..32_000.0);
            let (x2, y2) = (x + a.cos() * len, y + a.sin() * len);
            if (5000.0..GWIDTH - 5000.0).contains(&x2) && (5000.0..GWIDTH - 5000.0).contains(&y2) {
                let mut t = Terrain::new(TerrainKind::Corridor, name, (x, y), CORRIDOR_WIDTH / 2.0);
                (t.x2, t.y2) = (x2, y2);
                world.terrain.push(t);
                break;
            }
        }
    }
    let storm = place(world, 4500.0, -10_000.0).map(|p| {
        let mut t = Terrain::new(TerrainKind::IonStorm, "ion storm", p, 4500.0);
        let a = rng.gen_range(0.0..std::f64::consts::TAU);
        (t.vx, t.vy) = (a.cos() * 0.6 * WARP1, a.sin() * 0.6 * WARP1);
        t
    });
    add(world, storm);
    let mut comet = Terrain::new(TerrainKind::Comet, "comet", (0.0, 0.0), COMET_HEAD);
    launch_comet(&mut comet);
    world.terrain.push(comet);
}

/// Which way to steer (Netrek direction) to get clear of a deadly spot,
/// for robots.
pub fn escape(world: &World, x: f64, y: f64) -> Option<f64> {
    for t in &world.terrain {
        let d = dist(x, y, t.x, t.y);
        let danger = match t.kind {
            TerrainKind::BlackHole => d < t.r + 1000.0,
            TerrainKind::Star => d < STAR_CORE + 2500.0,
            TerrainKind::Comet => d < COMET_HEAD + 1500.0,
            _ => false,
        };
        if danger {
            return Some(dir_to(t.x, t.y, x, y));
        }
    }
    None
}

/// Whether (x, y) is inside an asteroid field (robots slow down there).
pub fn in_asteroids(world: &World, x: f64, y: f64) -> bool {
    world.terrain.iter().any(|t| t.kind == TerrainKind::Asteroids && dist(x, y, t.x, t.y) < t.r)
}

pub fn tick(world: &mut World) {
    let mut rng = rand::thread_rng();
    let tick = world.tick;

    // Drifting things.
    for t in world.terrain.iter_mut() {
        match t.kind {
            TerrainKind::IonStorm => {
                t.x += t.vx;
                t.y += t.vy;
                if t.x < t.r || t.x > GWIDTH - t.r {
                    t.vx = -t.vx;
                }
                if t.y < t.r || t.y > GWIDTH - t.r {
                    t.vy = -t.vy;
                }
            }
            TerrainKind::Comet => {
                t.x += t.vx;
                t.y += t.vy;
                update_tail(t);
                let out = |v: f64| !(-COMET_TAIL - 5000.0..GWIDTH + COMET_TAIL + 5000.0).contains(&v);
                if out(t.x) || out(t.y) {
                    launch_comet(t);
                }
            }
            TerrainKind::Pulsar => t.timer = (t.timer + 1) % PULSAR_PERIOD,
            _ => {}
        }
    }
    // Salvaged derelicts: another wreck drifts in somewhere else.
    for k in 0..world.terrain.len() {
        if world.terrain[k].respawn > 0 {
            world.terrain[k].respawn -= 1;
            if world.terrain[k].respawn == 0 {
                let t = world.terrain.remove(k);
                if let Some(p) = place(world, t.r, 3000.0) {
                    (world.terrain.push(Terrain { x: p.0, y: p.1, ..t }));
                } else {
                    world.terrain.push(Terrain { respawn: 0, ..t });
                }
                break;
            }
        }
    }

    for p in world.players.iter_mut() {
        p.hidden = false;
        p.in_storm = false;
        p.detected = false;
    }

    let mut hurt: Vec<(usize, f64, String)> = Vec::new();
    let mut doomed: Vec<(usize, String)> = Vec::new();
    let mut warns: Vec<(u8, String)> = Vec::new();
    let mut salvaged: Vec<(usize, usize)> = Vec::new();
    let mut near_wreck = [false; MAXPLAYER];
    let terrain = &world.terrain;
    let players = &mut world.players;
    for (ti, t) in terrain.iter().enumerate() {
        if !t.visible() {
            continue;
        }
        for j in 0..MAXPLAYER {
            let p = &mut players[j];
            if !p.alive() {
                continue;
            }
            let d = dist(p.x, p.y, t.x, t.y);
            // A tachyon grid shows up any cloak, alien or not.
            if t.kind == TerrainKind::TachyonGrid {
                if d < t.r && p.cloaked {
                    p.detected = true;
                }
                continue;
            }
            // Everything else only affects the empires' ships.
            if p.faction.is_some() {
                continue;
            }
            let s = p.stats();
            let id = p.id;
            match t.kind {
                TerrainKind::Nebula if d < t.r => {
                    p.hidden = true;
                    if p.shields_up {
                        p.shields_up = false;
                        warns.push((id, format!("Shields can't hold in the {}", t.name)));
                    }
                    p.desired_speed = p.desired_speed.min(NEBULA_SPEED);
                }
                TerrainKind::IonStorm if d < t.r => {
                    p.hidden = true;
                    p.in_storm = true;
                    if (tick + j as u32) % 30 == 0 && rng.gen_bool(0.35) {
                        hurt.push((j, 12.0, "was struck by lightning in an ion storm".into()));
                    }
                }
                TerrainKind::Asteroids if d < t.r && p.speed > 4 => {
                    hurt.push((j, (p.speed - 4) as f64 * 0.25, format!("was smashed in the {}", t.name)));
                }
                TerrainKind::BlackHole if d < t.r => {
                    if d < HORIZON {
                        doomed.push((j, format!("fell into the {}", t.name)));
                        continue;
                    }
                    if p.orbiting.is_none() {
                        let pull = 8.0 + 70.0 * (1.0 - d / t.r).powi(2);
                        p.x += (t.x - p.x) / d * pull;
                        p.y += (t.y - p.y) / d * pull;
                    }
                    if tick % 30 == j as u32 % 30 {
                        warns.push((id, "Gravity well! Go to full impulse to break free".into()));
                    }
                }
                TerrainKind::Pulsar => {
                    if t.timer == 0 && d < t.r {
                        hurt.push((j, 10.0 + 45.0 * (1.0 - d / t.r), format!("was irradiated by the {}", t.name)));
                    } else if t.timer == PULSAR_PERIOD - 20 && d < t.r + 2000.0 {
                        warns.push((id, "Pulsar pulse in 2 seconds!".into()));
                    }
                }
                TerrainKind::Wormhole => {
                    if tick < p.wormhole_until {
                        continue;
                    }
                    let d2 = dist(p.x, p.y, t.x2, t.y2);
                    let exit = if d < MOUTH {
                        Some((t.x2, t.y2))
                    } else if d2 < MOUTH {
                        Some((t.x, t.y))
                    } else {
                        None
                    };
                    if let Some((ex, ey)) = exit {
                        let (hx, hy) = dir_vec(p.dir);
                        p.x = (ex + hx * 1500.0).clamp(0.0, GWIDTH);
                        p.y = (ey + hy * 1500.0).clamp(0.0, GWIDTH);
                        p.leave_orbit_pub();
                        p.lock = super::world::Lock::None;
                        p.wormhole_until = tick + 5 * UPS as u32;
                        warns.push((id, format!("Transit through the {}!", t.name)));
                    }
                }
                TerrainKind::Derelict if d < SALVAGE_REACH => {
                    near_wreck[j] = true;
                    if p.speed <= 2 {
                        p.salvage += 1;
                        if p.salvage == 1 {
                            warns.push((id, format!("Salvaging the {}: hold position for 5 seconds", t.name)));
                        }
                        if p.salvage >= SALVAGE_TICKS {
                            p.salvage = 0;
                            salvaged.push((j, ti));
                        }
                    }
                }
                TerrainKind::Corridor if p.speed > 0 && p.orbiting.is_none() => {
                    if seg_dist(p.x, p.y, t.x, t.y, t.x2, t.y2) < CORRIDOR_WIDTH / 2.0 {
                        let along = dir_to(t.x, t.y, t.x2, t.y2);
                        let diff = dir_diff(p.dir, along).abs();
                        if diff < 32.0 || diff > 96.0 {
                            let (hx, hy) = dir_vec(p.dir);
                            p.x = (p.x + hx * 3.0 * WARP1).clamp(0.0, GWIDTH);
                            p.y = (p.y + hy * 3.0 * WARP1).clamp(0.0, GWIDTH);
                            p.fuel += s.warp_cost * p.speed as f64;
                        }
                    }
                }
                TerrainKind::Star if d < t.r => {
                    if d < STAR_CORE {
                        hurt.push((j, 6.0, format!("burned up in {}", t.name)));
                    }
                    p.fuel = (p.fuel + 4.0 * s.recharge).min(s.max_fuel);
                    p.etemp += 8.0;
                    p.wtemp += 4.0;
                }
                TerrainKind::Comet => {
                    if d < COMET_HEAD {
                        hurt.push((j, 4.0, "was struck by a comet".into()));
                    } else if seg_dist(p.x, p.y, t.x, t.y, t.x2, t.y2) < 1200.0 {
                        p.fuel = (p.fuel + 8.0 * s.recharge).min(s.max_fuel);
                    }
                }
                _ => {}
            }
        }
    }
    for (j, near) in near_wreck.iter().enumerate() {
        if !near {
            players[j].salvage = 0;
        }
    }

    // Torpedoes: soaked up by asteroids, dragged into the black hole.
    for t in world.torps.iter_mut().filter(|t| t.explode == 0) {
        for f in world.terrain.iter() {
            let d = dist(t.x, t.y, f.x, f.y);
            match f.kind {
                TerrainKind::Asteroids if d < f.r && rng.gen_bool(0.06) => t.fuse = 0,
                TerrainKind::BlackHole if d < f.r => {
                    if d < HORIZON {
                        t.fuse = 0;
                    } else {
                        let pull = 8.0 + 70.0 * (1.0 - d / f.r).powi(2);
                        t.x += (f.x - t.x) / d * pull;
                        t.y += (f.y - t.y) / d * pull;
                    }
                }
                _ => {}
            }
        }
    }

    for (id, text) in warns {
        world.warn(id, text);
    }
    for (j, dmg, how) in hurt {
        world.inflict(j, dmg, None, how);
    }
    for (j, how) in doomed {
        if world.players[j].alive() {
            world.kill(j, None, how);
        }
    }
    for (j, ti) in salvaged {
        salvage(world, j, ti);
    }
}

/// Reward for salvaging a derelict: fuel and repairs, stranded colonists
/// (armies), or the ship's tactical logs (a kill's worth of intelligence).
fn salvage(world: &mut World, j: usize, ti: usize) {
    let mut rng = rand::thread_rng();
    let name = world.terrain[ti].name.clone();
    world.terrain[ti].respawn = 60 * UPS as i32;
    let p = &mut world.players[j];
    let s = p.stats();
    let room = s.max_armies.saturating_sub(p.armies);
    let what = match rng.gen_range(0..3) {
        1 if room > 0 => {
            let n = room.min(2);
            p.armies += n;
            format!("rescues {} {} of stranded colonists", n, if n == 1 { "army" } else { "armies" })
        }
        2 => {
            p.kills += 1.0;
            p.total_kills += 1.0;
            "recovers its tactical logs (+1 kill)".to_string()
        }
        _ => {
            p.fuel = s.max_fuel;
            p.damage = 0.0;
            p.shield = s.max_shield;
            "strips it for deuterium and spare parts (full fuel and repairs)".to_string()
        }
    };
    let who = p.label();
    world.god(format!("{} salvages the {} and {}", who, name, what));
    world.events.push(GameEvent::Salvaged { player: j as u8 });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{ClientMsg, PState};
    use crate::server::world::Features;

    /// A world with terrain rules on (but none placed) and a Federation
    /// cruiser at (x, y).
    fn setup(x: f64, y: f64) -> (World, usize) {
        let mut w = World::new();
        w.features.terrain = true;
        let id = w.add_player("Kirk", false).unwrap();
        w.join(id, Team::Fed, ShipType::Cruiser).unwrap();
        let p = &mut w.players[id as usize];
        (p.x, p.y) = (x, y);
        (w, id as usize)
    }

    fn put(w: &mut World, kind: TerrainKind, x: f64, y: f64, r: f64) -> usize {
        w.terrain.push(Terrain::new(kind, "test", (x, y), r));
        w.terrain.len() - 1
    }

    #[test]
    fn galaxy_has_ten_kinds_of_terrain_clear_of_planets() {
        for _ in 0..20 {
            let w = World::with_features(Features { terrain: true, ..Features::default() });
            let mut kinds: Vec<TerrainKind> = w.terrain.iter().map(|t| t.kind).collect();
            kinds.sort_by_key(|k| *k as u8);
            kinds.dedup();
            assert!(kinds.len() >= 10, "only {} kinds: {:?}", kinds.len(), kinds);
            for t in w.terrain.iter().filter(|t| matches!(t.kind, TerrainKind::Nebula | TerrainKind::BlackHole | TerrainKind::Star | TerrainKind::Pulsar)) {
                for pl in &w.planets {
                    assert!(dist(t.x, t.y, pl.x, pl.y) > t.r, "{} sits on {}", t.name, pl.name);
                }
            }
        }
    }

    #[test]
    fn nebula_hides_and_grounds_shields() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        put(&mut w, TerrainKind::Nebula, 50_000.0, 50_000.0, 5000.0);
        w.players[k].desired_speed = 9;
        tick(&mut w);
        let p = &w.players[k];
        assert!(p.hidden && !p.shields_up && p.desired_speed == NEBULA_SPEED);
        // A Romulan far away sees only a blur; up close it sees us.
        let r = w.add_player("Tal", false).unwrap();
        w.join(r, Team::Rom, ShipType::Cruiser).unwrap();
        (w.players[r as usize].x, w.players[r as usize].y) = (60_000.0, 50_000.0);
        assert!(w.frame_for(r).players.iter().find(|q| q.id == k as u8).unwrap().fuzzy);
        (w.players[r as usize].x, w.players[r as usize].y) = (51_000.0, 50_000.0);
        assert!(!w.frame_for(r).players.iter().find(|q| q.id == k as u8).unwrap().fuzzy);
    }

    #[test]
    fn ion_storm_knocks_out_phasers() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        put(&mut w, TerrainKind::IonStorm, 50_000.0, 50_000.0, 4500.0);
        tick(&mut w);
        let fuel = w.players[k].fuel;
        w.handle(k as u8, ClientMsg::Phaser(0));
        assert!(w.phasers.is_empty() && w.players[k].fuel == fuel);
    }

    #[test]
    fn asteroids_hurt_fast_ships_only() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        put(&mut w, TerrainKind::Asteroids, 50_000.0, 50_000.0, 4000.0);
        w.players[k].shields_up = false;
        w.players[k].speed = 3;
        tick(&mut w);
        assert_eq!(w.players[k].damage, 0.0);
        w.players[k].speed = 9;
        tick(&mut w);
        assert!(w.players[k].damage > 0.0);
    }

    #[test]
    fn black_hole_pulls_and_swallows() {
        let (mut w, k) = setup(55_000.0, 50_000.0);
        put(&mut w, TerrainKind::BlackHole, 50_000.0, 50_000.0, 7000.0);
        tick(&mut w);
        assert!(w.players[k].x < 55_000.0, "pulled in");
        w.players[k].x = 50_300.0;
        tick(&mut w);
        assert_eq!(w.players[k].state, PState::Exploding);
    }

    #[test]
    fn pulsar_pulses() {
        let (mut w, k) = setup(52_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::Pulsar, 50_000.0, 50_000.0, 6500.0);
        w.players[k].shields_up = false;
        w.terrain[t].timer = PULSAR_PERIOD - 1;
        tick(&mut w);
        assert!(w.players[k].damage > 10.0);
    }

    #[test]
    fn wormhole_transit() {
        let (mut w, k) = setup(20_000.0, 20_000.0);
        let t = put(&mut w, TerrainKind::Wormhole, 20_000.0, 20_000.0, MOUTH);
        (w.terrain[t].x2, w.terrain[t].y2) = (80_000.0, 80_000.0);
        tick(&mut w);
        assert!(dist(w.players[k].x, w.players[k].y, 80_000.0, 80_000.0) < 2000.0);
        // No bouncing straight back.
        tick(&mut w);
        assert!(dist(w.players[k].x, w.players[k].y, 80_000.0, 80_000.0) < 2000.0);
    }

    #[test]
    fn derelicts_can_be_salvaged() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::Derelict, 50_200.0, 50_000.0, SALVAGE_REACH);
        w.players[k].fuel = 10.0;
        for _ in 0..SALVAGE_TICKS {
            tick(&mut w);
        }
        assert!(w.events.iter().any(|e| matches!(e, GameEvent::Salvaged { .. })));
        assert!(!w.terrain[t].visible(), "picked clean");
    }

    #[test]
    fn slipstream_speeds_you_along() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::Corridor, 40_000.0, 50_000.0, CORRIDOR_WIDTH / 2.0);
        (w.terrain[t].x2, w.terrain[t].y2) = (70_000.0, 50_000.0);
        w.players[k].speed = 5;
        w.players[k].dir = 64.0; // east, along the stream
        tick(&mut w);
        assert!((w.players[k].x - 50_000.0 - 3.0 * WARP1).abs() < 1.0);
        w.players[k].dir = 0.0; // across it: no boost
        let x = w.players[k].x;
        tick(&mut w);
        assert_eq!(w.players[k].x, x);
    }

    #[test]
    fn star_refuels_but_burns() {
        let (mut w, k) = setup(53_000.0, 50_000.0);
        put(&mut w, TerrainKind::Star, 50_000.0, 50_000.0, 5000.0);
        w.players[k].fuel = 100.0;
        tick(&mut w);
        assert!(w.players[k].fuel > 100.0 && w.players[k].etemp > 0.0);
        w.players[k].x = 50_500.0;
        w.players[k].shields_up = false;
        tick(&mut w);
        assert!(w.players[k].damage > 0.0);
    }

    #[test]
    fn comet_tail_refuels() {
        let (mut w, k) = setup(45_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::Comet, 50_000.0, 50_000.0, COMET_HEAD);
        (w.terrain[t].vx, w.terrain[t].vy) = (40.0, 0.0);
        update_tail(&mut w.terrain[t]);
        w.players[k].fuel = 100.0;
        tick(&mut w);
        assert!(w.players[k].fuel > 100.0);
    }

    #[test]
    fn tachyon_grid_exposes_cloaks() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        put(&mut w, TerrainKind::TachyonGrid, 50_000.0, 50_000.0, 6000.0);
        w.players[k].cloaked = true;
        tick(&mut w);
        let r = w.add_player("Tal", false).unwrap();
        w.join(r, Team::Rom, ShipType::Cruiser).unwrap();
        assert!(!w.frame_for(r).players.iter().find(|q| q.id == k as u8).unwrap().fuzzy);
    }
}
