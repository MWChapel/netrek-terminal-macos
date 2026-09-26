//! Alien incursions (the `--aliens` option): episodes from Star Trek that
//! drop into the galaxy every so often. At most two are active at once.
//!
//! Aliens are server-controlled ships on `Team::Ind`, so they are hostile to
//! all four empires and take damage and give kill credit like any ship.
//! Their special powers (webs, planet eating, assimilation...) live here.

use super::bot::lead;
use super::world::{Dest, Lock, Outgoing, PhaserShot, Web, World};
use crate::consts::*;
use crate::proto::{ChatMsg, ClientMsg, MsgKind, PState, PhaserInfo};
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
    /// Planet the incursion is centred on (Khan's stronghold, the Tholian web).
    anchor: usize,
    /// Per-ship planet objectives.
    goals: HashMap<u8, usize>,
    /// Borg: cube -> (victim, ticks held in the tractor beam).
    holds: HashMap<u8, (u8, i32)>,
    waypoint: (f64, f64),
    web_radius: f64,
    web_angle: f64,
    lost_any: bool,
    /// V'Ger merge progress (ship, ticks at the core); 8472 beam charge.
    progress: HashMap<u8, i32>,
    /// Whale probe: ticks spent at the current planet.
    dwell: i32,
}

pub struct Director {
    cfg: AlienConfig,
    events: Vec<Event>,
    next_spawn: u32,
    /// Jem'Hadar: the wormhole opens (warning) a few seconds before they arrive.
    wormhole: Option<(u32, f64, f64)>,
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
        Faction::Borg | Faction::Vger | Faction::Species8472 => 5,
        _ => 4,
    };
    mins * 60 * UPS as u32
}

impl Director {
    pub fn new(cfg: AlienConfig) -> Director {
        let first = cfg.interval.clamp(1, 120) / 2;
        Director { cfg, events: Vec::new(), next_spawn: first as u32 * UPS as u32, wormhole: None }
    }

    pub fn tick(&mut self, world: &mut World) {
        if self.cfg.kinds.is_empty() {
            return;
        }
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
            if e.ships.is_empty() || timed_out {
                ended.push(k);
            }
        }
        for k in ended.into_iter().rev() {
            let e = self.events.remove(k);
            if e.ships.is_empty() {
                announce(world, defeat_text(e.kind));
            } else {
                for &id in &e.ships {
                    world.remove_player(id);
                }
                announce(world, withdraw_text(e.kind));
            }
        }
    }

    fn spawn(&mut self, world: &mut World) {
        let mut rng = rand::thread_rng();
        let mut active: Vec<Faction> = self.events.iter().map(|e| e.kind).collect();
        if self.wormhole.is_some() {
            active.push(Faction::JemHadar);
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
        let mut rng = rand::thread_rng();
        let free = world.players.iter().filter(|p| !p.in_use).count();
        let need = match kind {
            Faction::Khan | Faction::Tholian | Faction::Species8472 => 3,
            Faction::Gorn | Faction::Mirror => 4,
            Faction::JemHadar => 5,
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
                for n in ["Loskene", "Tholian", "Tholian"] {
                    spawn(world, n, ShipType::TholianVessel, near(ax, ay, 3000.0), 5.0);
                }
                format!("Tholian vessels have appeared near {}. Beware the Tholian web!", world.planets[anchor].name)
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
        };
        if ships.is_empty() {
            return;
        }
        announce(world, text);
        self.events.push(Event {
            kind,
            ships,
            started: world.tick,
            anchor,
            goals: HashMap::new(),
            holds: HashMap::new(),
            waypoint: (ax, ay),
            web_radius: 2500.0,
            web_angle: 0.0,
            lost_any: false,
            progress: HashMap::new(),
            dwell: 0,
        });
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
            Faction::Tholian => tholian(world, e, i, n, ids.len(), tick),
            Faction::Fesarius => fesarius(world, e, i, tick),
            Faction::Doomsday => doomsday(world, e, i, tick),
            Faction::Amoeba => amoeba(world, e, i, tick),
            Faction::Borg => borg(world, e, i, tick),
            Faction::Vger => vger(world, e, i, tick),
            Faction::Crystal => crystal(world, e, i, tick),
            Faction::Probe => probe(world, e, i, tick),
            Faction::Species8472 => species8472(world, e, i, n, tick),
            Faction::JemHadar => jemhadar(world, i, tick),
        }
    }
    if e.kind == Faction::Tholian {
        e.web_radius = (e.web_radius + 2.5).min(9000.0);
        e.web_angle += 0.004;
        if tick % 12 == 0 {
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

/// Tholians: circle a planet in formation, spinning a web between them.
fn tholian(world: &mut World, e: &mut Event, i: usize, n: usize, count: usize, tick: u32) {
    let (cx, cy) = (world.planets[e.anchor].x, world.planets[e.anchor].y);
    let a = e.web_angle + n as f64 * TAU / count.max(1) as f64;
    let (tx, ty) = (cx + a.cos() * e.web_radius, cy + a.sin() * e.web_radius);
    let p = &world.players[i];
    let d = dist(p.x, p.y, tx, ty);
    let speed = ((d / 600.0) as i32).clamp(1, p.stats().max_speed);
    if tick % 2 == 0 {
        steer_to(world, i, tx, ty, speed);
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

fn spin_web(world: &mut World, e: &Event) {
    let alive: Vec<u8> = e.ships.iter().copied().filter(|&id| world.players[id as usize].alive()).collect();
    let (cx, cy) = (world.planets[e.anchor].x, world.planets[e.anchor].y);
    let add = |world: &mut World, x1: f64, y1: f64, x2: f64, y2: f64, owner: u8| {
        if dist(x1, y1, x2, y2) < 20_000.0 {
            world.webs.push(Web { x1, y1, x2, y2, ttl: 90 * UPS as i32, owner });
        }
    };
    for k in 0..alive.len() {
        let (a, b) = (alive[k] as usize, alive[(k + 1) % alive.len()] as usize);
        if a != b {
            let (pa, pb) = (&world.players[a], &world.players[b]);
            let (x1, y1, x2, y2) = (pa.x, pa.y, pb.x, pb.y);
            add(world, x1, y1, x2, y2, alive[k]);
        }
        // Radial strands back to the centre every so often.
        if world.tick % 36 == 0 {
            let pa = &world.players[a];
            let (x1, y1) = (pa.x, pa.y);
            add(world, x1, y1, cx, cy, alive[k]);
        }
    }
    let excess = world.webs.len().saturating_sub(160);
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
        q.desired_speed = q.desired_speed.min(3);
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
        world.players[j].powerless_until = tick + 3;
    }
    // Answering the call: any ship carrying two armies close by.
    if let Some(&j) = empire_ships_near(world, x, y, 4000.0).iter().find(|&&j| world.players[j].armies >= 2) {
        let q = &mut world.players[j];
        q.armies -= 2;
        q.kills += 3.0;
        q.total_kills += 3.0;
        let who = q.label();
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
        if let Some((t, d)) = nearest_enemy(world, i, 9000.0, None) {
            let p = &world.players[i];
            let q = &world.players[t];
            let dir = dir_to(p.x, p.y, q.x, q.y);
            if d < 8000.0 {
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
                        "Crystalline", "probe", "8472", "Jem'Hadar", "wormhole"]
                        .iter()
                        .any(|k| m.contains(k))
                })
                .collect();
            println!("--- {:?}: peak {} alien ships, {} webs left", kind, peak_aliens, world.webs.len());
            for m in alien_msgs.iter().take(8) {
                println!("  {}", m);
            }
            assert!(peak_aliens > 0, "{:?} never arrived", kind);
            assert!(!alien_msgs.is_empty(), "{:?} made no announcements", kind);
        }
    }

    /// Incursions never overlap with themselves, never exceed two at once,
    /// and each arrival is followed by a defeat or withdrawal before the
    /// same kind arrives again.
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
            let mut active: Vec<Faction> = Vec::new();
            for _ in 0..(UPS as u32 * 60 * 15) {
                director.tick(&mut world);
                for b in bots.iter_mut() {
                    b.think(&mut world);
                }
                world.tick();
                let kinds: Vec<Faction> = director.events.iter().map(|e| e.kind).collect();
                for k in &kinds {
                    assert_eq!(kinds.iter().filter(|x| *x == k).count(), 1, "{:?} active twice", k);
                }
                for o in world.outbox.drain(..) {
                    if o.msg.from != "ALERT" {
                        continue;
                    }
                    for f in Faction::ALL {
                        if o.msg.text == defeat_text(f) || o.msg.text == withdraw_text(f) {
                            active.retain(|x| *x != f);
                        }
                    }
                    if let Some(f) = Faction::ALL.into_iter().find(|&f| {
                        let e = director.events.iter().find(|e| e.kind == f);
                        e.map_or(false, |e| e.started == world.tick - 1 || e.started == world.tick)
                    }) {
                        if o.msg.text.contains("entered") || o.msg.text.contains("appeared") || o.msg.text.contains("seized")
                            || o.msg.text.contains("rift") || o.msg.text.contains("drifting") || o.msg.text.contains("We are the Borg")
                            || o.msg.text.contains("attacking the colonies")
                        {
                            assert!(!active.contains(&f), "{:?} announced again while still active", f);
                            active.push(f);
                        }
                    }
                }
                world.warnings.clear();
                if world.reset_timer > 0 {
                    active.clear();
                }
            }
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
}

