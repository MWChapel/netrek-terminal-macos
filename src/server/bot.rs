//! Simple robot pilots, in the spirit of the old Netrek "hoser" robots.
//! Robots drive the same command interface human clients use.

use super::world::World;
use crate::consts::*;
use crate::proto::{ClientMsg, PState};
use rand::Rng;

const NAMES: &[&str] = &[
    "Kirk", "Kang", "Kor", "Koloth", "Tomalak", "Sulu", "Decker", "Garth", "Hunter", "Ruk", "Sarek",
    "Maltz", "Sela", "Worf", "Riker", "Chekov", "Styles", "Terrell", "Kruge", "Valkris",
];

#[derive(Clone, Copy, PartialEq)]
enum Goal {
    Fight,
    Retreat,
    Bomb(usize),
    Pickup(usize),
    Invade(usize),
    Patrol(usize),
}

pub struct Bot {
    pub id: u8,
    pub team: Team,
    goal: Goal,
    think_timer: i32,
    jink: f64,
    aggression: f64,
}

pub fn spawn(world: &mut World, team: Team) -> Option<Bot> {
    let mut rng = rand::thread_rng();
    let name = NAMES[rng.gen_range(0..NAMES.len())];
    let id = world.add_player(name, true)?;
    Some(Bot {
        id,
        team,
        goal: Goal::Patrol(team.home_planet()),
        think_timer: 0,
        jink: 0.0,
        aggression: rng.gen_range(0.6..1.0),
    })
}

fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

/// Direction to fire so a torp traveling at `speed` meets a target moving
/// with velocity (tvx, tvy).
pub fn lead(sx: f64, sy: f64, tx: f64, ty: f64, tvx: f64, tvy: f64, speed: f64) -> f64 {
    let (dx, dy) = (tx - sx, ty - sy);
    let a = tvx * tvx + tvy * tvy - speed * speed;
    let b = 2.0 * (dx * tvx + dy * tvy);
    let c = dx * dx + dy * dy;
    let disc = b * b - 4.0 * a * c;
    let t = if a.abs() < 1e-6 || disc < 0.0 {
        0.0
    } else {
        let r1 = (-b + disc.sqrt()) / (2.0 * a);
        let r2 = (-b - disc.sqrt()) / (2.0 * a);
        [r1, r2].into_iter().filter(|t| *t > 0.0).fold(f64::MAX, f64::min)
    };
    let t = if t == f64::MAX { 0.0 } else { t };
    dir_to(sx, sy, tx + tvx * t, ty + tvy * t)
}

impl Bot {
    pub fn think(&mut self, world: &mut World) {
        let i = self.id as usize;
        let mut rng = rand::thread_rng();
        let p = &world.players[i];
        if !p.in_use {
            return;
        }
        if p.state == PState::Outfit {
            let ship = match rng.gen_range(0..10) {
                0 => ShipType::Scout,
                1 | 2 => ShipType::Destroyer,
                3 | 4 => ShipType::Battleship,
                5 => ShipType::Assault,
                _ => ShipType::Cruiser,
            };
            let mut team = self.team;
            if !world.open_teams().contains(&team) {
                if let Some(&t) = world.open_teams().first() {
                    team = t;
                    self.team = t;
                }
            }
            let _ = world.join(self.id, team, ship);
            self.goal = Goal::Patrol(team.home_planet());
            return;
        }
        if !p.alive() {
            return;
        }
        self.think_timer -= 1;
        if self.think_timer > 0 {
            return;
        }
        self.think_timer = 2;

        let s = p.stats();
        let (x, y, team) = (p.x, p.y, p.team);
        let mut cmds: Vec<ClientMsg> = Vec::new();

        // Nearest visible enemy.
        let enemy = world
            .players
            .iter()
            .filter(|q| q.alive() && world.hostile(q.team, team) && (!q.cloaked || q.detected))
            // Ships hidden in a nebula or ion storm only show up close by.
            .filter(|q| !q.hidden || dist(x, y, q.x, q.y) < super::terrain::SENSOR_RANGE)
            // Don't waste fire on Q, or on a champion only another empire can hurt.
            .filter(|q| q.ship != ShipType::QEntity && q.only_hurt_by.map_or(true, |t| t == team))
            .map(|q| (q, dist(x, y, q.x, q.y)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(q, d)| (q.id, q.x, q.y, q.dir, q.speed, d));

        // Incoming enemy torps.
        let threats = world
            .torps
            .iter()
            .filter(|t| world.hostile(t.team, team) && t.explode == 0 && dist(x, y, t.x, t.y) < DETDIST)
            .count();

        // Empires worth attacking: teams with ships in play (T-mode style),
        // plus independent worlds. Fall back to anyone if nobody else is playing.
        let active: Vec<Team> = Team::PLAYABLE
            .into_iter()
            .filter(|&t| t != team && world.players.iter().any(|q| q.in_use && q.team == t && q.state != PState::Outfit))
            .collect();
        let is_target = |owner: Team| {
            world.hostile(owner, team) && (active.is_empty() || owner == Team::Ind || active.contains(&owner))
        };

        let hurt = p.damage / s.max_damage;
        let low_fuel = p.fuel / s.max_fuel < 0.25;
        let hot = p.etemp / s.max_etemp > 0.8;

        // Pick a goal.
        let prev = self.goal;
        let fight_range = 18000.0 * self.aggression;
        self.goal = if (hurt > 0.6 || low_fuel || (prev == Goal::Retreat && (hurt > 0.1 || p.fuel / s.max_fuel < 0.9)))
            && p.armies == 0
        {
            Goal::Retreat
        } else if matches!(enemy, Some((.., d)) if d < fight_range) && p.armies == 0 {
            Goal::Fight
        } else if p.armies > 0 && (p.armies >= p.max_armies_now() || !matches!(prev, Goal::Pickup(_))) {
            // Invade: prefer a planet we can take outright, otherwise wear
            // down the weakest one so teammates can finish it.
            match self.pick_planet(world, x, y, |pl| is_target(pl.owner) && (pl.armies as u32) < p.armies) {
                Some(k) => Goal::Invade(k),
                None => {
                    let weakest = world
                        .planets
                        .iter()
                        .enumerate()
                        .filter(|(_, pl)| is_target(pl.owner))
                        .min_by(|a, b| {
                            let ka = a.1.armies as f64 * 20000.0 + dist(x, y, a.1.x, a.1.y);
                            let kb = b.1.armies as f64 * 20000.0 + dist(x, y, b.1.x, b.1.y);
                            ka.total_cmp(&kb)
                        })
                        .map(|(k, _)| k);
                    match weakest {
                        Some(k) => Goal::Invade(k),
                        None => Goal::Patrol(team.home_planet()),
                    }
                }
            }
        } else if p.max_armies_now() > p.armies {
            match self.pick_planet(world, x, y, |pl| pl.owner == team && pl.armies > 5) {
                Some(k) => Goal::Pickup(k),
                None => Goal::Patrol(team.home_planet()),
            }
        } else {
            match self.pick_planet(world, x, y, |pl| is_target(pl.owner) && pl.armies > 4) {
                Some(k) => Goal::Bomb(k),
                None => Goal::Patrol(team.home_planet()),
            }
        };

        // Shields.
        let hostile_planet = world
            .planets
            .iter()
            .any(|pl| world.hostile(pl.owner, team) && pl.armies > 0 && dist(x, y, pl.x, pl.y) < PFIREDIST * 2.0);
        let danger = threats > 0 || hostile_planet || matches!(enemy, Some((.., d)) if d < 9000.0);
        if danger != p.shields_up && p.fuel > 300.0 && !(self.goal == Goal::Retreat && p.orbiting.is_some()) {
            cmds.push(ClientMsg::Shields);
        }
        if threats >= 3 && p.fuel > 1500.0 && rng.gen_bool(0.5) {
            cmds.push(ClientMsg::DetEnemy);
        }

        match self.goal {
            Goal::Fight => {
                let (tid, tx, ty, tdir, tspeed, d) = enemy.unwrap();
                let (tvx, tvy) = dir_vec(tdir as f64);
                let (tvx, tvy) = (tvx * tspeed as f64 * WARP1, tvy * tspeed as f64 * WARP1);
                let tspd = s.torp_speed * WARP1;
                let aim = lead(x, y, tx, ty, tvx, tvy, tspd);
                // Weave while closing in.
                if rng.gen_bool(0.15) {
                    self.jink = rng.gen_range(-40.0..40.0);
                }
                let course = if d > 7000.0 { aim + self.jink * 0.4 } else { aim + 64.0 * self.jink.signum() * 0.6 + self.jink };
                cmds.push(ClientMsg::Course(course.rem_euclid(256.0) as u8));
                let mut want = if d > 9000.0 { s.max_speed } else { (s.max_speed * 2 / 3).max(3) };
                if hot {
                    want = want.min(s.max_speed / 2);
                }
                cmds.push(ClientMsg::Speed(want as u8));
                let torp_range = tspd * s.torp_fuse as f64 * 0.8;
                let cool = p.wtemp / s.max_wtemp < 0.7;
                if d < torp_range && cool && p.fuel > s.torp_cost * 4.0 && rng.gen_bool(0.45 * self.aggression) {
                    let spread = rng.gen_range(-4.0..4.0);
                    cmds.push(ClientMsg::Torp((aim + spread).rem_euclid(256.0) as u8));
                }
                let phrange = PHASEDIST * s.phaser_damage / 100.0;
                if d < phrange * 0.6 && cool && p.fuel > s.phaser_cost * 3.0 && rng.gen_bool(0.25) {
                    cmds.push(ClientMsg::Phaser(dir_to(x, y, tx, ty) as u8));
                }
                // Plasma is the only thing that hurts Species 8472 bioships.
                let vs_bioship = world.players[tid as usize].ship == ShipType::Bioship;
                let plasma_odds = if vs_bioship { 0.35 } else { 0.05 };
                if s.plasma_damage > 0.0 && p.kills >= 2.0 && d < 9000.0 && rng.gen_bool(plasma_odds) {
                    cmds.push(ClientMsg::Plasma(aim as u8));
                }
            }
            Goal::Retreat => {
                let target = self
                    .pick_planet(world, x, y, |pl| pl.owner == team && pl.flags & PL_REPAIR != 0)
                    .unwrap_or(team.home_planet());
                if p.orbiting == Some(target) {
                    if !p.repair_mode {
                        cmds.push(ClientMsg::Repair);
                    }
                } else {
                    self.go_to(world, i, target, &mut cmds);
                }
            }
            Goal::Bomb(k) | Goal::Pickup(k) | Goal::Invade(k) | Goal::Patrol(k) => {
                if p.orbiting == Some(k) {
                    match self.goal {
                        Goal::Bomb(_) if !p.bombing => cmds.push(ClientMsg::Bomb),
                        Goal::Pickup(_) if !p.beam_up => cmds.push(ClientMsg::BeamUp),
                        Goal::Invade(_) if !p.beam_down => cmds.push(ClientMsg::BeamDown),
                        _ => {}
                    }
                } else {
                    self.go_to(world, i, k, &mut cmds);
                }
            }
        }

        // Terrain: steer clear of black holes, stars and comets, and slow
        // down among asteroids.
        if world.features.terrain {
            if let Some(dir) = super::terrain::escape(world, x, y) {
                cmds.push(ClientMsg::Course(dir as u8));
                cmds.push(ClientMsg::Speed(s.max_speed as u8));
            } else if super::terrain::in_asteroids(world, x, y) && p.speed > 4 {
                cmds.push(ClientMsg::Speed(4));
            }
        }

        for c in cmds {
            world.handle(self.id, c);
        }
    }

    fn go_to(&self, world: &World, i: usize, k: usize, cmds: &mut Vec<ClientMsg>) {
        let p = &world.players[i];
        if p.lock != super::world::Lock::Planet(k) {
            cmds.push(ClientMsg::LockPlanet(k as u8));
        }
        if p.desired_speed == 0 && p.orbiting.is_none() {
            cmds.push(ClientMsg::Speed(p.stats().max_speed as u8));
        }
    }

    fn pick_planet(
        &self,
        world: &World,
        x: f64,
        y: f64,
        ok: impl Fn(&super::world::Planet) -> bool,
    ) -> Option<usize> {
        world
            .planets
            .iter()
            .enumerate()
            .filter(|(_, pl)| ok(pl))
            .map(|(k, pl)| (k, dist(x, y, pl.x, pl.y)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }
}
