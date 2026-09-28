//! Alien incursions (the `--aliens` option): episodes from Star Trek that
//! drop into the galaxy every so often. At most two are active at once.
//!
//! Aliens are server-controlled ships on `Team::Ind`, so they are hostile to
//! all four empires and take damage and give kill credit like any ship.
//! Their special powers (webs, planet eating, assimilation...) live here.

use super::bot::lead;
use super::terrain::Terrain;
use super::world::{is_tempest_minion, Dest, GameEvent, Lock, Loot, Outgoing, PhaserShot, RingSection, Ringworld, TempestWeb, Web, World, KZIN_ARMIES};
use crate::consts::*;
use crate::proto::{ChatMsg, ClientMsg, MsgKind, PState, PhaserInfo, TempestShape, TerrainKind, ZoneInfo, ZoneKind};
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashMap;
use std::f64::consts::TAU;

pub struct AlienConfig {
    /// Which incursions may happen (empty = aliens off).
    pub kinds: Vec<Faction>,
    /// Average seconds between incursions.
    pub interval: u64,
}

const MAX_ACTIVE: usize = 2;
const MAX_CUBES: usize = 3;

struct Event {
    kind: Faction,
    ships: Vec<u8>,
    started: u32,
    /// Per-ship planet objectives.
    goals: HashMap<u8, usize>,
    /// Borg: cube -> (victim, ticks held in the tractor beam).
    holds: HashMap<u8, (u8, i32)>,
    waypoint: (f64, f64),
    /// Tholians: the three planets the web is strung between.
    corners: Vec<usize>,
    /// Tholians: where each ship laid its last strand.
    trail: HashMap<u8, (f64, f64)>,
    lost_any: bool,
    /// V'Ger merge progress (ship, ticks at the core); 8472 beam charge.
    progress: HashMap<u8, i32>,
    /// Whale probe: ticks spent at the current planet.
    dwell: i32,
    /// Hirogen: the ship being hunted.
    prey: Option<u8>,
    /// Swarm: swarm ship -> the ship it has latched onto.
    latched: HashMap<u8, u8>,
    /// Q: the trial under way.
    trial: Option<Trial>,
    /// Tempest: the climbers on its web.
    climbers: HashMap<u8, Climber>,
    /// Tempest: flipper -> (ship it has grabbed, ticks held).
    grabs: HashMap<u8, (u8, i32)>,
    /// Tempest: the current wave, the climbers still to come out of the
    /// core, and when the next one (or the next wave) is due.
    level: u8,
    queue: Vec<ShipType>,
    next_at: u32,
    /// Metrons: the two champions in the arena.
    duel: Option<(u8, u8)>,
    /// Pakleds: what each clunker has taken.
    stolen: HashMap<u8, Stolen>,
    /// Dyson sphere: the ships shut inside it.
    inside: Vec<u8>,
    /// Dyson sphere: when the tractor beam can grab another ship.
    cooldown: u32,
}

/// Something climbing the Tempest's web.
#[derive(Clone, Copy, Debug)]
struct Climber {
    kind: ShipType,
    lane: i32,
    /// 0 at the core, 1 at the rim.
    prog: f64,
    /// Which way a fuseball drifts along its spoke (+1 out, -1 in).
    dir: i32,
    /// A flip under way: the lane it's flipping into, and how far (0..1).
    flip: Option<(i32, f64)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TrialKind {
    /// Lose no more than two of your planets.
    Hold,
    /// Bring Q five armies.
    Tribute,
    /// Destroy Q's champion (only your weapons can hurt it).
    Champion,
}

struct Trial {
    team: Team,
    kind: TrialKind,
    deadline: u32,
    /// Hold: the planets the empire owned when the trial began.
    owned: Vec<usize>,
    /// Tribute: armies delivered so far.
    tribute: u32,
}

pub struct Director {
    cfg: AlienConfig,
    events: Vec<Event>,
    next_spawn: u32,
    /// Jem'Hadar: the wormhole opens (warning) a few seconds before they arrive.
    wormhole: Option<(u32, f64, f64)>,
    /// The Kzinti, while their Ringworld is here.
    kzinti: Option<Kzinti>,
    /// The Ringworld has broken free and jumped away: it never comes back.
    ring_gone: bool,
}

fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

fn announce(world: &mut World, text: impl Into<String>) {
    world.outbox.push(Outgoing {
        dest: Dest::All,
        msg: ChatMsg { kind: MsgKind::System, from: "ALERT".into(), text: text.into() },
    });
}

fn duration(kind: Faction) -> u32 {
    let mins = match kind {
        Faction::Khan | Faction::Gorn | Faction::Mirror | Faction::Tholian => 6,
        Faction::Borg | Faction::Vger | Faction::Species8472 | Faction::Tribbles | Faction::Hirogen => 5,
        Faction::TenC | Faction::SphereBuilders | Faction::Dyson => 5,
        _ => 4,
    };
    mins * 60 * UPS as u32
}

impl Director {
    pub fn new(cfg: AlienConfig) -> Director {
        let first = cfg.interval.clamp(1, 120) / 2;
        Director { cfg, events: Vec::new(), next_spawn: first as u32 * UPS as u32, wormhole: None, kzinti: None, ring_gone: false }
    }

    pub fn tick(&mut self, world: &mut World) {
        if self.cfg.kinds.is_empty() {
            return;
        }
        world.zones.clear();
        // A galaxy reset sends everyone home, aliens included.
        if world.reset_timer > 0 {
            for e in self.events.drain(..) {
                for id in e.ships {
                    world.remove_player(id);
                }
            }
            return;
        }
        self.cleanup(world);
        if let Some((at, x, y)) = self.wormhole {
            if world.tick >= at {
                self.wormhole = None;
                self.spawn_kind(world, Faction::JemHadar, Some((x, y)));
            }
        }
        if world.tick >= self.next_spawn {
            if self.events.len() < MAX_ACTIVE {
                self.spawn(world);
            }
            let mut rng = rand::thread_rng();
            let secs = self.cfg.interval as f64 * rng.gen_range(0.7..1.3);
            self.next_spawn = world.tick + (secs * UPS as f64) as u32;
        }
        let mut events = std::mem::take(&mut self.events);
        for e in events.iter_mut() {
            run_event(world, e);
        }
        self.events = events;
        self.run_kzinti(world);
    }

    /// Drop destroyed ships and finish incursions that are over.
    fn cleanup(&mut self, world: &mut World) {
        let mut ended = Vec::new();
        for (k, e) in self.events.iter_mut().enumerate() {
            e.ships.retain(|&id| {
                let p = &world.players[id as usize];
                let gone = !p.in_use || matches!(p.state, PState::Dead | PState::Outfit);
                if gone {
                    world.remove_player(id);
                    e.lost_any = true;
                }
                !gone
            });
            let timed_out = world.tick.saturating_sub(e.started) > duration(e.kind);
            if beaten(world, e) || timed_out {
                ended.push(k);
            }
        }
        for k in ended.into_iter().rev() {
            let mut e = self.events.remove(k);
            if beaten(world, &e) {
                announce(world, defeat_text(e.kind));
            } else {
                for &id in &e.ships {
                    world.remove_player(id);
                }
                announce(world, withdraw_text(e.kind));
            }
            match e.kind {
                Faction::Tribbles => {
                    for pl in world.planets.iter_mut() {
                        pl.tribbles = false;
                    }
                    for p in world.players.iter_mut() {
                        p.tribbles = false;
                    }
                }
                Faction::Hirogen => {
                    for p in world.players.iter_mut() {
                        p.marked = false;
                    }
                }
                Faction::Tempest => {
                    world.tempest = None;
                    for p in world.players.iter_mut() {
                        p.zapped = false;
                    }
                    let left: Vec<u8> = world.players.iter().filter(|p| p.in_use && is_tempest_minion(p.ship)).map(|p| p.id).collect();
                    for id in left {
                        world.remove_player(id);
                    }
                }
                Faction::Nanites => {
                    for p in world.players.iter_mut() {
                        p.nanites = false;
                    }
                }
                Faction::Metrons => end_duel(world, &mut e),
                Faction::Pakleds => recover(world, &mut e),
                // Anomalies (and the Jenolan's wreck) go with whatever made them.
                Faction::SphereBuilders | Faction::Dyson => {
                    let players = &world.players;
                    world.terrain.retain(|t| t.owner.map_or(true, |o| players[o as usize].alive()));
                }
                _ => {}
            }
        }
    }

    fn spawn(&mut self, world: &mut World) {
        let mut rng = rand::thread_rng();
        let mut active: Vec<Faction> = self.events.iter().map(|e| e.kind).collect();
        if self.wormhole.is_some() {
            active.push(Faction::JemHadar);
        }
        // The Kzinti only ever come once: they stay until the Ringworld breaks free.
        if self.kzinti.is_some() || self.ring_gone {
            active.push(Faction::Kzinti);
        }
        let choices: Vec<Faction> = self.cfg.kinds.iter().copied().filter(|k| !active.contains(k)).collect();
        let Some(&kind) = choices.choose(&mut rng) else { return };
        if kind == Faction::JemHadar {
            // The wormhole opens first, giving everyone a few seconds' warning.
            let anchors: Vec<usize> = (0..world.planets.len()).filter(|&k| world.planets[k].flags & PL_HOME == 0).collect();
            let k = *anchors.choose(&mut rng).unwrap_or(&1);
            let (x, y) = (world.planets[k].x + rng.gen_range(-6000.0..6000.0), world.planets[k].y + rng.gen_range(-6000.0..6000.0));
            let name = world.planets[k].name;
            announce(world, format!("A wormhole is opening near {}! The Dominion is coming through.", name));
            self.wormhole = Some((world.tick + 6 * UPS as u32, x, y));
            return;
        }
        self.spawn_kind(world, kind, None);
    }

    fn spawn_kind(&mut self, world: &mut World, kind: Faction, at: Option<(f64, f64)>) {
        if kind == Faction::Kzinti {
            return self.arrive_kzinti(world);
        }
        let mut rng = rand::thread_rng();
        let free = world.players.iter().filter(|p| !p.in_use).count();
        let need = match kind {
            Faction::Khan | Faction::Tholian | Faction::Species8472 => 3,
            Faction::Gorn | Faction::Mirror => 4,
            Faction::JemHadar => 5,
            // As many as fit (up to eight).
            Faction::Swarm => 4,
            Faction::Changeling | Faction::Pakleds => 3,
            Faction::SphereBuilders => 4,
            Faction::Nanites => 0,
            // The core and a first climber; the rest climb out as slots free up.
            Faction::Tempest => 2,
            Faction::Tribbles => 0,
            _ => 1,
        };
        if free < need {
            return;
        }
        // Somewhere interesting: a planet that isn't a home world.
        let anchors: Vec<usize> = (0..world.planets.len())
            .filter(|&k| world.planets[k].flags & PL_HOME == 0 && world.planets[k].alien.is_none())
            .collect();
        let anchor = *anchors.choose(&mut rng).unwrap_or(&1);
        let (ax, ay) = (world.planets[anchor].x, world.planets[anchor].y);
        let edge = || {
            let mut rng = rand::thread_rng();
            match rng.gen_range(0..4) {
                0 => (rng.gen_range(5000.0..95000.0), 2000.0),
                1 => (rng.gen_range(5000.0..95000.0), 98000.0),
                2 => (2000.0, rng.gen_range(5000.0..95000.0)),
                _ => (98000.0, rng.gen_range(5000.0..95000.0)),
            }
        };
        let near = |x: f64, y: f64, spread: f64| {
            let mut rng = rand::thread_rng();
            (x + rng.gen_range(-spread..spread), y + rng.gen_range(-spread..spread))
        };
        let mut ships = Vec::new();
        let mut trial = None;
        let mut duel = None;
        let mut waypoint = (ax, ay);
        let mut inside = Vec::new();
        let mut corners = Vec::new();
        let mut spawn = |world: &mut World, name: &str, ship: ShipType, (x, y): (f64, f64), bounty: f64| {
            if let Some(id) = world.spawn_alien(name, kind, ship, x, y, bounty) {
                ships.push(id);
            }
        };
        let text = match kind {
            Faction::Khan => {
                let pl = &mut world.planets[anchor];
                let (name, old) = (pl.name, pl.owner);
                pl.owner = Team::Ind;
                pl.alien = Some(Faction::Khan);
                pl.armies = 40;
                pl.flags |= PL_REPAIR | PL_FUEL;
                for t in Team::PLAYABLE {
                    pl.known[t.idx()] = true;
                }
                world.check_genocide(old, Team::Ind);
                for n in ["Khan", "Joachim", "Otto"] {
                    spawn(world, n, ShipType::Augment, near(ax, ay, 2500.0), 10.0);
                }
                format!("Khan Noonien Singh has seized {}! His augments are coming for the Federation.", name)
            }
            Faction::Gorn => {
                let (x, y) = near(ax, ay, 6000.0);
                for _ in 0..4 {
                    spawn(world, "Gorn", ShipType::GornRaider, near(x, y, 1500.0), 5.0);
                }
                format!("Gorn raiders are attacking the colonies near {}!", world.planets[anchor].name)
            }
            Faction::Tholian => {
                // The web is strung between this planet and its two nearest neighbours.
                let mut others: Vec<usize> = (0..world.planets.len()).filter(|&k| k != anchor).collect();
                others.sort_by(|&a, &b| {
                    let (pa, pb) = (&world.planets[a], &world.planets[b]);
                    dist(ax, ay, pa.x, pa.y).total_cmp(&dist(ax, ay, pb.x, pb.y))
                });
                corners = vec![anchor, others[0], others[1]];
                for n in ["Loskene", "Tholian", "Tholian"] {
                    spawn(world, n, ShipType::TholianVessel, near(ax, ay, 3000.0), 5.0);
                }
                let names: Vec<&str> = corners.iter().map(|&k| world.planets[k].name).collect();
                format!(
                    "Tholian vessels are racing between {}, {} and {}, spinning a web across all three! Its strands burn any ship that touches them.",
                    names[0], names[1], names[2]
                )
            }
            Faction::Fesarius => {
                spawn(world, "Balok", ShipType::Fesarius, edge(), 30.0);
                "The Fesarius of the First Federation has entered the sector. Balok warns: you have ten of your minutes.".to_string()
            }
            Faction::Mirror => {
                let (x, y) = near(50_000.0, 50_000.0, 30_000.0);
                for (n, s) in [("Kirk", ShipType::Cruiser), ("Spock", ShipType::Cruiser), ("Sulu", ShipType::Destroyer), ("Chekov", ShipType::Battleship)] {
                    spawn(world, n, s, near(x, y, 1500.0), 5.0);
                }
                "A rift to the mirror universe has opened! The Terran Empire has come to conquer the sector.".to_string()
            }
            Faction::Doomsday => {
                spawn(world, "Planet Killer", ShipType::PlanetKiller, edge(), 40.0);
                "A planet killer has entered the galaxy and is devouring worlds! (Legend says a ship exploding in its maw can stop it.)".to_string()
            }
            Faction::Amoeba => {
                spawn(world, "Amoeba", ShipType::Amoeba, near(50_000.0, 50_000.0, 35_000.0), 25.0);
                "A giant space amoeba is drifting through the sector, draining ships of their energy.".to_string()
            }
            Faction::Vger => {
                // V'Ger enters from the far side of the galaxy from Earth.
                let (ex, ey) = (world.planets[0].x, world.planets[0].y);
                spawn(world, "V'Ger", ShipType::VgerCloud, (GWIDTH - ex, GWIDTH - ey), 0.0);
                "An immense energy cloud is heading for Earth, digitizing everything in its path. It calls itself V'Ger. Weapons are useless: someone must reach its core and join with it.".to_string()
            }
            Faction::Crystal => {
                let (x, y) = near(ax, ay, 8000.0);
                spawn(world, "Crystalline", ShipType::CrystalEntity, (x, y), 30.0);
                "The Crystalline Entity has appeared, stripping planets of all life. Only resonance can shatter it: phaser it from three ships at once!".to_string()
            }
            Faction::Probe => {
                spawn(world, "Probe", ShipType::WhaleProbe, edge(), 0.0);
                "An alien probe is crossing the galaxy, draining the power of every ship it passes. It is calling for humpback whales: bring it two armies to answer!".to_string()
            }
            Faction::Species8472 => {
                let (x, y) = edge();
                for _ in 0..3 {
                    spawn(world, "Bioship", ShipType::Bioship, near(x, y, 1500.0), 15.0);
                }
                "Species 8472 bioships have torn through from fluidic space! Conventional weapons are useless; only plasma torpedoes hurt them.".to_string()
            }
            Faction::JemHadar => {
                let (x, y) = at.unwrap_or((ax, ay));
                for _ in 0..5 {
                    spawn(world, "Jem'Hadar", ShipType::JemHadarFighter, near(x, y, 1200.0), 5.0);
                }
                "Jem'Hadar attack ships pour out of the wormhole! Victory is life!".to_string()
            }
            Faction::Borg => {
                spawn(world, "Locutus", ShipType::BorgCube, edge(), 30.0);
                "We are the Borg. Your biological and technological distinctiveness will be added to our own. Resistance is futile.".to_string()
            }
            Faction::Tribbles => {
                // Outbreak on a colony (never a Klingon one: tribbles hate Klingons).
                let colonies: Vec<usize> = (0..world.planets.len())
                    .filter(|&k| {
                        let pl = &world.planets[k];
                        Team::PLAYABLE.contains(&pl.owner) && pl.owner != Team::Kli && pl.flags & PL_HOME == 0 && pl.armies > 0
                    })
                    .collect();
                let Some(&k) = colonies.choose(&mut rng) else { return };
                world.planets[k].tribbles = true;
                format!(
                    "Tribbles have been found on {}! They breed fast and eat everything, and ships that orbit there carry them away. Tribbles hate Klingons: a Klingon ship in orbit drives them off.",
                    world.planets[k].name
                )
            }
            Faction::Chang => {
                spawn(world, "Chang", ShipType::BirdOfPrey, near(ax, ay, 8000.0), 20.0);
                "A Klingon Bird-of-Prey that can fire while cloaked is loose in the sector: General Chang! Any hit lights up its exhaust.".to_string()
            }
            Faction::Hirogen => {
                let (x, y) = edge();
                for n in ["Idrin", "Hirogen", "Hirogen"] {
                    spawn(world, n, ShipType::HirogenHunter, near(x, y, 1500.0), 10.0);
                }
                "Hirogen hunters have entered the sector, looking for worthy prey.".to_string()
            }
            Faction::Q => {
                let Some(team) = leading_empire(world) else { return };
                let (hx, hy) = (world.planets[team.home_planet()].x, world.planets[team.home_planet()].y);
                let (qx, qy) = ((hx * 0.7 + 50_000.0 * 0.3), (hy * 0.7 + 50_000.0 * 0.3));
                spawn(world, "Q", ShipType::QEntity, (qx, qy), 0.0);
                let kind = *[TrialKind::Hold, TrialKind::Tribute, TrialKind::Champion].choose(&mut rng).unwrap();
                let kind = if kind == TrialKind::Champion && free < 2 { TrialKind::Hold } else { kind };
                trial = Some(Trial {
                    team,
                    kind,
                    deadline: world.tick + 90 * UPS as u32,
                    owned: (0..world.planets.len()).filter(|&k| world.planets[k].owner == team).collect(),
                    tribute: 0,
                });
                if kind == TrialKind::Champion {
                    spawn(world, "Champion", ShipType::QChampion, near(qx, qy, 1500.0), 15.0);
                    if let Some(&c) = ships.get(1) {
                        world.players[c as usize].only_hurt_by = Some(team);
                    }
                }
                let task = match kind {
                    TrialKind::Hold => "Hold your territory for 90 seconds: lose more than two planets and you fail.".to_string(),
                    TrialKind::Tribute => "Bring me five armies within 90 seconds. I'm waiting near your home world.".to_string(),
                    TrialKind::Champion => "Defeat my champion within 90 seconds. Only your weapons can touch it.".to_string(),
                };
                format!("Q appears in a flash of light! \"The {} stand accused of being... winning. {}\"", team.plural(), task)
            }
            Faction::Ferengi => {
                let (x, y) = edge();
                for n in ["Bok", "Ferengi", "Ferengi"] {
                    spawn(world, n, ShipType::FerengiMarauder, near(x, y, 1500.0), 5.0);
                }
                "Ferengi marauders have entered the sector to plunder lightly defended colonies. Destroy them to recover the armies they steal!".to_string()
            }
            Faction::Tempest => {
                // A clear stretch of space for the web.
                let spot = (0..400).map(|_| (rng.gen_range(12_000.0..88_000.0), rng.gen_range(12_000.0..88_000.0))).find(|&(x, y)| {
                    world.planets.iter().all(|pl| dist(x, y, pl.x, pl.y) > TEMPEST_RIM + 2500.0)
                });
                let Some((x, y)) = spot else { return };
                spawn(world, "Tempest", ShipType::TempestCore, (x, y), 50.0);
                let shape = *TempestShape::ALL.choose(&mut rng).unwrap();
                world.tempest = Some(TempestWeb {
                    x,
                    y,
                    r_in: TEMPEST_CORE,
                    r_out: TEMPEST_RIM,
                    shape,
                    lanes: shape.lanes(),
                    exposed: false,
                    level: 1,
                    zapped_at: 0,
                });
                let near_pl = world.planets.iter().min_by(|a, b| dist(x, y, a.x, a.y).total_cmp(&dist(x, y, b.x, b.y))).map_or("", |p| p.name);
                format!(
                    "A TEMPEST has formed near {}! Any ship that touches its web is trapped on the rim. Shoot down what climbs out of the core (d = SUPERZAPPER, once); clear the web to expose the core.",
                    near_pl
                )
            }
            Faction::Swarm => {
                let (x, y) = near(ax, ay, 6000.0);
                for _ in 0..8 {
                    // Each tiny ship is worth only a fifth of a kill.
                spawn(world, "Swarm", ShipType::SwarmShip, near(x, y, 1000.0), -8.0);
                }
                "A swarm of tiny ships has entered the sector! They latch onto hulls and drain power. Detonate (d) to shake them off.".to_string()
            }
            Faction::Nomad => {
                spawn(world, "Nomad", ShipType::NomadProbe, edge(), 20.0);
                "A probe calling itself NOMAD has entered the sector to \"sterilise\" everything imperfect: damaged ships and weak colonies. Weapons can't touch it. But get within 3,000 and tell it (send a message) that it is imperfect...".to_string()
            }
            Faction::Armus => {
                spawn(world, "Armus", ShipType::ArmusSlick, near(ax, ay, 8000.0), 0.0);
                if let Some(&a) = ships.first() {
                    world.players[a as usize].last_hit = world.tick;
                }
                format!(
                    "A black, oily slick is spreading near {}. It is Armus, a creature of pure malice: it engulfs ships and feeds on violence. Every shot makes it bigger; starve it and it withers.",
                    world.planets[anchor].name
                )
            }
            Faction::Nanites => {
                let hosts: Vec<usize> = (0..MAXPLAYER)
                    .filter(|&j| world.players[j].alive() && world.players[j].faction.is_none() && world.players[j].ship != ShipType::Starbase)
                    .collect();
                let picked: Vec<usize> = hosts.choose_multiple(&mut rng, 2).copied().collect();
                if picked.is_empty() {
                    return;
                }
                let mut names = Vec::new();
                for &j in &picked {
                    world.players[j].nanites = true;
                    names.push(world.players[j].label());
                    world.warn(j as u8, "Your ship is infected with nanites! Orbit a friendly repair world, or detonate (d) to burn them out.");
                }
                format!(
                    "Nanites have escaped from a science lab and infected {}! Infected ships glitch, and the nanites spread to ships close by. Cure: orbit a friendly repair world, or detonate (d).",
                    names.join(" and ")
                )
            }
            Faction::Changeling => {
                let (x, y) = near(ax, ay, 8000.0);
                for _ in 0..3 {
                    spawn(world, "Changeling", ShipType::ChangelingShip, near(x, y, 2000.0), 8.0);
                }
                "Changelings have infiltrated the sector! To every empire they look like its own ships. They bomb colonies and pick off stragglers; any hit exposes one.".to_string()
            }
            Faction::Metrons => {
                let Some((a, b)) = pick_duellists(world) else { return };
                let Some((cx, cy)) = open_spot(world, ARENA_R + 4000.0) else { return };
                spawn(world, "Metron", ShipType::MetronPresence, (cx, cy - ARENA_R - 1500.0), 0.0);
                if ships.is_empty() {
                    return;
                }
                let teams = (world.players[a].team, world.players[b].team);
                for (j, side, foe) in [(a, -1.0, teams.1), (b, 1.0, teams.0)] {
                    let q = &mut world.players[j];
                    let s = q.stats();
                    q.leave_orbit_pub();
                    (q.x, q.y) = (cx + side * ARENA_R * 0.7, cy);
                    (q.dir, q.desired_dir) = (if side < 0.0 { 64.0 } else { 192.0 }, if side < 0.0 { 64.0 } else { 192.0 });
                    (q.lock, q.tractor) = (Lock::None, None);
                    (q.damage, q.shield, q.fuel) = (0.0, s.max_shield, s.max_fuel);
                    q.only_hurt_by = Some(foe);
                }
                let (na, nb) = (world.players[a].label(), world.players[b].label());
                for (j, other) in [(a, &nb), (b, &na)] {
                    world.warn(j as u8, format!("The Metrons have transported you into their arena! Defeat {} to go free.", other));
                }
                waypoint = (cx, cy);
                duel = Some((a as u8, b as u8));
                format!(
                    "The Metrons have seized {} and {} and sealed them in an arena near {}! \"You will fight to the death. The victor will be allowed to go.\" Nobody may enter or leave.",
                    na,
                    nb,
                    nearest_planet_name(world, cx, cy)
                )
            }
            Faction::Pakleds => {
                let (x, y) = edge();
                for n in ["Grebnedlog", "Pakled", "Pakled"] {
                    spawn(world, n, ShipType::PakledClunker, near(x, y, 1500.0), 5.0);
                }
                "Pakled clunkers have entered the sector. \"We look for things. Things to make us go.\" They tractor ships and steal advanced tech or empire upgrades; destroy the thief to get it back!".to_string()
            }
            Faction::TenC => {
                let (x, y) = edge();
                spawn(world, "10-C", ShipType::DarkMatterAnomaly, (x, y), 0.0);
                waypoint = (GWIDTH - x, GWIDTH - y);
                "A dark matter anomaly is crossing the galaxy, wiping out every world in its path and hurling ships aside. It is the work of Species 10-C, and weapons are useless. Make first contact: hold all three of its hyperfield beacons at once.".to_string()
            }
            Faction::Caretaker => {
                let Some((x, y)) = open_spot(world, 6000.0) else { return };
                spawn(world, "Caretaker", ShipType::CaretakerArray, (x, y), 30.0);
                if let Some(&c) = ships.first() {
                    world.players[c as usize].adapt = CARETAKER_SHIELD;
                }
                format!(
                    "The Caretaker's array has appeared near {}! Its displacement waves pull ships from across the galaxy to it. The array is heavily shielded, but every wave it sends out weakens it.",
                    nearest_planet_name(world, x, y)
                )
            }
            Faction::Horta => {
                let colonies: Vec<usize> = (0..world.planets.len())
                    .filter(|&k| {
                        let pl = &world.planets[k];
                        Team::PLAYABLE.contains(&pl.owner) && pl.flags & PL_HOME == 0 && pl.armies >= 3
                    })
                    .collect();
                let Some(&k) = colonies.choose(&mut rng) else { return };
                let (px, py, name) = (world.planets[k].x, world.planets[k].y, world.planets[k].name);
                spawn(world, "Horta", ShipType::Horta, (px, py), 10.0);
                format!(
                    "Something is tunnelling through the rock of {} and killing its armies: a Horta! Destroy it, or make peace: orbit its planet for 10 seconds without firing a shot.",
                    name
                )
            }
            Faction::SphereBuilders => {
                for k in 0..4 {
                    let Some(at) = open_spot(world, 5000.0) else { continue };
                    spawn(world, &format!("Sphere {}", 41 + k * 7), ShipType::DelphicSphere, at, 8.0);
                }
                "The Sphere Builders have planted Delphic Expanse spheres across the galaxy! Each one warps the space around it into anomalies until it is destroyed.".to_string()
            }
            // Handled by `arrive_kzinti` (they're not an ordinary incursion).
            Faction::Kzinti => return,
            Faction::Dyson => {
                // A colony with space around it for the shell.
                let lonely: Vec<usize> = (0..world.planets.len())
                    .filter(|&k| {
                        let pl = &world.planets[k];
                        pl.flags & PL_HOME == 0
                            && world.planets.iter().enumerate().all(|(m, o)| m == k || dist(pl.x, pl.y, o.x, o.y) > DYSON_R + 2500.0)
                            && pl.x.min(pl.y).min(GWIDTH - pl.x).min(GWIDTH - pl.y) > DYSON_R + 2000.0
                    })
                    .collect();
                let Some(&k) = lonely.choose(&mut rng) else { return };
                let (cx, cy, name) = (world.planets[k].x, world.planets[k].y, world.planets[k].name);
                let a = rng.gen_range(0.0..TAU);
                spawn(world, "Dyson sphere", ShipType::DysonHatch, (cx + a.cos() * DYSON_R, cy + a.sin() * DYSON_R), 40.0);
                let Some(&h) = ships.first() else { return };
                // The wreck of the Jenolan, crashed on the far side of the shell.
                let wreck = (cx - a.cos() * (DYSON_R + 1500.0), cy - a.sin() * (DYSON_R + 1500.0));
                world.terrain.push(Terrain::planted(TerrainKind::Derelict, "wreck of the USS Jenolan", wreck, super::terrain::SALVAGE_REACH, h));
                // Anyone caught inside is shut in with the planet.
                inside = (0..MAXPLAYER)
                    .filter(|&j| world.players[j].alive() && world.players[j].faction.is_none() && dist(world.players[j].x, world.players[j].y, cx, cy) < DYSON_R)
                    .map(|j| j as u8)
                    .collect();
                waypoint = (cx, cy);
                format!(
                    "A DYSON SPHERE has materialised around {}, sealing the whole world inside a shell! A tractor beam at its hatch drags passing ships in, and the hatch only opens to take one. A ship in the doorway holds it open. Destroy the hatch emitter to jam it open for good; legend says a ship blowing up in the doorway will do it. The wreck of the USS Jenolan lies on its surface.",
                    name
                )
            }
        };
        if ships.is_empty() && !matches!(kind, Faction::Tribbles | Faction::Nanites) {
            return;
        }
        announce(world, text);
        self.events.push(Event {
            kind,
            ships,
            started: world.tick,
            goals: HashMap::new(),
            holds: HashMap::new(),
            waypoint,
            corners,
            trail: HashMap::new(),
            lost_any: false,
            progress: HashMap::new(),
            dwell: 0,
            prey: None,
            latched: HashMap::new(),
            trial,
            climbers: HashMap::new(),
            grabs: HashMap::new(),
            level: 0,
            queue: Vec::new(),
            next_at: world.tick + 3 * UPS as u32,
            duel,
            stolen: HashMap::new(),
            inside,
            cooldown: world.tick + 3 * UPS as u32,
        });
    }
}

/// The empire holding the most planets.
fn leading_empire(world: &World) -> Option<Team> {
    Team::PLAYABLE
        .into_iter()
        .filter(|&t| world.team_planet_count(t) > 0)
        .max_by_key(|&t| world.team_planet_count(t))
}

/// Whether an incursion has been defeated (tribbles have no ships: they're
/// beaten when no planet or ship carries them any more).
fn beaten(world: &World, e: &Event) -> bool {
    if e.kind == Faction::Tempest {
        return !e.ships.iter().any(|&s| world.players[s as usize].ship == ShipType::TempestCore);
    }
    if e.kind == Faction::Nanites {
        return !world.players.iter().any(|p| p.nanites);
    }
    if e.kind == Faction::Tribbles {
        !world.planets.iter().any(|pl| pl.tribbles) && !world.players.iter().any(|p| p.tribbles)
    } else {
        e.ships.is_empty()
    }
}

fn defeat_text(kind: Faction) -> String {
    match kind {
        Faction::Khan => "Khan's augments have been defeated. \"From hell's heart I stab at thee...\"".into(),
        Faction::Gorn => "The Gorn raiders have been destroyed.".into(),
        Faction::Tholian => "The Tholian vessels have been destroyed. Their web will soon dissolve.".into(),
        Faction::Fesarius => "The Fesarius has been destroyed!".into(),
        Faction::Mirror => "The Terran Empire's fleet has been destroyed. The rift closes.".into(),
        Faction::Doomsday => "The planet killer has been destroyed!".into(),
        Faction::Amoeba => "The space amoeba has been destroyed.".into(),
        Faction::Borg => "The Borg have been defeated. For now.".into(),
        Faction::Vger => "V'Ger has joined with its creator and transcended into a new life form.".into(),
        Faction::Crystal => "The Crystalline Entity is gone.".into(),
        Faction::Probe => "The probe's call has been answered. It departs, and power returns.".into(),
        Faction::Species8472 => "The Species 8472 bioships have been destroyed. The rift to fluidic space closes.".into(),
        Faction::JemHadar => "The Jem'Hadar strike force has been destroyed.".into(),
        Faction::Tribbles => "The last of the tribbles are gone. The quadrotriticale is safe.".into(),
        Faction::Chang => "General Chang's Bird-of-Prey has been destroyed. \"To be... or not to be.\"".into(),
        Faction::Hirogen => "The Hirogen hunters have been destroyed. The hunt is over.".into(),
        Faction::Q => "Q snaps his fingers and vanishes in a flash of light.".into(),
        Faction::Ferengi => "The Ferengi marauders are gone.".into(),
        Faction::Swarm => "The Swarm has been scattered.".into(),
        Faction::Tempest => "The Tempest core is destroyed! Its web collapses and the trapped ships break free.".into(),
        Faction::Nomad => "Nomad is no more. Its search for perfection is over.".into(),
        Faction::Armus => "Armus is gone. The galaxy is a little less spiteful.".into(),
        Faction::Nanites => "The last of the nanites have been purged from the empires' ships.".into(),
        Faction::Changeling => "The Changelings have been rooted out and destroyed.".into(),
        Faction::Metrons => "The Metrons withdraw, their contest decided.".into(),
        Faction::Pakleds => "The Pakled clunkers have been destroyed. \"We are... not strong.\"".into(),
        Faction::TenC => "Species 10-C's anomaly has left the galaxy. First contact has been made.".into(),
        Faction::Caretaker => "The Caretaker's array has been destroyed! Nobody else will be taken.".into(),
        Faction::Horta => "The Horta is no longer a threat to the colonies.".into(),
        Faction::SphereBuilders => "The last Delphic sphere is destroyed, and the anomalies around it fade.".into(),
        Faction::Dyson => "The Dyson sphere's hatch emitter is destroyed! Its doors jam open and every ship inside flies free.".into(),
        Faction::Kzinti => "The Kzinti fleet is destroyed... for now.".into(),
    }
}

fn withdraw_text(kind: Faction) -> String {
    match kind {
        Faction::Khan => "Khan's augments withdraw to plan their revenge.".into(),
        Faction::Gorn => "The Gorn raiders withdraw.".into(),
        Faction::Tholian => "The Tholians withdraw. Their web will soon dissolve.".into(),
        Faction::Fesarius => "Balok and the Fesarius depart the sector.".into(),
        Faction::Mirror => "The mirror universe rift closes and the Terran Empire's fleet vanishes.".into(),
        Faction::Doomsday => "The planet killer drifts out of the galaxy.".into(),
        Faction::Amoeba => "The space amoeba drifts away into the void.".into(),
        Faction::Borg => "The Borg cube departs. They will return.".into(),
        Faction::Vger => "V'Ger turns away into deep space, still searching for its creator.".into(),
        Faction::Crystal => "The Crystalline Entity drifts away in search of other worlds.".into(),
        Faction::Probe => "The probe gives up its search and departs. Power returns.".into(),
        Faction::Species8472 => "The Species 8472 bioships withdraw into fluidic space.".into(),
        Faction::JemHadar => "The surviving Jem'Hadar withdraw through the wormhole.".into(),
        Faction::Tribbles => "The tribbles gorge themselves on poisoned grain and die off.".into(),
        Faction::Chang => "General Chang's Bird-of-Prey slips away under cloak. \"Cry havoc, and let slip the dogs of war!\"".into(),
        Faction::Hirogen => "The Hirogen break off the hunt and withdraw.".into(),
        Faction::Q => "Q grows bored and vanishes in a flash of light.".into(),
        Faction::Ferengi => "The Ferengi marauders warp out in search of better profits.".into(),
        Faction::Swarm => "The Swarm moves on to other territory.".into(),
        Faction::Tempest => "The Tempest's web fades away, releasing the ships trapped on it.".into(),
        Faction::Nomad => "Nomad turns away into deep space, still searching for perfection.".into(),
        Faction::Armus => "Armus sinks back into the dark, bored of tormenting you.".into(),
        Faction::Nanites => "The nanites go dormant and shut down.".into(),
        Faction::Changeling => "The Changelings slip away to rejoin the Great Link.".into(),
        Faction::Metrons => "The Metrons end their contest and withdraw.".into(),
        Faction::Pakleds => "The Pakleds lumber away with everything they took. \"We are strong.\"".into(),
        Faction::TenC => "The dark matter anomaly drifts out of the galaxy. Species 10-C remains a mystery.".into(),
        Faction::Caretaker => "The Caretaker's array fades away to search for another species.".into(),
        Faction::Horta => "The Horta burrows deep into the rock and falls silent.".into(),
        Faction::SphereBuilders => "The Sphere Builders withdraw their spheres, and the anomalies fade.".into(),
        Faction::Dyson => "The Dyson sphere's automated systems go dormant. Its hatch swings open and the trapped ships fly free.".into(),
        Faction::Kzinti => "The Kzinti never withdraw.".into(),
    }
}

// ----------------------------------------------------------------------
// behaviour

/// Nearest empire ship within `range`, preferring team `prefer` if one is in range.
fn nearest_enemy(world: &World, i: usize, range: f64, prefer: Option<Team>) -> Option<(usize, f64)> {
    let p = &world.players[i];
    let mut best: Option<(usize, f64)> = None;
    let mut best_pref: Option<(usize, f64)> = None;
    for (j, q) in world.players.iter().enumerate() {
        let warring = matches!((p.faction, q.faction), (Some(a), Some(b)) if a.at_war_with(b));
        if !q.alive() || (q.faction.is_some() && !warring) || q.cloaked {
            continue;
        }
        let d = dist(p.x, p.y, q.x, q.y);
        if d > range {
            continue;
        }
        if best.map_or(true, |b| d < b.1) {
            best = Some((j, d));
        }
        if Some(q.team) == prefer && best_pref.map_or(true, |b| d < b.1) {
            best_pref = Some((j, d));
        }
    }
    best_pref.or(best)
}

fn cmd(world: &mut World, i: usize, msg: ClientMsg) {
    world.handle(i as u8, msg);
}

fn steer_to(world: &mut World, i: usize, x: f64, y: f64, speed: i32) {
    let p = &world.players[i];
    let dir = dir_to(p.x, p.y, x, y);
    let lock = p.lock;
    if lock != Lock::None || (dir_diff(p.desired_dir, dir)).abs() > 1.0 {
        cmd(world, i, ClientMsg::Course(dir as u8));
    }
    if world.players[i].desired_speed != speed {
        cmd(world, i, ClientMsg::Speed(speed.max(0) as u8));
    }
}

/// Close in on ship `t` and shoot at it.
fn fight(world: &mut World, i: usize, t: usize) {
    let mut rng = rand::thread_rng();
    let (p, q) = (&world.players[i], &world.players[t]);
    let s = p.stats();
    let d = dist(p.x, p.y, q.x, q.y);
    let (tvx, tvy) = dir_vec(q.dir);
    let tv = q.speed as f64 * WARP1;
    let tspd = s.torp_speed * WARP1;
    let aim = lead(p.x, p.y, q.x, q.y, tvx * tv, tvy * tv, tspd);
    let straight = dir_to(p.x, p.y, q.x, q.y);
    let weave = if d < 6000.0 { rng.gen_range(-40.0..40.0) } else { rng.gen_range(-8.0..8.0) };
    let speed = if d > 8000.0 { s.max_speed } else { (s.max_speed * 2 / 3).max(2) };
    if p.desired_speed != speed {
        cmd(world, i, ClientMsg::Speed(speed as u8));
    }
    cmd(world, i, ClientMsg::Course((aim + weave).rem_euclid(256.0) as u8));
    let p = &world.players[i];
    if s.torp_damage > 0.0 && d < tspd * s.torp_fuse as f64 * 0.8 && p.wtemp < s.max_wtemp * 0.7 && rng.gen_bool(0.4) {
        cmd(world, i, ClientMsg::Torp((aim + rng.gen_range(-3.0..3.0)).rem_euclid(256.0) as u8));
    }
    if s.phaser_damage > 0.0 && d < PHASEDIST * s.phaser_damage / 100.0 * 0.75 && rng.gen_bool(0.25) {
        cmd(world, i, ClientMsg::Phaser(straight as u8));
    }
}

/// Fly to planet `k` and settle into orbit.
fn go_orbit(world: &mut World, i: usize, k: usize) -> bool {
    let p = &world.players[i];
    if p.orbiting == Some(k) {
        return true;
    }
    if p.lock != Lock::Planet(k) {
        cmd(world, i, ClientMsg::LockPlanet(k as u8));
    }
    if world.players[i].desired_speed == 0 {
        let max = world.players[i].stats().max_speed;
        cmd(world, i, ClientMsg::Speed(max as u8));
    }
    false
}

fn nearest_planet(world: &World, i: usize, ok: impl Fn(&super::world::Planet) -> bool) -> Option<usize> {
    let p = &world.players[i];
    world
        .planets
        .iter()
        .enumerate()
        .filter(|(_, pl)| ok(pl))
        .min_by(|a, b| dist(p.x, p.y, a.1.x, a.1.y).total_cmp(&dist(p.x, p.y, b.1.x, b.1.y)))
        .map(|(k, _)| k)
}

fn beam(world: &mut World, from: usize, to: usize, damage: f64, how: &str) {
    let (x1, y1) = (world.players[from].x, world.players[from].y);
    let (x2, y2) = (world.players[to].x, world.players[to].y);
    world.phasers.push(PhaserShot {
        info: PhaserInfo { owner: from as u8, x1: x1 as i32, y1: y1 as i32, x2: x2 as i32, y2: y2 as i32, hit: true },
        ticks: 6,
    });
    world.inflict(to, damage, Some(from as u8), how.into());
}

fn run_event(world: &mut World, e: &mut Event) {
    let tick = world.tick;
    let ids = e.ships.clone();
    for (n, &id) in ids.iter().enumerate() {
        let i = id as usize;
        if !world.players[i].alive() {
            continue;
        }
        match e.kind {
            Faction::Khan => khan(world, i, tick),
            Faction::Gorn => gorn(world, e, i, tick),
            Faction::Mirror => mirror(world, e, i, tick),
            Faction::Tholian => tholian(world, e, i, n, tick),
            Faction::Fesarius => fesarius(world, e, i, tick),
            Faction::Doomsday => doomsday(world, e, i, tick),
            Faction::Amoeba => amoeba(world, e, i, tick),
            Faction::Borg => borg(world, e, i, tick),
            Faction::Vger => vger(world, e, i, tick),
            Faction::Crystal => crystal(world, e, i, tick),
            Faction::Probe => probe(world, e, i, tick),
            Faction::Species8472 => species8472(world, e, i, n, tick),
            Faction::JemHadar => jemhadar(world, i, tick),
            Faction::Tribbles => {}
            Faction::Chang => chang(world, i, tick),
            Faction::Hirogen => hirogen(world, e, i, tick),
            Faction::Q => q(world, e, i, tick),
            Faction::Ferengi => ferengi(world, i, tick),
            Faction::Swarm => swarm(world, e, i, n, tick),
            Faction::Tempest => {}
            Faction::Nomad => nomad(world, e, i, tick),
            Faction::Armus => armus(world, e, i, tick),
            Faction::Nanites | Faction::Metrons => {}
            Faction::Changeling => changeling(world, e, i, tick),
            Faction::Pakleds => pakled(world, e, i, tick),
            Faction::TenC => ten_c(world, e, i, tick),
            Faction::Caretaker => caretaker(world, e, i, tick),
            Faction::Horta => horta(world, e, i, tick),
            Faction::SphereBuilders => sphere(world, i, n, tick),
            Faction::Dyson => dyson(world, e, i, tick),
            Faction::Kzinti => {}
        }
    }
    match e.kind {
        Faction::Nanites => nanites(world, e),
        Faction::Metrons => metrons(world, e, tick),
        Faction::Pakleds => recover(world, e),
        Faction::SphereBuilders | Faction::Dyson => {
            // Anomalies go when the sphere that made them does.
            let spheres: Vec<u8> = e.ships.iter().copied().filter(|&s| world.players[s as usize].alive()).collect();
            world.terrain.retain(|t| t.owner.map_or(true, |o| spheres.contains(&o)));
        }
        _ => {}
    }
    if e.kind == Faction::Tempest {
        tempest(world, e, tick);
    }
    if e.kind == Faction::Tribbles {
        tribbles(world, e, tick);
    }
    if e.kind == Faction::Tholian {
        if tick % 30 == 0 {
            spin_web(world, e);
        }
    }
}

/// Khan: hunt Federation ships first, and bomb Federation worlds.
fn khan(world: &mut World, i: usize, tick: u32) {
    if tick % 2 != 0 {
        return;
    }
    if let Some((t, _)) = nearest_enemy(world, i, 25_000.0, Some(Team::Fed)) {
        fight(world, i, t);
        return;
    }
    let target = nearest_planet(world, i, |pl| pl.owner == Team::Fed && pl.armies > 4)
        .or_else(|| nearest_planet(world, i, |pl| Team::PLAYABLE.contains(&pl.owner) && pl.armies > 4));
    if let Some(k) = target {
        if go_orbit(world, i, k) && !world.players[i].bombing {
            cmd(world, i, ClientMsg::Bomb);
        }
    }
}

/// Gorn: go from colony to colony and wipe out the inhabitants.
fn gorn(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let id = i as u8;
    if tick % 2 == 0 {
        if let Some((t, _)) = nearest_enemy(world, i, 8000.0, None) {
            fight(world, i, t);
            return;
        }
    }
    let goal = match e.goals.get(&id) {
        Some(&k) if world.planets[k].armies > 0 && Team::PLAYABLE.contains(&world.planets[k].owner) => k,
        _ => match nearest_planet(world, i, |pl| Team::PLAYABLE.contains(&pl.owner) && pl.armies > 0 && pl.flags & PL_HOME == 0) {
            Some(k) => {
                e.goals.insert(id, k);
                k
            }
            None => return,
        },
    };
    if go_orbit(world, i, goal) && tick % 5 == 0 {
        let pl = &mut world.planets[goal];
        pl.armies -= 1;
        if pl.armies <= 0 {
            let (name, old) = (pl.name, pl.owner);
            pl.armies = 0;
            pl.owner = Team::Ind;
            announce(world, format!("The Gorn have wiped out the colony on {}!", name));
            world.check_genocide(old, Team::Ind);
            e.goals.remove(&id);
        }
    }
}

/// Terran Empire: fight anyone, bomb planets, then conquer them.
fn mirror(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let id = i as u8;
    if tick % 2 != 0 {
        return;
    }
    if let Some((t, _)) = nearest_enemy(world, i, 16_000.0, None) {
        fight(world, i, t);
        return;
    }
    let goal = match e.goals.get(&id) {
        Some(&k) if world.planets[k].alien != Some(Faction::Mirror) => k,
        _ => match nearest_planet(world, i, |pl| Team::PLAYABLE.contains(&pl.owner) && pl.flags & PL_HOME == 0) {
            Some(k) => {
                e.goals.insert(id, k);
                k
            }
            None => return,
        },
    };
    if go_orbit(world, i, goal) {
        if world.planets[goal].armies > 4 {
            if !world.players[i].bombing {
                cmd(world, i, ClientMsg::Bomb);
            }
        } else {
            let pl = &mut world.planets[goal];
            let (name, old) = (pl.name, pl.owner);
            pl.owner = Team::Ind;
            pl.alien = Some(Faction::Mirror);
            pl.armies = 10;
            announce(world, format!("The Terran Empire has conquered {}!", name));
            world.check_genocide(old, Team::Ind);
            e.goals.remove(&id);
        }
    }
}

/// How long a Tholian strand lasts.
const WEB_TTL: i32 = 60 * UPS as i32;
/// How often (in ticks) a racing Tholian lays another strand behind it.
const STRAND_TICKS: u32 = 10;

/// Where the web's corner `k` is on a Tholian's `lap`: each lap runs a
/// little further inside the triangle, so the web fills in.
fn web_corner(world: &World, e: &Event, k: usize, lap: i32) -> (f64, f64) {
    let pts: Vec<(f64, f64)> = e.corners.iter().map(|&c| (world.planets[c].x, world.planets[c].y)).collect();
    let (gx, gy) = (pts.iter().map(|p| p.0).sum::<f64>() / 3.0, pts.iter().map(|p| p.1).sum::<f64>() / 3.0);
    let inset = (lap % 4) as f64 * 0.18;
    let (px, py) = pts[k];
    (px + (gx - px) * inset, py + (gy - py) * inset)
}

/// Tholians: race flat out round a triangle of three planets, laying web
/// strands behind them.
fn tholian(world: &mut World, e: &mut Event, i: usize, n: usize, tick: u32) {
    let id = i as u8;
    if e.corners.len() < 3 {
        return;
    }
    let lap = *e.progress.get(&id).unwrap_or(&0);
    let corner = *e.goals.entry(id).or_insert(n % 3);
    let (tx, ty) = web_corner(world, e, corner, lap);
    let (x, y) = (world.players[i].x, world.players[i].y);
    let max = world.players[i].stats().max_speed;
    steer_to(world, i, tx, ty, max);
    let lay = |world: &mut World, e: &mut Event| {
        let (px, py) = e.trail.get(&id).copied().unwrap_or((x, y));
        if dist(px, py, x, y) > 300.0 {
            world.webs.push(Web { x1: px, y1: py, x2: x, y2: y, ttl: WEB_TTL, owner: id });
        }
        e.trail.insert(id, (x, y));
    };
    if dist(x, y, tx, ty) < 900.0 {
        // Round the corner: finish this side's strand at the corner itself.
        lay(world, e);
        let next = (corner + 1) % 3;
        e.goals.insert(id, next);
        if next == n % 3 {
            e.progress.insert(id, lap + 1);
        }
    } else if tick % STRAND_TICKS == (n as u32 * 3) % STRAND_TICKS {
        lay(world, e);
    }
    if tick % 6 == 0 {
        if let Some((t, d)) = nearest_enemy(world, i, 4000.0, None) {
            if d < 3500.0 {
                let p = &world.players[i];
                let q = &world.players[t];
                let dir = dir_to(p.x, p.y, q.x, q.y);
                cmd(world, i, ClientMsg::Phaser(dir as u8));
            }
        }
    }
}

/// Cross strands strung between the racing Tholians, across the web.
fn spin_web(world: &mut World, e: &Event) {
    let alive: Vec<u8> = e.ships.iter().copied().filter(|&id| world.players[id as usize].alive()).collect();
    for k in 0..alive.len() {
        let (a, b) = (alive[k] as usize, alive[(k + 1) % alive.len()] as usize);
        if a == b {
            continue;
        }
        let (pa, pb) = (&world.players[a], &world.players[b]);
        let (x1, y1, x2, y2) = (pa.x, pa.y, pb.x, pb.y);
        if dist(x1, y1, x2, y2) < 25_000.0 {
            world.webs.push(Web { x1, y1, x2, y2, ttl: WEB_TTL, owner: alive[k] });
        }
    }
    let excess = world.webs.len().saturating_sub(240);
    world.webs.drain(..excess);
}

/// Fesarius: wander the galaxy blasting and tractoring ships.
fn fesarius(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let mut rng = rand::thread_rng();
    let p = &world.players[i];
    if dist(p.x, p.y, e.waypoint.0, e.waypoint.1) < 3000.0 || tick % 600 == 0 {
        e.waypoint = (rng.gen_range(10_000.0..90_000.0), rng.gen_range(10_000.0..90_000.0));
    }
    let target = nearest_enemy(world, i, 30_000.0, None);
    let (wx, wy) = match target {
        Some((t, _)) => (world.players[t].x, world.players[t].y),
        None => e.waypoint,
    };
    if tick % 4 == 0 {
        steer_to(world, i, wx, wy, 3);
    }
    if let Some((t, d)) = target {
        if tick % 15 == 0 && d < 8000.0 {
            let p = &world.players[i];
            let q = &world.players[t];
            let dir = dir_to(p.x, p.y, q.x, q.y);
            cmd(world, i, ClientMsg::Phaser(dir as u8));
        }
        if tick % 20 == 0 && d < 5500.0 && world.players[i].tractor.is_none() {
            cmd(world, i, ClientMsg::Tractor { target: Some(t as u8), pressor: false });
        }
    }
}

/// Planet killer: drift toward the nearest world and devour it. An
/// antiproton beam lashes out at ships, and anything in its maw is eaten.
fn doomsday(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let id = i as u8;
    let goal = match e.goals.get(&id) {
        Some(&k) if world.planets[k].alien != Some(Faction::Doomsday) => k,
        _ => match nearest_planet(world, i, |pl| pl.alien != Some(Faction::Doomsday)) {
            Some(k) => {
                e.goals.insert(id, k);
                k
            }
            None => return,
        },
    };
    let (px, py) = (world.planets[goal].x, world.planets[goal].y);
    let p = &world.players[i];
    let d = dist(p.x, p.y, px, py);
    if tick % 4 == 0 {
        steer_to(world, i, px, py, if d < 1800.0 { 0 } else { 2 });
    }
    if d < 1800.0 && tick % 4 == 0 {
        let pl = &mut world.planets[goal];
        pl.armies -= 1;
        if pl.armies <= 0 {
            let (name, old) = (pl.name, pl.owner);
            pl.armies = 0;
            pl.owner = Team::Ind;
            pl.flags = 0;
            pl.alien = Some(Faction::Doomsday);
            announce(world, format!("The planet killer has devoured {}!", name));
            world.check_genocide(old, Team::Ind);
            e.goals.remove(&id);
        }
    }
    // Ships in front of the maw are torn apart.
    let (x, y, dir) = (world.players[i].x, world.players[i].y, world.players[i].dir);
    let (fx, fy) = dir_vec(dir);
    let (mx, my) = (x + fx * 1600.0, y + fy * 1600.0);
    let victims: Vec<usize> = (0..MAXPLAYER)
        .filter(|&j| j != i && world.players[j].alive() && world.players[j].faction.is_none())
        .filter(|&j| dist(world.players[j].x, world.players[j].y, mx, my) < 1400.0)
        .collect();
    for j in victims {
        world.inflict(j, 6.0, Some(id), "was consumed by the planet killer".into());
    }
    if tick % 25 == 0 {
        if let Some((t, d)) = nearest_enemy(world, i, 7000.0, None) {
            if d < 7000.0 {
                beam(world, i, t, 80.0, "was destroyed by the planet killer's antiproton beam");
            }
        }
    }
}

/// Space amoeba: drift toward ships and drain everything nearby.
fn amoeba(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let mut rng = rand::thread_rng();
    let id = i as u8;
    let target = nearest_enemy(world, i, 30_000.0, None);
    let (wx, wy) = match target {
        Some((t, _)) => (world.players[t].x, world.players[t].y),
        None => {
            let p = &world.players[i];
            if dist(p.x, p.y, e.waypoint.0, e.waypoint.1) < 3000.0 {
                e.waypoint = (rng.gen_range(10_000.0..90_000.0), rng.gen_range(10_000.0..90_000.0));
            }
            e.waypoint
        }
    };
    if tick % 4 == 0 {
        steer_to(world, i, wx, wy, 3);
    }
    let (x, y) = (world.players[i].x, world.players[i].y);
    let victims: Vec<usize> = (0..MAXPLAYER)
        .filter(|&j| j != i && world.players[j].alive() && world.players[j].faction.is_none())
        .filter(|&j| dist(world.players[j].x, world.players[j].y, x, y) < 3200.0)
        .collect();
    for &j in &victims {
        let q = &mut world.players[j];
        q.fuel = (q.fuel - 150.0).max(0.0);
        world.inflict(j, 3.0, Some(id), "was absorbed by the space amoeba".into());
    }
    if tick % 20 == 0 {
        match victims.first() {
            Some(&j) if world.players[i].tractor.is_none() => {
                cmd(world, i, ClientMsg::Tractor { target: Some(j as u8), pressor: false });
            }
            None if world.players[i].tractor.is_some() => {
                cmd(world, i, ClientMsg::Tractor { target: None, pressor: false });
            }
            _ => {}
        }
    }
}

/// Borg: chase ships, cut them apart, and assimilate what they catch.
/// Every assimilated ship becomes a new cube (up to three).
fn borg(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let mut rng = rand::thread_rng();
    let id = i as u8;
    let Some((t, d)) = nearest_enemy(world, i, 60_000.0, None) else {
        if tick % 4 == 0 {
            let (wx, wy) = e.waypoint;
            steer_to(world, i, wx, wy, 4);
        }
        return;
    };
    if tick % 4 == 0 {
        let (tx, ty) = (world.players[t].x, world.players[t].y);
        steer_to(world, i, tx, ty, if d < 2000.0 { 1 } else { 6 });
    }
    if tick % 20 == 0 && d < 7000.0 {
        let p = &world.players[i];
        let q = &world.players[t];
        let dir = dir_to(p.x, p.y, q.x, q.y);
        cmd(world, i, ClientMsg::Phaser(dir as u8));
    }
    if tick % 10 == 0 && d < 9000.0 && rng.gen_bool(0.5) {
        let p = &world.players[i];
        let q = &world.players[t];
        let dir = dir_to(p.x, p.y, q.x, q.y);
        cmd(world, i, ClientMsg::Torp(dir as u8));
    }
    if d < 6500.0 && world.players[i].tractor.map(|x| x.0) != Some(t as u8) && tick % 10 == 0 {
        cmd(world, i, ClientMsg::Tractor { target: Some(t as u8), pressor: false });
    }
    // Assimilation: hold a ship in the tractor beam, close in, and take it.
    let held = world.players[i].tractor.map(|x| x.0 as usize);
    match held {
        Some(v) if world.players[v].alive()
            && world.players[v].faction.is_none()
            && dist(world.players[v].x, world.players[v].y, world.players[i].x, world.players[i].y) < 2600.0 =>
        {
            let entry = e.holds.entry(id).or_insert((v as u8, 0));
            if entry.0 != v as u8 {
                *entry = (v as u8, 0);
            }
            entry.1 += 1;
            if entry.1 >= 40 {
                e.holds.remove(&id);
                let (vx, vy) = (world.players[v].x, world.players[v].y);
                world.kill(v, None, "was assimilated by the Borg".into());
                world.players[i].kills += 1.0;
                let cubes = e.ships.iter().filter(|&&s| world.players[s as usize].alive()).count();
                if cubes < MAX_CUBES {
                    if let Some(nid) = world.spawn_alien("Borg", Faction::Borg, ShipType::BorgCube, vx, vy, 20.0) {
                        e.ships.push(nid);
                        announce(world, "A new Borg cube emerges from the assimilated ship!");
                    }
                }
            }
        }
        _ => {
            e.holds.remove(&id);
        }
    }
}

/// Empire ships (not aliens) within `r` of (x, y).
fn empire_ships_near(world: &World, x: f64, y: f64, r: f64) -> Vec<usize> {
    (0..MAXPLAYER)
        .filter(|&j| world.players[j].alive() && world.players[j].faction.is_none())
        .filter(|&j| dist(world.players[j].x, world.players[j].y, x, y) < r)
        .collect()
}

const VGER_CLOUD: f64 = 6000.0;
const VGER_CORE: f64 = 1200.0;

/// V'Ger: drift toward Earth (then other home worlds), slowing ships in the
/// cloud and digitizing them with plasma bolts. A ship that holds position
/// at the core for 10 seconds joins with V'Ger and ends the threat.
fn vger(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let mut rng = rand::thread_rng();
    let id = i as u8;
    let (x, y) = (world.players[i].x, world.players[i].y);
    // Heading: Earth first, then the nearest home world still standing.
    let goal = match e.goals.get(&id) {
        Some(&k) if world.planets[k].armies > 0 => k,
        _ => {
            let homes = [0usize, 10, 20, 30];
            let k = if world.planets[0].armies > 0 {
                0
            } else {
                *homes
                    .iter()
                    .filter(|&&k| world.planets[k].armies > 0)
                    .min_by(|a, b| {
                        let (pa, pb) = (&world.planets[**a], &world.planets[**b]);
                        dist(x, y, pa.x, pa.y).total_cmp(&dist(x, y, pb.x, pb.y))
                    })
                    .unwrap_or(&0)
            };
            e.goals.insert(id, k);
            k
        }
    };
    let (gx, gy) = (world.planets[goal].x, world.planets[goal].y);
    if tick % 4 == 0 {
        steer_to(world, i, gx, gy, if dist(x, y, gx, gy) < 1500.0 { 0 } else { 2 });
    }
    if dist(x, y, gx, gy) < 2000.0 && world.planets[goal].armies > 0 {
        let pl = &mut world.planets[goal];
        let (name, old) = (pl.name, pl.owner);
        pl.armies = 0;
        pl.owner = Team::Ind;
        announce(world, format!("V'Ger has purged {} of all carbon units!", name));
        world.check_genocide(old, Team::Ind);
        e.goals.remove(&id);
    }
    // Everything inside the cloud crawls.
    for j in empire_ships_near(world, x, y, VGER_CLOUD) {
        let q = &mut world.players[j];
        if !q.techs.contains(&Tech::MetaphasicShields) {
            q.desired_speed = q.desired_speed.min(3);
        }
    }
    // Merging: hold position at the core.
    let at_core = empire_ships_near(world, x, y, VGER_CORE);
    e.progress.retain(|s, _| at_core.contains(&(*s as usize)));
    for &j in &at_core {
        let t = e.progress.entry(j as u8).or_insert(0);
        *t += 1;
        if *t == 30 {
            let who = world.players[j].label();
            announce(world, format!("{} is joining with V'Ger...", who));
        }
        if *t >= 100 {
            let who = world.players[j].label();
            world.events.push(GameEvent::Honour { player: j as u8, text: "Joined with V'Ger".into() });
            world.players[j].total_kills += 5.0;
            world.kill(j, None, "joined with V'Ger".into());
            announce(world, format!("{} has joined with V'Ger, and a new life form is born. Earth is saved!", who));
            world.remove_player(id);
            return;
        }
    }
    // Plasma bolts digitize ships in or near the cloud (but not at the core:
    // V'Ger is curious about whoever reaches it).
    if tick % 50 == 25 {
        let targets: Vec<usize> = empire_ships_near(world, x, y, VGER_CLOUD + 3000.0)
            .into_iter()
            .filter(|j| !at_core.contains(j))
            .collect();
        if let Some(&t) = targets.choose(&mut rng) {
            let (tx, ty) = (world.players[t].x, world.players[t].y);
            world.phasers.push(PhaserShot {
                info: PhaserInfo { owner: id, x1: x as i32, y1: y as i32, x2: tx as i32, y2: ty as i32, hit: true },
                ticks: 8,
            });
            world.kill(t, None, "was digitized by V'Ger".into());
        }
    }
}

/// Crystalline Entity: strip the life from planets (farming worlds first)
/// and lash out at nearby ships.
fn crystal(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let id = i as u8;
    let alive = |pl: &super::world::Planet| Team::PLAYABLE.contains(&pl.owner) && pl.armies > 0;
    let goal = match e.goals.get(&id) {
        Some(&k) if world.planets[k].armies > 0 => k,
        _ => match nearest_planet(world, i, |pl| alive(pl) && pl.flags & PL_AGRI != 0).or_else(|| nearest_planet(world, i, alive)) {
            Some(k) => {
                e.goals.insert(id, k);
                k
            }
            None => return,
        },
    };
    let (px, py) = (world.planets[goal].x, world.planets[goal].y);
    let p = &world.players[i];
    let d = dist(p.x, p.y, px, py);
    if tick % 4 == 0 {
        steer_to(world, i, px, py, if d < 2000.0 { 0 } else { 4 });
    }
    if d < 2200.0 && tick % 3 == 0 {
        let pl = &mut world.planets[goal];
        pl.armies -= 1;
        if pl.armies <= 0 {
            pl.armies = 0;
            pl.flags &= !PL_AGRI;
            let name = pl.name;
            announce(world, format!("The Crystalline Entity has stripped all life from {}!", name));
            e.goals.remove(&id);
        }
    }
    if tick % 20 == 0 {
        if let Some((t, d)) = nearest_enemy(world, i, 6000.0, None) {
            if d < 6000.0 {
                beam(world, i, t, 40.0, "was shredded by the Crystalline Entity");
            }
        }
    }
}

const PROBE_DRAIN: f64 = 10_000.0;

/// Whale probe: travel planet to planet, draining the power of every ship
/// in range and silencing the planets it visits. Bringing it two armies
/// ("whales") answers its call and sends it away.
fn probe(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let id = i as u8;
    let (x, y) = (world.players[i].x, world.players[i].y);
    let goal = match e.goals.get(&id) {
        Some(&k) if e.dwell < 150 => k,
        _ => {
            let prev = e.goals.get(&id).copied();
            let k = nearest_planet(world, i, |pl| pl.owner != Team::Ind).filter(|&k| Some(k) != prev);
            let k = k.or_else(|| nearest_planet(world, i, |_| true)).unwrap_or(0);
            // Head somewhere new: the nearest planet it hasn't just visited, a fair way off.
            let far: Vec<usize> = (0..world.planets.len())
                .filter(|&j| Some(j) != prev && dist(x, y, world.planets[j].x, world.planets[j].y) > 15_000.0)
                .collect();
            let k = far
                .into_iter()
                .min_by(|a, b| {
                    let (pa, pb) = (&world.planets[*a], &world.planets[*b]);
                    dist(x, y, pa.x, pa.y).total_cmp(&dist(x, y, pb.x, pb.y))
                })
                .unwrap_or(k);
            e.goals.insert(id, k);
            e.dwell = 0;
            k
        }
    };
    let (px, py) = (world.planets[goal].x, world.planets[goal].y);
    let d = dist(x, y, px, py);
    if tick % 4 == 0 {
        steer_to(world, i, px, py, if d < 2000.0 { 0 } else { 3 });
    }
    if d < 3000.0 {
        e.dwell += 1;
        world.planets[goal].silenced_until = tick + 20;
    }
    let drained = empire_ships_near(world, x, y, PROBE_DRAIN);
    for &j in &drained {
        if !world.players[j].techs.contains(&Tech::MetaphasicShields) {
            world.players[j].powerless_until = tick + 3;
        }
    }
    // Answering the call: any ship carrying two armies close by.
    if let Some(&j) = empire_ships_near(world, x, y, 4000.0).iter().find(|&&j| world.players[j].armies >= 2) {
        let q = &mut world.players[j];
        q.armies -= 2;
        q.kills += 3.0;
        q.total_kills += 3.0;
        let who = q.label();
        world.events.push(GameEvent::Honour { player: j as u8, text: "Answered the whale probe".into() });
        announce(world, format!("{} answers the probe with the song of the humpback whales!", who));
        world.remove_player(id);
    }
}

/// Species 8472: hunt ships (and the Borg) with devastating beams. When all
/// the bioships gather at one planet they focus their beams and destroy it.
fn species8472(world: &mut World, e: &mut Event, i: usize, n: usize, tick: u32) {
    let id = i as u8;
    // The group shares one target planet, chosen by the lead ship.
    let lead_id = e.ships.iter().copied().find(|&s| world.players[s as usize].alive()).unwrap_or(id);
    let goal = match e.goals.get(&lead_id) {
        Some(&k) if world.planets[k].armies > 0 => k,
        _ => {
            let p = &world.players[lead_id as usize];
            let (x, y) = (p.x, p.y);
            let k = (0..world.planets.len())
                .filter(|&k| {
                    let pl = &world.planets[k];
                    Team::PLAYABLE.contains(&pl.owner) && pl.armies > 0 && pl.flags & PL_HOME == 0
                })
                .min_by(|a, b| {
                    let (pa, pb) = (&world.planets[*a], &world.planets[*b]);
                    dist(x, y, pa.x, pa.y).total_cmp(&dist(x, y, pb.x, pb.y))
                });
            let Some(k) = k else { return };
            e.goals.insert(lead_id, k);
            let charge = e.progress.entry(lead_id).or_insert(0);
            *charge = (*charge).min(0);
            k
        }
    };
    if tick % 2 == 0 {
        if let Some((t, d)) = nearest_enemy(world, i, 7000.0, None) {
            let p = &world.players[i];
            let q = &world.players[t];
            let dir = dir_to(p.x, p.y, q.x, q.y);
            if d < 6000.0 {
                cmd(world, i, ClientMsg::Phaser(dir as u8));
            }
        }
    }
    // Fly to the planet and hold station around it.
    let (px, py) = (world.planets[goal].x, world.planets[goal].y);
    let a = n as f64 * TAU / 3.0 + tick as f64 * 0.01;
    let (tx, ty) = (px + a.cos() * 2500.0, py + a.sin() * 2500.0);
    let p = &world.players[i];
    let d = dist(p.x, p.y, tx, ty);
    if tick % 3 == 0 {
        steer_to(world, i, tx, ty, ((d / 500.0) as i32).clamp(1, 11));
    }
    // The lead ship tracks whether the whole group is in position.
    if id == lead_id && tick % 2 == 0 {
        let alive: Vec<usize> = e.ships.iter().map(|&s| s as usize).filter(|&s| world.players[s].alive()).collect();
        let gathered = alive.len() >= 2 && alive.iter().all(|&s| dist(world.players[s].x, world.players[s].y, px, py) < 4000.0);
        let charge = e.progress.entry(lead_id).or_insert(0);
        *charge = if *charge < 0 {
            *charge + 2
        } else if gathered {
            *charge + 2
        } else {
            0
        };
        if *charge == 20 {
            announce(world, format!("Species 8472 bioships are focusing their beams on {}!", world.planets[goal].name));
        }
        if *charge >= 60 {
            for &s in &alive {
                let (sx, sy) = (world.players[s].x, world.players[s].y);
                world.phasers.push(PhaserShot {
                    info: PhaserInfo { owner: s as u8, x1: sx as i32, y1: sy as i32, x2: px as i32, y2: py as i32, hit: true },
                    ticks: 10,
                });
            }
            let pl = &mut world.planets[goal];
            let (name, old) = (pl.name, pl.owner);
            pl.armies = 0;
            pl.owner = Team::Ind;
            pl.flags = 0;
            pl.alien = Some(Faction::Species8472);
            announce(world, format!("Species 8472 has destroyed {}!", name));
            world.check_genocide(old, Team::Ind);
            e.goals.remove(&lead_id);
            // Recharge before the next planet (about 40 seconds).
            e.progress.insert(lead_id, -400);
        }
    }
}

/// Jem'Hadar: fast strikes with shield-piercing polaron beams; a badly
/// damaged fighter rams the nearest enemy.
fn jemhadar(world: &mut World, i: usize, tick: u32) {
    let id = i as u8;
    let p = &world.players[i];
    let hurt = p.damage / p.stats().max_damage;
    if hurt >= 0.7 {
        if let Some((t, d)) = nearest_enemy(world, i, 30_000.0, None) {
            let (tx, ty) = (world.players[t].x, world.players[t].y);
            steer_to(world, i, tx, ty, 11);
            if d < 700.0 {
                let (me, them) = (world.players[i].label(), world.players[t].label());
                announce(world, format!("A Jem'Hadar fighter rams {}!", them));
                world.inflict(t, 150.0, Some(id), format!("was rammed by {}", me));
                world.kill(i, None, "rammed its target".into());
            }
        }
        return;
    }
    if tick % 2 == 0 {
        if let Some((t, _)) = nearest_enemy(world, i, 40_000.0, None) {
            fight(world, i, t);
        }
    }
}

const TRIBBLE_CAP: usize = 12;

/// Tribbles: they breed on planets (stopping army growth and slowly eating
/// the armies' food), stow away on ships that orbit there, and spread from
/// world to world. Tribbles hate Klingons: a Klingon ship in orbit drives
/// them off a planet, and they flee any ship a Klingon comes near.
fn tribbles(world: &mut World, e: &mut Event, tick: u32) {
    let mut rng = rand::thread_rng();
    for pl in world.planets.iter_mut().filter(|pl| pl.owner == Team::Kli) {
        pl.tribbles = false;
    }
    for j in 0..MAXPLAYER {
        let p = &world.players[j];
        if !p.alive() || p.faction.is_some() {
            continue;
        }
        let id = j as u8;
        let (x, y, orbiting) = (p.x, p.y, p.orbiting);
        if p.team == Team::Kli {
            match orbiting.filter(|&k| world.planets[k].tribbles) {
                Some(k) => {
                    let t = e.progress.entry(id).or_insert(0);
                    *t += 1;
                    if *t >= 30 {
                        e.progress.remove(&id);
                        world.planets[k].tribbles = false;
                        let (name, who) = (world.planets[k].name, world.players[j].label());
                        world.events.push(GameEvent::Honour { player: id, text: "Tribble exterminator".into() });
                        announce(world, format!("The tribbles on {} flee screeching from {}!", name, who));
                    }
                }
                None => {
                    e.progress.remove(&id);
                }
            }
            for q in empire_ships_near(world, x, y, 2000.0) {
                if world.players[q].tribbles {
                    world.players[q].tribbles = false;
                    world.warn(q as u8, "The tribbles aboard flee screeching from the Klingon ship!");
                }
            }
            continue;
        }
        if let Some(k) = orbiting {
            let infested = world.planets.iter().filter(|pl| pl.tribbles).count();
            if world.planets[k].tribbles && !world.players[j].tribbles {
                world.players[j].tribbles = true;
                world.warn(id, "Tribbles have come aboard! They're eating the ship's stores. Don't take them anywhere else!");
            } else if world.players[j].tribbles && !world.planets[k].tribbles && world.planets[k].owner != Team::Kli && infested < TRIBBLE_CAP {
                world.planets[k].tribbles = true;
                let (name, who) = (world.planets[k].name, world.players[j].label());
                announce(world, format!("{} has carried tribbles to {}!", who, name));
            }
        }
        if world.players[j].tribbles {
            let q = &mut world.players[j];
            q.fuel = (q.fuel - 12.0).max(0.0);
        }
    }
    // Infested planets slowly lose armies as the tribbles eat their food.
    if tick % 150 == 0 {
        for pl in world.planets.iter_mut().filter(|pl| pl.tribbles && pl.armies > 1) {
            pl.armies -= 1;
        }
    }
    // Every 30 seconds they spread to a neighbouring world.
    if tick % 300 == 0 {
        let infested: Vec<usize> = (0..world.planets.len()).filter(|&k| world.planets[k].tribbles).collect();
        if infested.len() >= TRIBBLE_CAP {
            return;
        }
        let Some(&k) = infested.choose(&mut rng) else { return };
        let (x, y) = (world.planets[k].x, world.planets[k].y);
        let next = (0..world.planets.len())
            .filter(|&m| {
                let pl = &world.planets[m];
                !pl.tribbles && Team::PLAYABLE.contains(&pl.owner) && pl.owner != Team::Kli
            })
            .min_by(|&a, &b| {
                let (pa, pb) = (&world.planets[a], &world.planets[b]);
                dist(x, y, pa.x, pa.y).total_cmp(&dist(x, y, pb.x, pb.y))
            });
        if let Some(m) = next {
            world.planets[m].tribbles = true;
            let (from, to) = (world.planets[k].name, world.planets[m].name);
            announce(world, format!("The tribbles have bred their way from {} to {}!", from, to));
        }
    }
}

const CHANG_QUOTES: [&str; 5] = [
    "Cry havoc, and let slip the dogs of war!",
    "Once more unto the breach, dear friends, once more!",
    "I am constant as the northern star.",
    "Tickle us, do we not laugh? Prick us, do we not bleed? Wrong us, shall we not revenge?",
    "Our revels now are ended.",
];

/// General Chang: hunt ships under cloak, firing all the while. Any hit
/// lights up the Bird-of-Prey's exhaust for 20 seconds.
fn chang(world: &mut World, i: usize, tick: u32) {
    let mut rng = rand::thread_rng();
    let revealed = tick < world.players[i].revealed_until;
    world.players[i].cloaked = !revealed;
    if tick % 350 == 0 && rng.gen_bool(0.6) {
        let quote = CHANG_QUOTES.choose(&mut rng).unwrap();
        world.outbox.push(Outgoing {
            dest: Dest::All,
            msg: ChatMsg { kind: MsgKind::All, from: "Chang".into(), text: format!("\"{}\"", quote) },
        });
    }
    if tick % 2 != 0 {
        return;
    }
    if let Some((t, _)) = nearest_enemy(world, i, 30_000.0, None) {
        fight(world, i, t);
    }
}

/// Hirogen: mark the best pilot in the galaxy as prey and hunt them down,
/// fending off anyone else who gets close.
fn hirogen(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let prey_ok = e.prey.map_or(false, |p| world.players[p as usize].alive() && world.players[p as usize].marked);
    if !prey_ok {
        e.prey = None;
        let best = (0..MAXPLAYER)
            .filter(|&j| world.players[j].alive() && world.players[j].faction.is_none())
            .max_by(|&a, &b| {
                let (pa, pb) = (&world.players[a], &world.players[b]);
                pa.kills.total_cmp(&pb.kills).then(pa.total_kills.total_cmp(&pb.total_kills))
            });
        if let Some(j) = best {
            for p in world.players.iter_mut() {
                p.marked = false;
            }
            world.players[j].marked = true;
            e.prey = Some(j as u8);
            let who = world.players[j].label();
            announce(world, format!("The Hirogen have chosen their prey: {}! The hunt begins.", who));
            world.warn(j as u8, "You are being hunted by the Hirogen! Stay with your allies, or fight back: a hunter is worth an extra kill to you.");
        }
    }
    if tick % 2 != 0 {
        return;
    }
    if let Some((t, d)) = nearest_enemy(world, i, 2500.0, None) {
        if Some(t as u8) != e.prey && d < 2500.0 {
            fight(world, i, t);
            return;
        }
    }
    match e.prey {
        Some(p) => fight(world, i, p as usize),
        None => {
            if let Some((t, _)) = nearest_enemy(world, i, 20_000.0, None) {
                fight(world, i, t);
            }
        }
    }
}

/// Q: judge the leading empire, then vanish. His champion (if he summoned
/// one) hunts only the empire on trial.
fn q(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let Some(trial) = e.trial.as_mut() else { return };
    let team = trial.team;
    if world.players[i].ship == ShipType::QChampion {
        let (x, y) = (world.players[i].x, world.players[i].y);
        let target = (0..MAXPLAYER)
            .filter(|&j| world.players[j].alive() && world.players[j].team == team && world.players[j].faction.is_none())
            .min_by(|&a, &b| {
                let (pa, pb) = (&world.players[a], &world.players[b]);
                dist(x, y, pa.x, pa.y).total_cmp(&dist(x, y, pb.x, pb.y))
            });
        if let (Some(t), 0) = (target, tick % 2) {
            fight(world, i, t);
        }
        return;
    }
    if world.players[i].desired_speed != 0 {
        cmd(world, i, ClientMsg::Speed(0));
    }
    let verdict = match trial.kind {
        TrialKind::Hold => {
            let lost = trial.owned.iter().filter(|&&k| world.planets[k].owner != team).count();
            if lost > 2 {
                Some(false)
            } else if tick >= trial.deadline {
                Some(true)
            } else {
                None
            }
        }
        TrialKind::Tribute => {
            let (x, y) = (world.players[i].x, world.players[i].y);
            for j in empire_ships_near(world, x, y, 3000.0) {
                let p = &mut world.players[j];
                if p.team != team || p.armies == 0 {
                    continue;
                }
                let (n, who) = (p.armies, p.label());
                p.armies = 0;
                trial.tribute += n;
                let got = trial.tribute.min(5);
                announce(world, format!("{} offers Q {} armies ({} of 5).", who, n, got));
            }
            if trial.tribute >= 5 {
                Some(true)
            } else if tick >= trial.deadline {
                Some(false)
            } else {
                None
            }
        }
        TrialKind::Champion => {
            let champion = e.ships.iter().any(|&s| world.players[s as usize].ship == ShipType::QChampion && world.players[s as usize].alive());
            if !champion {
                Some(true)
            } else if tick >= trial.deadline {
                Some(false)
            } else {
                None
            }
        }
    };
    let Some(passed) = verdict else { return };
    if passed {
        // Reward: every ship made whole, and reinforcements at home.
        for p in world.players.iter().filter(|p| p.alive() && p.team == team && p.faction.is_none()) {
            world.events.push(GameEvent::Honour { player: p.id, text: "Passed the trial of Q".into() });
        }
        for p in world.players.iter_mut().filter(|p| p.alive() && p.team == team && p.faction.is_none()) {
            let s = p.stats();
            p.damage = 0.0;
            p.shield = s.max_shield;
            p.fuel = s.max_fuel;
        }
        let home = team.home_planet();
        let bonus = if world.planets[home].owner == team {
            world.planets[home].armies += 5;
            format!(" Their ships are restored and {} gains 5 armies.", world.planets[home].name)
        } else {
            " Their ships are restored.".to_string()
        };
        announce(world, format!("Q: \"Oh, very well. The {} pass... this time.\"{}", team.plural(), bonus));
    } else {
        // Penalty: Q hands the empire's richest colony to the weakest empire.
        let colony = (0..world.planets.len())
            .filter(|&k| world.planets[k].owner == team && world.planets[k].flags & PL_HOME == 0)
            .max_by_key(|&k| world.planets[k].armies);
        let weakest = Team::PLAYABLE
            .into_iter()
            .filter(|&t| t != team && world.team_planet_count(t) > 0)
            .min_by_key(|&t| world.team_planet_count(t));
        match (colony, weakest) {
            (Some(k), Some(w)) => {
                let pl = &mut world.planets[k];
                pl.owner = w;
                pl.known[w.idx()] = true;
                let name = pl.name;
                announce(world, format!("Q: \"How disappointing.\" He snaps his fingers and gives {} to the {}.", name, w.plural()));
                world.check_genocide(team, w);
            }
            _ => announce(world, "Q: \"How disappointing.\""),
        }
    }
    for s in e.ships.clone() {
        world.remove_player(s);
    }
}

/// Ferengi: plunder armies from poorly defended colonies and run for the
/// edge of the galaxy with them. They surrender to anything big.
fn ferengi(world: &mut World, i: usize, tick: u32) {
    let id = i as u8;
    let (x, y) = (world.players[i].x, world.players[i].y);
    let armies = world.players[i].armies;
    let big = empire_ships_near(world, x, y, 2500.0)
        .into_iter()
        .find(|&j| matches!(world.players[j].ship, ShipType::Battleship | ShipType::Starbase));
    if let Some(j) = big {
        if armies > 0 {
            world.loot.push(Loot { x, y, armies, ttl: 60 * UPS as i32 });
        }
        let p = &mut world.players[j];
        p.kills += 1.0;
        p.total_kills += 1.0;
        let who = p.label();
        let loot = match armies {
            0 => String::new(),
            1 => " and hands over 1 stolen army".to_string(),
            n => format!(" and hands over {} stolen armies", n),
        };
        announce(world, format!("A Ferengi marauder surrenders to {}{}! (+1 kill)", who, loot));
        world.remove_player(id);
        return;
    }
    if armies >= world.players[i].stats().max_armies {
        // Run for the nearest edge of the galaxy with the loot.
        let (ex, ey) = if x.min(GWIDTH - x) < y.min(GWIDTH - y) {
            (if x < GWIDTH / 2.0 { 0.0 } else { GWIDTH }, y)
        } else {
            (x, if y < GWIDTH / 2.0 { 0.0 } else { GWIDTH })
        };
        if tick % 3 == 0 {
            let max = world.players[i].stats().max_speed;
            steer_to(world, i, ex, ey, max);
        }
        if x.min(GWIDTH - x).min(y).min(GWIDTH - y) < 1500.0 {
            announce(world, format!("A Ferengi marauder escapes with {} stolen armies!", armies));
            world.remove_player(id);
        }
        return;
    }
    // Tractor passing ships and siphon their fuel.
    if tick % 20 == 0 {
        let near = nearest_enemy(world, i, 3500.0, None);
        match near {
            Some((t, _)) if world.players[i].tractor.is_none() => {
                cmd(world, i, ClientMsg::Tractor { target: Some(t as u8), pressor: false });
            }
            None if world.players[i].tractor.is_some() => {
                cmd(world, i, ClientMsg::Tractor { target: None, pressor: false });
            }
            _ => {}
        }
    }
    if let Some((t, false)) = world.players[i].tractor {
        let q = &mut world.players[t as usize];
        q.fuel = (q.fuel - 60.0).max(0.0);
    }
    if tick % 12 == 0 {
        if let Some((t, d)) = nearest_enemy(world, i, 4500.0, None) {
            if d < 4500.0 {
                let q = &world.players[t];
                let dir = dir_to(x, y, q.x, q.y);
                cmd(world, i, ClientMsg::Phaser(dir as u8));
            }
        }
    }
    // Loot the nearest undefended colony (any colony, failing that).
    let rich = |pl: &super::world::Planet| Team::PLAYABLE.contains(&pl.owner) && pl.flags & PL_HOME == 0 && pl.armies >= 3;
    let goal = nearest_planet(world, i, |pl| rich(pl) && empire_ships_near(world, pl.x, pl.y, 8000.0).is_empty())
        .or_else(|| nearest_planet(world, i, rich));
    let Some(k) = goal else { return };
    if go_orbit(world, i, k) && tick % 8 == 0 {
        world.planets[k].armies -= 1;
        world.players[i].armies += 1;
    }
}

/// The Swarm: tiny ships that latch onto hulls and drain them. They keep
/// just enough fuel in the tank for the victim to detonate them off.
fn swarm(world: &mut World, e: &mut Event, i: usize, n: usize, tick: u32) {
    let id = i as u8;
    if let Some(&t) = e.latched.get(&id) {
        let ti = t as usize;
        if world.players[ti].alive() {
            let a = n as f64 * TAU / 8.0 + tick as f64 * 0.05;
            let (tx, ty) = (world.players[ti].x, world.players[ti].y);
            let p = &mut world.players[i];
            p.x = tx + a.cos() * 300.0;
            p.y = ty + a.sin() * 300.0;
            p.speed = 0;
            p.desired_speed = 0;
            let q = &mut world.players[ti];
            if q.fuel > 200.0 {
                q.fuel = (q.fuel - 25.0).max(200.0);
            }
            if tick % 10 == 0 {
                world.inflict(ti, 1.5, Some(id), "was eaten away by the Swarm".into());
            }
            return;
        }
        e.latched.remove(&id);
    }
    if tick % 3 != 0 {
        return;
    }
    let Some((t, d)) = nearest_enemy(world, i, 40_000.0, None) else { return };
    if d < 700.0 {
        let first = !e.latched.values().any(|&v| v == t as u8);
        e.latched.insert(id, t as u8);
        if first {
            world.warn(t as u8, "Swarm ships have latched onto your hull! Detonate (d) to shake them off.");
        }
        return;
    }
    let (tx, ty) = (world.players[t].x, world.players[t].y);
    steer_to(world, i, tx, ty, 12);
}

pub const TEMPEST_RIM: f64 = 6500.0;
pub const TEMPEST_CORE: f64 = 1200.0;
/// Ticks a flipper must hold a ship to drag it into the core.
const DRAG_TICKS: i32 = 50;

/// The climbers of wave `level`: more of them, and nastier kinds, as the
/// levels go up (as in Atari's Tempest).
fn tempest_wave(level: u8) -> Vec<ShipType> {
    let n = (2 + level as usize).min(8);
    (0..n)
        .map(|k| match k % 6 {
            1 if level >= 2 => ShipType::Tanker,
            3 if level >= 3 => ShipType::Pulsar,
            5 if level >= 4 => ShipType::Fuseball,
            4 if level >= 5 => ShipType::Tanker,
            _ => ShipType::Flipper,
        })
        .collect()
}

/// Bring a climber out of the core onto a lane.
fn launch_climber(world: &mut World, e: &mut Event, kind: ShipType, lane: i32, prog: f64) -> bool {
    let Some(web) = world.tempest.clone() else { return false };
    let (x, y) = web.lane_point(lane, prog);
    let bounty = if kind == ShipType::Tanker { -5.0 } else { -8.0 };
    let name = kind.stats().name;
    let Some(id) = world.spawn_alien(name, Faction::Tempest, kind, x, y, bounty) else { return false };
    let dir = if rand::thread_rng().gen_bool(0.5) { 1 } else { -1 };
    e.climbers.insert(id, Climber { kind, lane, prog, dir, flip: None });
    e.ships.push(id);
    true
}

/// Shortest step (-1, 0 or 1) from lane `a` toward lane `b`.
fn lane_step(a: i32, b: i32, lanes: i32) -> i32 {
    let d = (b - a).rem_euclid(lanes);
    if d == 0 {
        0
    } else if d <= lanes / 2 {
        1
    } else {
        -1
    }
}

/// The Tempest: waves climb its web from the core. Flippers flip lane to
/// lane along the rim and drag trapped ships into the core; tankers split
/// into two flippers; pulsars electrify their lane; fuseballs roll around
/// the rim. The core is only exposed, briefly, when a wave has been cleared.
fn tempest(world: &mut World, e: &mut Event, tick: u32) {
    let mut rng = rand::thread_rng();
    let Some(web) = world.tempest.clone() else { return };
    let Some(core) = e.ships.iter().copied().find(|&s| world.players[s as usize].ship == ShipType::TempestCore) else { return };
    let lanes = web.lanes as i32;
    {
        let c = &mut world.players[core as usize];
        (c.x, c.y, c.speed, c.desired_speed) = (web.x, web.y, 0, 0);
        c.dir = (c.dir + 1.5).rem_euclid(256.0);
        // Shielded, the core knits itself back together.
        if !web.exposed {
            c.damage = (c.damage - 1.0).max(0.0);
        }
    }

    // Climbers that died: tankers split into two flippers (unless zapped).
    let dead: Vec<u8> = e.climbers.keys().copied().filter(|&id| !world.players[id as usize].alive()).collect();
    for id in dead {
        let c = e.climbers.remove(&id).unwrap();
        e.grabs.remove(&id);
        if c.kind == ShipType::Tanker && world.players[id as usize].state == PState::Exploding && world.players[id as usize].state_timer >= 9 {
            let zapped = tick.saturating_sub(web.zapped_at) <= 2 && web.zapped_at > 0;
            if !zapped {
                launch_climber(world, e, ShipType::Flipper, c.lane - 1, c.prog);
                launch_climber(world, e, ShipType::Flipper, c.lane + 1, c.prog);
            }
        }
    }

    // Waves.
    if web.exposed {
        if tick >= e.next_at {
            e.level += 1;
            e.queue = tempest_wave(e.level);
            // As in the arcade, each level brings a new shape of web.
            let t = world.tempest.as_mut().unwrap();
            t.exposed = false;
            t.level = e.level;
            t.shape = t.shape.next();
            t.lanes = t.shape.lanes();
            let shape = t.shape.name();
            announce(world, format!("Tempest level {}: the web reshapes into a {} and fills again!", e.level, shape));
        }
    } else if e.queue.is_empty() && e.climbers.is_empty() {
        if e.level == 0 {
            if tick >= e.next_at {
                e.level = 1;
                e.queue = tempest_wave(1);
            }
        } else {
            world.tempest.as_mut().unwrap().exposed = true;
            e.next_at = tick + 20 * UPS as u32;
            announce(world, "The web is clear: the Tempest core is exposed for 20 seconds! Fire everything (ships on the rim hit hardest)!");
        }
    } else if !e.queue.is_empty() && tick >= e.next_at {
        let kind = e.queue[0];
        if launch_climber(world, e, kind, rng.gen_range(0..lanes), 0.0) {
            e.queue.remove(0);
        }
        e.next_at = tick + 12;
    }

    // Who is trapped, and on which lane.
    let trapped: Vec<(usize, i32)> = (0..MAXPLAYER)
        .filter(|&j| world.players[j].alive() && world.players[j].trapped)
        .map(|j| (j, web.lane_of(world.players[j].x, world.players[j].y)))
        .collect();
    let climb = 0.004 + 0.0015 * e.level as f64;

    let ids: Vec<u8> = e.climbers.keys().copied().collect();
    // A flip takes half a second, a little quicker at higher levels.
    let flip_step = (0.2 + 0.02 * e.level as f64).min(0.4);
    let nearest_lane = |from: i32| {
        trapped
            .iter()
            .map(|&(_, l)| l)
            .min_by_key(|&l| (l - from).rem_euclid(lanes).min((from - l).rem_euclid(lanes)))
    };
    for id in ids {
        let i = id as usize;
        let mut c = e.climbers[&id];
        let grabbing = e.grabs.contains_key(&id);
        // Finish (or advance) a flip in progress.
        if let Some((to, t)) = c.flip {
            let t = t + flip_step;
            if t >= 1.0 {
                c.lane = to.rem_euclid(lanes);
                c.flip = None;
            } else {
                c.flip = Some((to, t));
            }
        }
        let flipping = c.flip.is_some();
        // Start a flip into the next lane, toward the nearest trapped ship
        // most of the time, otherwise at random.
        let start_flip = |c: &mut Climber, rng: &mut rand::rngs::ThreadRng| {
            let step = match nearest_lane(c.lane) {
                Some(l) if l != c.lane && rng.gen_bool(0.7) => lane_step(c.lane, l, lanes),
                _ => if rng.gen_bool(0.5) { 1 } else { -1 },
            };
            c.flip = Some((c.lane + step, 0.0));
        };
        match c.kind {
            ShipType::Flipper => {
                if c.prog < 1.0 {
                    if !flipping {
                        c.prog = (c.prog + climb).min(1.0);
                        if rng.gen_bool(0.02) {
                            start_flip(&mut c, &mut rng);
                        }
                    }
                } else if !grabbing && !flipping {
                    // On the rim: flip toward the nearest trapped ship.
                    match nearest_lane(c.lane) {
                        Some(l) if l != c.lane => c.flip = Some((c.lane + lane_step(c.lane, l, lanes), 0.0)),
                        _ => {}
                    }
                }
                // At the top of a lane it grabs whatever is on the rim in that
                // lane (as in the arcade, however wide the lane is).
                if c.prog >= 1.0 && !grabbing && c.flip.is_none() {
                    let victim = trapped.iter().find(|&&(_, l)| l == c.lane);
                    if let Some(&(j, _)) = victim {
                        e.grabs.insert(id, (j as u8, 0));
                        world.warn(j as u8, "A FLIPPER HAS YOU! Shoot it before it drags you into the core!");
                    }
                }
            }
            ShipType::Tanker => {
                // Tankers climb dead straight.
                c.prog += climb * 0.6;
                if c.prog >= 1.0 {
                    // At the rim a tanker bursts into two flippers.
                    world.remove_player(id);
                    e.climbers.remove(&id);
                    launch_climber(world, e, ShipType::Flipper, c.lane - 1, 1.0);
                    launch_climber(world, e, ShipType::Flipper, c.lane + 1, 1.0);
                    continue;
                }
            }
            ShipType::Pulsar => {
                if !flipping {
                    c.prog = (c.prog + climb * 0.7).min(0.85);
                    if rng.gen_bool(0.01) {
                        start_flip(&mut c, &mut rng);
                    }
                }
                // High on the web it electrifies its whole lane now and then.
                if c.prog > 0.5 && !flipping && tick % 25 == (id as u32 % 25) {
                    let (x1, y1) = web.lane_point(c.lane, c.prog);
                    let (x2, y2) = web.lane_point(c.lane, 1.0);
                    world.phasers.push(PhaserShot {
                        info: PhaserInfo { owner: id, x1: x1 as i32, y1: y1 as i32, x2: x2 as i32, y2: y2 as i32, hit: true },
                        ticks: 8,
                    });
                    for &(j, l) in &trapped {
                        if l == c.lane {
                            world.inflict(j, 25.0, Some(id), "was electrocuted by a pulsar".into());
                        }
                    }
                }
            }
            ShipType::Fuseball => {
                // Fuseballs ride the spokes (lane edges), drifting in and out,
                // and now and then dart across a lane to the next spoke.
                if !flipping {
                    c.prog = (c.prog + c.dir as f64 * climb * 1.4).clamp(0.05, 1.0);
                    if c.prog >= 1.0 {
                        // Lingers on the rim, then now and then dives back in.
                        if rng.gen_bool(0.03) {
                            c.dir = -1;
                        }
                    } else if c.prog <= 0.05 {
                        c.dir = 1;
                    } else if rng.gen_bool(0.02) {
                        c.dir = -c.dir;
                    }
                    if rng.gen_bool(if c.prog >= 1.0 { 0.06 } else { 0.025 }) {
                        start_flip(&mut c, &mut rng);
                    }
                }
            }
            _ => {}
        }
        // Where it is on the web: lanes are measured to their centres, so a
        // fuseball on a spoke sits half a lane over.
        let edge = if c.kind == ShipType::Fuseball { 0.5 } else { 0.0 };
        let (lane_f, turn) = match c.flip {
            Some((to, t)) => (c.lane as f64 + (to - c.lane) as f64 * t, t),
            None => (c.lane as f64, 0.0),
        };
        let (x, y) = web.web_point(lane_f + edge, c.prog);
        let p = &mut world.players[i];
        (p.x, p.y, p.speed, p.desired_speed) = (x, y, 0, 0);
        // Facing out of the tube; a flip turns the sprite over end to end.
        p.dir = (dir_to(web.x, web.y, x, y) + turn * 128.0).rem_euclid(256.0);
        // A fuseball can only be hit while it's crossing a lane.
        if c.kind == ShipType::Fuseball && c.flip.is_none() {
            p.phased_until = tick + 2;
        }
        e.climbers.insert(id, c);
        if c.kind == ShipType::Fuseball && c.prog > 0.9 {
            for &(j, _) in &trapped {
                if dist(world.players[j].x, world.players[j].y, x, y) < 800.0 {
                    world.inflict(j, 3.0, Some(id), "was fried by a fuseball".into());
                }
            }
        }
    }

    // Grabbed ships are held, then dragged into the core.
    let grabs: Vec<(u8, (u8, i32))> = e.grabs.iter().map(|(&k, &v)| (k, v)).collect();
    for (f, (v, held)) in grabs {
        let (fi, vi) = (f as usize, v as usize);
        if !world.players[fi].alive() || !world.players[vi].alive() || !world.players[vi].trapped {
            e.grabs.remove(&f);
            continue;
        }
        let q = &mut world.players[vi];
        (q.speed, q.desired_speed) = (0, 0);
        if held + 1 >= DRAG_TICKS {
            e.grabs.remove(&f);
            let who = world.players[vi].label();
            world.kill(vi, Some(f), "was dragged into the Tempest".into());
            announce(world, format!("{} is dragged down into the Tempest!", who));
            if let Some(c) = e.climbers.get_mut(&f) {
                c.prog = 0.3;
            }
        } else {
            e.grabs.insert(f, (v, held + 1));
        }
    }
}

// ----------------------------------------------------------------------
// The second wave: Nomad, Armus, nanites, Changelings, the Metrons, the
// Pakleds, Species 10-C, the Caretaker, the Horta and the Sphere Builders.

/// A clear stretch of space, at least `clear` from every planet.
fn open_spot(world: &World, clear: f64) -> Option<(f64, f64)> {
    let mut rng = rand::thread_rng();
    (0..400)
        .map(|_| (rng.gen_range(12_000.0..88_000.0), rng.gen_range(12_000.0..88_000.0)))
        .find(|&(x, y)| world.planets.iter().all(|pl| dist(x, y, pl.x, pl.y) > clear))
}

fn nearest_planet_name(world: &World, x: f64, y: f64) -> &'static str {
    world.planets.iter().min_by(|a, b| dist(x, y, a.x, a.y).total_cmp(&dist(x, y, b.x, b.y))).map_or("", |p| p.name)
}

fn award(world: &mut World, j: usize, kills: f64, honour: &str) {
    let p = &mut world.players[j];
    p.kills += kills;
    p.total_kills += kills;
    world.events.push(GameEvent::Honour { player: j as u8, text: honour.into() });
}

/// Wipe out a colony's armies a little at a time from orbit; `done` is
/// announced when the last one dies.
fn raid(world: &mut World, e: &mut Event, i: usize, tick: u32, done: &str) {
    let id = i as u8;
    let colony = |pl: &super::world::Planet| Team::PLAYABLE.contains(&pl.owner) && pl.armies > 0 && pl.flags & PL_HOME == 0;
    let goal = match e.goals.get(&id) {
        Some(&k) if colony(&world.planets[k]) => k,
        _ => match nearest_planet(world, i, colony) {
            Some(k) => {
                e.goals.insert(id, k);
                k
            }
            None => return,
        },
    };
    if go_orbit(world, i, goal) && tick % 6 == 0 {
        let pl = &mut world.planets[goal];
        pl.armies -= 1;
        if pl.armies <= 0 {
            let (name, old) = (pl.name, pl.owner);
            pl.armies = 0;
            pl.owner = Team::Ind;
            announce(world, done.replace("{}", name));
            world.check_genocide(old, Team::Ind);
            e.goals.remove(&id);
        }
    }
}

/// How close you must be for Nomad to hear you.
pub const NOMAD_EARSHOT: f64 = 3500.0;
const NOMAD_WORDS: [&str; 5] = ["imperfect", "error", "mistake", "flaw", "not perfect"];

/// Nomad: "sterilise" the most damaged ships and the weakest colonies.
/// Weapons can't touch it, but Kirk's trick works: tell it that it is
/// imperfect and it destroys itself.
fn nomad(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let (x, y) = (world.players[i].x, world.players[i].y);
    let talker = world
        .chatter
        .iter()
        .find(|(from, text)| {
            let q = &world.players[*from as usize];
            let t = text.to_lowercase();
            q.alive() && q.faction.is_none() && dist(x, y, q.x, q.y) < NOMAD_EARSHOT && NOMAD_WORDS.iter().any(|w| t.contains(w))
        })
        .map(|c| c.0);
    if let Some(k) = talker {
        let who = world.players[k as usize].label();
        announce(world, format!("{} tells Nomad it is imperfect. NOMAD: \"ERROR... ERROR... EXAMINE... STERILISE...\" The probe destroys itself!", who));
        world.kill(i, Some(k), "destroyed itself".into());
        return;
    }
    if tick % 3 != 0 {
        return;
    }
    // The most imperfect ship around: the most damaged one.
    let hurt = |j: usize| world.players[j].damage / world.players[j].stats().max_damage;
    let worst = (0..MAXPLAYER)
        .filter(|&j| {
            let q = &world.players[j];
            q.alive() && q.faction.is_none() && !q.cloaked && q.damage > 0.0 && dist(x, y, q.x, q.y) < 25_000.0
        })
        .max_by(|&a, &b| hurt(a).total_cmp(&hurt(b)));
    if let Some(t) = worst.or_else(|| nearest_enemy(world, i, 6000.0, None).map(|t| t.0)) {
        fight(world, i, t);
        return;
    }
    // Nobody to fix: sterilise the weakest colony instead.
    let id = i as u8;
    let weak = |pl: &super::world::Planet| Team::PLAYABLE.contains(&pl.owner) && (1..=6).contains(&pl.armies) && pl.flags & PL_HOME == 0;
    let goal = match e.goals.get(&id) {
        Some(&k) if weak(&world.planets[k]) => k,
        _ => match nearest_planet(world, i, weak) {
            Some(k) => {
                e.goals.insert(id, k);
                k
            }
            None => return,
        },
    };
    let (px, py) = (world.planets[goal].x, world.planets[goal].y);
    let d = dist(x, y, px, py);
    steer_to(world, i, px, py, if d < 2000.0 { 0 } else { 6 });
    if d < 2500.0 && tick % 10 == 0 {
        let pl = &mut world.planets[goal];
        pl.armies -= 1;
        if pl.armies <= 0 {
            let (name, old) = (pl.name, pl.owner);
            pl.armies = 0;
            pl.owner = Team::Ind;
            announce(world, format!("Nomad has \"sterilised\" {}: its colony was imperfect.", name));
            world.check_genocide(old, Team::Ind);
            e.goals.remove(&id);
        }
    }
}

/// The slick's size before it has been fed.
pub const ARMUS_BASE: f64 = 2500.0;
/// Seconds Armus waits, unfed, before it starts to wither...
const ARMUS_PATIENCE: u32 = 45;
/// ...and ticks it takes to wither away.
const ARMUS_WITHER: i32 = 300;

/// Armus: an oily slick that oozes after ships, holds them and eats them.
/// Every shot fired into it makes it bigger (see `World::inflict`); left
/// alone it withers away.
fn armus(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let id = i as u8;
    let unfed = tick.saturating_sub(world.players[i].last_hit) > ARMUS_PATIENCE * UPS as u32;
    if !unfed {
        e.dwell = 0;
    } else if world.players[i].swell > 0.0 {
        world.players[i].swell = (world.players[i].swell - 25.0).max(0.0);
    } else {
        e.dwell += 1;
    }
    if e.dwell >= ARMUS_WITHER {
        announce(world, "Starved of the anger it feeds on, Armus shrivels into a puddle of tar and is gone.");
        world.remove_player(id);
        return;
    }
    let (x, y) = (world.players[i].x, world.players[i].y);
    let r = ARMUS_BASE * (1.0 - e.dwell as f64 / ARMUS_WITHER as f64) + world.players[i].swell;
    world.zones.push(ZoneInfo { kind: ZoneKind::Slick, x: x as i32, y: y as i32, r: r as i32 });
    if tick % 5 == 0 {
        match nearest_enemy(world, i, 30_000.0, None) {
            Some((t, _)) => {
                let (tx, ty) = (world.players[t].x, world.players[t].y);
                steer_to(world, i, tx, ty, 1);
            }
            None => steer_to(world, i, x, y, 0),
        }
    }
    let held = empire_ships_near(world, x, y, r);
    e.progress.retain(|s, _| held.contains(&(*s as usize)));
    for j in held {
        if !e.progress.contains_key(&(j as u8)) {
            e.progress.insert(j as u8, 0);
            world.warn(j as u8, "Armus has engulfed your ship! It feeds on violence: every shot makes it stronger.");
        }
        let q = &mut world.players[j];
        // Held fast: barely able to move, and dragged toward the middle.
        q.speed = q.speed.min(2);
        q.x += (x - q.x) * 0.005;
        q.y += (y - q.y) * 0.005;
        if tick % 5 == 0 {
            world.inflict(j, 2.0, Some(id), "was swallowed by Armus".into());
            if !world.players[j].alive() {
                // Every ship it eats makes it bigger.
                let p = &mut world.players[i];
                p.swell = (p.swell + 1500.0).min(super::world::ARMUS_MAX_SWELL);
                p.last_hit = tick;
            }
        }
    }
}

/// Most ships the nanites can be in at once.
const NANITE_CAP: usize = 10;

/// Nanites: they make an infected ship's systems glitch, spread to ships
/// flying close, and are flushed out at a friendly repair world or burnt
/// out by a detonation (see `World::det_enemy`).
fn nanites(world: &mut World, e: &mut Event) {
    let mut rng = rand::thread_rng();
    let infected: Vec<usize> = (0..MAXPLAYER).filter(|&j| world.players[j].alive() && world.players[j].nanites).collect();
    e.progress.retain(|s, _| infected.contains(&(*s as usize)));
    for &j in &infected {
        let id = j as u8;
        let p = &world.players[j];
        let docked = p.orbiting.map_or(false, |k| world.planets[k].owner == p.team && world.planets[k].flags & PL_REPAIR != 0);
        if docked {
            let t = e.progress.entry(id).or_insert(0);
            *t += 1;
            if *t >= 30 {
                e.progress.remove(&id);
                world.players[j].nanites = false;
                world.warn(id, "The repair crews flush the nanites out of your systems.");
                continue;
            }
        } else {
            e.progress.remove(&id);
        }
        if !rng.gen_bool(1.0 / 40.0) {
            continue;
        }
        match rng.gen_range(0..3) {
            0 if world.players[j].orbiting.is_none() => {
                let p = &mut world.players[j];
                p.desired_dir = (p.desired_dir + rng.gen_range(-50.0..50.0)).rem_euclid(256.0);
                p.lock = Lock::None;
                world.warn(id, "Nanites in the helm! Your ship swings off course.");
            }
            1 if world.players[j].shields_up => {
                world.players[j].shields_up = false;
                world.warn(id, "Nanites in the shield generators! Shields down.");
            }
            _ => {
                world.handle(id, ClientMsg::Torp(rng.gen_range(0..=255)));
                world.warn(id, "Nanites in the fire control! A torpedo misfires.");
            }
        }
    }
    let mut count = infected.len();
    for &j in &infected {
        if !world.players[j].nanites {
            continue;
        }
        let (x, y) = (world.players[j].x, world.players[j].y);
        for k in empire_ships_near(world, x, y, 1500.0) {
            if k == j || count >= NANITE_CAP || world.players[k].nanites || !rng.gen_bool(1.0 / 120.0) {
                continue;
            }
            count += 1;
            world.players[k].nanites = true;
            let (from, to) = (world.players[j].label(), world.players[k].label());
            world.warn(k as u8, "Nanites have crossed over to your ship! Orbit a friendly repair world, or detonate (d) to burn them out.");
            announce(world, format!("Nanites have spread from {} to {}!", from, to));
        }
    }
}

/// Changelings: pose as empire ships (see `World::frame_for`), pick off
/// ships flying alone, and bomb colonies. A hit exposes one for a while,
/// and then it fights back.
fn changeling(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    if tick % 2 != 0 {
        return;
    }
    let (x, y) = (world.players[i].x, world.players[i].y);
    if tick < world.players[i].revealed_until {
        if let Some((t, _)) = nearest_enemy(world, i, 12_000.0, None) {
            fight(world, i, t);
            return;
        }
    }
    // A straggler: a ship with no friends within 8,000.
    let alone = |j: usize| {
        let q = &world.players[j];
        !world.players.iter().any(|o| {
            o.alive() && o.id != q.id && o.faction.is_none() && (o.team == q.team || world.allied(o.team, q.team)) && dist(o.x, o.y, q.x, q.y) < 8000.0
        })
    };
    let straggler = (0..MAXPLAYER)
        .filter(|&j| {
            let q = &world.players[j];
            q.alive() && q.faction.is_none() && !q.cloaked && dist(x, y, q.x, q.y) < 12_000.0 && alone(j)
        })
        .min_by(|&a, &b| {
            let (pa, pb) = (&world.players[a], &world.players[b]);
            dist(x, y, pa.x, pa.y).total_cmp(&dist(x, y, pb.x, pb.y))
        });
    if let Some(t) = straggler {
        fight(world, i, t);
        return;
    }
    raid(world, e, i, tick, "Changelings posing as its own garrison ships have wiped out the colony on {}!");
}

/// Radius of the Metrons' arena.
pub const ARENA_R: f64 = 5000.0;
/// How long the Metrons wait for a result.
const DUEL_TIME: u32 = 120 * UPS as u32;

/// The best pilot in the galaxy, and the best one of an empire at war with theirs.
fn pick_duellists(world: &World) -> Option<(usize, usize)> {
    let mut pilots: Vec<usize> = (0..MAXPLAYER)
        .filter(|&j| {
            let p = &world.players[j];
            p.alive() && p.faction.is_none() && Team::PLAYABLE.contains(&p.team) && !p.trapped && !matches!(p.ship, ShipType::Starbase | ShipType::Freighter)
        })
        .collect();
    pilots.sort_by(|&a, &b| {
        let (pa, pb) = (&world.players[a], &world.players[b]);
        pb.kills.total_cmp(&pa.kills).then(pb.total_kills.total_cmp(&pa.total_kills))
    });
    let &a = pilots.first()?;
    let &b = pilots.iter().find(|&&b| world.hostile(world.players[a].team, world.players[b].team))?;
    Some((a, b))
}

/// Let the Metrons' champions go (they can be hurt by anyone again).
fn end_duel(world: &mut World, e: &mut Event) {
    if let Some((a, b)) = e.duel.take() {
        for s in [a, b] {
            world.players[s as usize].only_hurt_by = None;
        }
    }
}

/// The Metrons: two champions fight in a sealed arena. Nobody else can get
/// in (or hurt them), and they can't get out. The winner is rewarded.
fn metrons(world: &mut World, e: &mut Event, tick: u32) {
    let Some((a, b)) = e.duel else { return };
    let (cx, cy) = e.waypoint;
    world.zones.push(ZoneInfo { kind: ZoneKind::Arena, x: cx as i32, y: cy as i32, r: ARENA_R as i32 });
    let (ai, bi) = (a as usize, b as usize);
    let (a_ok, b_ok) = (world.players[ai].alive(), world.players[bi].alive());
    if a_ok && b_ok && tick < e.started + DUEL_TIME {
        // The arena wall: the champions can't leave, and nobody else can get in.
        for j in 0..MAXPLAYER {
            let p = &mut world.players[j];
            if !p.alive() || p.faction == Some(Faction::Metrons) {
                continue;
            }
            if dist(p.x, p.y, cx, cy) < 1.0 {
                p.x += 1.0;
            }
            let d = dist(p.x, p.y, cx, cy);
            let champion = j == ai || j == bi;
            let edge = if champion { ARENA_R - 200.0 } else { ARENA_R + 400.0 };
            if (champion && d > edge) || (!champion && d < edge) {
                p.x = cx + (p.x - cx) / d * edge;
                p.y = cy + (p.y - cy) / d * edge;
                p.leave_orbit_pub();
            }
        }
        // Torpedoes can't cross the wall either.
        world.torps.retain(|t| {
            let inside = dist(t.x, t.y, cx, cy) < ARENA_R;
            inside == (t.owner == a || t.owner == b)
        });
        return;
    }
    end_duel(world, e);
    match (a_ok, b_ok) {
        (true, true) => announce(world, "The Metrons: \"Neither of you has the will to win. How disappointing.\" Both champions are released."),
        (false, false) => announce(world, "Both champions have fallen. The Metrons are unimpressed."),
        _ => {
            let w = if a_ok { ai } else { bi };
            award(world, w, 3.0, "Won the Metrons' arena");
            let p = &mut world.players[w];
            let s = p.stats();
            (p.damage, p.shield, p.fuel) = (0.0, s.max_shield, s.max_fuel);
            let who = p.label();
            announce(world, format!("{} wins the Metrons' contest! \"You have shown mercy... or at least skill.\" The victor's ship is restored (+3 kills).", who));
        }
    }
    for s in e.ships.clone() {
        world.remove_player(s);
    }
}

/// What a Pakled has taken.
#[derive(Clone, Copy, Debug)]
enum Stolen {
    Tech { victim: u8, tech: Tech },
    Upgrade { team: Team, which: usize },
}

fn stolen_name(s: Stolen) -> String {
    match s {
        Stolen::Tech { tech, .. } => tech.name().to_string(),
        Stolen::Upgrade { team, which } => format!("a level of the {} {} upgrade", team.name(), UPGRADES[which].0),
    }
}

/// Pakleds: lumbering clunkers that tractor a ship and take something
/// clever (advanced tech, or a level of an empire upgrade), then run for
/// the edge of the galaxy with it. Destroy the thief to get it back.
fn pakled(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let id = i as u8;
    let (x, y) = (world.players[i].x, world.players[i].y);
    if let Some(&loot) = e.stolen.get(&id) {
        let (ex, ey) = if x.min(GWIDTH - x) < y.min(GWIDTH - y) {
            (if x < GWIDTH / 2.0 { 0.0 } else { GWIDTH }, y)
        } else {
            (x, if y < GWIDTH / 2.0 { 0.0 } else { GWIDTH })
        };
        if tick % 3 == 0 {
            let max = world.players[i].stats().max_speed;
            steer_to(world, i, ex, ey, max);
        }
        if x.min(GWIDTH - x).min(y).min(GWIDTH - y) < 1500.0 {
            e.stolen.remove(&id);
            announce(world, format!("A Pakled clunker gets away with {}! \"We are strong.\"", stolen_name(loot)));
            world.remove_player(id);
        }
        return;
    }
    if tick % 2 != 0 {
        return;
    }
    let Some((t, d)) = nearest_enemy(world, i, 40_000.0, None) else { return };
    if d > 2500.0 {
        if world.players[i].tractor.is_some() {
            cmd(world, i, ClientMsg::Tractor { target: None, pressor: false });
        }
        e.progress.remove(&id);
        fight(world, i, t);
        return;
    }
    // Grab it and hold on.
    if world.players[i].tractor.map(|h| h.0) != Some(t as u8) {
        cmd(world, i, ClientMsg::Tractor { target: Some(t as u8), pressor: false });
    }
    let (tx, ty) = (world.players[t].x, world.players[t].y);
    steer_to(world, i, tx, ty, 2);
    let held = e.progress.entry(id).or_insert(0);
    *held += 2;
    if *held < 40 {
        return;
    }
    e.progress.remove(&id);
    cmd(world, i, ClientMsg::Tractor { target: None, pressor: false });
    steal(world, e, i, t);
}

fn steal(world: &mut World, e: &mut Event, i: usize, t: usize) {
    let mut rng = rand::thread_rng();
    let who = world.players[t].label();
    if !world.players[t].techs.is_empty() {
        let k = rng.gen_range(0..world.players[t].techs.len());
        let tech = world.players[t].techs.remove(k);
        e.stolen.insert(i as u8, Stolen::Tech { victim: t as u8, tech });
        announce(world, format!("The Pakleds tractor {} and take its {}! \"It is ours now.\" Destroy the thief to get it back!", who, tech.name()));
        return;
    }
    let team = world.players[t].team;
    if world.features.supply {
        let levels: Vec<usize> = (0..UPGRADES.len()).filter(|&u| world.supply[team.idx()].levels[u] > 0).collect();
        if let Some(&u) = levels.choose(&mut rng) {
            world.supply[team.idx()].levels[u] -= 1;
            let loot = Stolen::Upgrade { team, which: u };
            e.stolen.insert(i as u8, loot);
            announce(world, format!("The Pakleds tractor {} and make off with {}! Destroy the thief to get it back!", who, stolen_name(loot)));
            return;
        }
    }
    // Nothing clever aboard: they help themselves to fuel instead.
    let q = &mut world.players[t];
    q.fuel *= 0.4;
    world.warn(t as u8, "The Pakleds siphon off most of your fuel. \"We need things. Things to make us go.\"");
}

/// Give back whatever destroyed Pakleds had taken.
fn recover(world: &mut World, e: &mut Event) {
    let lost: Vec<u8> = e.stolen.keys().copied().filter(|s| !e.ships.contains(s) || !world.players[*s as usize].alive()).collect();
    for s in lost {
        let Some(loot) = e.stolen.remove(&s) else { continue };
        match loot {
            Stolen::Tech { victim, tech } => {
                let q = &mut world.players[victim as usize];
                if q.in_use && q.faction.is_none() && !q.techs.iter().any(|t| t.tier() == tech.tier()) {
                    q.techs.push(tech);
                }
                let who = q.label();
                announce(world, format!("The Pakled thief is destroyed, and {}'s {} is recovered!", who, tech.name()));
            }
            Stolen::Upgrade { team, which } => {
                let l = &mut world.supply[team.idx()].levels[which];
                *l = (*l + 1).min(MAX_UPGRADE);
                announce(world, format!("The Pakled thief is destroyed, and {} is recovered!", stolen_name(loot)));
            }
        }
    }
}

/// How far the anomaly's gravitational shear reaches.
const SHEAR: f64 = 7000.0;
/// The hyperfield beacons circle the anomaly this far out...
pub const BEACON_ORBIT: f64 = 10_000.0;
/// ...and a ship this close to one is holding it.
pub const BEACON_R: f64 = 1500.0;

/// Species 10-C's dark matter anomaly: it drifts across the galaxy wiping
/// out planets and hurling ships aside, and can't be hurt. Holding all
/// three of its hyperfield beacons at once makes first contact.
fn ten_c(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let mut rng = rand::thread_rng();
    let id = i as u8;
    let (x, y) = (world.players[i].x, world.players[i].y);
    let (wx, wy) = e.waypoint;
    if dist(x, y, wx, wy) < 3000.0 {
        e.waypoint = (rng.gen_range(10_000.0..90_000.0), rng.gen_range(10_000.0..90_000.0));
    }
    if tick % 5 == 0 {
        steer_to(world, i, wx, wy, 2);
    }
    for k in 0..world.planets.len() {
        let pl = &mut world.planets[k];
        if dist(x, y, pl.x, pl.y) < 3000.0 && (pl.armies > 0 || pl.owner != Team::Ind) {
            let (name, old) = (pl.name, pl.owner);
            pl.armies = 0;
            pl.owner = Team::Ind;
            announce(world, format!("The dark matter anomaly passes over {} and wipes it clean!", name));
            world.check_genocide(old, Team::Ind);
        }
    }
    for j in empire_ships_near(world, x, y, SHEAR) {
        let q = &mut world.players[j];
        let d = dist(q.x, q.y, x, y).max(1.0);
        let push = 45.0 * (1.0 - d / SHEAR);
        q.x = (q.x + (q.x - x) / d * push).clamp(0.0, GWIDTH);
        q.y = (q.y + (q.y - y) / d * push).clamp(0.0, GWIDTH);
        q.leave_orbit_pub();
        if d < 3500.0 && tick % 5 == 0 {
            world.inflict(j, 3.0, Some(id), "was torn apart by the dark matter anomaly".into());
        }
    }
    // The hyperfield beacons.
    let spin = tick as f64 * 0.002;
    let mut holders = Vec::new();
    let mut lit = 0;
    for k in 0..3 {
        let a = spin + k as f64 * TAU / 3.0;
        let bx = (x + a.cos() * BEACON_ORBIT).clamp(1000.0, GWIDTH - 1000.0);
        let by = (y + a.sin() * BEACON_ORBIT).clamp(1000.0, GWIDTH - 1000.0);
        let here = empire_ships_near(world, bx, by, BEACON_R);
        let kind = if here.is_empty() { ZoneKind::Beacon } else { ZoneKind::BeaconLit };
        if !here.is_empty() {
            lit += 1;
            holders.extend(here);
        }
        world.zones.push(ZoneInfo { kind, x: bx as i32, y: by as i32, r: BEACON_R as i32 });
    }
    if lit < 3 {
        e.dwell = 0;
        return;
    }
    e.dwell += 1;
    if e.dwell == 1 {
        announce(world, "All three hyperfield beacons are lit! Hold them...");
    }
    if e.dwell >= 50 {
        holders.sort_unstable();
        holders.dedup();
        for &j in &holders {
            award(world, j, 2.0, "Made first contact with Species 10-C");
        }
        announce(world, "First contact! Species 10-C answers the hyperfield with a pattern of its own. The anomaly stops, then slowly withdraws. (+2 kills to each ship at a beacon)");
        world.remove_player(id);
    }
}

/// The Caretaker's array starts off taking this share of the damage done
/// to it; every displacement wave it sends out adds more.
pub const CARETAKER_SHIELD: f64 = 0.15;
const WAVE_TICKS: i32 = 200;

/// The Caretaker: a heavily shielded array whose displacement waves pull
/// ships from all over the galaxy to it. Every wave weakens its shields.
fn caretaker(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let mut rng = rand::thread_rng();
    let (x, y) = (world.players[i].x, world.players[i].y);
    if tick % 15 == 0 {
        if let Some((t, _)) = nearest_enemy(world, i, 6000.0, None) {
            beam(world, i, t, 30.0, "was burned by the Caretaker's array");
        }
    }
    e.dwell += 1;
    if e.level > 0 && e.dwell < 20 {
        world.zones.push(ZoneInfo { kind: ZoneKind::Displacement, x: x as i32, y: y as i32, r: 800 + e.dwell * 700 });
    }
    if e.dwell < WAVE_TICKS {
        return;
    }
    e.dwell = 0;
    e.level = e.level.saturating_add(1);
    let far: Vec<usize> = (0..MAXPLAYER)
        .filter(|&j| {
            let q = &world.players[j];
            q.alive() && q.faction.is_none() && q.ship != ShipType::Starbase && !q.trapped && q.only_hurt_by.is_none() && dist(x, y, q.x, q.y) > 10_000.0
        })
        .collect();
    let mut names = Vec::new();
    for j in far.choose_multiple(&mut rng, 2).copied().collect::<Vec<_>>() {
        let a = rng.gen_range(0.0..TAU);
        let r = rng.gen_range(3500.0..5000.0);
        let q = &mut world.players[j];
        q.leave_orbit_pub();
        q.lock = Lock::None;
        q.tractor = None;
        q.x = (x + a.cos() * r).clamp(500.0, GWIDTH - 500.0);
        q.y = (y + a.sin() * r).clamp(500.0, GWIDTH - 500.0);
        names.push(q.label());
        world.warn(j as u8, "A displacement wave has pulled you across the galaxy to the Caretaker's array!");
    }
    let p = &mut world.players[i];
    p.adapt = (p.adapt + 0.15).min(1.0);
    let shielding = ((1.0 - p.adapt) * 100.0).round();
    if names.is_empty() {
        announce(world, format!("The Caretaker's array sends out a displacement wave, but finds no one to take. (Its shielding is down to {}%.)", shielding));
    } else {
        announce(world, format!("A displacement wave pulls {} to the Caretaker's array! (Its shielding is down to {}%.)", names.join(" and "), shielding));
    }
}

/// Ticks a ship must orbit the Horta's planet without firing to make peace.
const HORTA_PEACE: i32 = 100;

/// The Horta: tunnels through a colony killing its armies, then moves on
/// to the next. Kill it, or make peace: orbit its planet for ten seconds
/// without firing, and the colony gains armies and repair yards.
fn horta(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let id = i as u8;
    let colony = |pl: &super::world::Planet| Team::PLAYABLE.contains(&pl.owner) && pl.armies > 0 && pl.flags & PL_HOME == 0;
    let goal = match e.goals.get(&id) {
        Some(&k) if colony(&world.planets[k]) => k,
        _ => match nearest_planet(world, i, colony) {
            Some(k) => {
                e.goals.insert(id, k);
                e.progress.clear();
                k
            }
            None => return,
        },
    };
    let (px, py) = (world.planets[goal].x, world.planets[goal].y);
    if dist(world.players[i].x, world.players[i].y, px, py) > 900.0 {
        if tick % 4 == 0 {
            steer_to(world, i, px, py, 3);
        }
        return;
    }
    // Inside the rock.
    {
        let p = &mut world.players[i];
        (p.x, p.y, p.speed, p.desired_speed) = (px, py, 0, 0);
    }
    if tick % 25 == 0 {
        let pl = &mut world.planets[goal];
        pl.armies -= 1;
        if pl.armies <= 0 {
            let (name, old) = (pl.name, pl.owner);
            pl.armies = 0;
            pl.owner = Team::Ind;
            announce(world, format!("The Horta has killed every army on {}, and tunnels on to the next colony.", name));
            world.check_genocide(old, Team::Ind);
            e.goals.remove(&id);
            return;
        }
    }
    // Peace: a ship orbiting quietly.
    for j in 0..MAXPLAYER {
        let q = &world.players[j];
        let sid = j as u8;
        if !q.alive() || q.faction.is_some() || q.orbiting != Some(goal) {
            e.progress.remove(&sid);
            continue;
        }
        let firing = q.phaser_timer > 0 || world.torps.iter().any(|t| t.owner == sid);
        let n = e.progress.entry(sid).or_insert(0);
        *n = if firing { 0 } else { *n + 1 };
        if *n == 1 {
            world.warn(sid, "The Horta is watching you. Hold your fire and stay in orbit to make peace...");
        }
        if *n < HORTA_PEACE {
            continue;
        }
        let who = world.players[j].label();
        let pl = &mut world.planets[goal];
        pl.armies += 10;
        pl.flags |= PL_REPAIR;
        let name = pl.name;
        award(world, j, 2.0, "Made peace with the Horta");
        announce(
            world,
            format!("{} makes peace with the Horta! \"NO KILL I.\" Its children join the colony on {} (+10 armies) and its tunnels become repair yards.", who, name),
        );
        world.remove_player(id);
        return;
    }
}

/// What a Delphic sphere can do to the space around it.
const SPHERE_ANOMALIES: [(TerrainKind, &str, f64); 4] = [
    (TerrainKind::GravitonEddy, "spatial eddy", 5000.0),
    (TerrainKind::ChronitonField, "chroniton field", 4500.0),
    (TerrainKind::TetryonField, "tetryon field", 4500.0),
    (TerrainKind::FluidicRift, "spatial rift", 700.0),
];
/// Most anomalies one sphere keeps going at once.
const SPHERE_MAX: usize = 3;

/// Delphic sphere: scorches ships close by, and every so often warps the
/// space around it into an anomaly. They last until the sphere is destroyed.
fn sphere(world: &mut World, i: usize, n: usize, tick: u32) {
    let mut rng = rand::thread_rng();
    let id = i as u8;
    let (x, y) = (world.players[i].x, world.players[i].y);
    if tick % 50 == 0 {
        for j in empire_ships_near(world, x, y, 3500.0) {
            world.inflict(j, 15.0, Some(id), "was scorched by a Delphic sphere".into());
        }
    }
    if (tick + n as u32 * 50) % 200 != 0 {
        return;
    }
    let made = world.terrain.iter().filter(|t| t.owner == Some(id)).count();
    if made >= SPHERE_MAX {
        return;
    }
    let (kind, name, r) = *SPHERE_ANOMALIES.choose(&mut rng).unwrap();
    let a = rng.gen_range(0.0..TAU);
    let d = r.max(2000.0) + rng.gen_range(2500.0..4500.0);
    let at = ((x + a.cos() * d).clamp(3000.0, GWIDTH - 3000.0), (y + a.sin() * d).clamp(3000.0, GWIDTH - 3000.0));
    world.terrain.push(Terrain::planted(kind, name, at, r, id));
    if made == 0 {
        announce(world, format!("A Delphic sphere near {} is warping space into anomalies!", nearest_planet_name(world, x, y)));
    }
}

/// Radius of a Dyson sphere's shell. (In "Relics" it enclosed a star; here
/// it encloses a planet.)
pub const DYSON_R: f64 = 4500.0;
/// How close to the hatch counts as being in the doorway.
pub const HATCH_REACH: f64 = 900.0;
/// How far the tractor beam reaches from the hatch, and how hard it pulls.
const DYSON_REACH: f64 = 15_000.0;
const DYSON_PULL: f64 = 70.0;
/// Damage to the emitter that breaks its hold on a ship.
const DYSON_BREAK: f64 = 200.0;

/// What the emitter can still take (shields plus hull).
fn toughness(p: &super::world::Player) -> f64 {
    p.shield + p.stats().max_damage - p.damage
}

/// A Dyson sphere (TNG "Relics"): its shell encloses a planet, and nothing
/// crosses it except through the hatch. An automated tractor beam at the
/// hatch drags ships inside; the hatch only opens to take one in, but a ship
/// sitting in the doorway holds it open (as the Jenolan did). Destroying the
/// hatch emitter jams the doors open for good.
fn dyson(world: &mut World, e: &mut Event, i: usize, tick: u32) {
    let (cx, cy) = e.waypoint;
    let (hx, hy) = (world.players[i].x, world.players[i].y);
    let empire = |w: &World, j: usize| w.players[j].alive() && w.players[j].faction.is_none();
    e.inside.retain(|&s| empire(world, s as usize));
    // A ship in the doorway holds the doors open.
    let mut open = tick < e.next_at;
    let in_door = (0..MAXPLAYER).any(|j| empire(world, j) && Some(j as u8) != e.prey && dist(world.players[j].x, world.players[j].y, hx, hy) < HATCH_REACH);
    if open && in_door {
        e.next_at = tick + 10;
    }
    open = open || (in_door && tick < e.next_at);
    world.zones.push(ZoneInfo { kind: ZoneKind::DysonShell, x: cx as i32, y: cy as i32, r: DYSON_R as i32 });
    let hatch = if open { ZoneKind::DysonHatchOpen } else { ZoneKind::DysonHatch };
    world.zones.push(ZoneInfo { kind: hatch, x: hx as i32, y: hy as i32, r: HATCH_REACH as i32 });

    // The shell: ships inside stay in, ships outside stay out.
    for j in 0..MAXPLAYER {
        if !empire(world, j) {
            continue;
        }
        let sid = j as u8;
        let p = &mut world.players[j];
        if dist(p.x, p.y, cx, cy) < 1.0 {
            p.x += 1.0;
        }
        let d = dist(p.x, p.y, cx, cy);
        let doorway = open && dist(p.x, p.y, hx, hy) < HATCH_REACH;
        let inside = e.inside.contains(&sid);
        let wall = |p: &mut super::world::Player, r: f64| {
            p.x = cx + (p.x - cx) / d * r;
            p.y = cy + (p.y - cy) / d * r;
        };
        if inside {
            if d > DYSON_R + 300.0 && doorway {
                e.inside.retain(|&s| s != sid);
                world.warn(sid, "You slip out of the Dyson sphere through the open hatch!");
            } else if d > DYSON_R + 3000.0 {
                // Carried off by something else (a transwarp jump, the Caretaker...).
                e.inside.retain(|&s| s != sid);
            } else if d > DYSON_R - 300.0 && !doorway {
                wall(p, DYSON_R - 300.0);
            }
        } else if d < DYSON_R - 300.0 && doorway {
            e.inside.push(sid);
            world.warn(sid, "You fly in through the hatch of the Dyson sphere.");
        } else if d < DYSON_R + 300.0 && !doorway && Some(sid) != e.prey {
            wall(p, DYSON_R + 300.0);
            p.leave_orbit_pub();
        }
    }
    // Torpedoes can't get through the shell either, except by the open hatch.
    let inside = e.inside.clone();
    world.torps.retain(|t| {
        let through = open && dist(t.x, t.y, hx, hy) < HATCH_REACH;
        through || (dist(t.x, t.y, cx, cy) < DYSON_R) == inside.contains(&t.owner)
    });

    // The tractor beam.
    if let Some(v) = e.prey {
        let vi = v as usize;
        let lost = !empire(world, vi) || e.inside.contains(&v);
        let broken = !lost && toughness(&world.players[i]) < e.dwell as f64 - DYSON_BREAK;
        if lost || broken {
            e.prey = None;
            world.players[i].tractor = None;
            e.cooldown = tick + 10 * UPS as u32;
            if broken {
                let who = world.players[vi].label();
                announce(world, format!("Fire on the hatch emitter breaks the Dyson sphere's hold on {}!", who));
            }
        } else {
            let q = &mut world.players[vi];
            q.leave_orbit_pub();
            q.speed = q.speed.min(2);
            let d = dist(q.x, q.y, hx, hy).max(1.0);
            let step = DYSON_PULL.min(d);
            q.x += (hx - q.x) / d * step;
            q.y += (hy - q.y) / d * step;
            if d < 700.0 {
                // Swallowed: the hatch opens, takes it in, and shuts behind it.
                let r = dist(hx, hy, cx, cy).max(1.0);
                q.x = cx + (hx - cx) / r * (DYSON_R - 600.0);
                q.y = cy + (hy - cy) / r * (DYSON_R - 600.0);
                let who = q.label();
                e.inside.push(v);
                e.prey = None;
                e.next_at = tick + 3 * UPS as u32;
                e.cooldown = tick + 10 * UPS as u32;
                world.players[i].tractor = None;
                world.warn(v, "You're inside the Dyson sphere, and the hatch is closing! It only opens again to take another ship.");
                announce(world, format!("{} is dragged into the Dyson sphere!", who));
            }
        }
    } else if tick >= e.cooldown {
        let victim = (0..MAXPLAYER)
            .filter(|&j| {
                let q = &world.players[j];
                empire(world, j) && !e.inside.contains(&(j as u8)) && !q.trapped && q.only_hurt_by.is_none() && dist(q.x, q.y, hx, hy) < DYSON_REACH
            })
            .min_by(|&a, &b| {
                let (pa, pb) = (&world.players[a], &world.players[b]);
                dist(pa.x, pa.y, hx, hy).total_cmp(&dist(pb.x, pb.y, hx, hy))
            });
        if let Some(t) = victim {
            e.prey = Some(t as u8);
            e.dwell = toughness(&world.players[i]) as i32;
            world.players[i].tractor = Some((t as u8, false));
            let who = world.players[t].label();
            world.warn(t as u8, "The Dyson sphere's tractor beam has you! Engines can't break it; heavy fire on the hatch emitter can.");
            announce(world, format!("The Dyson sphere's tractor beam locks onto {}!", who));
        }
    }
}

// ----------------------------------------------------------------------
// The Kzinti and their Ringworld: they come once and stay for good.

/// The three Kzinti ships (from "The Slaver Weapon"), and what they fly.
pub const KZINTI_FLEET: [(&str, ShipType); 3] =
    [("Chuft-Captain", ShipType::KzintiDreadnought), ("Telepath", ShipType::KzintiCruiser), ("Flyer", ShipType::KzintiStriker)];
/// How long a destroyed Kzinti ship takes to come back.
pub const KZINTI_RESPAWN: u32 = 60 * UPS as u32;
/// The Ringworld's sections (other than Kzin), after Niven's Ringworld.
const RING_SECTIONS: [&str; 9] =
    ["Fist-of-God", "Great Ocean", "Map of Earth", "Spill Mtn", "Rim Wall", "Scrith Deck", "Heaven", "Shadow Sq", "Arch Point"];

/// The Kzinti fleet: always three ships, relaunched from Kzin when lost.
struct Kzinti {
    ships: [Option<u8>; 3],
    back_at: [u32; 3],
    /// Each ship's current conquest.
    goals: HashMap<u8, usize>,
}

fn kzinti_owns(world: &World, k: usize) -> bool {
    world.planets[k].owner == Team::Ind && world.planets[k].alien == Some(Faction::Kzinti)
}

/// Somewhere to hang a ring of ten sections around a planet, clear of
/// other planets, terrain and the edge of the galaxy.
fn design_ring(world: &World) -> Option<Ringworld> {
    let mut rng = rand::thread_rng();
    let mut centres: Vec<usize> = (0..world.planets.len()).filter(|&k| world.planets[k].flags & PL_HOME == 0).collect();
    centres.shuffle(&mut rng);
    for &c in &centres {
        let (cx, cy) = (world.planets[c].x, world.planets[c].y);
        for r in [9000.0, 8500.0, 8000.0, 7500.0, 7000.0] {
            for _ in 0..8 {
                let off = rng.gen_range(0.0..TAU);
                let pts: Vec<(f64, f64)> = (0..10).map(|k| {
                    let a = off + k as f64 * TAU / 10.0;
                    (cx + a.cos() * r, cy + a.sin() * r)
                }).collect();
                let clear = pts.iter().all(|&(x, y)| {
                    x.min(y).min(GWIDTH - x).min(GWIDTH - y) > 4000.0
                        && world.planets.iter().all(|pl| dist(x, y, pl.x, pl.y) > 3500.0)
                        && world.terrain.iter().all(|t| dist(x, y, t.x, t.y) > t.r + 1500.0)
                });
                if !clear {
                    continue;
                }
                let kzin = rng.gen_range(0..10);
                let mut flags = vec![
                    PL_REPAIR | PL_FUEL,
                    PL_REPAIR,
                    PL_FUEL,
                    PL_FUEL,
                    PL_AGRI,
                    PL_AGRI | PL_FUEL,
                    PL_AGRI,
                    PL_REPAIR | PL_AGRI,
                    0,
                ];
                flags.shuffle(&mut rng);
                let mut names = RING_SECTIONS.iter();
                let sections = pts
                    .iter()
                    .enumerate()
                    .map(|(k, &(x, y))| {
                        if k == kzin {
                            RingSection { name: "Kzin", x, y, flags: PL_REPAIR | PL_FUEL | PL_AGRI, armies: KZIN_ARMIES }
                        } else {
                            RingSection { name: names.next().unwrap(), x, y, flags: flags.pop().unwrap_or(0), armies: rng.gen_range(4..=10) }
                        }
                    })
                    .collect();
                return Some(Ringworld { x: cx, y: cy, r, sections, kzin });
            }
        }
    }
    None
}

impl Director {
    /// The Kzinti arrive with their Ringworld. There's only ever one, and it
    /// stays until the Kzinti have lost every section of it.
    fn arrive_kzinti(&mut self, world: &mut World) {
        if self.ring_gone || world.ring.is_some() || world.players.iter().filter(|p| !p.in_use).count() < KZINTI_FLEET.len() {
            return;
        }
        let Some(ring) = design_ring(world) else { return };
        let centre = world.planets.iter().find(|pl| pl.x == ring.x && pl.y == ring.y).map_or("", |pl| pl.name);
        world.ring = Some(ring);
        world.place_ring();
        self.kzinti = Some(Kzinti { ships: [None; 3], back_at: [0; 3], goals: HashMap::new() });
        announce(
            world,
            format!(
                "A RINGWORLD has appeared around {}: ten habitable sections that any empire can claim. One of them is Kzin, home of the warlike Kzinti, and their three warships are already launching. It stays for as long as the Kzinti hold a section of it; lose them all, and it jumps away with every section on it. \"Scream and leap!\"",
                centre
            ),
        );
        self.run_kzinti(world);
    }

    /// Keep the fleet at three ships and fly them. Once the Kzinti hold no
    /// section of the Ringworld, it breaks free and jumps away.
    fn run_kzinti(&mut self, world: &mut World) {
        if self.kzinti.is_none() || world.reset_timer > 0 {
            return;
        }
        if !(PLANETS.len()..world.planets.len()).any(|k| kzinti_owns(world, k)) {
            return self.ring_departs(world);
        }
        let Some(kz) = self.kzinti.as_mut() else { return };
        let Some(home) = world.kzin() else { return };
        let tick = world.tick;
        // Kzinti worlds grow, slowly.
        if tick % 450 == 0 {
            for k in 0..world.planets.len() {
                if kzinti_owns(world, k) && world.planets[k].armies < 12 {
                    world.planets[k].armies += 1;
                }
            }
        }
        for n in 0..KZINTI_FLEET.len() {
            match kz.ships[n] {
                Some(id) if world.players[id as usize].in_use && world.players[id as usize].faction == Some(Faction::Kzinti) => {
                    let p = &world.players[id as usize];
                    if p.alive() {
                        kzinti_ship(world, kz, id as usize, home, tick);
                        continue;
                    }
                    if p.state == PState::Exploding {
                        continue;
                    }
                    world.remove_player(id);
                    kz.goals.remove(&id);
                    kz.ships[n] = None;
                    kz.back_at[n] = tick + KZINTI_RESPAWN;
                }
                Some(_) => {
                    kz.ships[n] = None;
                    kz.back_at[n] = tick + KZINTI_RESPAWN;
                }
                None if tick >= kz.back_at[n] => {
                    // Launch from Kzin, or any Kzinti world, or failing that the ring itself.
                    let base = if kzinti_owns(world, home) { Some(home) } else { (0..world.planets.len()).find(|&k| kzinti_owns(world, k)) };
                    let (bx, by) = (world.planets[base.unwrap_or(home)].x, world.planets[base.unwrap_or(home)].y);
                    let a = n as f64 * TAU / 3.0;
                    let (name, ship) = KZINTI_FLEET[n];
                    if let Some(id) = world.spawn_alien(name, Faction::Kzinti, ship, bx + a.cos() * 1500.0, by + a.sin() * 1500.0, 10.0) {
                        kz.ships[n] = Some(id);
                        if kz.back_at[n] > 0 {
                            let from = world.planets[base.unwrap_or(home)].name;
                            announce(world, format!("The Kzinti {} launches again from {}.", name, from));
                        }
                    }
                }
                None => {}
            }
        }
    }
}

impl Director {
    /// The Kzinti have lost their last section: the Ringworld is free, and
    /// jumps out of the galaxy with all ten sections (whoever holds them)
    /// and the Kzinti fleet.
    fn ring_departs(&mut self, world: &mut World) {
        let n = PLANETS.len();
        let mut held: Vec<(Team, Vec<&'static str>)> = Vec::new();
        for k in n..world.planets.len() {
            let pl = &world.planets[k];
            if !Team::PLAYABLE.contains(&pl.owner) {
                continue;
            }
            match held.iter_mut().find(|h| h.0 == pl.owner) {
                Some(h) => h.1.push(pl.name),
                None => held.push((pl.owner, vec![pl.name])),
            }
        }
        // Ships at the sections are left behind in open space.
        for p in world.players.iter_mut() {
            if p.orbiting.map_or(false, |k| k >= n) {
                p.leave_orbit_pub();
            }
            if matches!(p.lock, Lock::Planet(k) if k >= n) {
                p.lock = Lock::None;
            }
        }
        // Other incursions let go of the sections too.
        for e in self.events.iter_mut() {
            e.goals.retain(|_, k| *k < n);
            if let Some(t) = e.trial.as_mut() {
                t.owned.retain(|&k| k < n);
            }
            if e.corners.iter().any(|&k| k >= n) {
                // A Tholian web strung across the ring re-anchors on the nearest real worlds.
                let (cx, cy) = (world.planets[e.corners[0]].x, world.planets[e.corners[0]].y);
                let mut near: Vec<usize> = (0..n).collect();
                near.sort_by(|&a, &b| dist(cx, cy, world.planets[a].x, world.planets[a].y).total_cmp(&dist(cx, cy, world.planets[b].x, world.planets[b].y)));
                e.corners = near[..3].to_vec();
            }
        }
        world.events.retain(|ev| match ev {
            GameEvent::PlanetTaken { planet, .. } | GameEvent::Bombed { planet, .. } | GameEvent::Reinforced { planet, .. } => *planet < n,
            _ => true,
        });
        if let Some(kz) = self.kzinti.take() {
            for id in kz.ships.into_iter().flatten() {
                world.remove_player(id);
            }
        }
        self.ring_gone = true;
        world.ring = None;
        world.planets.truncate(n);
        let taken = if held.is_empty() {
            String::new()
        } else {
            let v: Vec<String> = held.iter().map(|(t, names)| format!("the {}' {}", t.plural(), names.join(", "))).collect();
            format!(" It takes with it {}.", v.join(" and "))
        };
        announce(world, format!("The Kzinti have lost their last hold on the Ringworld. Free at last, it jumps out of the galaxy!{}", taken));
        for (t, _) in held {
            world.check_genocide(t, Team::Ind);
        }
    }
}

/// One Kzinti ship: patch up at home when hurt, defend Kzin, attack anything
/// that comes close ("scream and leap"), and conquer: first the Ringworld,
/// then the worlds around it. They carry their own warriors from home.
fn kzinti_ship(world: &mut World, kz: &mut Kzinti, i: usize, home: usize, tick: u32) {
    if tick % 2 != 0 {
        return;
    }
    let id = i as u8;
    let (x, y) = (world.players[i].x, world.players[i].y);
    let p = &world.players[i];
    let s = p.stats();
    let at_dock = p.orbiting.map_or(false, |k| kzinti_owns(world, k) && world.planets[k].flags & PL_REPAIR != 0);
    let needs_care = p.damage > s.max_damage * 0.6 || p.fuel < s.max_fuel * 0.2;
    let still_mending = at_dock && (p.damage > s.max_damage * 0.1 || p.fuel < s.max_fuel * 0.8);
    if needs_care || still_mending {
        let dock = nearest_planet(world, i, |pl| pl.owner == Team::Ind && pl.alien == Some(Faction::Kzinti) && pl.flags & PL_REPAIR != 0);
        if let Some(k) = dock {
            go_orbit(world, i, k);
            return;
        }
    }
    // Defend Kzin.
    let (hx, hy) = (world.planets[home].x, world.planets[home].y);
    if kzinti_owns(world, home) && dist(x, y, hx, hy) < 40_000.0 {
        let raider = (0..MAXPLAYER)
            .filter(|&j| {
                let q = &world.players[j];
                q.alive() && q.faction.is_none() && !q.cloaked && dist(q.x, q.y, hx, hy) < 10_000.0
            })
            .min_by(|&a, &b| {
                let (pa, pb) = (&world.players[a], &world.players[b]);
                dist(pa.x, pa.y, hx, hy).total_cmp(&dist(pb.x, pb.y, hx, hy))
            });
        if let Some(t) = raider {
            fight(world, i, t);
            return;
        }
    }
    // Scream and leap (though with warriors aboard, only at what's close).
    let leap = if world.players[i].armies > 0 { 4000.0 } else { 8000.0 };
    if let Some((t, _)) = nearest_enemy(world, i, leap, None) {
        fight(world, i, t);
        return;
    }
    // Take on warriors at a Kzinti world.
    let p = &world.players[i];
    let (armies, cap) = (p.armies, p.stats().max_armies);
    let at_depot = p.orbiting.filter(|&k| kzinti_owns(world, k) && world.planets[k].armies > 4);
    if armies < cap && (armies == 0 || at_depot.is_some()) {
        let depot = at_depot.or_else(|| {
            if kzinti_owns(world, home) && world.planets[home].armies > 4 {
                Some(home)
            } else {
                nearest_planet(world, i, |pl| pl.owner == Team::Ind && pl.alien == Some(Faction::Kzinti) && pl.armies > 4)
            }
        });
        if let Some(k) = depot {
            if go_orbit(world, i, k) && tick % 10 == 0 {
                world.planets[k].armies -= 1;
                world.players[i].armies += 1;
            }
            return;
        }
        if armies == 0 {
            return;
        }
    }
    // Conquest: Kzin first if it's been lost, then the Ringworld, then its neighbours.
    // Conquest is all about the Ringworld: they want every section of it.
    let target = kz.goals.get(&id).copied().filter(|&k| k < world.planets.len() && !kzinti_owns(world, k)).or_else(|| {
        if !kzinti_owns(world, home) {
            return Some(home);
        }
        // The easiest pickings: few defenders, not too far (each army is worth 1,500 units).
        let cost = |k: usize| dist(x, y, world.planets[k].x, world.planets[k].y) + world.planets[k].armies as f64 * 1500.0;
        (PLANETS.len()..world.planets.len()).filter(|&k| !kzinti_owns(world, k)).min_by(|&a, &b| cost(a).total_cmp(&cost(b)))
    });
    let Some(k) = target else {
        // The whole ring is theirs: patrol it, and hunt anything that comes near.
        let ring = world.ring.clone();
        if let Some(r) = ring {
            if let Some((t, _)) = nearest_enemy(world, i, r.r + 8000.0, None).filter(|&(t, _)| dist(world.players[t].x, world.players[t].y, r.x, r.y) < r.r + 8000.0) {
                fight(world, i, t);
            } else {
                go_orbit(world, i, home);
            }
        }
        return;
    };
    kz.goals.insert(id, k);
    if !go_orbit(world, i, k) || tick % 6 != 0 {
        return;
    }
    let pl = &mut world.planets[k];
    if pl.armies > 0 {
        // Bomb the defenders down first.
        pl.armies -= 1;
    } else if world.players[i].armies > 0 {
        world.players[i].armies -= 1;
        let pl = &mut world.planets[k];
        let (name, old) = (pl.name, pl.owner);
        pl.owner = Team::Ind;
        pl.alien = Some(Faction::Kzinti);
        pl.armies = 1;
        for t in Team::PLAYABLE {
            pl.known[t.idx()] = true;
        }
        let what = if k == home { "retaken their homeworld" } else { "claimed" };
        announce(world, format!("The Kzinti have {} {}! \"Scream and leap!\"", what, name));
        world.check_genocide(old, Team::Ind);
        kz.goals.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::bot;
    use crate::server::world::HitKind;

    /// Run each incursion on its own against a four-empire robot war, and
    /// check it arrives, does its thing, and ends cleanly.
    #[test]
    fn every_incursion_plays_out() {
        for kind in Faction::ALL {
            let mut world = World::new();
            let mut bots = Vec::new();
            for t in Team::PLAYABLE {
                for _ in 0..3 {
                    bots.push(bot::spawn(&mut world, t).unwrap());
                }
            }
            let mut director = Director::new(AlienConfig { kinds: vec![kind], interval: 30 });
            director.next_spawn = 5;
            let mut log = Vec::new();
            let mut peak_aliens = 0;
            for _ in 0..(UPS as u32 * 60 * 7) {
                director.tick(&mut world);
                for b in bots.iter_mut() {
                    b.think(&mut world);
                }
                world.tick();
                assert!(director.events.len() <= MAX_ACTIVE);
                let aliens = world.players.iter().filter(|p| p.in_use && p.faction.is_some()).count();
                peak_aliens = peak_aliens.max(aliens);
                log.extend(world.outbox.drain(..).map(|o| o.msg.text));
                world.warnings.clear();
            }
            let alien_msgs: Vec<&String> = log
                .iter()
                .filter(|m| {
                    ["Khan", "Gorn", "Tholian", "Fesarius", "Balok", "Terran", "planet killer", "amoeba", "Borg", "V'Ger",
                        "Crystalline", "probe", "8472", "Jem'Hadar", "wormhole", "ribble", "Chang", "Hirogen", "Q", "Ferengi",
                        "warm", "TEMPEST", "Tempest", "NOMAD", "Nomad", "Armus", "anite", "hangeling", "Metron", "Pakled", "10-C",
                        "Caretaker", "Horta", "Sphere", "DYSON", "Dyson", "Kzin"]
                        .iter()
                        .any(|k| m.contains(k))
                })
                .collect();
            println!("--- {:?}: peak {} alien ships, {} webs left", kind, peak_aliens, world.webs.len());
            for m in alien_msgs.iter().take(8) {
                println!("  {}", m);
            }
            // Tribbles have no ships: they arrive on a planet.
            assert!(peak_aliens > 0 || matches!(kind, Faction::Tribbles | Faction::Nanites), "{:?} never arrived", kind);
            assert!(!alien_msgs.is_empty(), "{:?} made no announcements", kind);
        }
    }

    /// Incursions never overlap with themselves, never exceed two at once,
    /// and an incursion that's under way is never restarted.
    #[test]
    fn incursions_do_not_repeat_while_active() {
        for _ in 0..6 {
            let mut world = World::new();
            let mut bots = Vec::new();
            for t in Team::PLAYABLE {
                for _ in 0..3 {
                    bots.push(bot::spawn(&mut world, t).unwrap());
                }
            }
            let mut director = Director::new(AlienConfig { kinds: Faction::ALL.to_vec(), interval: 20 });
            let mut previous: HashMap<Faction, u32> = HashMap::new();
            let mut arrivals = 0;
            for _ in 0..(UPS as u32 * 60 * 15) {
                director.tick(&mut world);
                for b in bots.iter_mut() {
                    b.think(&mut world);
                }
                world.tick();
                assert!(director.events.len() <= MAX_ACTIVE);
                let now: HashMap<Faction, u32> = director.events.iter().map(|e| (e.kind, e.started)).collect();
                assert_eq!(now.len(), director.events.len(), "an incursion is active twice");
                for (kind, started) in &now {
                    match previous.get(kind) {
                        Some(before) => assert_eq!(before, started, "{:?} restarted while still active", kind),
                        None => arrivals += 1,
                    }
                }
                previous = now;
                world.outbox.clear();
                world.warnings.clear();
            }
            assert!(arrivals > 3, "only {} incursions arrived", arrivals);
        }
    }

    /// Set up a world with one Federation cruiser (slot returned) near (x, y).
    fn with_cruiser(x: f64, y: f64) -> (World, u8) {
        let mut w = World::new();
        let id = w.add_player("Kirk", false).unwrap();
        w.join(id, Team::Fed, ShipType::Cruiser).unwrap();
        let p = &mut w.players[id as usize];
        p.x = x;
        p.y = y;
        (w, id)
    }

    fn alerts(w: &mut World) -> Vec<String> {
        w.outbox.drain(..).map(|o| o.msg.text).collect()
    }

    #[test]
    fn vger_merge_ends_the_threat() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Vger], interval: 9999 });
        d.spawn_kind(&mut w, Faction::Vger, None);
        let v = d.events[0].ships[0] as usize;
        let mut log = Vec::new();
        for _ in 0..140 {
            // Kirk holds position at V'Ger's core.
            let (vx, vy) = (w.players[v].x, w.players[v].y);
            if w.players[kirk as usize].alive() {
                w.players[kirk as usize].x = vx + 300.0;
                w.players[kirk as usize].y = vy;
            }
            d.tick(&mut w);
            w.tick();
            log.extend(alerts(&mut w));
        }
        assert!(log.iter().any(|m| m.contains("has joined with V'Ger")), "{:?}", log);
        assert!(d.events.is_empty(), "V'Ger should be gone");
    }

    #[test]
    fn crystal_shatters_on_resonance() {
        let mut w = World::new();
        let mut ids = Vec::new();
        for (k, name) in ["Kirk", "Picard", "Sisko"].iter().enumerate() {
            let id = w.add_player(name, false).unwrap();
            w.join(id, Team::Fed, ShipType::Cruiser).unwrap();
            let p = &mut w.players[id as usize];
            p.x = 40_000.0;
            p.y = 40_000.0 + k as f64 * 400.0;
            ids.push(id);
        }
        let c = w.spawn_alien("Crystalline Entity", Faction::Crystal, ShipType::CrystalEntity, 44_000.0, 40_400.0, 30.0).unwrap();
        // One ship's phasers alone barely scratch it.
        w.handle(ids[0], ClientMsg::Phaser(dir_to(40_000.0, 40_000.0, 44_000.0, 40_400.0) as u8));
        assert!(w.players[c as usize].alive());
        // Three ships together shatter it.
        for &id in &ids[1..] {
            let p = &w.players[id as usize];
            let dir = dir_to(p.x, p.y, 44_000.0, 40_400.0);
            w.handle(id, ClientMsg::Phaser(dir as u8));
        }
        assert!(!w.players[c as usize].alive(), "resonance should shatter the entity");
    }

    #[test]
    fn whale_probe_drains_and_is_answered() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Probe], interval: 9999 });
        d.spawn_kind(&mut w, Faction::Probe, None);
        let pr = d.events[0].ships[0] as usize;
        w.players[pr].x = 55_000.0;
        w.players[pr].y = 50_000.0;
        w.handle(kirk, ClientMsg::Speed(9));
        for _ in 0..40 {
            d.tick(&mut w);
            w.tick();
        }
        let k = &w.players[kirk as usize];
        assert!(k.speed <= 1 && !k.shields_up, "ship should be powerless near the probe");
        // Bring it "whales".
        w.players[kirk as usize].armies = 2;
        w.players[kirk as usize].x = w.players[pr].x + 2000.0;
        w.players[kirk as usize].y = w.players[pr].y;
        let mut log = Vec::new();
        for _ in 0..5 {
            d.tick(&mut w);
            w.tick();
            log.extend(alerts(&mut w));
        }
        assert!(log.iter().any(|m| m.contains("humpback")), "{:?}", log);
        assert!(d.events.is_empty());
    }

    #[test]
    fn bioships_only_fear_plasma() {
        let mut w = World::new();
        let b = w.spawn_alien("Bioship", Faction::Species8472, ShipType::Bioship, 50_000.0, 50_000.0, 15.0).unwrap() as usize;
        w.hit_kind = HitKind::Photon;
        w.inflict(b, 100.0, None, "torp".into());
        assert!((w.players[b].damage - 10.0).abs() < 1e-6, "photons do 10%");
        w.hit_kind = HitKind::Plasma;
        w.inflict(b, 100.0, None, "plasma".into());
        assert!((w.players[b].damage - 110.0).abs() < 1e-6, "plasma does full damage");
    }

    #[test]
    fn polaron_beams_ignore_shields() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let j = w.spawn_alien("Jem'Hadar", Faction::JemHadar, ShipType::JemHadarFighter, 52_000.0, 50_000.0, 5.0).unwrap();
        assert!(w.players[kirk as usize].shields_up);
        let before = w.players[kirk as usize].shield;
        let dir = dir_to(52_000.0, 50_000.0, 50_000.0, 50_000.0);
        w.handle(j, ClientMsg::Phaser(dir as u8));
        let k = &w.players[kirk as usize];
        assert_eq!(k.shield, before, "shields untouched");
        assert!(k.damage > 0.0, "hull takes the hit");
    }

    #[test]
    fn borg_and_8472_fight_each_other() {
        let mut w = World::new();
        let b = w.spawn_alien("Borg", Faction::Borg, ShipType::BorgCube, 50_000.0, 50_000.0, 20.0).unwrap() as usize;
        let s = w.spawn_alien("Bioship", Faction::Species8472, ShipType::Bioship, 53_000.0, 50_000.0, 15.0).unwrap() as usize;
        let g = w.spawn_alien("Gorn", Faction::Gorn, ShipType::GornRaider, 50_000.0, 53_000.0, 5.0).unwrap() as usize;
        assert!(w.at_war(b, s) && w.at_war(s, b));
        assert!(!w.at_war(b, g), "other aliens don't fight each other");
        assert!(nearest_enemy(&w, s, 20_000.0, None).map(|t| t.0) == Some(b));
    }

    #[test]
    fn chang_fires_cloaked_until_hit() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let c = w.spawn_alien("Chang", Faction::Chang, ShipType::BirdOfPrey, 54_000.0, 50_000.0, 20.0).unwrap() as usize;
        chang(&mut w, c, 1);
        assert!(w.players[c].cloaked);
        let dir = dir_to(54_000.0, 50_000.0, 50_000.0, 50_000.0);
        w.handle(c as u8, ClientMsg::Torp(dir as u8));
        assert_eq!(w.torps.len(), 1, "fires while cloaked");
        w.inflict(c, 5.0, Some(kirk), "torp".into());
        assert!(!w.players[c].cloaked, "a hit reveals it");
        let now = w.tick;
        chang(&mut w, c, now + 1);
        assert!(!w.players[c].cloaked, "stays visible for a while");
        chang(&mut w, c, now + 20 * UPS as u32 + 1);
        assert!(w.players[c].cloaked, "then cloaks again");
    }

    #[test]
    fn tribbles_spread_by_ship_and_flee_klingons() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let mut e = test_event(Faction::Tribbles);
        let (a, b) = (3, 4);
        w.planets[a].tribbles = true;
        w.players[kirk as usize].orbiting = Some(a);
        tribbles(&mut w, &mut e, 1);
        assert!(w.players[kirk as usize].tribbles, "picked up tribbles in orbit");
        w.players[kirk as usize].orbiting = Some(b);
        tribbles(&mut w, &mut e, 2);
        assert!(w.planets[b].tribbles, "carried them to the next planet");
        let growth = w.planets[b].armies;
        for _ in 0..200 {
            w.tick();
        }
        assert!(w.planets[b].armies <= growth, "no growth while infested");
        // A Klingon in orbit clears the planet, and scares them off the cruiser.
        let k = w.add_player("Koloth", false).unwrap();
        w.join(k, Team::Kli, ShipType::Cruiser).unwrap();
        let (bx, by) = (w.players[kirk as usize].x, w.players[kirk as usize].y);
        w.players[k as usize].x = bx;
        w.players[k as usize].y = by;
        w.players[k as usize].orbiting = Some(b);
        for t in 0..31 {
            tribbles(&mut w, &mut e, 10 + t);
        }
        assert!(!w.planets[b].tribbles);
        assert!(!w.players[kirk as usize].tribbles);
    }

    #[test]
    fn hirogen_trophy_and_bonus() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let kirk = kirk as usize;
        w.players[kirk].kills = 4.0;
        w.players[kirk].total_kills = 10.0;
        let h = w.spawn_alien("Hirogen", Faction::Hirogen, ShipType::HirogenHunter, 55_000.0, 50_000.0, 10.0).unwrap() as usize;
        let mut e = test_event(Faction::Hirogen);
        hirogen(&mut w, &mut e, h, 1);
        assert_eq!(e.prey, Some(kirk as u8));
        assert!(w.players[kirk].marked);
        // The prey kills a hunter: 1 + bounty credit, plus the bonus kill.
        w.kill(h, Some(kirk as u8), "test".into());
        assert!((w.players[kirk].kills - (4.0 + 2.0 + 1.0)).abs() < 1e-6, "{}", w.players[kirk].kills);
        // A hunter kills the prey: half this life's kills come off the career.
        let h2 = w.spawn_alien("Hirogen", Faction::Hirogen, ShipType::HirogenHunter, 55_000.0, 50_000.0, 10.0).unwrap() as usize;
        let before = w.players[kirk].total_kills;
        w.kill(kirk, Some(h2 as u8), "test".into());
        assert!((w.players[kirk].total_kills - (before - 3.5)).abs() < 1e-6);
        assert!(!w.players[kirk].marked);
    }

    #[test]
    fn q_champion_only_hurt_by_the_accused() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let r = w.add_player("Tomalak", false).unwrap();
        w.join(r, Team::Rom, ShipType::Cruiser).unwrap();
        let c = w.spawn_alien("Champion", Faction::Q, ShipType::QChampion, 52_000.0, 50_000.0, 15.0).unwrap() as usize;
        w.players[c].only_hurt_by = Some(Team::Fed);
        w.players[c].shields_up = false;
        w.inflict(c, 50.0, Some(r), "test".into());
        assert_eq!(w.players[c].damage, 0.0, "Romulan weapons pass through");
        w.inflict(c, 50.0, Some(kirk), "test".into());
        assert_eq!(w.players[c].damage, 50.0);
    }

    #[test]
    fn q_tribute_trial() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let qid = w.spawn_alien("Q", Faction::Q, ShipType::QEntity, 51_000.0, 50_000.0, 0.0).unwrap() as usize;
        let mut e = test_event(Faction::Q);
        e.ships = vec![qid as u8];
        e.trial = Some(Trial { team: Team::Fed, kind: TrialKind::Tribute, deadline: 1000, owned: vec![], tribute: 0 });
        let home = w.planets[0].armies;
        w.players[kirk as usize].armies = 5;
        q(&mut w, &mut e, qid, 1);
        assert_eq!(w.players[kirk as usize].armies, 0);
        assert_eq!(w.planets[0].armies, home + 5, "reward");
        assert!(!w.players[qid].in_use, "Q departs");
    }

    #[test]
    fn ferengi_loot_is_dropped_and_recovered() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let f = w.spawn_alien("Bok", Faction::Ferengi, ShipType::FerengiMarauder, 50_400.0, 50_000.0, 5.0).unwrap() as usize;
        w.players[f].armies = 4;
        w.kill(f, Some(kirk), "test".into());
        assert_eq!(w.loot.len(), 1);
        w.tick();
        assert_eq!(w.players[kirk as usize].armies, 4, "flew over the loot");
        assert!(w.loot.is_empty());
    }

    #[test]
    fn swarm_latches_and_detonation_shakes_it_off() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let mut e = test_event(Faction::Swarm);
        let s = w.spawn_alien("Swarm", Faction::Swarm, ShipType::SwarmShip, 50_500.0, 50_000.0, -8.0).unwrap() as usize;
        swarm(&mut w, &mut e, s, 0, 3);
        assert_eq!(e.latched.get(&(s as u8)), Some(&kirk));
        let fuel = w.players[kirk as usize].fuel;
        swarm(&mut w, &mut e, s, 0, 4);
        assert!(w.players[kirk as usize].fuel < fuel, "drains fuel");
        w.handle(kirk, ClientMsg::DetEnemy);
        assert!(!w.players[s].alive(), "detonation kills it");
    }

    fn say(w: &mut World, id: u8, text: &str) {
        w.handle(id, ClientMsg::Message { to: crate::proto::MsgTarget::All, text: text.into() });
    }

    #[test]
    fn nomad_is_talked_to_death() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let mut e = test_event(Faction::Nomad);
        let n = w.spawn_alien("Nomad", Faction::Nomad, ShipType::NomadProbe, 53_000.0, 50_000.0, 20.0).unwrap() as usize;
        e.ships = vec![n as u8];
        w.inflict(n, 500.0, Some(kirk), "test".into());
        assert_eq!(w.players[n].damage, 0.0, "weapons can't touch it");
        // Out of earshot, or the wrong words, and nothing happens.
        say(&mut w, kirk, "hello there");
        nomad(&mut w, &mut e, n, 1);
        assert!(w.players[n].alive());
        say(&mut w, kirk, "You are imperfect, Nomad!");
        nomad(&mut w, &mut e, n, 2);
        assert!(!w.players[n].alive(), "the logic bomb works");
        assert!(w.players[kirk as usize].kills >= 3.0, "{}", w.players[kirk as usize].kills);
    }

    #[test]
    fn armus_grows_when_shot_and_withers_when_ignored() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let mut e = test_event(Faction::Armus);
        let a = w.spawn_alien("Armus", Faction::Armus, ShipType::ArmusSlick, 51_000.0, 50_000.0, 0.0).unwrap() as usize;
        e.ships = vec![a as u8];
        w.inflict(a, 50.0, Some(kirk), "test".into());
        assert!(w.players[a].swell > 0.0 && w.players[a].damage == 0.0, "shots feed it");
        // It holds and hurts the cruiser inside it.
        w.players[kirk as usize].speed = 9;
        let now = w.tick;
        armus(&mut w, &mut e, a, now + 5);
        assert!(w.players[kirk as usize].speed <= 2);
        assert!(w.players[kirk as usize].shield < w.players[kirk as usize].stats().max_shield);
        // Left alone, it withers away.
        w.players[kirk as usize].x = 90_000.0;
        let start = w.tick;
        for t in 0..2000 {
            armus(&mut w, &mut e, a, start + ARMUS_PATIENCE * UPS as u32 + 1 + t);
            if !w.players[a].in_use {
                break;
            }
        }
        assert!(!w.players[a].in_use, "withered away");
    }

    #[test]
    fn nanites_spread_and_are_cured() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let k = kirk as usize;
        let s = w.add_player("Sulu", false).unwrap() as usize;
        w.join(s as u8, Team::Fed, ShipType::Cruiser).unwrap();
        (w.players[s].x, w.players[s].y) = (50_500.0, 50_000.0);
        w.players[k].nanites = true;
        let mut e = test_event(Faction::Nanites);
        for _ in 0..2000 {
            nanites(&mut w, &mut e);
            if w.players[s].nanites {
                break;
            }
        }
        assert!(w.players[s].nanites, "spread to the ship alongside");
        w.handle(kirk, ClientMsg::DetEnemy);
        assert!(!w.players[k].nanites && !w.players[s].nanites, "detonation burns them out");
        let frame = w.frame_for(kirk);
        assert!(frame.players.iter().all(|p| p.flags & crate::proto::pf::NANITES == 0));
    }

    #[test]
    fn changelings_look_like_your_own_until_hit() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let c = w.spawn_alien("Changeling", Faction::Changeling, ShipType::ChangelingShip, 52_000.0, 50_000.0, 8.0).unwrap();
        let seen = |w: &World| w.frame_for(kirk).players.into_iter().find(|p| p.id == c).unwrap();
        let p = seen(&w);
        assert_eq!((p.team, p.ship, p.faction), (Team::Fed, ShipType::Cruiser, None), "disguised");
        w.inflict(c as usize, 5.0, Some(kirk), "test".into());
        let p = seen(&w);
        assert_eq!((p.ship, p.faction), (ShipType::ChangelingShip, Some(Faction::Changeling)), "exposed");
    }

    #[test]
    fn metrons_arena_duel() {
        let (mut w, kirk) = with_cruiser(20_000.0, 20_000.0);
        let r = w.add_player("Tomalak", false).unwrap();
        w.join(r, Team::Rom, ShipType::Cruiser).unwrap();
        let o = w.add_player("Outsider", false).unwrap();
        w.join(o, Team::Kli, ShipType::Cruiser).unwrap();
        w.players[kirk as usize].kills = 3.0;
        w.players[r as usize].kills = 2.0;
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Metrons], interval: 9999 });
        d.spawn_kind(&mut w, Faction::Metrons, None);
        assert_eq!(d.events[0].duel, Some((kirk, r)));
        let (cx, cy) = d.events[0].waypoint;
        // The outsider can't get in, or hurt the champions.
        (w.players[o as usize].x, w.players[o as usize].y) = (cx, cy);
        d.tick(&mut w);
        assert!(dist(w.players[o as usize].x, w.players[o as usize].y, cx, cy) > ARENA_R);
        w.inflict(kirk as usize, 50.0, Some(o), "test".into());
        assert_eq!(w.players[kirk as usize].damage, 0.0);
        // The champions can't get out.
        (w.players[kirk as usize].x, w.players[kirk as usize].y) = (cx + ARENA_R * 2.0, cy);
        d.tick(&mut w);
        assert!(dist(w.players[kirk as usize].x, w.players[kirk as usize].y, cx, cy) < ARENA_R);
        // Kirk wins.
        let before = w.players[kirk as usize].kills;
        w.kill(r as usize, Some(kirk), "test".into());
        d.tick(&mut w);
        assert!(w.players[kirk as usize].kills >= before + 3.0);
        assert!(w.players[kirk as usize].only_hurt_by.is_none());
        for _ in 0..3 {
            d.tick(&mut w);
            w.tick();
        }
        assert!(d.events.is_empty(), "the Metrons leave");
    }

    #[test]
    fn pakleds_steal_tech_and_give_it_back_when_destroyed() {
        let (mut w, kirk) = with_cruiser(50_000.0, 50_000.0);
        let k = kirk as usize;
        w.players[k].techs = vec![Tech::QuantumTorps];
        let p = w.spawn_alien("Pakled", Faction::Pakleds, ShipType::PakledClunker, 51_500.0, 50_000.0, 5.0).unwrap() as usize;
        let mut e = test_event(Faction::Pakleds);
        e.ships = vec![p as u8];
        for t in 0..60 {
            pakled(&mut w, &mut e, p, t * 2);
        }
        assert!(w.players[k].techs.is_empty(), "stolen");
        assert!(e.stolen.contains_key(&(p as u8)));
        w.kill(p, Some(kirk), "test".into());
        recover(&mut w, &mut e);
        assert_eq!(w.players[k].techs, vec![Tech::QuantumTorps], "recovered");
    }

    #[test]
    fn ten_c_first_contact() {
        let mut w = World::new();
        let a = w.spawn_alien("10-C", Faction::TenC, ShipType::DarkMatterAnomaly, 50_000.0, 50_000.0, 0.0).unwrap() as usize;
        let mut e = test_event(Faction::TenC);
        e.ships = vec![a as u8];
        e.waypoint = (50_000.0, 50_000.0);
        w.inflict(a, 5000.0, None, "test".into());
        assert_eq!(w.players[a].damage, 0.0, "weapons are useless");
        let mut ids = Vec::new();
        for (k, name) in ["Burnham", "Tilly", "Saru"].iter().enumerate() {
            let id = w.add_player(name, false).unwrap();
            w.join(id, [Team::Fed, Team::Rom, Team::Kli][k], ShipType::Cruiser).unwrap();
            ids.push(id as usize);
        }
        let tick = 1000;
        for t in 0..60 {
            // Keep one ship on each beacon as they turn.
            let spin = (tick + t) as f64 * 0.002;
            for (k, &j) in ids.iter().enumerate() {
                let a = spin + k as f64 * TAU / 3.0;
                (w.players[j].x, w.players[j].y) = (50_000.0 + a.cos() * BEACON_ORBIT, 50_000.0 + a.sin() * BEACON_ORBIT);
            }
            w.zones.clear();
            ten_c(&mut w, &mut e, a, tick + t);
            if !w.players[a].in_use {
                break;
            }
        }
        assert!(!w.players[a].in_use, "first contact sends it away");
        assert!(ids.iter().all(|&j| w.players[j].kills >= 2.0));
    }

    #[test]
    fn caretaker_pulls_ships_in_and_weakens() {
        let (mut w, kirk) = with_cruiser(90_000.0, 90_000.0);
        let c = w.spawn_alien("Caretaker", Faction::Caretaker, ShipType::CaretakerArray, 20_000.0, 20_000.0, 30.0).unwrap() as usize;
        w.players[c].adapt = CARETAKER_SHIELD;
        let mut e = test_event(Faction::Caretaker);
        e.ships = vec![c as u8];
        w.players[c].shields_up = false;
        w.inflict(c, 100.0, Some(kirk), "test".into());
        let first = w.players[c].damage;
        assert!((first - 15.0).abs() < 1e-6, "heavily shielded: {}", first);
        for t in 0..WAVE_TICKS as u32 {
            caretaker(&mut w, &mut e, c, t * 7 + 1);
        }
        let k = &w.players[kirk as usize];
        assert!(dist(k.x, k.y, 20_000.0, 20_000.0) < 6000.0, "pulled to the array");
        w.inflict(c, 100.0, Some(kirk), "test".into());
        assert!(w.players[c].damage - first > 15.0 + 1e-6, "weaker after firing");
    }

    #[test]
    fn horta_peace() {
        let (mut w, kirk) = with_cruiser(0.0, 0.0);
        let k = kirk as usize;
        let pl = (0..w.planets.len()).find(|&p| w.planets[p].owner == Team::Fed && w.planets[p].flags & PL_HOME == 0).unwrap();
        let (px, py) = (w.planets[pl].x, w.planets[pl].y);
        w.planets[pl].armies = 8;
        w.planets[pl].flags &= !PL_REPAIR;
        let h = w.spawn_alien("Horta", Faction::Horta, ShipType::Horta, px, py, 10.0).unwrap() as usize;
        let mut e = test_event(Faction::Horta);
        e.ships = vec![h as u8];
        (w.players[k].x, w.players[k].y) = (px + 800.0, py);
        w.players[k].orbiting = Some(pl);
        for t in 1..=(HORTA_PEACE as u32 + 1) {
            horta(&mut w, &mut e, h, t * 100 + 1);
            if !w.players[h].in_use {
                break;
            }
        }
        assert!(!w.players[h].in_use, "peace made");
        assert!(w.planets[pl].armies >= 18 && w.planets[pl].flags & PL_REPAIR != 0);
        assert!(w.players[k].kills >= 2.0);
    }

    #[test]
    fn spheres_plant_anomalies_that_vanish_with_them() {
        let mut w = World::new();
        let s = w.spawn_alien("Sphere 41", Faction::SphereBuilders, ShipType::DelphicSphere, 50_000.0, 50_000.0, 8.0).unwrap() as usize;
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::SphereBuilders], interval: 9999 });
        let mut e = test_event(Faction::SphereBuilders);
        e.ships = vec![s as u8];
        d.events.push(e);
        for _ in 0..700 {
            d.tick(&mut w);
            w.tick();
        }
        let made = w.terrain.iter().filter(|t| t.owner == Some(s as u8)).count();
        assert!(made >= 2 && made <= SPHERE_MAX, "{}", made);
        w.kill(s, None, "test".into());
        d.tick(&mut w);
        assert!(w.terrain.is_empty(), "anomalies fade with the sphere");
    }

    /// The Kzinti arrive with their Ringworld: ten sections that are planets,
    /// one of them Kzin. It stays while they hold it, even after a galaxy reset, and
    /// the Kzinti always have their three ships.
    #[test]
    fn kzinti_ringworld_arrives_and_stays() {
        let mut w = World::new();
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Kzinti], interval: 9999 });
        d.spawn_kind(&mut w, Faction::Kzinti, None);
        assert_eq!(w.planets.len(), PLANETS.len() + 10, "ten sections");
        let home = w.kzin().unwrap();
        assert!(kzinti_owns(&w, home) && w.planets[home].name == "Kzin");
        assert_eq!((PLANETS.len()..w.planets.len()).filter(|&k| kzinti_owns(&w, k)).count(), 1);
        let ring = w.ring.clone().unwrap();
        for s in &ring.sections {
            assert!((dist(s.x, s.y, ring.x, ring.y) - ring.r).abs() < 1.0);
            assert!(PLANETS.iter().all(|p| dist(s.x, s.y, p.x, p.y) > 3000.0), "{} sits on a planet", s.name);
        }
        let fleet = |w: &World| w.players.iter().filter(|p| p.alive() && p.faction == Some(Faction::Kzinti)).count();
        assert_eq!(fleet(&w), 3);
        // They never go away on their own...
        for _ in 0..(UPS as u32 * 60 * 8) {
            d.tick(&mut w);
            w.tick();
            w.outbox.clear();
        }
        assert!(d.events.is_empty() && d.kzinti.is_some());
        // ...a lost ship comes back...
        let c = w.players.iter().position(|p| p.alive() && p.faction == Some(Faction::Kzinti)).unwrap();
        w.kill(c, None, "test".into());
        for _ in 0..(KZINTI_RESPAWN + 30) {
            d.tick(&mut w);
            w.tick();
        }
        assert_eq!(fleet(&w), 3, "always three ships");
        // ...and the Ringworld survives a galaxy reset, Kzin with it.
        w.reset_galaxy();
        assert_eq!(w.planets.len(), PLANETS.len() + 10);
        assert!(kzinti_owns(&w, home));
        // It never arrives twice.
        d.spawn(&mut w);
        assert_eq!(w.planets.len(), PLANETS.len() + 10);
    }

    #[test]
    fn ring_sections_are_planets_anyone_can_claim() {
        let (mut w, kirk) = with_cruiser(0.0, 0.0);
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Kzinti], interval: 9999 });
        d.spawn_kind(&mut w, Faction::Kzinti, None);
        let home = w.kzin().unwrap();
        let k = (PLANETS.len()..w.planets.len()).find(|&k| k != home).unwrap();
        // An empire ship orbits a section and lands armies on it.
        w.planets[k].armies = 0;
        let (px, py) = (w.planets[k].x, w.planets[k].y);
        let ki = kirk as usize;
        (w.players[ki].x, w.players[ki].y) = (px + 700.0, py);
        w.players[ki].kills = 2.0;
        w.players[ki].armies = 2;
        w.handle(kirk, ClientMsg::Orbit);
        assert_eq!(w.players[ki].orbiting, Some(k), "ships can dock at a section");
        w.handle(kirk, ClientMsg::BeamDown);
        for _ in 0..50 {
            w.tick();
        }
        assert_eq!(w.planets[k].owner, Team::Fed, "claimed for the Federation");
        // The client sees the ring and where each section is.
        let f = w.frame_for(kirk);
        let ring = f.ring.expect("ring in the frame");
        assert_eq!((ring.sections.len(), f.planets.len()), (10, PLANETS.len() + 10));
        assert_eq!(ring.sections[ring.kzin as usize].name, "Kzin");
        // A Kzinti ship carrying warriors takes an undefended section.
        // (With nobody close enough to leap at.)
        w.players[ki].orbiting = None;
        (w.players[ki].x, w.players[ki].y) = (1000.0, 1000.0);
        let j = (PLANETS.len()..w.planets.len()).find(|&j| j != home && j != k).unwrap();
        w.planets[j].armies = 0;
        let kz = d.kzinti.as_ref().unwrap().ships[0].unwrap() as usize;
        (w.players[kz].x, w.players[kz].y) = (w.planets[j].x, w.planets[j].y);
        w.players[kz].orbiting = Some(j);
        w.players[kz].armies = 3;
        let mut fleet = d.kzinti.take().unwrap();
        fleet.goals.insert(kz as u8, j);
        for t in 0..20 {
            kzinti_ship(&mut w, &mut fleet, kz, home, 6 * t);
        }
        assert!(kzinti_owns(&w, j), "claimed by the Kzinti");
    }

    /// Once the Kzinti hold no section, the Ringworld jumps away with every
    /// section, and nothing is left pointing at the planets it took.
    #[test]
    fn ringworld_departs_when_the_kzinti_lose_it() {
        let mut world = World::new();
        let mut bots = Vec::new();
        for t in Team::PLAYABLE {
            for _ in 0..3 {
                bots.push(bot::spawn(&mut world, t).unwrap());
            }
        }
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Kzinti, Faction::Tholian, Faction::Horta], interval: 20 });
        d.spawn_kind(&mut world, Faction::Kzinti, None);
        let n = PLANETS.len();
        // A Tholian web strung across the ring.
        d.spawn_kind(&mut world, Faction::Tholian, None);
        if let Some(e) = d.events.iter_mut().find(|e| e.kind == Faction::Tholian) {
            e.corners = vec![n, n + 1, n + 2];
        }
        for _ in 0..(UPS as u32 * 60) {
            d.tick(&mut world);
            for b in bots.iter_mut() {
                b.think(&mut world);
            }
            world.tick();
            world.outbox.clear();
        }
        // The Federation holds a section, with a ship in orbit there; the Kzinti lose the rest.
        let fed = n + 1;
        world.planets[fed].owner = Team::Fed;
        let (kirk, _) = (world.add_player("Kirk", false).unwrap(), ());
        world.join(kirk, Team::Fed, ShipType::Cruiser).unwrap();
        let k = kirk as usize;
        (world.players[k].x, world.players[k].y) = (world.planets[fed].x + 800.0, world.planets[fed].y);
        world.players[k].orbiting = Some(fed);
        for j in n..world.planets.len() {
            if kzinti_owns(&world, j) {
                world.planets[j].owner = Team::Rom;
                world.planets[j].alien = None;
            }
        }
        d.tick(&mut world);
        let log = alerts(&mut world);
        assert!(log.iter().any(|m| m.contains("jumps out of the galaxy") && m.contains("Federation")), "{:?}", log);
        assert_eq!(world.planets.len(), n, "the sections have gone");
        assert!(world.ring.is_none() && d.kzinti.is_none());
        assert!(!world.players.iter().any(|p| p.in_use && p.faction == Some(Faction::Kzinti)));
        assert_eq!(world.players[k].orbiting, None, "left in open space");
        // Everything carries on without the sections.
        for _ in 0..(UPS as u32 * 90) {
            d.tick(&mut world);
            for b in bots.iter_mut() {
                b.think(&mut world);
            }
            world.tick();
            world.outbox.clear();
        }
        assert_eq!(world.planets.len(), n, "it never comes back");
        world.frame_for(kirk);
    }

    /// Ring frenzy: once the Ringworld is here, robots fight only for it.
    #[test]
    fn robots_go_into_a_ring_frenzy() {
        let mut world = World::new();
        let mut bots = Vec::new();
        for t in Team::PLAYABLE {
            for _ in 0..3 {
                bots.push(bot::spawn(&mut world, t).unwrap());
            }
        }
        // Let the war get going, then bring in the Ringworld.
        let run = |world: &mut World, d: &mut Director, bots: &mut Vec<bot::Bot>, secs: u32| {
            for _ in 0..(UPS as u32 * secs) {
                d.tick(world);
                for b in bots.iter_mut() {
                    b.think(world);
                }
                world.tick();
                world.outbox.clear();
                world.warnings.clear();
            }
        };
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Kzinti], interval: 9999 });
        run(&mut world, &mut d, &mut bots, 30);
        d.spawn_kind(&mut world, Faction::Kzinti, None);
        let before: Vec<Team> = world.planets[..PLANETS.len()].iter().map(|pl| pl.owner).collect();
        run(&mut world, &mut d, &mut bots, 180);
        let ring = world.ring.clone().expect("still here");
        // Over the last minute, how much of the time robots spend at the ring
        // (its neighbourhood is under a fifth of the galaxy).
        let (mut near, mut all) = (0, 0);
        for _ in 0..60 {
            run(&mut world, &mut d, &mut bots, 1);
            for p in world.players.iter().filter(|p| p.alive() && p.robot && p.faction.is_none() && p.ship != ShipType::Freighter) {
                all += 1;
                near += usize::from(dist(p.x, p.y, ring.x, ring.y) < ring.r + 15_000.0);
            }
        }
        assert!(near * 100 >= all * 40, "robots at the ring {} of {} samples", near, all);
        // Nobody took anyone's ordinary planets from them...
        for (k, &was) in before.iter().enumerate() {
            let now = world.planets[k].owner;
            assert!(!(Team::PLAYABLE.contains(&was) && Team::PLAYABLE.contains(&now) && was != now), "{} changed hands", world.planets[k].name);
        }
        let claimed = (PLANETS.len()..world.planets.len()).filter(|&k| Team::PLAYABLE.contains(&world.planets[k].owner)).count();
        assert!(claimed > 0, "the empires are claiming sections");
    }

    #[test]
    fn tholians_web_three_planets() {
        let mut w = World::new();
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Tholian], interval: 9999 });
        d.spawn_kind(&mut w, Faction::Tholian, None);
        let corners = d.events[0].corners.clone();
        assert_eq!(corners.len(), 3);
        for _ in 0..(UPS as u32 * 40) {
            d.tick(&mut w);
            w.tick();
        }
        let top = w.players.iter().filter(|p| p.alive() && p.faction == Some(Faction::Tholian)).map(|p| p.speed).max();
        assert_eq!(top, Some(12), "racing at warp 12");
        // Strands reach every one of the three planets.
        for &k in &corners {
            let (px, py) = (w.planets[k].x, w.planets[k].y);
            let near = w.webs.iter().any(|s| dist(s.x1, s.y1, px, py) < 1500.0 || dist(s.x2, s.y2, px, py) < 1500.0);
            assert!(near, "no strand at {}", w.planets[k].name);
        }
        assert!(w.webs.len() > 30, "{} strands", w.webs.len());
    }

    #[test]
    fn dyson_sphere_swallows_ships_and_the_jenolan_gambit() {
        let (mut w, kirk) = with_cruiser(0.0, 0.0);
        let k = kirk as usize;
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Dyson], interval: 9999 });
        d.spawn_kind(&mut w, Faction::Dyson, None);
        let (cx, cy) = d.events[0].waypoint;
        let h = d.events[0].ships[0] as usize;
        let (hx, hy) = (w.players[h].x, w.players[h].y);
        assert!(w.terrain.iter().any(|t| t.name.contains("Jenolan")));
        // Kirk flies past, outside the shell: the beam drags him in.
        let r = dist(hx, hy, cx, cy);
        (w.players[k].x, w.players[k].y) = (cx + (hx - cx) / r * (DYSON_R + 5000.0), cy + (hy - cy) / r * (DYSON_R + 5000.0));
        for _ in 0..200 {
            d.tick(&mut w);
            w.tick();
            if d.events[0].inside.contains(&kirk) {
                break;
            }
        }
        assert!(d.events[0].inside.contains(&kirk), "swallowed");
        // Once the hatch shuts, he can't get out through the shell...
        for _ in 0..40 {
            d.tick(&mut w);
            w.tick();
        }
        assert!(w.players[k].alive() && d.events[0].inside.contains(&kirk));
        (w.players[k].x, w.players[k].y) = (cx - (hx - cx) / r * (DYSON_R + 1000.0), cy - (hy - cy) / r * (DYSON_R + 1000.0));
        d.tick(&mut w);
        assert!(dist(w.players[k].x, w.players[k].y, cx, cy) < DYSON_R, "held inside");
        // ...and a ship outside can't get in.
        let r2 = w.add_player("Tomalak", false).unwrap() as usize;
        w.join(r2 as u8, Team::Rom, ShipType::Cruiser).unwrap();
        (w.players[r2].x, w.players[r2].y) = (cx + 100.0, cy + 2000.0);
        d.tick(&mut w);
        assert!(dist(w.players[r2].x, w.players[r2].y, cx, cy) > DYSON_R);
        // The Jenolan gambit: a ship blowing up in the doorway wrecks the hatch.
        let before = toughness(&w.players[h]);
        (w.players[r2].x, w.players[r2].y) = (hx + 100.0, hy);
        w.kill(r2, None, "test".into());
        for _ in 0..12 {
            w.tick();
        }
        assert!(before - toughness(&w.players[h]) >= 500.0, "{} -> {}", before, toughness(&w.players[h]));
    }

    fn test_event(kind: Faction) -> Event {
        Event {
            kind,
            ships: Vec::new(),
            started: 0,
            goals: HashMap::new(),
            holds: HashMap::new(),
            waypoint: (0.0, 0.0),
            corners: Vec::new(),
            trail: HashMap::new(),
            lost_any: false,
            progress: HashMap::new(),
            dwell: 0,
            prey: None,
            latched: HashMap::new(),
            trial: None,
            climbers: HashMap::new(),
            grabs: HashMap::new(),
            level: 0,
            queue: Vec::new(),
            next_at: 0,
            duel: None,
            stolen: HashMap::new(),
            inside: Vec::new(),
            cooldown: 0,
        }
    }

    /// A Tempest with a Federation cruiser flying into it.
    fn tempest_world() -> (World, Director, usize) {
        let (mut w, kirk) = with_cruiser(0.0, 0.0);
        let mut d = Director::new(AlienConfig { kinds: vec![Faction::Tempest], interval: 30 });
        d.spawn_kind(&mut w, Faction::Tempest, None);
        let web = w.tempest.clone().expect("the web formed");
        let k = kirk as usize;
        // Halfway out to the rim, whatever shape the web is.
        let (rx, ry) = web.rim_toward(web.x + 1.0, web.y);
        (w.players[k].x, w.players[k].y) = ((web.x + rx) / 2.0, (web.y + ry) / 2.0);
        (w, d, k)
    }

    #[test]
    fn tempest_traps_ships_on_its_rim() {
        let (mut w, mut d, k) = tempest_world();
        w.tick();
        let web = w.tempest.clone().unwrap();
        assert!(w.players[k].trapped);
        let on_rim = |w: &World| {
            let (x, y) = (w.players[k].x, w.players[k].y);
            let (rx, ry) = web.rim_toward(x, y);
            dist(x, y, rx, ry) < 1.0
        };
        assert!(on_rim(&w), "pinned to the rim");
        // Trying to fly away just slides it along the rim.
        (w.players[k].dir, w.players[k].desired_dir, w.players[k].speed, w.players[k].desired_speed) = (64.0, 64.0, 9, 9);
        for _ in 0..50 {
            d.tick(&mut w);
            w.tick();
        }
        assert!(w.players[k].trapped && on_rim(&w));
    }

    #[test]
    fn tempest_core_is_only_exposed_when_the_web_is_clear() {
        let (mut w, mut d, k) = tempest_world();
        // Let the first wave climb out.
        for _ in 0..(UPS as u32 * 12) {
            d.tick(&mut w);
            w.tick();
            if !w.players[k].alive() {
                break;
            }
        }
        let core = w.players.iter().position(|p| p.in_use && p.ship == ShipType::TempestCore).unwrap();
        assert!(w.players.iter().any(|p| p.alive() && is_tempest_minion(p.ship)), "climbers on the web");
        w.inflict(core, 500.0, None, "test".into());
        assert_eq!(w.players[core].damage, 0.0, "shielded by its web");
        // The Superzapper clears the web, once.
        if !w.players[k].alive() {
            w.players[k].state = PState::Outfit;
            w.join(k as u8, Team::Fed, ShipType::Cruiser).unwrap();
            let web = w.tempest.clone().unwrap();
            let (rx, ry) = web.rim_toward(web.x + 1.0, web.y);
            (w.players[k].x, w.players[k].y) = ((web.x + rx) / 2.0, (web.y + ry) / 2.0);
            w.tick();
        }
        assert!(w.players[k].trapped);
        // No more climbers queued, so the zap clears the web for good.
        d.events[0].queue.clear();
        w.handle(k as u8, ClientMsg::DetEnemy);
        assert!(w.players[k].zapped);
        w.tick();
        d.tick(&mut w);
        assert!(!w.players.iter().any(|p| p.alive() && is_tempest_minion(p.ship)), "all zapped");
        for _ in 0..3 {
            d.tick(&mut w);
            w.tick();
        }
        assert!(w.tempest.as_ref().unwrap().exposed, "the core is exposed");
        let outsider = w.add_player("Outsider", false).unwrap();
        w.join(outsider, Team::Rom, ShipType::Cruiser).unwrap();
        w.inflict(core, 400.0, Some(outsider), "test".into());
        assert!((w.players[core].damage - 200.0).abs() < 1e-9, "ships off the web hit at half strength");
        w.inflict(core, 400.0, Some(k as u8), "test".into());
        assert!((w.players[core].damage - 600.0).abs() < 1e-9, "ships on the rim hit full on");
        // Only one Superzapper per ship.
        w.players[k].fuel = 5000.0;
        w.handle(k as u8, ClientMsg::DetEnemy);
        assert_eq!(w.outbox.iter().filter(|o| o.msg.text.contains("fires the SUPERZAPPER")).count(), 1, "only once");
    }

    #[test]
    fn flippers_drag_ships_into_the_core() {
        let (mut w, mut d, k) = tempest_world();
        w.tick();
        let web = w.tempest.clone().unwrap();
        let lane = web.lane_of(w.players[k].x, w.players[k].y);
        // Put a flipper right on the rim beside the ship.
        d.events[0].queue.clear();
        d.events[0].level = 1;
        assert!(launch_climber(&mut w, &mut d.events[0], ShipType::Flipper, lane, 1.0));
        w.players[k].shields_up = true;
        for _ in 0..(DRAG_TICKS + 5) {
            d.tick(&mut w);
            w.tick();
        }
        assert!(!w.players[k].alive(), "dragged down");
    }

    /// All three web shapes: lanes line up with the rim, the lane under a
    /// point is found again, and ships are pinned to that shape's rim.
    #[test]
    fn tempest_shapes() {
        for shape in TempestShape::ALL {
            let web = TempestWeb { x: 50_000.0, y: 50_000.0, r_in: TEMPEST_CORE, r_out: TEMPEST_RIM, shape, lanes: shape.lanes(), exposed: false, level: 1, zapped_at: 0 };
            for lane in 0..shape.lanes() as i32 {
                // The rim point of each lane is found in that lane again.
                let (x, y) = web.lane_point(lane, 1.0);
                assert_eq!(web.lane_of(x, y), lane, "{:?} lane {}", shape, lane);
                // Climbing moves outward toward the rim.
                let (x0, y0) = web.lane_point(lane, 0.0);
                assert!(dist(x0, y0, 50_000.0, 50_000.0) < dist(x, y, 50_000.0, 50_000.0));
            }
            // A ship flying in from any direction lands on this shape's rim.
            let (mut w, kirk) = with_cruiser(0.0, 0.0);
            w.tempest = Some(web.clone());
            for a in 0..24 {
                let ang = a as f64 / 24.0 * std::f64::consts::TAU;
                let k = kirk as usize;
                w.players[k].trapped = false;
                (w.players[k].x, w.players[k].y) = (50_000.0 + ang.cos() * 3000.0, 50_000.0 + ang.sin() * 3000.0);
                w.tick();
                let (x, y) = (w.players[k].x, w.players[k].y);
                let (rx, ry) = web.rim_toward(x, y);
                assert!(w.players[k].trapped && dist(x, y, rx, ry) < 1.0, "{:?} at angle {}", shape, a);
            }
        }
        assert_eq!(TempestShape::Circle.next().next().next(), TempestShape::Circle);
    }
}
