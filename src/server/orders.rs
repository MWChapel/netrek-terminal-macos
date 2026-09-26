//! Orders from command (the --orders option). Every few minutes each human
//! player gets a short task: scout, reinforce, guard, bomb, capture, escort
//! the supply convoy, salvage a derelict or survey the terrain. Finishing
//! one in time earns a kill and a full resupply.

use super::supply::Logistics;
use super::world::{GameEvent, World};
use crate::consts::*;
use crate::proto::{PState, TerrainKind};
use rand::seq::SliceRandom;
use std::collections::HashMap;

/// Seconds between finishing (or failing) an order and the next one.
const REST_SECS: u32 = 20;

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// Fly within 6,000 of each planet.
    Scout(Vec<usize>),
    Reinforce(usize),
    Guard(usize),
    Bomb(usize),
    Capture(usize),
    Escort,
    Salvage,
    /// Visit a named terrain feature.
    Survey(String),
}

struct Order {
    kind: Kind,
    text: String,
    need: i32,
    progress: i32,
    deadline: u32,
    /// Scout: planets already visited.
    seen: Vec<usize>,
}

struct Slot {
    name: String,
    order: Option<Order>,
    next_at: u32,
}

#[derive(Default)]
pub struct Orders {
    slots: HashMap<u8, Slot>,
}

fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

/// Who gives each empire its orders.
pub fn command_name(t: Team) -> &'static str {
    match t {
        Team::Fed => "Starfleet Command",
        Team::Rom => "Romulan High Command",
        Team::Kli => "Klingon High Council",
        Team::Ori => "Orion Syndicate",
        Team::Ind => "Command",
    }
}

impl Orders {
    pub fn new() -> Orders {
        Orders::default()
    }

    pub fn tick(&mut self, world: &mut World, logistics: &Logistics, events: &[GameEvent], commands: &[(u8, String)]) {
        let tick = world.tick;
        // Track the human players.
        self.slots.retain(|&id, s| {
            let p = &world.players[id as usize];
            p.in_use && !p.robot && p.name == s.name
        });
        for p in world.players.iter().filter(|p| p.in_use && !p.robot) {
            self.slots.entry(p.id).or_insert(Slot { name: p.name.clone(), order: None, next_at: tick + 15 * UPS as u32 });
        }
        for (id, cmd) in commands {
            if cmd.starts_with("order") {
                let text = match self.slots.get(id).and_then(|s| s.order.as_ref()) {
                    Some(o) => format!("Current orders: {}", o.text),
                    None => "No orders right now. Stand by.".into(),
                };
                world.reply(*id, text);
            }
        }
        let ids: Vec<u8> = self.slots.keys().copied().collect();
        for id in ids {
            let i = id as usize;
            let team = world.players[i].team;
            let alive = world.players[i].state == PState::Alive;
            let slot = self.slots.get_mut(&id).unwrap();
            if world.players[i].state == PState::Outfit && slot.order.is_none() {
                world.players[i].order = None;
                continue;
            }
            if slot.order.is_none() {
                if alive && tick >= slot.next_at {
                    if let Some(o) = issue(world, logistics, i) {
                        let head = command_name(team);
                        world.reply(id, format!("{}: new orders. {}", head, o.text));
                        world.warn(id, format!("New orders: {}", o.text));
                        slot.order = Some(o);
                    } else {
                        slot.next_at = tick + 10 * UPS as u32;
                    }
                }
                world.players[i].order = None;
                continue;
            }
            let o = slot.order.as_mut().unwrap();
            if alive {
                progress(world, logistics, i, o, events);
            }
            if o.progress >= o.need {
                let head = command_name(team);
                let p = &mut world.players[i];
                p.kills += 1.0;
                p.total_kills += 1.0;
                if p.alive() {
                    let s = p.stats();
                    p.fuel = s.max_fuel;
                    p.damage = 0.0;
                    p.shield = s.max_shield;
                }
                world.reply(id, format!("{}: well done. Orders complete (+1 kill, full resupply).", head));
                world.warn(id, "Orders complete! +1 kill and a full resupply");
                world.events.push(GameEvent::OrderDone { player: id });
                slot.order = None;
                slot.next_at = tick + REST_SECS * UPS as u32;
                world.players[i].order = None;
                continue;
            }
            if tick >= o.deadline {
                world.reply(id, format!("{}: your orders have expired.", command_name(team)));
                slot.order = None;
                slot.next_at = tick + REST_SECS * UPS as u32;
                world.players[i].order = None;
                continue;
            }
            let left = (o.deadline - tick) / UPS as u32;
            let shown = match o.kind {
                Kind::Guard(_) | Kind::Escort => format!("{} — {}s of {}s", o.text, o.progress / UPS as i32, o.need / UPS as i32),
                _ if o.need > 1 => format!("{} — {}/{}", o.text, o.progress, o.need),
                _ => o.text.clone(),
            };
            world.players[i].order = Some(format!("{} ({}s left)", shown, left));
        }
    }
}

/// Pick an order that makes sense for this player right now.
fn issue(world: &World, logistics: &Logistics, i: usize) -> Option<Order> {
    let mut rng = rand::thread_rng();
    let p = &world.players[i];
    let (team, x, y) = (p.team, p.x, p.y);
    let tick = world.tick;
    let secs = |s: u32| tick + s * UPS as u32;
    let near = |ok: &dyn Fn(usize) -> bool| {
        (0..world.planets.len())
            .filter(|&k| ok(k))
            .min_by(|&a, &b| {
                let (pa, pb) = (&world.planets[a], &world.planets[b]);
                dist(x, y, pa.x, pa.y).total_cmp(&dist(x, y, pb.x, pb.y))
            })
    };
    let enemy = |k: usize| world.hostile(world.planets[k].owner, team) && world.planets[k].owner != Team::Ind;
    // A frontier world: one of ours close to the enemy.
    let frontier = (0..world.planets.len())
        .filter(|&k| world.planets[k].owner == team && world.planets[k].flags & PL_HOME == 0)
        .min_by(|&a, &b| {
            let gap = |k: usize| {
                let pl = &world.planets[k];
                (0..world.planets.len())
                    .filter(|&e| enemy(e))
                    .map(|e| dist(pl.x, pl.y, world.planets[e].x, world.planets[e].y))
                    .fold(f64::MAX, f64::min)
            };
            gap(a).total_cmp(&gap(b))
        });
    let carrier = p.max_armies_now() > 0;
    let mut options: Vec<Order> = Vec::new();
    let order = |kind: Kind, text: String, need: i32, secs_left: u32| Order { kind, text, need, progress: 0, deadline: secs(secs_left), seen: Vec::new() };

    // Scout three planets we know least about.
    let mut far: Vec<usize> = (0..world.planets.len()).filter(|&k| world.planets[k].owner != team).collect();
    far.sort_by(|&a, &b| {
        let key = |k: usize| (world.planets[k].known[team.idx()], -(dist(x, y, world.planets[k].x, world.planets[k].y) as i64) / 20_000);
        key(a).cmp(&key(b))
    });
    far.truncate(6);
    far.shuffle(&mut rng);
    far.truncate(3);
    if far.len() == 3 {
        let names: Vec<&str> = far.iter().map(|&k| world.planets[k].name).collect();
        options.push(order(Kind::Scout(far.clone()), format!("Scout {}, {} and {}", names[0], names[1], names[2]), 3, 150));
    }
    if let Some(k) = frontier {
        let name = world.planets[k].name;
        options.push(order(Kind::Guard(k), format!("Guard {} (stay within 5,000)", name), 45 * UPS as i32, 120));
        if carrier {
            options.push(order(Kind::Reinforce(k), format!("Reinforce {}: beam down 3 armies", name), 3, 180));
        }
    }
    if let Some(k) = near(&|k| enemy(k) && world.planets[k].armies > 6) {
        options.push(order(Kind::Bomb(k), format!("Bomb {} down by 4 armies", world.planets[k].name), 4, 150));
    }
    if carrier {
        let target = near(&|k| {
            let o = world.planets[k].owner;
            o != team && world.hostile(o, team) && world.planets[k].armies <= 5
        });
        if let Some(k) = target {
            options.push(order(Kind::Capture(k), format!("Capture {}", world.planets[k].name), 1, 240));
        }
    }
    if world.features.supply && logistics.freighter(world, team).is_some() {
        options.push(order(Kind::Escort, "Escort our supply freighter (stay within 4,000)".into(), 40 * UPS as i32, 120));
    }
    if world.features.terrain {
        if let Some(t) = world.terrain.iter().filter(|t| t.kind == TerrainKind::Derelict && t.visible()).min_by(|a, b| dist(x, y, a.x, a.y).total_cmp(&dist(x, y, b.x, b.y))) {
            options.push(order(Kind::Salvage, format!("Salvage the {}", t.name), 1, 150));
        }
        let sights: Vec<&super::terrain::Terrain> = world
            .terrain
            .iter()
            .filter(|t| matches!(t.kind, TerrainKind::Nebula | TerrainKind::Pulsar | TerrainKind::BlackHole | TerrainKind::Star | TerrainKind::TachyonGrid | TerrainKind::Asteroids))
            .collect();
        if let Some(t) = sights.choose(&mut rng) {
            options.push(order(Kind::Survey(t.name.clone()), format!("Survey the {}", t.name), 1, 150));
        }
    }
    let n = options.len();
    if n == 0 {
        return None;
    }
    Some(options.swap_remove(rand::Rng::gen_range(&mut rng, 0..n)))
}

fn progress(world: &World, logistics: &Logistics, i: usize, o: &mut Order, events: &[GameEvent]) {
    let p = &world.players[i];
    let (x, y, id, team) = (p.x, p.y, p.id, p.team);
    match &o.kind {
        Kind::Scout(targets) => {
            for &k in targets {
                if !o.seen.contains(&k) && dist(x, y, world.planets[k].x, world.planets[k].y) < 6000.0 {
                    o.seen.push(k);
                }
            }
            o.progress = o.seen.len() as i32;
        }
        Kind::Guard(k) => {
            let pl = &world.planets[*k];
            if pl.owner != team {
                o.deadline = world.tick; // lost the planet: the order is moot
            } else if dist(x, y, pl.x, pl.y) < 5000.0 {
                o.progress += 1;
            }
        }
        Kind::Escort => {
            if let Some(f) = logistics.freighter(world, team) {
                let q = &world.players[f as usize];
                if dist(x, y, q.x, q.y) < 4000.0 {
                    o.progress += 1;
                }
            }
        }
        Kind::Survey(name) => {
            if let Some(t) = world.terrain.iter().find(|t| &t.name == name) {
                if dist(x, y, t.x, t.y) < t.r + 1500.0 {
                    o.progress = 1;
                }
            }
        }
        Kind::Reinforce(k) | Kind::Bomb(k) | Kind::Capture(k) => {
            let k = *k;
            for e in events {
                match (&o.kind, e) {
                    (Kind::Reinforce(_), GameEvent::Reinforced { player, planet }) if *player == id && *planet == k => o.progress += 1,
                    (Kind::Bomb(_), GameEvent::Bombed { player, planet, armies }) if *player == id && *planet == k => o.progress += armies,
                    (Kind::Capture(_), GameEvent::PlanetTaken { player, planet }) if *player == id && *planet == k => o.progress += 1,
                    _ => {}
                }
            }
        }
        Kind::Salvage => {
            if events.iter().any(|e| matches!(e, GameEvent::Salvaged { player } if *player == id)) {
                o.progress = 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pilot gets orders, carries them out, and is rewarded.
    #[test]
    fn scouting_orders_pay_off() {
        let mut w = World::new();
        w.features.orders = true;
        let id = w.add_player("Kirk", false).unwrap();
        w.join(id, Team::Fed, ShipType::Scout).unwrap();
        let l = Logistics::new();
        let mut o = Orders::new();
        o.tick(&mut w, &l, &[], &[]);
        // Skip ahead to the first order, and make it a scouting one.
        let targets = vec![20, 21, 22];
        loop {
            w.tick += 1;
            o.tick(&mut w, &l, &[], &[]);
            if let Some(ord) = o.slots.get_mut(&id).and_then(|s| s.order.as_mut()) {
                ord.kind = Kind::Scout(targets.clone());
                ord.need = 3;
                break;
            }
        }
        let kills = w.players[id as usize].kills;
        for &k in &targets {
            (w.players[id as usize].x, w.players[id as usize].y) = (w.planets[k].x, w.planets[k].y + 3000.0);
            w.tick += 1;
            o.tick(&mut w, &l, &[], &[]);
        }
        assert_eq!(w.players[id as usize].kills, kills + 1.0, "rewarded with a kill");
        assert!(w.events.iter().any(|e| matches!(e, GameEvent::OrderDone { .. })));
        assert!(o.slots[&id].order.is_none());
    }
}
