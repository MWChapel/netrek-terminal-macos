//! Space terrain (the --terrain option): nebulae, ion storms, asteroid
//! fields, a black hole, a pulsar, a wormhole, derelicts to salvage,
//! subspace slipstreams, a star, a comet, a tachyon detection grid, the
//! Briar Patch, the Badlands, the Nexus, the Guardian of Forever and more.
//! Generated fresh with each galaxy, clear of the planets.

use super::world::{Away, GameEvent, Past, Tear, World, TEAR_TTL};
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
    /// Minefield: where its (hidden) mines are.
    pub mines: Vec<(f64, f64)>,
    /// The Delphic sphere that made it (it goes when the sphere does).
    pub owner: Option<u8>,
    /// Neutral Zone: the two empires it lies between.
    pub sides: Option<(Team, Team)>,
}

pub const NEBULA_SPEED: i32 = 6;
pub const MINEFIELD_R: f64 = 4000.0;
pub const MINES: usize = 14;
/// How close a ship must come to a mine to set it off.
pub const MINE_REACH: f64 = 450.0;
pub const METREON_BLAST: f64 = 2500.0;
/// Half the thickness of the galactic barrier.
pub const BARRIER_REACH: f64 = 350.0;
pub const STATION_REACH: f64 = 1500.0;
pub const HORIZON: f64 = 600.0;
/// The Black Hole (the Disney film): how far its gravity reaches, and its
/// event horizon.
pub const GREAT_HOLE_R: f64 = 12_000.0;
pub const GREAT_HORIZON: f64 = 1500.0;
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
/// The Briar Patch: top speed inside, and hull healed per tick.
pub const BRIAR_SPEED: i32 = 4;
pub const BRIAR_HEAL: f64 = 0.3;
/// The Badlands: ticks between plasma eruptions, how many, their reach
/// and how hard they hit.
pub const BADLANDS_PERIOD: i32 = 15;
pub const ERUPTIONS: usize = 4;
pub const ERUPTION_REACH: f64 = 1200.0;
pub const ERUPTION_DAMAGE: f64 = 20.0;
/// The Nexus ribbon: its length, how close counts as touching, and how
/// long a ship stays inside.
pub const NEXUS_LEN: f64 = 16_000.0;
pub const NEXUS_REACH: f64 = 600.0;
pub const NEXUS_TICKS: u32 = 20 * UPS as u32;
/// Tyken's Rift: size, fuel drained and pull per tick, and how close to
/// the centre a torpedo must go off to collapse it.
pub const TYKEN_R: f64 = 3500.0;
pub const TYKEN_DRAIN: f64 = 60.0;
pub const TYKEN_PULL: f64 = 45.0;
pub const TYKEN_CORE: f64 = 800.0;
/// The Genesis Planet: its solid surface, how close a ship must hold to
/// take on colonists (one every GENESIS_GROW ticks), how long until it
/// tears itself apart, and the blast.
pub const GENESIS_R: f64 = 1500.0;
pub const GENESIS_REACH: f64 = 3500.0;
pub const GENESIS_GROW: u32 = 30;
pub const GENESIS_LIFE: i32 = 120 * UPS as i32;
pub const GENESIS_BLAST: f64 = 6000.0;
pub const GENESIS_DAMAGE: f64 = 50.0;
/// Quantum filaments: the field, how many strands, how long, and how close
/// you can pass.
pub const FILAMENT_R: f64 = 6000.0;
pub const FILAMENTS: usize = 7;
pub const FILAMENT_LEN: f64 = 2500.0;
pub const FILAMENT_REACH: f64 = 200.0;
/// The Hekaras Corridor: its width, and the warp above which ships tear
/// subspace.
pub const HEKARAS_WIDTH: f64 = 2600.0;
pub const HEKARAS_SPEED: i32 = 5;
/// An Omega particle zone: its size when it forms and at most, the core,
/// and how long a ship must hold there to neutralise it.
pub const OMEGA_R: f64 = 3000.0;
pub const OMEGA_MAX: f64 = 9000.0;
pub const OMEGA_CORE: f64 = 800.0;
pub const OMEGA_TICKS: i32 = 10 * UPS as i32;
pub const OMEGA_REWARD: f64 = 2.0;
/// The Neutral Zone: its width, and what its border outposts do to a ship
/// that fires inside it (at most every NEUTRAL_EVERY ticks).
pub const NEUTRAL_WIDTH: f64 = 3000.0;
pub const NEUTRAL_FIRE: f64 = 15.0;
pub const NEUTRAL_EVERY: u32 = 3 * UPS as u32;
/// The Guardian of Forever: its portal, and how far back it sends you
/// (a snapshot every PAST_EVERY ticks, PAST_LEN of them).
pub const GUARDIAN_R: f64 = 800.0;
pub const PAST_EVERY: u32 = 5;
pub const PAST_LEN: usize = 61;
/// Theta radiation: how big a patch is, how long it lasts, and how often
/// it kills an army aboard.
pub const THETA_R: f64 = 2500.0;
pub const THETA_TTL: i32 = 90 * UPS as i32;
pub const THETA_SICKNESS: u32 = 20;
/// How far a red matter singularity pulls.
pub const RED_MATTER_R: f64 = 6000.0;

impl Terrain {
    fn new(kind: TerrainKind, name: &str, (x, y): (f64, f64), r: f64) -> Terrain {
        Terrain { kind, name: name.into(), x, y, r, x2: x, y2: y, vx: 0.0, vy: 0.0, timer: 0, respawn: 0, mines: Vec::new(), owner: None, sides: None }
    }

    /// An anomaly planted by a Delphic sphere (alien incursion).
    pub fn planted(kind: TerrainKind, name: &str, at: (f64, f64), r: f64, owner: u8) -> Terrain {
        Terrain { owner: Some(owner), ..Terrain::new(kind, name, at, r) }
    }

    /// The black hole a planet becomes when the Narada's red matter
    /// reaches its core.
    pub fn singularity(name: &str, at: (f64, f64)) -> Terrain {
        Terrain::new(TerrainKind::BlackHole, name, at, RED_MATTER_R)
    }

    /// A patch of theta radiation dumped by a Malon freighter: it lingers
    /// for a while after the freighter has gone.
    pub fn theta(at: (f64, f64)) -> Terrain {
        Terrain { timer: THETA_TTL, ..Terrain::new(TerrainKind::ThetaRadiation, "theta radiation", at, THETA_R) }
    }

    pub fn visible(&self) -> bool {
        self.respawn == 0
    }

    pub fn info(&self) -> TerrainInfo {
        let phase = match self.kind {
            TerrainKind::Pulsar => (PULSAR_PERIOD - self.timer).clamp(0, 255) as u8,
            TerrainKind::GenesisPlanet => (self.timer as f64 / GENESIS_LIFE as f64 * 255.0).clamp(0.0, 255.0) as u8,
            TerrainKind::ThetaRadiation => (self.timer as f64 / THETA_TTL as f64 * 255.0).clamp(0.0, 255.0) as u8,
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
            TerrainKind::Corridor | TerrainKind::Comet | TerrainKind::IonStorm | TerrainKind::Nexus | TerrainKind::HekarasCorridor | TerrainKind::NeutralZone => true,
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

/// Kinds of terrain each galaxy gets, drawn at random from all of them.
pub const ACTIVE_KINDS: usize = 10;

pub const ALL_KINDS: [TerrainKind; 32] = [
    TerrainKind::Nebula,
    TerrainKind::IonStorm,
    TerrainKind::Asteroids,
    TerrainKind::BlackHole,
    TerrainKind::Pulsar,
    TerrainKind::Wormhole,
    TerrainKind::Derelict,
    TerrainKind::Corridor,
    TerrainKind::Star,
    TerrainKind::Comet,
    TerrainKind::TachyonGrid,
    TerrainKind::Minefield,
    TerrainKind::ChronitonField,
    TerrainKind::GravitonEddy,
    TerrainKind::Magnetar,
    TerrainKind::MetreonCloud,
    TerrainKind::TetryonField,
    TerrainKind::Planetoid,
    TerrainKind::GalacticBarrier,
    TerrainKind::FluidicRift,
    TerrainKind::AbandonedStation,
    TerrainKind::GreatBlackHole,
    TerrainKind::BriarPatch,
    TerrainKind::Badlands,
    TerrainKind::Nexus,
    TerrainKind::TykensRift,
    TerrainKind::GenesisPlanet,
    TerrainKind::QuantumFilaments,
    TerrainKind::HekarasCorridor,
    TerrainKind::OmegaZone,
    TerrainKind::NeutralZone,
    TerrainKind::GuardianOfForever,
];

/// A fresh set of terrain: ten kinds, picked at random, each placed clear
/// of the planets.
pub fn generate(world: &mut World) {
    let mut rng = rand::thread_rng();
    world.terrain.clear();
    let mut kinds = ALL_KINDS.to_vec();
    kinds.shuffle(&mut rng);
    // The Black Hole needs a vast stretch of open space: if it's one of this
    // galaxy's kinds, it goes down first.
    if let Some(k) = kinds.iter().take(ACTIVE_KINDS).position(|&k| k == TerrainKind::GreatBlackHole) {
        let hole = kinds.remove(k);
        kinds.insert(0, hole);
    }
    let mut active = 0;
    for kind in kinds {
        if active >= ACTIVE_KINDS {
            break;
        }
        if spawn_kind(world, kind) > 0 {
            active += 1;
        }
    }
}

/// A random straight segment of about `len`, well inside the galaxy.
fn segment(len: std::ops::Range<f64>) -> Option<((f64, f64), (f64, f64))> {
    let mut rng = rand::thread_rng();
    for _ in 0..100 {
        let (x, y) = (rng.gen_range(8000.0..92_000.0), rng.gen_range(8000.0..92_000.0));
        let a = rng.gen_range(0.0..std::f64::consts::TAU);
        let l = rng.gen_range(len.clone());
        let (x2, y2) = (x + a.cos() * l, y + a.sin() * l);
        if (5000.0..GWIDTH - 5000.0).contains(&x2) && (5000.0..GWIDTH - 5000.0).contains(&y2) {
            return Some(((x, y), (x2, y2)));
        }
    }
    None
}

/// Random points inside a circle.
fn scatter(x: f64, y: f64, r: f64, n: usize) -> Vec<(f64, f64)> {
    let mut rng = rand::thread_rng();
    (0..n)
        .map(|_| {
            let a = rng.gen_range(0.0..std::f64::consts::TAU);
            let d = rng.gen_range(0.0f64..1.0).sqrt() * r * 0.95;
            (x + a.cos() * d, y + a.sin() * d)
        })
        .collect()
}

/// Place the features of one kind; returns how many were placed.
fn spawn_kind(world: &mut World, kind: TerrainKind) -> usize {
    let mut rng = rand::thread_rng();
    let before = world.terrain.len();
    let add = |world: &mut World, t: Option<Terrain>| {
        if let Some(t) = t {
            world.terrain.push(t);
        }
    };
    let many = |world: &mut World, names: &[&str], take: usize, r: std::ops::Range<f64>, clear: f64| {
        let mut names = names.to_vec();
        names.shuffle(&mut rand::thread_rng());
        for name in names.into_iter().take(take) {
            let rr = rand::thread_rng().gen_range(r.clone());
            if let Some(p) = place(world, rr, clear) {
                world.terrain.push(Terrain::new(kind, name, p, rr));
            }
        }
    };
    match kind {
        TerrainKind::Nebula => many(world, &["Mutara Nebula", "Hourglass Nebula", "Paulson Nebula", "Mar Oscura"], 3, 5000.0..7000.0, 1500.0),
        TerrainKind::Asteroids => many(world, &["Maluria asteroid belt", "Hanoran asteroid field"], 2, 4000.0..5000.0, 1500.0),
        TerrainKind::BlackHole => {
            let t = place(world, 7000.0, 3000.0).map(|p| Terrain::new(kind, "Tarsus singularity", p, 7000.0));
            add(world, t);
        }
        TerrainKind::Pulsar => {
            let t = place(world, 6500.0, 1500.0).map(|p| Terrain::new(kind, "Zeta Lantis pulsar", p, 6500.0));
            add(world, t);
        }
        TerrainKind::Star => {
            let t = place(world, 5000.0, 2000.0).map(|p| Terrain::new(kind, "Amargosa", p, 5000.0));
            add(world, t);
        }
        TerrainKind::TachyonGrid => {
            let t = place(world, 6000.0, -3000.0).map(|p| Terrain::new(kind, "tachyon detection grid", p, 6000.0));
            add(world, t);
        }
        TerrainKind::Wormhole => {
            // Two mouths far apart.
            if let Some(a) = place(world, MOUTH, 3000.0) {
                for _ in 0..50 {
                    if let Some(b) = place(world, MOUTH, 3000.0) {
                        if dist(a.0, a.1, b.0, b.1) > 40_000.0 {
                            let mut t = Terrain::new(kind, "Barzan wormhole", a, MOUTH);
                            (t.x2, t.y2) = b;
                            world.terrain.push(t);
                            break;
                        }
                    }
                }
            }
        }
        TerrainKind::Derelict => many(
            world,
            &["derelict freighter", "derelict USS Valiant", "derelict Klingon cruiser", "derelict Romulan scout"],
            3,
            SALVAGE_REACH..SALVAGE_REACH + 1.0,
            3000.0,
        ),
        TerrainKind::Corridor => {
            for name in ["Delta slipstream", "Theta slipstream"] {
                if let Some((a, b)) = segment(22_000.0..32_000.0) {
                    let mut t = Terrain::new(kind, name, a, CORRIDOR_WIDTH / 2.0);
                    (t.x2, t.y2) = b;
                    world.terrain.push(t);
                }
            }
        }
        TerrainKind::IonStorm => {
            let storm = place(world, 4500.0, -10_000.0).map(|p| {
                let mut t = Terrain::new(kind, "ion storm", p, 4500.0);
                let a = rng.gen_range(0.0..std::f64::consts::TAU);
                (t.vx, t.vy) = (a.cos() * 0.6 * WARP1, a.sin() * 0.6 * WARP1);
                t
            });
            add(world, storm);
        }
        TerrainKind::Comet => {
            let mut comet = Terrain::new(kind, "comet", (0.0, 0.0), COMET_HEAD);
            launch_comet(&mut comet);
            world.terrain.push(comet);
        }
        TerrainKind::Minefield => {
            for name in ["Romulan minefield", "Cardassian minefield"] {
                if let Some(p) = place(world, MINEFIELD_R, 1500.0) {
                    let mut t = Terrain::new(kind, name, p, MINEFIELD_R);
                    t.mines = scatter(p.0, p.1, MINEFIELD_R, MINES);
                    world.terrain.push(t);
                }
            }
        }
        TerrainKind::ChronitonField => many(world, &["chroniton field"], 1, 4500.0..5500.0, 1500.0),
        TerrainKind::GravitonEddy => many(world, &["graviton eddy"], 1, 5500.0..6500.0, 1500.0),
        TerrainKind::Magnetar => many(world, &["magnetar Sigma Draconis"], 1, 5500.0..6500.0, 1500.0),
        TerrainKind::MetreonCloud => many(world, &["Metreon cloud", "Metreon cloud"], 2, 4000.0..5000.0, 1500.0),
        TerrainKind::TetryonField => many(world, &["tetryon field"], 1, 4500.0..5500.0, 1000.0),
        TerrainKind::Planetoid => many(world, &["rogue planetoid Gamma", "rogue planetoid Delta"], 2, 1200.0..1600.0, 2500.0),
        TerrainKind::GalacticBarrier => {
            if let Some((a, b)) = segment(20_000.0..30_000.0) {
                let mut t = Terrain::new(kind, "galactic barrier", a, BARRIER_REACH);
                (t.x2, t.y2) = b;
                world.terrain.push(t);
            }
        }
        TerrainKind::FluidicRift => many(world, &["fluidic rift"], 1, 700.0..701.0, 3000.0),
        TerrainKind::AbandonedStation => many(world, &["abandoned station K-7"], 1, STATION_REACH..STATION_REACH + 1.0, 3000.0),
        TerrainKind::GreatBlackHole => {
            // Planets may sit at the very edge of its pull, but no closer.
            let t = place(world, GREAT_HOLE_R, -1500.0).map(|p| Terrain::new(kind, "the Black Hole", p, GREAT_HOLE_R));
            add(world, t);
        }
        TerrainKind::BriarPatch => many(world, &["the Briar Patch"], 1, 6000.0..7000.0, 1500.0),
        TerrainKind::Badlands => many(world, &["the Badlands"], 1, 6500.0..7500.0, 1500.0),
        TerrainKind::Nexus => {
            let (cx, cy) = (rng.gen_range(25_000.0..75_000.0), rng.gen_range(25_000.0..75_000.0));
            let a = rng.gen_range(0.0..std::f64::consts::TAU);
            let (ux, uy) = (a.cos(), a.sin());
            let mut t = Terrain::new(kind, "the Nexus", (cx - ux * NEXUS_LEN / 2.0, cy - uy * NEXUS_LEN / 2.0), NEXUS_REACH);
            (t.x2, t.y2) = (cx + ux * NEXUS_LEN / 2.0, cy + uy * NEXUS_LEN / 2.0);
            // It sweeps sideways across the galaxy.
            (t.vx, t.vy) = (-uy * WARP1, ux * WARP1);
            world.terrain.push(t);
        }
        TerrainKind::TykensRift => many(world, &["Tyken's Rift"], 1, TYKEN_R..TYKEN_R + 1.0, 2000.0),
        TerrainKind::GenesisPlanet => many(world, &["the Genesis Planet"], 1, GENESIS_R..GENESIS_R + 1.0, GENESIS_REACH + 1000.0),
        TerrainKind::QuantumFilaments => {
            if let Some(p) = place(world, FILAMENT_R, 1500.0) {
                let mut t = Terrain::new(kind, "quantum filaments", p, FILAMENT_R);
                t.mines = (0..FILAMENTS).flat_map(|_| filament(p.0, p.1, FILAMENT_R)).collect();
                world.terrain.push(t);
            }
        }
        TerrainKind::HekarasCorridor => {
            if let Some((a, b)) = segment(20_000.0..28_000.0) {
                let mut t = Terrain::new(kind, "the Hekaras Corridor", a, HEKARAS_WIDTH / 2.0);
                (t.x2, t.y2) = b;
                world.terrain.push(t);
            }
        }
        TerrainKind::OmegaZone => {
            let t = place(world, OMEGA_MAX * 0.8, -2000.0).map(|p| Terrain::new(kind, "Omega particle zone", p, OMEGA_R));
            add(world, t);
        }
        TerrainKind::NeutralZone => {
            // Across the border between two neighbouring empires' home worlds.
            let pairs = [(Team::Fed, Team::Rom), (Team::Rom, Team::Kli), (Team::Kli, Team::Ori), (Team::Ori, Team::Fed)];
            let (a, b) = *pairs.choose(&mut rng).unwrap();
            let (pa, pb) = (&world.planets[a.home_planet()], &world.planets[b.home_planet()]);
            let (mx, my) = ((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
            let d = dist(pa.x, pa.y, pb.x, pb.y).max(1.0);
            let (ux, uy) = ((pb.y - pa.y) / d, -(pb.x - pa.x) / d);
            let end = |s: f64| ((mx + ux * s).clamp(3000.0, GWIDTH - 3000.0), (my + uy * s).clamp(3000.0, GWIDTH - 3000.0));
            let mut t = Terrain::new(kind, &format!("the {}-{} Neutral Zone", a.name(), b.name()), end(-15_000.0), NEUTRAL_WIDTH / 2.0);
            (t.x2, t.y2) = end(15_000.0);
            t.sides = Some((a, b));
            world.terrain.push(t);
        }
        TerrainKind::GuardianOfForever => many(world, &["the Guardian of Forever"], 1, GUARDIAN_R..GUARDIAN_R + 1.0, 3000.0),
        // Only ever dumped by a Malon freighter.
        TerrainKind::ThetaRadiation => {}
    }
    world.terrain.len() - before
}

/// One quantum filament somewhere in a field: its two ends.
fn filament(x: f64, y: f64, r: f64) -> [(f64, f64); 2] {
    let mut rng = rand::thread_rng();
    let (cx, cy) = scatter(x, y, r * 0.8, 1)[0];
    let a = rng.gen_range(0.0..std::f64::consts::TAU);
    let h = FILAMENT_LEN / 2.0;
    [(cx - a.cos() * h, cy - a.sin() * h), (cx + a.cos() * h, cy + a.sin() * h)]
}

/// Somewhere random and open, clear of the planets (fluidic rift exits).
pub fn random_open_spot(world: &World) -> (f64, f64) {
    let mut rng = rand::thread_rng();
    for _ in 0..200 {
        let (x, y) = (rng.gen_range(5000.0..95_000.0), rng.gen_range(5000.0..95_000.0));
        if world.planets.iter().all(|pl| dist(x, y, pl.x, pl.y) > 3000.0) {
            return (x, y);
        }
    }
    (50_000.0, 50_000.0)
}

/// An explosion to look at (no damage of its own).
fn flash(world: &mut World, x: f64, y: f64, owner: u8) {
    world.torps.push(super::world::Torp {
        owner,
        team: Team::Ind,
        kind: crate::proto::TorpKind::Photon,
        x,
        y,
        dir: 0.0,
        speed: 0.0,
        fuse: 0,
        damage: 0.0,
        explode: 1,
        quantum: false,
        deflect_tried: false,
    });
}

/// Firing a weapon inside a Metreon cloud ignites it: a blast hurts the
/// shooter and everyone nearby. (The gas takes a few seconds to gather
/// again before it can go off twice.)
pub fn metreon_ignite(world: &mut World, i: usize) {
    let (x, y) = (world.players[i].x, world.players[i].y);
    let Some(k) = world
        .terrain
        .iter()
        .position(|t| t.kind == TerrainKind::MetreonCloud && t.timer == 0 && dist(x, y, t.x, t.y) < t.r)
    else {
        return;
    };
    world.terrain[k].timer = 3 * UPS as i32;
    flash(world, x, y, i as u8);
    let caught: Vec<usize> = (0..MAXPLAYER).filter(|&j| world.players[j].alive() && dist(world.players[j].x, world.players[j].y, x, y) < METREON_BLAST).collect();
    let who = world.players[i].label();
    world.god(format!("{} ignites the Metreon gas!", who));
    for j in caught {
        world.inflict(j, 30.0, Some(i as u8), "was caught in a Metreon gas explosion".into());
    }
}

/// A shot fired inside the Neutral Zone: its border outposts open fire on
/// the shooter, and (with --diplomacy) a captain who fires there breaks
/// the treaty between the two empires it divides.
pub fn neutral_zone_shot(world: &mut World, i: usize) {
    let p = &world.players[i];
    if p.faction.is_some() || p.away.is_some() {
        return;
    }
    let (x, y, team, tick) = (p.x, p.y, p.team, world.tick);
    let Some(t) = world.terrain.iter().find(|t| t.kind == TerrainKind::NeutralZone && t.visible() && seg_dist(x, y, t.x, t.y, t.x2, t.y2) < t.r) else {
        return;
    };
    let (name, sides) = (t.name.clone(), t.sides);
    if let Some((a, b)) = sides {
        if world.features.diplomacy && !world.players[i].robot && (team == a || team == b) && world.allied(a, b) && !world.breaking.iter().any(|&(x, _, _)| x == team) {
            let who = world.players[i].label();
            world.god(format!("{} opens fire inside {}! The {} have violated their treaty.", who, name, team.plural()));
            world.break_treaty(team, None);
        }
    }
    if tick < world.players[i].violated_until {
        return;
    }
    world.players[i].violated_until = tick + NEUTRAL_EVERY;
    world.warn(i as u8, format!("Weapons fire inside {}! Its border outposts open fire on you", name));
    world.inflict(i, NEUTRAL_FIRE, None, format!("was destroyed by the outposts of {}", name));
}

/// A terrain feature, for tests elsewhere.
#[cfg(test)]
pub fn tests_make(kind: TerrainKind, x: f64, y: f64, r: f64) -> Terrain {
    Terrain::new(kind, "test", (x, y), r)
}

/// Which way to steer (Netrek direction) to get clear of a deadly spot,
/// for robots.
pub fn escape(world: &World, x: f64, y: f64) -> Option<f64> {
    for t in &world.terrain {
        let d = dist(x, y, t.x, t.y);
        let danger = match t.kind {
            TerrainKind::BlackHole => d < t.r + 1000.0,
            // Robots will go in after the Cygnus, but not too deep.
            TerrainKind::GreatBlackHole => d < t.r * 0.35,
            TerrainKind::Star => d < STAR_CORE + 2500.0,
            TerrainKind::Comet => d < COMET_HEAD + 1500.0,
            TerrainKind::TykensRift => d < t.r + 1000.0,
            TerrainKind::GenesisPlanet => t.timer > GENESIS_LIFE - 20 * UPS as i32 && d < GENESIS_BLAST + 1000.0,
            TerrainKind::OmegaZone | TerrainKind::ThetaRadiation => d < t.r + 500.0,
            _ => false,
        };
        if danger {
            return Some(dir_to(t.x, t.y, x, y));
        }
    }
    None
}

/// How hard the Black Hole drags things in, per tick, at distance `d` from
/// its centre: barely at all at the edge, beyond any ship's engines at the
/// event horizon.
pub fn great_pull(d: f64, r: f64) -> f64 {
    10.0 + 230.0 * (1.0 - d / r).max(0.0).powi(3)
}

/// The Cygnus's anti-gravity field holds it steady against any pull.
pub fn anti_gravity(ship: ShipType) -> bool {
    ship == ShipType::CygnusRelic
}

/// The Black Hole whose gravity well (x, y) is in, if any.
pub fn in_great_hole(world: &World, x: f64, y: f64) -> Option<&Terrain> {
    world.terrain.iter().find(|t| t.kind == TerrainKind::GreatBlackHole && dist(x, y, t.x, t.y) < t.r)
}

/// Whether (x, y) is inside an asteroid field (robots slow down there).
pub fn in_asteroids(world: &World, x: f64, y: f64) -> bool {
    world.terrain.iter().any(|t| t.kind == TerrainKind::Asteroids && dist(x, y, t.x, t.y) < t.r)
}

/// A Bajoran solar sail rides the tachyon eddies round stars, pulsars and
/// comets: +5 warp, free.
pub const SAIL_WARP: f64 = 5.0;

fn sail(p: &mut super::world::Player) {
    if p.ship == ShipType::SolarSail && p.speed > 0 && p.orbiting.is_none() {
        let (hx, hy) = dir_vec(p.dir);
        p.x = (p.x + hx * SAIL_WARP * WARP1).clamp(0.0, GWIDTH);
        p.y = (p.y + hy * SAIL_WARP * WARP1).clamp(0.0, GWIDTH);
    }
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
            TerrainKind::MetreonCloud if t.timer > 0 => t.timer -= 1,
            TerrainKind::Nexus => {
                (t.x, t.y, t.x2, t.y2) = (t.x + t.vx, t.y + t.vy, t.x2 + t.vx, t.y2 + t.vy);
                let out = |v: f64| !(2000.0..GWIDTH - 2000.0).contains(&v);
                if out(t.x) || out(t.y) || out(t.x2) || out(t.y2) {
                    (t.vx, t.vy) = (-t.vx, -t.vy);
                }
            }
            TerrainKind::Badlands => {
                t.timer = (t.timer + 1) % BADLANDS_PERIOD;
                if t.timer == 0 {
                    t.mines = scatter(t.x, t.y, t.r, ERUPTIONS);
                }
            }
            TerrainKind::GenesisPlanet if t.visible() => t.timer += 1,
            TerrainKind::OmegaZone if t.visible() => t.r = (t.r + 1.0).min(OMEGA_MAX),
            TerrainKind::ThetaRadiation => t.timer -= 1,
            _ => {}
        }
    }
    world.terrain.retain(|t| t.kind != TerrainKind::ThetaRadiation || t.timer > 0);
    // The Guardian of Forever remembers where everyone has been.
    if tick % PAST_EVERY == 0 && world.terrain.iter().any(|t| t.kind == TerrainKind::GuardianOfForever) {
        for p in world.players.iter_mut().filter(|p| p.alive() && p.faction.is_none() && p.away.is_none()) {
            p.past.push_back(Past { x: p.x, y: p.y, dir: p.dir, speed: p.speed, fuel: p.fuel, shield: p.shield, damage: p.damage, etemp: p.etemp, wtemp: p.wtemp });
            while p.past.len() > PAST_LEN {
                p.past.pop_front();
            }
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
        p.no_transport = false;
    }

    let mut hurt: Vec<(usize, f64, String)> = Vec::new();
    let mut doomed: Vec<(usize, String)> = Vec::new();
    let mut warns: Vec<(u8, String)> = Vec::new();
    let mut salvaged: Vec<(usize, usize)> = Vec::new();
    let mut near_wreck = [false; MAXPLAYER];
    let mut mine_hits: Vec<(usize, usize, usize)> = Vec::new();
    let mut barrier_hits: Vec<usize> = Vec::new();
    let mut rifts: Vec<usize> = Vec::new();
    let mut nexus: Vec<usize> = Vec::new();
    let mut filament_hits: Vec<(usize, usize, usize)> = Vec::new();
    let mut new_tears: Vec<Tear> = Vec::new();
    let mut omega_holders: Vec<(usize, usize)> = Vec::new();
    let mut guardian: Vec<usize> = Vec::new();
    let terrain = &world.terrain;
    let players = &mut world.players;
    for (ti, t) in terrain.iter().enumerate() {
        if !t.visible() {
            continue;
        }
        for j in 0..MAXPLAYER {
            let p = &mut players[j];
            if !p.alive() || p.away.is_some() {
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
                    if p.orbiting.is_none() && !p.techs.contains(&Tech::MetaphasicShields) && !anti_gravity(p.ship) {
                        let pull = 8.0 + 70.0 * (1.0 - d / t.r).powi(2);
                        p.x += (t.x - p.x) / d * pull;
                        p.y += (t.y - p.y) / d * pull;
                    }
                    if tick % 30 == j as u32 % 30 {
                        warns.push((id, "Gravity well! Go to full impulse to break free".into()));
                    }
                }
                TerrainKind::GreatBlackHole if d < t.r => {
                    if d < GREAT_HORIZON {
                        doomed.push((j, format!("was swallowed by {}", t.name)));
                        continue;
                    }
                    // Orbiting a planet at the edge of the pull holds you.
                    if p.orbiting.is_none() && !p.techs.contains(&Tech::MetaphasicShields) && !anti_gravity(p.ship) {
                        let pull = great_pull(d, t.r);
                        p.x += (t.x - p.x) / d * pull;
                        p.y += (t.y - p.y) / d * pull;
                        // And the whirlpool sweeps you round as it drags you in.
                        let (rx, ry) = ((p.x - t.x) / d, (p.y - t.y) / d);
                        let swirl = pull * 0.35;
                        p.x += ry * swirl;
                        p.y -= rx * swirl;
                    }
                    if tick % 30 == j as u32 % 30 {
                        let escape = (great_pull(d, t.r) / WARP1).ceil() as i32 + 1;
                        warns.push((id, format!("Caught in the Black Hole's gravity! You need warp {} or more to break free", escape)));
                    }
                }
                TerrainKind::Pulsar => {
                    if d < t.r {
                        sail(p);
                    }
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
                        // The Baxial's crew are born scroungers.
                        let first = p.salvage == 0;
                        p.salvage += if p.ship == ShipType::Baxial { 3 } else { 1 };
                        if first {
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
                    sail(p);
                    if d < STAR_CORE {
                        hurt.push((j, 6.0, format!("burned up in {}", t.name)));
                    }
                    p.fuel = (p.fuel + 4.0 * s.recharge).min(s.max_fuel);
                    if !p.techs.contains(&Tech::MetaphasicShields) {
                        p.etemp += 8.0;
                        p.wtemp += 4.0;
                    }
                }
                TerrainKind::Comet => {
                    if d < COMET_HEAD {
                        hurt.push((j, 4.0, "was struck by a comet".into()));
                    } else if seg_dist(p.x, p.y, t.x, t.y, t.x2, t.y2) < 1200.0 {
                        p.fuel = (p.fuel + 8.0 * s.recharge).min(s.max_fuel);
                        sail(p);
                    }
                }
                TerrainKind::Minefield if d < t.r + MINE_REACH => {
                    if let Some(m) = t.mines.iter().position(|&(mx, my)| dist(p.x, p.y, mx, my) < MINE_REACH) {
                        if !mine_hits.iter().any(|h| h.0 == ti && h.1 == m) {
                            mine_hits.push((ti, m, j));
                        }
                    }
                }
                TerrainKind::ChronitonField if d < t.r && p.orbiting.is_none() && p.speed > 0 => {
                    // Time runs at half speed: undo half of this tick's move.
                    let (hx, hy) = dir_vec(p.dir);
                    p.x -= hx * p.speed as f64 * WARP1 * 0.5;
                    p.y -= hy * p.speed as f64 * WARP1 * 0.5;
                }
                TerrainKind::GravitonEddy if d < t.r && d > 1.0 && p.orbiting.is_none() && !anti_gravity(p.ship) => {
                    // Swept around the centre, hardest near the middle.
                    let push = 10.0 + 25.0 * (1.0 - d / t.r);
                    let (rx, ry) = ((p.x - t.x) / d, (p.y - t.y) / d);
                    p.x = (p.x - ry * push).clamp(0.0, GWIDTH);
                    p.y = (p.y + rx * push).clamp(0.0, GWIDTH);
                }
                TerrainKind::Magnetar if d < t.r => {
                    if p.tractor.is_some() {
                        p.tractor = None;
                        warns.push((id, "The magnetar's field breaks your tractor beam".into()));
                    }
                    if d < 500.0 {
                        hurt.push((j, 5.0, format!("was crushed by the {}", t.name)));
                    }
                }
                TerrainKind::TetryonField if d < t.r => {
                    p.shield = (p.shield - 1.5).max(0.0);
                }
                TerrainKind::Planetoid if d < t.r && d > 1.0 => {
                    // Solid rock: pushed back out to the surface.
                    p.x = t.x + (p.x - t.x) / d * t.r;
                    p.y = t.y + (p.y - t.y) / d * t.r;
                    p.leave_orbit_pub();
                }
                TerrainKind::GalacticBarrier if tick >= p.wormhole_until => {
                    if seg_dist(p.x, p.y, t.x, t.y, t.x2, t.y2) < BARRIER_REACH {
                        p.wormhole_until = tick + 3 * UPS as u32;
                        barrier_hits.push(j);
                    }
                }
                TerrainKind::FluidicRift if d < t.r && tick >= p.wormhole_until => {
                    p.wormhole_until = tick + 5 * UPS as u32;
                    rifts.push(j);
                }
                TerrainKind::AbandonedStation if d < t.r && p.speed <= 2 => {
                    p.damage = (p.damage - s.repair * 2.0 / 1000.0).max(0.0);
                    p.shield = (p.shield + s.repair * 2.0 / 1000.0).min(s.max_shield);
                    p.fuel = (p.fuel + 6.0 * s.recharge).min(s.max_fuel);
                }
                TerrainKind::BriarPatch if d < t.r => {
                    p.hidden = true;
                    p.no_transport = true;
                    p.desired_speed = p.desired_speed.min(BRIAR_SPEED);
                    // Metaphasic radiation: the hull knits itself back together.
                    p.damage = (p.damage - BRIAR_HEAL).max(0.0);
                }
                TerrainKind::Badlands if d < t.r => {
                    p.hidden = true;
                    if t.timer == 0 && t.mines.iter().any(|&(ex, ey)| dist(p.x, p.y, ex, ey) < ERUPTION_REACH) {
                        hurt.push((j, ERUPTION_DAMAGE, format!("was caught in a plasma storm in {}", t.name)));
                    }
                }
                TerrainKind::Nexus if tick >= p.wormhole_until && seg_dist(p.x, p.y, t.x, t.y, t.x2, t.y2) < NEXUS_REACH => nexus.push(j),
                TerrainKind::TykensRift if d < t.r => {
                    p.fuel = (p.fuel - TYKEN_DRAIN).max(0.0);
                    if p.orbiting.is_none() && d > 1.0 {
                        p.x += (t.x - p.x) / d * TYKEN_PULL.min(d);
                        p.y += (t.y - p.y) / d * TYKEN_PULL.min(d);
                    }
                    if (tick + j as u32) % 30 == 0 {
                        warns.push((id, format!("{} is draining your power! Detonate a torpedo at its centre to collapse it", t.name)));
                    }
                }
                TerrainKind::GenesisPlanet if d < GENESIS_REACH => {
                    if d < t.r && d > 1.0 {
                        // Protomatter rock: solid.
                        p.x = t.x + (p.x - t.x) / d * t.r;
                        p.y = t.y + (p.y - t.y) / d * t.r;
                        p.leave_orbit_pub();
                    }
                    if p.speed <= 2 && (tick + j as u32) % GENESIS_GROW == 0 && p.armies < s.max_armies {
                        p.armies += 1;
                        warns.push((id, format!("Colonists from {} beam aboard (+1 army)", t.name)));
                    }
                    let left = GENESIS_LIFE - t.timer;
                    if left == 15 * UPS as i32 || left == 5 * UPS as i32 {
                        warns.push((id, format!("{} is breaking up! It blows in {} seconds", t.name, left / UPS as i32)));
                    }
                }
                TerrainKind::QuantumFilaments if d < t.r + FILAMENT_LEN => {
                    if let Some(k) = (0..t.mines.len() / 2).find(|&k| {
                        let (a, b) = (t.mines[2 * k], t.mines[2 * k + 1]);
                        seg_dist(p.x, p.y, a.0, a.1, b.0, b.1) < FILAMENT_REACH
                    }) {
                        if !filament_hits.iter().any(|h| h.0 == ti && h.1 == k) {
                            filament_hits.push((ti, k, j));
                        }
                    }
                }
                TerrainKind::HekarasCorridor if p.speed > HEKARAS_SPEED && p.orbiting.is_none() => {
                    if seg_dist(p.x, p.y, t.x, t.y, t.x2, t.y2) < t.r {
                        if (tick + j as u32) % 8 == 0 {
                            let (hx, hy) = dir_vec(p.dir);
                            new_tears.push(Tear { x: p.x - hx * 900.0, y: p.y - hy * 900.0, ttl: TEAR_TTL, owner: None, team: Team::Ind });
                        }
                        if (tick + j as u32) % 40 == 0 {
                            warns.push((id, format!("Above warp {} in {} your warp field is tearing subspace!", HEKARAS_SPEED, t.name)));
                        }
                    }
                }
                TerrainKind::OmegaZone if d < t.r => {
                    // Subspace itself is gone: no warp, no transwarp.
                    p.desired_speed = p.desired_speed.min(2);
                    p.jump_at = None;
                    if d < OMEGA_CORE && p.speed <= 2 {
                        omega_holders.push((ti, j));
                    }
                }
                TerrainKind::NeutralZone if p.cloaked && seg_dist(p.x, p.y, t.x, t.y, t.x2, t.y2) < t.r => p.detected = true,
                TerrainKind::GuardianOfForever if d < t.r && tick >= p.wormhole_until => guardian.push(j),
                TerrainKind::ThetaRadiation if d < t.r => {
                    if (tick + j as u32) % THETA_SICKNESS == 0 && p.armies > 0 {
                        p.armies -= 1;
                        warns.push((id, "Theta radiation sickness: one of the armies aboard has died".into()));
                    }
                    hurt.push((j, 0.3, "was poisoned by theta radiation".into()));
                }
                _ => {}
            }
        }
    }
    // Metaphasic shields shrug off every hazard.
    let meta = |j: usize| players[j].techs.contains(&Tech::MetaphasicShields);
    hurt.retain(|h| !meta(h.0));
    doomed.retain(|h| !meta(h.0));
    for (j, near) in near_wreck.iter().enumerate() {
        if !near {
            players[j].salvage = 0;
        }
    }

    // Torpedoes: soaked up by asteroids, dragged into the black hole,
    // slowed by chronitons, bent by the magnetar, stopped by rock and the
    // barrier.
    for t in world.torps.iter_mut().filter(|t| t.explode == 0) {
        for f in world.terrain.iter() {
            let d = dist(t.x, t.y, f.x, f.y);
            match f.kind {
                TerrainKind::Asteroids if d < f.r && rng.gen_bool(0.06) => t.fuse = 0,
                TerrainKind::ChronitonField if d < f.r => {
                    let (vx, vy) = dir_vec(t.dir);
                    t.x -= vx * t.speed * 0.5;
                    t.y -= vy * t.speed * 0.5;
                }
                TerrainKind::Magnetar if d < f.r => t.dir = (t.dir + 1.5).rem_euclid(256.0),
                TerrainKind::Planetoid if d < f.r => t.fuse = 0,
                TerrainKind::GenesisPlanet if d < f.r => t.fuse = 0,
                TerrainKind::GalacticBarrier if seg_dist(t.x, t.y, f.x, f.y, f.x2, f.y2) < BARRIER_REACH => t.fuse = 0,
                TerrainKind::GreatBlackHole if d < f.r => {
                    if d < GREAT_HORIZON {
                        t.fuse = 0;
                    } else {
                        let pull = great_pull(d, f.r);
                        t.x += (f.x - t.x) / d * pull;
                        t.y += (f.y - t.y) / d * pull;
                    }
                }
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

    // A torpedo going off at the heart of Tyken's Rift collapses it.
    let mut collapsed: Vec<(usize, u8)> = Vec::new();
    for t in world.torps.iter().filter(|t| t.explode == 1 && t.damage > 0.0) {
        for (k, f) in world.terrain.iter().enumerate() {
            if f.kind == TerrainKind::TykensRift && f.visible() && dist(t.x, t.y, f.x, f.y) < TYKEN_CORE && !collapsed.iter().any(|c| c.0 == k) {
                collapsed.push((k, t.owner));
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
    // Mines: each goes off once, and a new one is laid elsewhere in the field.
    for (ti, m, j) in mine_hits {
        let (mx, my) = world.terrain[ti].mines[m];
        flash(world, mx, my, j as u8);
        let (tx, ty, r) = (world.terrain[ti].x, world.terrain[ti].y, world.terrain[ti].r);
        world.terrain[ti].mines[m] = scatter(tx, ty, r, 1)[0];
        let name = world.terrain[ti].name.clone();
        world.warn(j as u8, format!("Mine! You've strayed into the {}", name));
        world.inflict(j, 40.0, None, format!("hit a mine in the {}", name));
    }
    for j in barrier_hits {
        let p = &mut world.players[j];
        p.fuel = (p.fuel - 2000.0).max(0.0);
        world.warn(j as u8, "Crossing the galactic barrier: power drained, hull damaged");
        world.hit_kind = super::world::HitKind::Polaron;
        world.inflict(j, 25.0, None, "was torn apart crossing the galactic barrier".into());
    }
    for (k, owner) in collapsed {
        world.terrain[k].respawn = 90 * UPS as i32;
        let who = world.players.get(owner as usize).filter(|p| p.in_use).map_or("A torpedo".to_string(), |p| p.label());
        let name = world.terrain[k].name.clone();
        world.god(format!("{} detonates a torpedo at the heart of {} and it collapses!", who, name));
    }
    for j in nexus {
        world.send_away(j, Away::Nexus, NEXUS_TICKS);
        let who = world.players[j].label();
        world.warn(j as u8, "You are swept into the Nexus... time has no meaning here. (20 s)");
        world.god(format!("{} is swept into the Nexus", who));
    }
    for (ti, k, j) in filament_hits {
        let (tx, ty, r) = (world.terrain[ti].x, world.terrain[ti].y, world.terrain[ti].r);
        let [a, b] = filament(tx, ty, r);
        (world.terrain[ti].mines[2 * k], world.terrain[ti].mines[2 * k + 1]) = (a, b);
        let (px, py) = (world.players[j].x, world.players[j].y);
        flash(world, px, py, j as u8);
        if world.features.subsystems {
            let sy = System::ALL[rng.gen_range(0..System::ALL.len())];
            world.warn(j as u8, format!("A quantum filament slices through the hull: {} knocked out!", sy.name().to_lowercase()));
            world.damage_system(j, sy, 100.0);
        } else {
            world.players[j].shields_up = false;
            world.warn(j as u8, "A quantum filament slices through the hull! Shields down");
            world.inflict(j, 20.0, None, "was cut apart by a quantum filament".into());
        }
    }
    world.tears.extend(new_tears);
    // The Badlands' plasma eruptions (to look at; the damage is done above).
    let eruptions: Vec<(f64, f64)> = world
        .terrain
        .iter()
        .filter(|t| t.kind == TerrainKind::Badlands && t.visible() && t.timer == 0)
        .filter(|t| world.players.iter().any(|p| p.alive() && p.faction.is_none() && dist(p.x, p.y, t.x, t.y) < t.r + 15_000.0))
        .flat_map(|t| t.mines.clone())
        .collect();
    for (x, y) in eruptions {
        flash(world, x, y, 0);
    }
    // The Genesis Planet tears itself apart.
    for k in 0..world.terrain.len() {
        let t = &world.terrain[k];
        if t.kind != TerrainKind::GenesisPlanet || !t.visible() || t.timer < GENESIS_LIFE {
            continue;
        }
        let (x, y, name) = (t.x, t.y, t.name.clone());
        world.terrain[k].timer = 0;
        world.terrain[k].respawn = 60 * UPS as i32;
        for (fx, fy) in scatter(x, y, GENESIS_R * 2.0, 6) {
            flash(world, fx, fy, 0);
        }
        world.god(format!("The protomatter at its heart gives way, and {} tears itself apart!", name));
        let caught: Vec<(usize, f64)> = (0..MAXPLAYER)
            .filter(|&j| world.players[j].alive() && world.players[j].faction.is_none() && world.players[j].away.is_none())
            .map(|j| (j, dist(world.players[j].x, world.players[j].y, x, y)))
            .filter(|&(_, d)| d < GENESIS_BLAST)
            .collect();
        for (j, d) in caught {
            if !world.players[j].techs.contains(&Tech::MetaphasicShields) {
                world.inflict(j, GENESIS_DAMAGE * (1.0 - 0.6 * d / GENESIS_BLAST), None, format!("was caught in the death of {}", name));
            }
        }
    }
    // Omega: neutralised by a ship holding at its core, or it grows.
    for k in 0..world.terrain.len() {
        if world.terrain[k].kind != TerrainKind::OmegaZone || !world.terrain[k].visible() {
            continue;
        }
        let Some(&(_, j)) = omega_holders.iter().find(|h| h.0 == k) else {
            world.terrain[k].timer = 0;
            continue;
        };
        world.terrain[k].timer += 1;
        if world.terrain[k].timer == 1 {
            world.warn(j as u8, "Holding at the Omega molecule: keep still for 10 seconds to break it down");
        }
        if world.terrain[k].timer >= OMEGA_TICKS {
            let t = &mut world.terrain[k];
            (t.timer, t.respawn, t.r) = (0, 120 * UPS as i32, OMEGA_R);
            let p = &mut world.players[j];
            p.kills += OMEGA_REWARD;
            p.total_kills += OMEGA_REWARD;
            let who = p.label();
            world.events.push(GameEvent::Honour { player: j as u8, text: "Neutralised an Omega molecule".into() });
            world.god(format!("{} neutralises the Omega molecule! Subspace begins to heal (+{} kills)", who, OMEGA_REWARD));
        }
    }
    // The Guardian of Forever: back to where you were.
    for j in guardian {
        let Some(&then) = world.players[j].past.front() else { continue };
        let secs = world.players[j].past.len() as u32 * PAST_EVERY / UPS as u32;
        world.players[j].leave_orbit_pub();
        let p = &mut world.players[j];
        (p.x, p.y, p.dir, p.desired_dir, p.speed, p.desired_speed) = (then.x, then.y, then.dir, then.dir, then.speed, then.speed);
        (p.fuel, p.shield, p.damage, p.etemp, p.wtemp) = (then.fuel, then.shield, then.damage, then.etemp, then.wtemp);
        (p.lock, p.tractor) = (super::world::Lock::None, None);
        p.past.clear();
        p.wormhole_until = tick + 10 * UPS as u32;
        world.warn(j as u8, format!("\"A question. Since before your sun burned hot in space...\" The Guardian of Forever sends you back {} seconds", secs));
    }
    for j in rifts {
        let (x, y) = random_open_spot(world);
        let p = &mut world.players[j];
        (p.x, p.y) = (x, y);
        p.leave_orbit_pub();
        p.lock = super::world::Lock::None;
        p.tractor = None;
        world.warn(j as u8, "The fluidic rift flings you across the galaxy!");
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

    /// Every galaxy gets exactly ten kinds of terrain, a different mix each
    /// time (all 21 turn up over enough galaxies), placed clear of planets.
    #[test]
    fn galaxy_has_ten_kinds_of_terrain_clear_of_planets() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..80 {
            let w = World::with_features(Features { terrain: true, ..Features::default() });
            let mut kinds: Vec<TerrainKind> = w.terrain.iter().map(|t| t.kind).collect();
            kinds.sort_by_key(|k| *k as u8);
            kinds.dedup();
            assert_eq!(kinds.len(), ACTIVE_KINDS, "{:?}", kinds);
            seen.extend(kinds);
            for t in w.terrain.iter().filter(|t| matches!(t.kind, TerrainKind::Nebula | TerrainKind::BlackHole | TerrainKind::Star | TerrainKind::Pulsar | TerrainKind::Planetoid | TerrainKind::Minefield)) {
                for pl in &w.planets {
                    assert!(dist(t.x, t.y, pl.x, pl.y) > t.r, "{} sits on {}", t.name, pl.name);
                }
            }
        }
        assert_eq!(seen.len(), ALL_KINDS.len(), "every kind turns up sometimes");
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
    fn great_black_hole_drags_ships_in() {
        let (mut w, k) = setup(58_000.0, 50_000.0);
        put(&mut w, TerrainKind::GreatBlackHole, 50_000.0, 50_000.0, GREAT_HOLE_R);
        tick(&mut w);
        let d = dist(w.players[k].x, w.players[k].y, 50_000.0, 50_000.0);
        assert!(d < 8000.0 - 10.0, "pulled in");
        // It pulls far harder than the ordinary kind, and from much further out.
        assert!(great_pull(3000.0, GREAT_HOLE_R) > 8.0 + 70.0 * (1.0f64 - 3000.0 / 7000.0).powi(2));
        // Deep inside, only a fast ship gets out.
        assert!(great_pull(2000.0, GREAT_HOLE_R) > 6.0 * WARP1);
        w.players[k].x = 50_000.0 + GREAT_HORIZON - 100.0;
        w.players[k].y = 50_000.0;
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

    #[test]
    fn minefield_mines_go_off_and_are_relaid() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::Minefield, 50_000.0, 50_000.0, MINEFIELD_R);
        w.terrain[t].mines = vec![(50_100.0, 50_000.0)];
        w.players[k].shields_up = false;
        tick(&mut w);
        assert_eq!(w.players[k].damage, 40.0);
        assert_ne!(w.terrain[t].mines[0], (50_100.0, 50_000.0), "a new mine was laid");
    }

    #[test]
    fn chroniton_field_halves_speed() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        put(&mut w, TerrainKind::ChronitonField, 50_000.0, 50_000.0, 5000.0);
        (w.players[k].speed, w.players[k].dir) = (8, 64.0);
        tick(&mut w);
        assert!((w.players[k].x - (50_000.0 - 4.0 * WARP1)).abs() < 1e-6, "half of this tick's move undone");
    }

    #[test]
    fn graviton_eddy_sweeps_ships_round() {
        let (mut w, k) = setup(53_000.0, 50_000.0);
        put(&mut w, TerrainKind::GravitonEddy, 50_000.0, 50_000.0, 6000.0);
        tick(&mut w);
        assert!(w.players[k].y > 50_000.0, "pushed sideways around the centre");
    }

    #[test]
    fn magnetar_breaks_tractors_and_bends_torps() {
        let (mut w, k) = setup(52_000.0, 50_000.0);
        put(&mut w, TerrainKind::Magnetar, 50_000.0, 50_000.0, 6000.0);
        w.players[k].tractor = Some((5, false));
        w.handle(k as u8, ClientMsg::Torp(0));
        let dir = w.torps[0].dir;
        tick(&mut w);
        assert!(w.players[k].tractor.is_none());
        assert_ne!(w.torps[0].dir, dir, "the torpedo curves");
    }

    #[test]
    fn metreon_gas_ignites_when_you_fire() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        put(&mut w, TerrainKind::MetreonCloud, 50_000.0, 50_000.0, 4500.0);
        w.players[k].shields_up = false;
        w.handle(k as u8, ClientMsg::Torp(0));
        assert_eq!(w.players[k].damage, 30.0, "the shooter is caught in it");
    }

    #[test]
    fn tetryons_strip_shields() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        put(&mut w, TerrainKind::TetryonField, 50_000.0, 50_000.0, 5000.0);
        let before = w.players[k].shield;
        for _ in 0..20 {
            w.tick();
        }
        assert!(w.players[k].shield < before - 20.0);
    }

    #[test]
    fn planetoids_are_solid() {
        let (mut w, k) = setup(50_200.0, 50_000.0);
        put(&mut w, TerrainKind::Planetoid, 50_000.0, 50_000.0, 1500.0);
        tick(&mut w);
        assert!((dist(w.players[k].x, w.players[k].y, 50_000.0, 50_000.0) - 1500.0).abs() < 1.0);
        let (mut w, k) = setup(47_000.0, 50_000.0);
        put(&mut w, TerrainKind::Planetoid, 50_000.0, 50_000.0, 1500.0);
        w.handle(k as u8, ClientMsg::Torp(64));
        for _ in 0..10 {
            w.tick();
        }
        assert!(w.torps.iter().all(|t| t.x < 49_000.0 || t.explode > 0), "stopped by the rock");
    }

    #[test]
    fn galactic_barrier_costs_to_cross() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::GalacticBarrier, 50_000.0, 40_000.0, BARRIER_REACH);
        (w.terrain[t].x2, w.terrain[t].y2) = (50_000.0, 60_000.0);
        let fuel = w.players[k].fuel;
        tick(&mut w);
        assert!(w.players[k].fuel <= fuel - 2000.0 && w.players[k].damage >= 25.0);
    }

    #[test]
    fn fluidic_rift_flings_you_away() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        put(&mut w, TerrainKind::FluidicRift, 50_000.0, 50_000.0, 700.0);
        tick(&mut w);
        assert!(dist(w.players[k].x, w.players[k].y, 50_000.0, 50_000.0) > 1000.0);
    }

    #[test]
    fn briar_patch_hides_slows_heals_and_jams_transporters() {
        let mut w = World::with_features(Features { terrain: true, boarding: true, ..Features::default() });
        let id = w.add_player("Kirk", false).unwrap();
        w.join(id, Team::Fed, ShipType::Cruiser).unwrap();
        let k = id as usize;
        (w.players[k].x, w.players[k].y) = (50_000.0, 50_000.0);
        w.terrain.clear();
        put(&mut w, TerrainKind::BriarPatch, 50_000.0, 50_000.0, 6500.0);
        (w.players[k].desired_speed, w.players[k].damage, w.players[k].armies) = (9, 40.0, 4);
        tick(&mut w);
        let p = &w.players[k];
        assert!(p.hidden && p.no_transport && p.desired_speed == BRIAR_SPEED && p.damage < 40.0);
        let r = w.add_player("Tal", false).unwrap();
        w.join(r, Team::Rom, ShipType::Cruiser).unwrap();
        (w.players[r as usize].x, w.players[r as usize].y, w.players[r as usize].shields_up) = (50_500.0, 50_000.0, false);
        w.handle(id, ClientMsg::Board(r));
        assert!(w.boardings.is_empty(), "transporters jammed");
    }

    #[test]
    fn badlands_erupt() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::Badlands, 50_000.0, 50_000.0, 7000.0);
        w.players[k].shields_up = false;
        w.terrain[t].timer = BADLANDS_PERIOD - 1;
        // Make sure an eruption lands on us.
        let mut hurt = false;
        for _ in 0..40 {
            w.terrain[t].timer = BADLANDS_PERIOD - 1;
            tick(&mut w);
            assert!(w.players[k].hidden);
            if w.terrain[t].mines.iter().any(|&(ex, ey)| dist(ex, ey, 50_000.0, 50_000.0) < ERUPTION_REACH) {
                hurt = true;
                break;
            }
        }
        if hurt {
            assert!(w.players[k].damage >= ERUPTION_DAMAGE);
        }
        assert!(w.torps.iter().any(|t| t.explode > 0), "eruptions flash");
    }

    #[test]
    fn nexus_takes_ships_away_and_sends_them_back_whole() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::Nexus, 40_000.0, 50_000.0, NEXUS_REACH);
        (w.terrain[t].x2, w.terrain[t].y2) = (60_000.0, 50_000.0);
        w.players[k].damage = 60.0;
        tick(&mut w);
        assert!(w.players[k].away.is_some() && !w.players[k].alive(), "swept in");
        let r = w.add_player("Tal", false).unwrap();
        w.join(r, Team::Rom, ShipType::Cruiser).unwrap();
        assert!(!w.frame_for(r).players.iter().any(|p| p.id == k as u8), "gone from sight");
        assert!(w.frame_for(k as u8).players.iter().any(|p| p.id == k as u8), "but it can see itself");
        w.inflict(k, 500.0, None, "test".into());
        assert_eq!(w.players[k].state, PState::Alive, "untouchable in there");
        w.terrain.clear();
        for _ in 0..NEXUS_TICKS + 2 {
            w.tick();
        }
        assert!(w.players[k].alive());
        assert_eq!(w.players[k].damage, 0.0, "made whole");
    }

    #[test]
    fn tykens_rift_drains_and_collapses() {
        let (mut w, k) = setup(51_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::TykensRift, 50_000.0, 50_000.0, TYKEN_R);
        let fuel = w.players[k].fuel;
        tick(&mut w);
        assert!(w.players[k].fuel <= fuel - TYKEN_DRAIN && w.players[k].x < 51_000.0);
        // A torpedo detonated at the centre collapses it.
        w.players[k].x = 50_000.0 + TYKEN_CORE * 0.5;
        w.handle(k as u8, ClientMsg::Torp(0));
        w.handle(k as u8, ClientMsg::DetOwn);
        for _ in 0..3 {
            w.tick();
        }
        assert!(!w.terrain[t].visible(), "collapsed");
    }

    #[test]
    fn genesis_planet_gives_armies_then_blows() {
        let (mut w, k) = setup(52_500.0, 50_000.0);
        let t = put(&mut w, TerrainKind::GenesisPlanet, 50_000.0, 50_000.0, GENESIS_R);
        (w.players[k].speed, w.players[k].armies, w.players[k].shields_up) = (0, 0, false);
        for _ in 0..GENESIS_GROW * 3 {
            tick(&mut w);
        }
        assert!(w.players[k].armies >= 2, "colonists aboard: {}", w.players[k].armies);
        assert!(w.terrain[t].info().phase > 0);
        w.terrain[t].timer = GENESIS_LIFE - 1;
        tick(&mut w);
        assert!(w.players[k].damage > 20.0, "caught in the blast");
        assert!(!w.terrain[t].visible());
    }

    #[test]
    fn quantum_filaments_knock_out_systems() {
        let mut w = World::with_features(Features { terrain: true, subsystems: true, ..Features::default() });
        let id = w.add_player("Kirk", false).unwrap();
        w.join(id, Team::Fed, ShipType::Cruiser).unwrap();
        let k = id as usize;
        w.terrain.clear();
        (w.players[k].x, w.players[k].y) = (50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::QuantumFilaments, 50_000.0, 50_000.0, FILAMENT_R);
        w.terrain[t].mines = vec![(49_000.0, 50_000.0), (51_000.0, 50_000.0)];
        tick(&mut w);
        assert!(w.players[k].systems.iter().any(|&h| h == 0.0), "a system is out");
        assert_ne!(w.terrain[t].mines[0], (49_000.0, 50_000.0), "the strand moves on");
    }

    #[test]
    fn hekaras_corridor_tears_subspace_above_warp_five() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::HekarasCorridor, 30_000.0, 50_000.0, HEKARAS_WIDTH / 2.0);
        (w.terrain[t].x2, w.terrain[t].y2) = (70_000.0, 50_000.0);
        w.players[k].speed = 5;
        for _ in 0..16 {
            tick(&mut w);
        }
        assert!(w.tears.is_empty(), "warp 5 is safe");
        w.players[k].speed = 8;
        for _ in 0..16 {
            tick(&mut w);
        }
        assert!(!w.tears.is_empty());
        assert!(w.tears.iter().all(|t| t.owner.is_none()));
    }

    #[test]
    fn omega_zone_kills_warp_and_can_be_neutralised() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::OmegaZone, 50_000.0, 50_000.0, OMEGA_R);
        w.players[k].desired_speed = 9;
        w.players[k].jump_at = Some(999);
        tick(&mut w);
        assert!(w.players[k].desired_speed <= 2 && w.players[k].jump_at.is_none());
        assert!(w.terrain[t].r > OMEGA_R, "it grows");
        let kills = w.players[k].kills;
        w.players[k].speed = 0;
        for _ in 0..OMEGA_TICKS {
            tick(&mut w);
        }
        assert!(!w.terrain[t].visible(), "neutralised");
        assert_eq!(w.players[k].kills, kills + OMEGA_REWARD);
    }

    #[test]
    fn neutral_zone_shows_cloaks_and_punishes_fire() {
        let mut w = World::with_features(Features { terrain: true, diplomacy: true, ..Features::default() });
        w.terrain.clear();
        let id = w.add_player("Kirk", false).unwrap();
        w.join(id, Team::Fed, ShipType::Cruiser).unwrap();
        let k = id as usize;
        (w.players[k].x, w.players[k].y) = (50_000.0, 50_000.0);
        let t = put(&mut w, TerrainKind::NeutralZone, 50_000.0, 30_000.0, NEUTRAL_WIDTH / 2.0);
        (w.terrain[t].x2, w.terrain[t].y2, w.terrain[t].sides) = (50_000.0, 70_000.0, Some((Team::Fed, Team::Rom)));
        w.players[k].cloaked = true;
        tick(&mut w);
        assert!(w.players[k].detected, "cloaks show in the zone");
        w.players[k].cloaked = false;
        w.treaties.push((Team::Fed, Team::Rom));
        w.players[k].shields_up = false;
        w.handle(id, ClientMsg::Phaser(0));
        assert!(w.players[k].damage >= NEUTRAL_FIRE, "the outposts open fire");
        assert!(w.breaking.iter().any(|&(a, b, _)| a == Team::Fed && b == Team::Rom), "the treaty is broken");
    }

    #[test]
    fn guardian_of_forever_sends_you_back() {
        let (mut w, k) = setup(20_000.0, 20_000.0);
        put(&mut w, TerrainKind::GuardianOfForever, 50_000.0, 50_000.0, GUARDIAN_R);
        w.players[k].fuel = 3000.0;
        for _ in 0..PAST_EVERY * 4 {
            w.tick += 1;
            tick(&mut w);
        }
        assert!(w.players[k].past.len() >= 4);
        (w.players[k].x, w.players[k].y, w.players[k].fuel, w.players[k].damage) = (50_000.0, 50_000.0, 100.0, 70.0);
        tick(&mut w);
        let p = &w.players[k];
        assert!(dist(p.x, p.y, 20_000.0, 20_000.0) < 100.0, "back where it was");
        assert!(p.fuel > 2000.0 && p.damage == 0.0, "and how it was");
    }

    #[test]
    fn theta_radiation_sickens_armies_and_fades() {
        let (mut w, k) = setup(50_000.0, 50_000.0);
        w.terrain.push(Terrain::theta((50_000.0, 50_000.0)));
        w.players[k].armies = 3;
        for _ in 0..THETA_SICKNESS * 2 {
            w.tick += 1;
            tick(&mut w);
        }
        assert!(w.players[k].armies < 3);
        w.terrain[0].timer = 1;
        tick(&mut w);
        assert!(w.terrain.is_empty(), "it fades away");
    }

    #[test]
    fn abandoned_station_repairs_anyone() {
        let (mut w, k) = setup(50_500.0, 50_000.0);
        put(&mut w, TerrainKind::AbandonedStation, 50_000.0, 50_000.0, STATION_REACH);
        (w.players[k].damage, w.players[k].fuel) = (50.0, 100.0);
        tick(&mut w);
        assert!(w.players[k].damage < 50.0 && w.players[k].fuel > 100.0);
    }
}
