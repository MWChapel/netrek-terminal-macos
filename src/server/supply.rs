//! Supply lines (the --supply option). Fuel and farming worlds produce
//! supplies; each empire's robot freighter collects them and hauls them
//! home, where they buy upgrades for the whole empire. Destroy a convoy and
//! its cargo is lost.

use super::world::World;
use crate::consts::*;
use crate::proto::{ClientMsg, PState};
use rand::seq::SliceRandom;
use std::collections::HashMap;

/// Supplies a freighter can carry.
pub const HOLD: u32 = 10;
/// Supplies a world can stockpile waiting for pickup.
pub const PLANET_CAP: u32 = 12;
/// Seconds between production rounds.
const PRODUCE_SECS: u32 = 20;
/// Seconds before a lost freighter is replaced.
const RESPAWN_SECS: u32 = 30;
/// Empires with players buy upgrades themselves, until their stock gets this big.
const AUTO_BUY_STOCK: u32 = 60;

#[derive(Default)]
struct Convoy {
    id: Option<u8>,
    /// Tick the freighter was lost (to time its replacement).
    lost_at: Option<u32>,
    /// Supplies unloaded on the current trip home.
    delivered: u32,
}

#[derive(Default)]
pub struct Logistics {
    convoys: HashMap<Team, Convoy>,
}

impl Logistics {
    pub fn new() -> Logistics {
        Logistics::default()
    }

    /// The empire's freighter, if one is in space.
    pub fn freighter(&self, world: &World, team: Team) -> Option<u8> {
        self.convoys.get(&team).and_then(|c| c.id).filter(|&id| world.players[id as usize].alive())
    }

    pub fn tick(&mut self, world: &mut World) {
        if !world.features.supply || world.reset_timer > 0 {
            return;
        }
        let tick = world.tick;
        if tick % (PRODUCE_SECS * UPS as u32) == 0 {
            for pl in world.planets.iter_mut() {
                if Team::PLAYABLE.contains(&pl.owner) && pl.flags & PL_HOME == 0 && pl.flags & (PL_FUEL | PL_AGRI) != 0 {
                    pl.supply = (pl.supply + 1).min(PLANET_CAP);
                }
            }
        }
        for team in Team::PLAYABLE {
            self.run_convoy(world, team);
        }
        if tick % (5 * UPS as u32) == 0 {
            for team in Team::PLAYABLE {
                auto_buy(world, team);
            }
        }
    }

    fn run_convoy(&mut self, world: &mut World, team: Team) {
        let tick = world.tick;
        let c = self.convoys.entry(team).or_default();
        let alive_team = world.team_planet_count(team) > 0;
        // Keep a freighter in space for every empire still in the game.
        let id = match c.id {
            Some(id) if world.players[id as usize].in_use && world.players[id as usize].ship == ShipType::Freighter => id,
            _ => {
                c.id = None;
                if !alive_team || c.lost_at.map_or(false, |t| tick < t + RESPAWN_SECS * UPS as u32) {
                    return;
                }
                let Some(id) = world.add_player("Freighter", true) else { return };
                c.id = Some(id);
                id
            }
        };
        let i = id as usize;
        match world.players[i].state {
            PState::Alive => {}
            PState::Outfit => {
                if !alive_team {
                    world.remove_player(id);
                    c.id = None;
                    return;
                }
                if c.lost_at.map_or(true, |t| tick >= t + RESPAWN_SECS * UPS as u32) {
                    c.lost_at = None;
                    c.delivered = 0;
                    if world.join(id, team, ShipType::Freighter).is_err() {
                        world.remove_player(id);
                        c.id = None;
                    } else {
                        world.players[i].cargo = 0;
                    }
                }
                return;
            }
            _ => {
                c.lost_at.get_or_insert(tick);
                return;
            }
        }

        // Under attack: run for the nearest friendly world, whose guns help.
        let (x, y) = (world.players[i].x, world.players[i].y);
        let threat = world.players.iter().any(|q| {
            q.alive() && world.hostile(q.team, team) && !q.cloaked && ((q.x - x).powi(2) + (q.y - y).powi(2)).sqrt() < 7000.0
        });
        // Sheltering in orbit: keep loading or unloading there.
        if let (true, Some(k)) = (threat, world.players[i].orbiting) {
            if world.planets[k].owner == team {
                work_in_orbit(world, c, team, i, k);
                return;
            }
        }
        if threat {
            let refuge = world
                .planets
                .iter()
                .enumerate()
                .filter(|(_, pl)| pl.owner == team && pl.armies > 0)
                .min_by(|a, b| {
                    let da = (a.1.x - x).powi(2) + (a.1.y - y).powi(2);
                    let db = (b.1.x - x).powi(2) + (b.1.y - y).powi(2);
                    da.total_cmp(&db)
                })
                .map(|(k, _)| k);
            if let Some(k) = refuge {
                if world.players[i].orbiting != Some(k) {
                    steer(world, id, k);
                }
                return;
            }
        }
        let p = &world.players[i];
        let cargo = p.cargo;
        let waiting = |w: &World| {
            w.planets.iter().enumerate().filter(|(_, pl)| pl.owner == team && pl.flags & PL_HOME == 0 && pl.supply > 0).map(|(k, _)| k).collect::<Vec<_>>()
        };
        let pickups = waiting(world);
        let go_home = cargo >= HOLD || (cargo > 0 && pickups.is_empty());
        if go_home {
            let home = team.home_planet();
            let home = if world.planets[home].owner == team {
                home
            } else {
                match world.planets.iter().position(|pl| pl.owner == team) {
                    Some(k) => k,
                    None => return,
                }
            };
            if world.players[i].orbiting == Some(home) {
                work_in_orbit(world, c, team, i, home);
            } else {
                steer(world, id, home);
            }
            return;
        }
        // Collect from the world with the most waiting (nearest breaks ties).
        let (x, y) = (world.players[i].x, world.players[i].y);
        let best = pickups.into_iter().max_by(|&a, &b| {
            let (pa, pb) = (&world.planets[a], &world.planets[b]);
            let da = ((pa.x - x).powi(2) + (pa.y - y).powi(2)).sqrt();
            let db = ((pb.x - x).powi(2) + (pb.y - y).powi(2)).sqrt();
            (pa.supply as f64 * 10_000.0 - da).total_cmp(&(pb.supply as f64 * 10_000.0 - db))
        });
        match best {
            Some(k) if world.players[i].orbiting == Some(k) => work_in_orbit(world, c, team, i, k),
            Some(k) => steer(world, id, k),
            None => {
                // Nothing to collect yet: wait at home.
                let home = team.home_planet();
                if world.planets[home].owner == team && world.players[i].orbiting != Some(home) {
                    steer(world, id, home);
                }
            }
        }
    }
}

/// A freighter in orbit around one of its own worlds: unload at the home
/// world (or wherever it delivers), load anywhere supplies are waiting.
fn work_in_orbit(world: &mut World, c: &mut Convoy, team: Team, i: usize, k: usize) {
    if world.tick % 5 != 0 {
        return;
    }
    let home = world.planets[k].flags & PL_HOME != 0;
    if home && world.players[i].cargo > 0 {
        world.players[i].cargo -= 1;
        world.supply[team.idx()].stock += 1;
        c.delivered += 1;
        if world.players[i].cargo == 0 {
            let (n, stock) = (c.delivered, world.supply[team.idx()].stock);
            c.delivered = 0;
            let name = world.planets[k].name;
            world.team_msg(team, format!("Our convoy delivers {} supplies to {} (stockpile {}). Spend them with /upgrade.", n, name, stock));
        }
    } else if !home && world.planets[k].supply > 0 && world.players[i].cargo < HOLD {
        world.planets[k].supply -= 1;
        world.players[i].cargo += 1;
    }
}

fn steer(world: &mut World, id: u8, k: usize) {
    let p = &world.players[id as usize];
    if p.lock != super::world::Lock::Planet(k) {
        world.handle(id, ClientMsg::LockPlanet(k as u8));
    }
    let p = &world.players[id as usize];
    if p.desired_speed == 0 && p.orbiting.is_none() {
        world.handle(id, ClientMsg::Speed(ShipType::Freighter.stats().max_speed as u8));
    }
}

/// Empires run by robots spend supplies as they arrive; empires with
/// players leave the choice to them unless the stockpile grows large.
fn auto_buy(world: &mut World, team: Team) {
    let humans = world.players.iter().any(|p| p.in_use && !p.robot && p.team == team && p.state != PState::Outfit);
    let s = world.supply[team.idx()];
    if humans && s.stock < AUTO_BUY_STOCK {
        return;
    }
    let low = (0..UPGRADES.len()).filter(|&u| s.levels[u] < MAX_UPGRADE).map(|u| s.levels[u]).min();
    let Some(low) = low else { return };
    if s.stock < upgrade_cost(low) {
        return;
    }
    let choices: Vec<usize> = (0..UPGRADES.len()).filter(|&u| s.levels[u] == low).collect();
    if let Some(&u) = choices.choose(&mut rand::thread_rng()) {
        world.buy_upgrade(team, UPGRADES[u].0, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::world::Features;

    /// A convoy collects supplies from a fuel world, hauls them home, and a
    /// robot empire spends them.
    #[test]
    fn convoys_deliver_and_robots_buy() {
        let mut w = World::with_features(Features { supply: true, ..Features::default() });
        let mut l = Logistics::new();
        let colony = (0..w.planets.len()).find(|&k| w.planets[k].owner == Team::Fed && w.planets[k].flags & PL_FUEL != 0 && w.planets[k].flags & PL_HOME == 0).unwrap();
        w.planets[colony].supply = PLANET_CAP;
        let mut delivered = false;
        for _ in 0..(UPS as u32 * 60 * 4) {
            l.tick(&mut w);
            w.tick();
            let s = w.supply[Team::Fed.idx()];
            if s.stock > 0 || s.levels.iter().any(|&l| l > 0) {
                delivered = true;
                break;
            }
        }
        assert!(delivered, "the Federation convoy never delivered");
        assert!(l.freighter(&w, Team::Fed).is_some());
        // Stock up a robot empire: it buys an upgrade on its own.
        w.supply[Team::Rom.idx()].stock = 30;
        for _ in 0..(UPS as u32 * 6) {
            l.tick(&mut w);
            w.tick();
        }
        assert!(w.supply[Team::Rom.idx()].levels.iter().any(|&l| l > 0));
    }
}
