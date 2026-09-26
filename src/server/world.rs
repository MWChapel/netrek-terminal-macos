//! Authoritative game simulation. One call to `tick()` is one Netrek update
//! (1/10th of a second).

use crate::consts::*;
use crate::proto::*;
use rand::seq::SliceRandom;
use rand::Rng;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lock {
    None,
    Planet(usize),
    Player(u8),
}

pub struct Player {
    pub id: u8,
    pub in_use: bool,
    pub robot: bool,
    pub name: String,
    pub team: Team,
    pub ship: ShipType,
    pub state: PState,
    pub state_timer: i32,
    pub just_exploded: bool,
    pub x: f64,
    pub y: f64,
    pub dir: f64,
    pub desired_dir: f64,
    pub sub_dir: f64,
    pub speed: i32,
    pub desired_speed: i32,
    pub sub_speed: i32,
    pub fuel: f64,
    pub shield: f64,
    pub damage: f64,
    pub wtemp: f64,
    pub etemp: f64,
    pub w_overheat: i32,
    pub e_overheat: i32,
    pub armies: u32,
    pub kills: f64,
    pub shields_up: bool,
    pub cloaked: bool,
    pub repair_mode: bool,
    pub bombing: bool,
    pub beam_up: bool,
    pub beam_down: bool,
    pub orbiting: Option<usize>,
    pub tractor: Option<(u8, bool)>,
    pub lock: Lock,
    pub action_timer: i32,
    pub phaser_timer: i32,
    pub deaths: u32,
    pub total_kills: f64,
    /// Alien incursion this ship belongs to (aliens fly as `Team::Ind`).
    pub faction: Option<Faction>,
    /// Extra kill credit for destroying this ship (aliens).
    pub bounty: f64,
    /// Borg adaptation: fraction of incoming damage still taken.
    pub adapt: f64,
    /// Crystalline Entity: recent phaser hits (tick, shooter).
    pub resonance: Vec<(u32, u8)>,
    /// Whale probe: engines, shields and recharge are dead until this tick.
    pub powerless_until: u32,
}

impl Player {
    fn empty(id: u8) -> Player {
        Player {
            id,
            in_use: false,
            robot: false,
            name: String::new(),
            team: Team::Ind,
            ship: ShipType::Cruiser,
            state: PState::Outfit,
            state_timer: 0,
            just_exploded: false,
            x: 0.0,
            y: 0.0,
            dir: 0.0,
            desired_dir: 0.0,
            sub_dir: 0.0,
            speed: 0,
            desired_speed: 0,
            sub_speed: 0,
            fuel: 0.0,
            shield: 0.0,
            damage: 0.0,
            wtemp: 0.0,
            etemp: 0.0,
            w_overheat: 0,
            e_overheat: 0,
            armies: 0,
            kills: 0.0,
            shields_up: false,
            cloaked: false,
            repair_mode: false,
            bombing: false,
            beam_up: false,
            beam_down: false,
            orbiting: None,
            tractor: None,
            lock: Lock::None,
            action_timer: 0,
            phaser_timer: 0,
            deaths: 0,
            total_kills: 0.0,
            faction: None,
            bounty: 0.0,
            adapt: 1.0,
            resonance: Vec::new(),
            powerless_until: 0,
        }
    }

    pub fn alive(&self) -> bool {
        self.in_use && self.state == PState::Alive
    }

    pub fn stats(&self) -> &'static ShipStats {
        self.ship.stats()
    }

    /// Callsign like "F0" (aliens: "KH3").
    pub fn tag(&self) -> String {
        match self.faction {
            Some(f) => format!("{}{}", f.short(), slot_char(self.id)),
            None => format!("{}{}", self.team.letter(), slot_char(self.id)),
        }
    }

    pub fn label(&self) -> String {
        format!("{} ({})", self.name, self.tag())
    }

    pub fn max_speed_now(&self) -> i32 {
        let s = self.stats();
        let m = (s.max_speed + 2) as f64 - (s.max_speed + 1) as f64 * (self.damage / s.max_damage);
        (m as i32).clamp(0, s.max_speed)
    }

    pub fn max_armies_now(&self) -> u32 {
        let s = self.stats();
        let per_kill = if self.ship == ShipType::Assault { 3.0 } else { 2.0 };
        ((self.kills * per_kill) as u32).min(s.max_armies)
    }

    fn leave_orbit(&mut self) {
        self.orbiting = None;
        self.bombing = false;
        self.beam_up = false;
        self.beam_down = false;
    }
}

pub struct Torp {
    pub owner: u8,
    pub team: Team,
    pub kind: TorpKind,
    pub x: f64,
    pub y: f64,
    pub dir: f64,
    pub speed: f64,
    pub fuse: i32,
    pub damage: f64,
    pub explode: u8,
}

pub struct Planet {
    pub name: &'static str,
    pub x: f64,
    pub y: f64,
    pub owner: Team,
    pub armies: i32,
    pub flags: u8,
    pub known: [bool; 5],
    /// Held by an alien power, or devoured by the planet killer.
    pub alien: Option<Faction>,
    /// Whale probe: no army growth until this tick.
    pub silenced_until: u32,
}

/// One strand of a Tholian web: damages any non-Tholian ship touching it.
pub struct Web {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub ttl: i32,
    pub owner: u8,
}

/// What the next call to `inflict` is being hit by (some aliens care).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HitKind {
    Other,
    Photon,
    Plasma,
    Phaser,
    /// Jem'Hadar phased polaron beams go straight through shields.
    Polaron,
}

pub const WEB_REACH: f64 = 300.0;
pub const WEB_DAMAGE: f64 = 1.5;

pub struct PhaserShot {
    pub info: PhaserInfo,
    pub ticks: i32,
}

pub enum Dest {
    All,
    Team(Team),
    Player(u8),
}

pub struct Outgoing {
    pub dest: Dest,
    pub msg: ChatMsg,
}

pub struct World {
    pub players: Vec<Player>,
    pub torps: Vec<Torp>,
    pub phasers: Vec<PhaserShot>,
    pub planets: Vec<Planet>,
    pub tick: u32,
    pub outbox: Vec<Outgoing>,
    pub banner: Option<String>,
    pub reset_timer: i32,
    /// Warnings for a single player (the "Helmsman:" line in the original client).
    pub warnings: Vec<(u8, String)>,
    pub webs: Vec<Web>,
    pub hit_kind: HitKind,
}

impl World {
    pub fn new() -> World {
        let mut w = World {
            players: (0..MAXPLAYER as u8).map(Player::empty).collect(),
            torps: Vec::new(),
            phasers: Vec::new(),
            planets: Vec::new(),
            tick: 0,
            outbox: Vec::new(),
            banner: None,
            reset_timer: 0,
            warnings: Vec::new(),
            webs: Vec::new(),
            hit_kind: HitKind::Other,
        };
        w.reset_galaxy();
        w
    }

    pub fn reset_galaxy(&mut self) {
        let mut rng = rand::thread_rng();
        self.planets = PLANETS
            .iter()
            .map(|d| Planet {
                name: d.name,
                x: d.x,
                y: d.y,
                owner: d.team,
                armies: START_ARMIES,
                flags: 0,
                known: [false; 5],
                alien: None,
                silenced_until: 0,
            })
            .collect();
        for team in Team::PLAYABLE {
            let home = team.home_planet();
            let p = &mut self.planets[home];
            p.flags = PL_HOME | PL_REPAIR | PL_FUEL | PL_AGRI;
            p.armies = HOME_ARMIES;
            // Each quadrant gets a few repair, fuel and agricultural worlds.
            let mut others: Vec<usize> = (home + 1..home + 10).collect();
            others.shuffle(&mut rng);
            for (n, &i) in others.iter().enumerate() {
                let f = match n {
                    0 => PL_REPAIR | PL_FUEL,
                    1 => PL_REPAIR,
                    2 | 3 => PL_FUEL,
                    4 => PL_AGRI,
                    5 => PL_AGRI | PL_FUEL,
                    _ => 0,
                };
                self.planets[i].flags = f;
            }
            for i in home..home + 10 {
                self.planets[i].known[team.idx()] = true;
            }
        }
        self.torps.clear();
        self.phasers.clear();
        self.webs.clear();
        self.banner = None;
        self.reset_timer = 0;
    }

    // ------------------------------------------------------------------
    // messaging helpers

    pub fn god(&mut self, text: impl Into<String>) {
        self.outbox.push(Outgoing {
            dest: Dest::All,
            msg: ChatMsg { kind: MsgKind::System, from: "GOD".into(), text: text.into() },
        });
    }

    fn team_msg(&mut self, team: Team, text: impl Into<String>) {
        self.outbox.push(Outgoing {
            dest: Dest::Team(team),
            msg: ChatMsg { kind: MsgKind::System, from: team.letter().to_string(), text: text.into() },
        });
    }

    fn warn(&mut self, id: u8, text: impl Into<String>) {
        self.warnings.push((id, text.into()));
    }

    // ------------------------------------------------------------------
    // slots

    pub fn add_player(&mut self, name: &str, robot: bool) -> Option<u8> {
        let slot = self.players.iter().position(|p| !p.in_use)?;
        let mut p = Player::empty(slot as u8);
        p.in_use = true;
        p.robot = robot;
        p.name = name.chars().filter(|c| !c.is_control()).take(16).collect();
        if p.name.is_empty() {
            p.name = "guest".into();
        }
        self.players[slot] = p;
        Some(slot as u8)
    }

    pub fn remove_player(&mut self, id: u8) {
        let i = id as usize;
        if !self.players[i].in_use {
            return;
        }
        let was_playing = self.players[i].state != PState::Outfit;
        let label = self.players[i].label();
        let robot = self.players[i].robot;
        self.players[i] = Player::empty(id);
        self.torps.retain(|t| t.owner != id);
        for p in self.players.iter_mut() {
            if matches!(p.tractor, Some((t, _)) if t == id) {
                p.tractor = None;
            }
            if p.lock == Lock::Player(id) {
                p.lock = Lock::None;
            }
        }
        if was_playing && !robot {
            self.god(format!("{} has left the game", label));
        }
    }

    pub fn team_planet_count(&self, team: Team) -> usize {
        self.planets.iter().filter(|p| p.owner == team).count()
    }

    /// Teams a player may join: any team that still owns planets.
    pub fn open_teams(&self) -> Vec<Team> {
        if self.reset_timer > 0 {
            return Vec::new();
        }
        Team::PLAYABLE
            .into_iter()
            .filter(|&t| self.team_planet_count(t) > 0)
            .collect()
    }

    fn has_starbase(&self, team: Team, except: u8) -> bool {
        self.players.iter().any(|p| {
            p.in_use && p.id != except && p.team == team && p.ship == ShipType::Starbase && p.state != PState::Outfit
        })
    }

    pub fn join(&mut self, id: u8, team: Team, ship: ShipType) -> Result<(), String> {
        let i = id as usize;
        if self.players[i].state != PState::Outfit {
            return Err("You are already in play".into());
        }
        if !self.open_teams().contains(&team) {
            return Err(format!("The {} are not accepting new recruits", team.plural()));
        }
        if ship == ShipType::Starbase && self.has_starbase(team, id) {
            return Err("Your team already has a starbase".into());
        }
        let mut rng = rand::thread_rng();
        // Spawn near home, or near any planet the team still owns.
        let home = team.home_planet();
        let start = if self.planets[home].owner == team {
            home
        } else {
            self.planets.iter().position(|p| p.owner == team).unwrap_or(home)
        };
        let (px, py) = (self.planets[start].x, self.planets[start].y);
        let s = ship.stats();
        let p = &mut self.players[i];
        p.team = team;
        p.ship = ship;
        p.state = PState::Alive;
        p.x = (px + rng.gen_range(-5000.0..5000.0)).clamp(1000.0, GWIDTH - 1000.0);
        p.y = (py + rng.gen_range(-5000.0..5000.0)).clamp(1000.0, GWIDTH - 1000.0);
        p.dir = rng.gen_range(0.0..256.0);
        p.desired_dir = p.dir;
        p.speed = 0;
        p.desired_speed = 0;
        p.sub_speed = 0;
        p.fuel = s.max_fuel;
        p.shield = s.max_shield;
        p.damage = 0.0;
        p.wtemp = 0.0;
        p.etemp = 0.0;
        p.w_overheat = 0;
        p.e_overheat = 0;
        p.armies = 0;
        p.kills = 0.0;
        p.shields_up = true;
        p.cloaked = false;
        p.repair_mode = false;
        p.leave_orbit();
        p.tractor = None;
        p.lock = Lock::None;
        p.phaser_timer = 0;
        p.just_exploded = false;
        if !p.robot {
            let msg = format!("{} has joined the {} in a {}", p.label(), team.plural(), s.name);
            self.god(msg);
        }
        Ok(())
    }

    /// Bring an alien ship into play at (x, y). Destroying it is worth
    /// `1 + bounty / 10` kills.
    pub fn spawn_alien(&mut self, name: &str, faction: Faction, ship: ShipType, x: f64, y: f64, bounty: f64) -> Option<u8> {
        let id = self.add_player(name, true)?;
        let s = ship.stats();
        let mut rng = rand::thread_rng();
        let p = &mut self.players[id as usize];
        p.team = Team::Ind;
        p.faction = Some(faction);
        p.ship = ship;
        p.state = PState::Alive;
        p.x = x.clamp(500.0, GWIDTH - 500.0);
        p.y = y.clamp(500.0, GWIDTH - 500.0);
        p.dir = rng.gen_range(0.0..256.0);
        p.desired_dir = p.dir;
        p.fuel = s.max_fuel;
        p.shield = s.max_shield;
        p.shields_up = s.max_shield > 0.0;
        p.bounty = bounty;
        Some(id)
    }

    // ------------------------------------------------------------------
    // commands

    pub fn handle(&mut self, id: u8, msg: ClientMsg) {
        let i = id as usize;
        if !self.players[i].in_use {
            return;
        }
        if let ClientMsg::Message { to, text } = &msg {
            self.chat(id, *to, text);
            return;
        }
        if let ClientMsg::Join { team, ship } = msg {
            if let Err(e) = self.join(id, team, ship) {
                self.warn(id, e);
            }
            return;
        }
        if !self.players[i].alive() {
            return;
        }
        match msg {
            ClientMsg::Course(d) => {
                let p = &mut self.players[i];
                p.desired_dir = d as f64;
                p.lock = Lock::None;
                if p.orbiting.is_some() {
                    p.leave_orbit();
                }
            }
            ClientMsg::Speed(s) => self.set_speed(i, s as i32),
            ClientMsg::Torp(d) => self.fire_torp(i, d as f64, TorpKind::Photon),
            ClientMsg::Plasma(d) => self.fire_torp(i, d as f64, TorpKind::Plasma),
            ClientMsg::Phaser(d) => self.fire_phaser(i, d as f64),
            ClientMsg::Shields => {
                let p = &mut self.players[i];
                p.shields_up = !p.shields_up;
                if p.shields_up {
                    p.repair_mode = false;
                }
            }
            ClientMsg::Cloak => {
                let p = &mut self.players[i];
                p.cloaked = !p.cloaked;
            }
            ClientMsg::Orbit => self.orbit(i),
            ClientMsg::Bomb => self.toggle_bomb(i),
            ClientMsg::BeamUp => self.toggle_beam(i, true),
            ClientMsg::BeamDown => self.toggle_beam(i, false),
            ClientMsg::Repair => {
                let p = &mut self.players[i];
                p.repair_mode = true;
                p.shields_up = false;
                p.desired_speed = 0;
            }
            ClientMsg::Tractor { target, pressor } => self.tractor(i, target, pressor),
            ClientMsg::DetEnemy => self.det_enemy(i),
            ClientMsg::DetOwn => {
                for t in self.torps.iter_mut() {
                    if t.owner == id && t.explode == 0 && t.kind == TorpKind::Photon {
                        t.fuse = 0;
                    }
                }
            }
            ClientMsg::LockPlanet(pl) => {
                if (pl as usize) < self.planets.len() {
                    let name = self.planets[pl as usize].name;
                    let p = &mut self.players[i];
                    if p.orbiting != Some(pl as usize) {
                        p.leave_orbit();
                        p.lock = Lock::Planet(pl as usize);
                    }
                    self.warn(id, format!("Locking onto {}", name));
                }
            }
            ClientMsg::LockPlayer(t) => {
                if (t as usize) < MAXPLAYER && self.players[t as usize].alive() && t != id {
                    let tag = self.players[t as usize].tag();
                    let p = &mut self.players[i];
                    p.leave_orbit();
                    p.lock = Lock::Player(t);
                    self.warn(id, format!("Locking onto {}", tag));
                }
            }
            ClientMsg::Refit(ship) => self.refit(i, ship),
            ClientMsg::Quit => {
                // Self-destruct, like Netrek's 'Q'.
                self.kill(i, None, "self-destructed".into());
            }
            ClientMsg::Hello { .. } | ClientMsg::Join { .. } | ClientMsg::Message { .. } => {}
        }
    }

    fn chat(&mut self, id: u8, to: MsgTarget, text: &str) {
        let text: String = text.chars().filter(|c| !c.is_control()).take(80).collect();
        if text.trim().is_empty() {
            return;
        }
        let from = self.players[id as usize].tag();
        let (dest, kind) = match to {
            MsgTarget::All => (Dest::All, MsgKind::All),
            MsgTarget::Team(t) => (Dest::Team(t), MsgKind::Team),
            MsgTarget::Player(p) => (Dest::Player(p), MsgKind::Indiv),
        };
        let to_label = match to {
            MsgTarget::All => "ALL".to_string(),
            MsgTarget::Team(t) => t.abbr().to_string(),
            MsgTarget::Player(p) => self.players.get(p as usize).map(|pl| pl.tag()).unwrap_or_default(),
        };
        let msg = ChatMsg { kind, from: format!("{}->{}", from, to_label), text };
        // The sender sees their own individual messages too.
        if let Dest::Player(p) = dest {
            if p != id {
                self.outbox.push(Outgoing { dest: Dest::Player(id), msg: msg.clone() });
            }
        }
        self.outbox.push(Outgoing { dest, msg });
    }

    fn set_speed(&mut self, i: usize, s: i32) {
        let max = self.players[i].max_speed_now();
        if self.players[i].e_overheat > 0 {
            return self.warn(i as u8, "Engines are overheated!");
        }
        let p = &mut self.players[i];
        p.desired_speed = s.clamp(0, max);
        p.repair_mode = false;
        if p.orbiting.is_some() && s > 0 {
            p.leave_orbit();
        }
    }

    fn fire_torp(&mut self, i: usize, dir: f64, kind: TorpKind) {
        let id = i as u8;
        let p = &self.players[i];
        let s = p.stats();
        if p.cloaked {
            return self.warn(id, "Weapons disabled while cloaked");
        }
        if p.w_overheat > 0 {
            return self.warn(id, "Weapons overheated!");
        }
        let (cost, damage, speed, fuse) = match kind {
            TorpKind::Photon => {
                let out = self.torps.iter().filter(|t| t.owner == id && t.kind == TorpKind::Photon).count();
                if out >= MAXTORP {
                    return self.warn(id, "Torps limited to 8 at a time");
                }
                (s.torp_cost, s.torp_damage, s.torp_speed, s.torp_fuse)
            }
            TorpKind::Plasma => {
                if s.plasma_damage <= 0.0 {
                    return self.warn(id, "This ship has no plasma torpedoes");
                }
                if p.kills < 2.0 {
                    return self.warn(id, "You need 2 kills to fire plasma");
                }
                if self.torps.iter().any(|t| t.owner == id && t.kind == TorpKind::Plasma) {
                    return self.warn(id, "Plasma already in flight");
                }
                (s.plasma_cost, s.plasma_damage, s.plasma_speed, s.plasma_fuse)
            }
        };
        if p.fuel < cost {
            return self.warn(id, "Not enough fuel to fire");
        }
        let mut rng = rand::thread_rng();
        let t = Torp {
            owner: id,
            team: p.team,
            kind,
            x: p.x,
            y: p.y,
            dir,
            speed: speed * WARP1,
            fuse: fuse + rng.gen_range(0..4),
            damage,
            explode: 0,
        };
        let p = &mut self.players[i];
        p.fuel -= cost;
        p.wtemp += cost / 10.0;
        p.repair_mode = false;
        self.torps.push(t);
    }

    fn fire_phaser(&mut self, i: usize, dir: f64) {
        let id = i as u8;
        let p = &self.players[i];
        let s = p.stats();
        if p.cloaked {
            return self.warn(id, "Weapons disabled while cloaked");
        }
        if p.w_overheat > 0 {
            return self.warn(id, "Weapons overheated!");
        }
        if p.phaser_timer > 0 {
            return;
        }
        if p.fuel < s.phaser_cost {
            return self.warn(id, "Not enough fuel for phaser");
        }
        let range = PHASEDIST * s.phaser_damage / 100.0;
        let (vx, vy) = dir_vec(dir);
        let (x, y, team) = (p.x, p.y, p.team);
        // Nearest enemy ship close to the beam line.
        let mut best: Option<(usize, f64)> = None;
        for (j, q) in self.players.iter().enumerate() {
            if j == i || !q.alive() || !self.at_war(i, j) {
                continue;
            }
            let (dx, dy) = (q.x - x, q.y - y);
            let along = dx * vx + dy * vy;
            if along <= 0.0 || along > range {
                continue;
            }
            let perp = (dx * vy - dy * vx).abs();
            if perp < q.ship.hit_radius().max(800.0) && best.map_or(true, |(_, d)| along < d) {
                best = Some((j, along));
            }
        }
        let (x2, y2, hit) = match best {
            Some((j, dist)) => {
                let dmg = s.phaser_damage * (1.0 - dist / range);
                let (tx, ty) = (self.players[j].x, self.players[j].y);
                let label = self.players[i].tag();
                self.hit_kind = if self.players[i].faction == Some(Faction::JemHadar) { HitKind::Polaron } else { HitKind::Phaser };
                if self.players[j].ship == ShipType::CrystalEntity {
                    self.resonate(j, id);
                }
                self.inflict(j, dmg, Some(id), format!("phaser from {}", label));
                (tx, ty, true)
            }
            None => (x + vx * range, y + vy * range, false),
        };
        // Phasers also shoot down enemy plasma along the beam.
        for t in self.torps.iter_mut() {
            if t.kind == TorpKind::Plasma && t.team != team && t.explode == 0 {
                let (dx, dy) = (t.x - x, t.y - y);
                let along = dx * vx + dy * vy;
                if along > 0.0 && along < range && (dx * vy - dy * vx).abs() < 500.0 {
                    t.explode = 1;
                    t.damage = 0.0;
                }
            }
        }
        let p = &mut self.players[i];
        p.fuel -= s.phaser_cost;
        p.wtemp += s.phaser_cost / 10.0;
        p.phaser_timer = 10;
        p.repair_mode = false;
        self.phasers.push(PhaserShot {
            info: PhaserInfo { owner: id, x1: x as i32, y1: y as i32, x2: x2 as i32, y2: y2 as i32, hit },
            ticks: 6,
        });
    }

    /// A phaser hit on the Crystalline Entity. Phasers from three or more
    /// different ships within 2.5 seconds reach resonance and shatter it.
    fn resonate(&mut self, j: usize, shooter: u8) {
        let tick = self.tick;
        let p = &mut self.players[j];
        p.resonance.retain(|&(t, _)| tick.saturating_sub(t) <= 25);
        p.resonance.push((tick, shooter));
        let mut shooters: Vec<u8> = p.resonance.iter().map(|r| r.1).collect();
        shooters.sort_unstable();
        shooters.dedup();
        if shooters.len() >= 3 {
            p.resonance.clear();
            self.outbox.push(Outgoing {
                dest: Dest::All,
                msg: ChatMsg { kind: MsgKind::System, from: "ALERT".into(), text: "Resonance! The Crystalline Entity shatters!".into() },
            });
            self.kill(j, Some(shooter), "shattered".into());
        }
    }

    fn nearest_planet(&self, x: f64, y: f64) -> (usize, f64) {
        self.planets
            .iter()
            .enumerate()
            .map(|(k, pl)| (k, ((pl.x - x).powi(2) + (pl.y - y).powi(2)).sqrt()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
    }

    fn orbit(&mut self, i: usize) {
        let id = i as u8;
        let (x, y, speed) = (self.players[i].x, self.players[i].y, self.players[i].speed);
        if speed > ORBSPEED {
            return self.warn(id, "Helmsman: Captain, the maximum safe speed for docking or orbiting is warp 2!");
        }
        let (k, d) = self.nearest_planet(x, y);
        if d > ENTORBDIST {
            return self.warn(id, "Helmsman: We are not in orbit range of any planet");
        }
        self.enter_orbit(i, k);
    }

    fn enter_orbit(&mut self, i: usize, k: usize) {
        let (px, py, name) = (self.planets[k].x, self.planets[k].y, self.planets[k].name);
        let team = self.players[i].team;
        let p = &mut self.players[i];
        p.orbiting = Some(k);
        p.lock = Lock::None;
        p.speed = 0;
        p.desired_speed = 0;
        p.dir = (dir_to(px, py, p.x, p.y) + 64.0).rem_euclid(256.0);
        p.desired_dir = p.dir;
        p.tractor = None;
        self.planets[k].known[team.idx()] = true;
        let id = p.id;
        self.warn(id, format!("Helmsman: Entering orbit around {}", name));
    }

    fn toggle_bomb(&mut self, i: usize) {
        let id = i as u8;
        let p = &self.players[i];
        if p.bombing {
            self.players[i].bombing = false;
            return;
        }
        let Some(k) = p.orbiting else {
            return self.warn(id, "Must be orbiting to bomb");
        };
        if self.planets[k].owner == p.team {
            return self.warn(id, "Don't bomb your own planets!");
        }
        if self.planets[k].armies <= 4 {
            return self.warn(id, "Too few armies left to bomb");
        }
        let p = &mut self.players[i];
        p.bombing = true;
        p.beam_up = false;
        p.beam_down = false;
        p.action_timer = 0;
    }

    fn toggle_beam(&mut self, i: usize, up: bool) {
        let id = i as u8;
        let p = &self.players[i];
        if (up && p.beam_up) || (!up && p.beam_down) {
            let p = &mut self.players[i];
            p.beam_up = false;
            p.beam_down = false;
            return;
        }
        let Some(k) = p.orbiting else {
            return self.warn(id, "Must be orbiting to beam armies");
        };
        if up {
            if self.planets[k].owner != p.team {
                return self.warn(id, "Can only beam up armies from your own planets");
            }
            if p.max_armies_now() == 0 {
                return self.warn(id, "You need kills to carry armies (2 per kill)");
            }
        } else if p.armies == 0 {
            return self.warn(id, "You have no armies on board");
        }
        let p = &mut self.players[i];
        p.beam_up = up;
        p.beam_down = !up;
        p.bombing = false;
        p.action_timer = 0;
    }

    fn tractor(&mut self, i: usize, target: Option<u8>, pressor: bool) {
        let id = i as u8;
        let Some(t) = target else {
            self.players[i].tractor = None;
            return;
        };
        let ti = t as usize;
        if ti >= MAXPLAYER || ti == i || !self.players[ti].alive() {
            return;
        }
        let p = &self.players[i];
        let range = TRACTDIST * p.stats().tract_range;
        let q = &self.players[ti];
        if ((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt() > range {
            return self.warn(id, "Target out of tractor range");
        }
        self.players[i].tractor = Some((t, pressor));
    }

    fn det_enemy(&mut self, i: usize) {
        let id = i as u8;
        let (x, y, team) = (self.players[i].x, self.players[i].y, self.players[i].team);
        if self.players[i].fuel < 100.0 || self.players[i].w_overheat > 0 {
            return;
        }
        self.players[i].fuel -= 100.0;
        self.players[i].wtemp += 10.0;
        for t in self.torps.iter_mut() {
            if t.team != team && t.explode == 0 && t.kind == TorpKind::Photon {
                if ((t.x - x).powi(2) + (t.y - y).powi(2)).sqrt() < DETDIST {
                    // Detonated torps become ours, so they cannot hurt us.
                    t.owner = id;
                    t.team = team;
                    t.fuse = 0;
                }
            }
        }
    }

    fn refit(&mut self, i: usize, ship: ShipType) {
        let id = i as u8;
        let p = &self.players[i];
        let ok = p.orbiting.map_or(false, |k| {
            self.planets[k].owner == p.team && self.planets[k].flags & PL_HOME != 0
        });
        if !ok {
            return self.warn(id, "You must orbit your home planet to refit");
        }
        if p.armies > 0 {
            return self.warn(id, "Beam your armies down before refitting");
        }
        if ship == ShipType::Starbase && self.has_starbase(p.team, id) {
            return self.warn(id, "Your team already has a starbase");
        }
        let old = p.stats();
        let new = ship.stats();
        let p = &mut self.players[i];
        p.fuel = p.fuel / old.max_fuel * new.max_fuel;
        p.shield = p.shield / old.max_shield * new.max_shield;
        p.damage = p.damage / old.max_damage * new.max_damage;
        p.ship = ship;
        self.warn(id, format!("Refitted to a {}", new.name));
    }

    // ------------------------------------------------------------------
    // damage & death

    /// Whether ships `a` and `b` are enemies: different teams, or alien
    /// factions at war with each other (the Borg and Species 8472).
    pub fn at_war(&self, a: usize, b: usize) -> bool {
        let (pa, pb) = (&self.players[a], &self.players[b]);
        pa.team != pb.team || matches!((pa.faction, pb.faction), (Some(x), Some(y)) if x.at_war_with(y))
    }

    pub fn inflict(&mut self, i: usize, amount: f64, killer: Option<u8>, how: String) {
        let kind = std::mem::replace(&mut self.hit_kind, HitKind::Other);
        let p = &mut self.players[i];
        if !p.alive() || amount <= 0.0 {
            return;
        }
        let amount = match p.ship {
            // Invulnerable: they have to be dealt with some other way.
            ShipType::VgerCloud | ShipType::WhaleProbe => return,
            // Only resonance (several phasers at once) can shatter it.
            ShipType::CrystalEntity => amount * 0.05,
            // Impervious to conventional weapons; plasma is our "nanoprobe" warhead.
            ShipType::Bioship if kind == HitKind::Plasma => amount,
            ShipType::Bioship => amount * 0.1,
            // Neutronium hull: ordinary weapons barely scratch it.
            ShipType::PlanetKiller => amount * 0.4,
            // The Borg adapt to whatever hits them.
            ShipType::BorgCube => {
                let before = p.adapt;
                p.adapt = (p.adapt * 0.99).max(0.3);
                if before > 0.5 && p.adapt <= 0.5 {
                    self.outbox.push(Outgoing {
                        dest: Dest::All,
                        msg: ChatMsg { kind: MsgKind::System, from: "ALERT".into(), text: "The Borg have adapted to your weapons!".into() },
                    });
                }
                amount * before
            }
            _ => amount,
        };
        let p = &mut self.players[i];
        if p.shields_up && kind != HitKind::Polaron {
            p.shield -= amount;
            if p.shield < 0.0 {
                p.damage -= p.shield;
                p.shield = 0.0;
            }
        } else {
            p.damage += amount;
        }
        p.repair_mode = false;
        if p.damage >= p.stats().max_damage {
            self.kill(i, killer, how);
        }
    }

    pub fn kill(&mut self, i: usize, killer: Option<u8>, how: String) {
        let victim_kills = self.players[i].kills + self.players[i].bounty;
        let victim_armies = self.players[i].armies;
        let vlabel = self.players[i].label();
        let vship = self.players[i].stats().abbr;
        {
            let p = &mut self.players[i];
            p.state = PState::Exploding;
            p.state_timer = 10;
            p.just_exploded = true;
            p.deaths += 1;
            p.speed = 0;
            p.leave_orbit();
            p.tractor = None;
            p.lock = Lock::None;
        }
        let with_armies = if victim_armies > 0 { format!(" (carrying {} armies)", victim_armies) } else { String::new() };
        match killer.map(|k| k as usize).filter(|&k| k != i && self.players[k].in_use) {
            Some(k) => {
                let credit = 1.0 + victim_kills * 0.1 + victim_armies as f64 * 0.1;
                let kp = &mut self.players[k];
                kp.kills += credit;
                kp.total_kills += credit;
                let msg = format!(
                    "{} [{}] was kill {:.2} for {}{}",
                    vlabel,
                    vship,
                    kp.kills,
                    kp.label(),
                    with_armies
                );
                self.god(msg);
            }
            None => self.god(format!("{} [{}] {}{}", vlabel, vship, how, with_armies)),
        }
        for p in self.players.iter_mut() {
            if matches!(p.tractor, Some((t, _)) if t as usize == i) {
                p.tractor = None;
            }
            if p.lock == Lock::Player(i as u8) {
                p.lock = Lock::None;
            }
        }
    }

    fn ship_explosion(&mut self, i: usize) {
        let (x, y) = (self.players[i].x, self.players[i].y);
        let base = match self.players[i].ship {
            ShipType::Starbase => 200.0,
            ShipType::Scout => 75.0,
            s if s.is_alien() && s.hit_radius() > EXPDIST => 250.0,
            _ => 100.0,
        };
        let tag = self.players[i].tag();
        let faction = self.players[i].faction;
        for j in 0..MAXPLAYER {
            if j == i || !self.players[j].alive() {
                continue;
            }
            // Aliens don't blow up their own kind.
            if faction.is_some() && self.players[j].faction == faction {
                continue;
            }
            let rim = self.players[j].ship.hit_radius() - EXPDIST;
            let d = (((self.players[j].x - x).powi(2) + (self.players[j].y - y).powi(2)).sqrt() - rim).max(0.0);
            if d > SHIPDAMDIST {
                continue;
            }
            let mut dmg = if d <= EXPDIST { base } else { base * (SHIPDAMDIST - d) / (SHIPDAMDIST - EXPDIST) };
            // Commodore Decker's gambit: an exploding ship in the planet
            // killer's maw hurts it far more than anything else can.
            if self.players[j].ship == ShipType::PlanetKiller {
                dmg *= 8.0;
            }
            self.inflict(j, dmg, Some(i as u8), format!("caught in the explosion of {}", tag));
        }
    }

    // ------------------------------------------------------------------
    // the update

    pub fn tick(&mut self) {
        self.tick += 1;
        if self.reset_timer > 0 {
            self.reset_timer -= 1;
            if self.reset_timer == 0 {
                self.reset_galaxy();
                for p in self.players.iter_mut().filter(|p| p.in_use) {
                    p.state = PState::Outfit;
                }
                self.god("The galaxy has been reset. Choose your team!");
            }
        }
        for i in 0..MAXPLAYER {
            match self.players[i].state {
                PState::Alive if self.players[i].in_use => self.update_player(i),
                PState::Exploding => {
                    if self.players[i].just_exploded {
                        self.players[i].just_exploded = false;
                        self.ship_explosion(i);
                    }
                    let p = &mut self.players[i];
                    p.state_timer -= 1;
                    if p.state_timer <= 0 {
                        p.state = PState::Dead;
                        p.state_timer = 10;
                    }
                }
                PState::Dead => {
                    let p = &mut self.players[i];
                    p.state_timer -= 1;
                    if p.state_timer <= 0 {
                        p.state = PState::Outfit;
                    }
                }
                _ => {}
            }
        }
        self.update_tractors();
        self.update_torps();
        self.update_webs();
        if self.tick % 5 == 0 {
            self.planet_fire();
        }
        if self.tick % 10 == 0 {
            self.planet_growth();
        }
        self.update_scouting();
        for ph in self.phasers.iter_mut() {
            ph.ticks -= 1;
        }
        self.phasers.retain(|p| p.ticks > 0);
    }

    fn update_player(&mut self, i: usize) {
        let mut rng = rand::thread_rng();
        let id = i as u8;

        // Locks steer the ship.
        match self.players[i].lock {
            Lock::Planet(k) => {
                let (px, py) = (self.planets[k].x, self.planets[k].y);
                let p = &mut self.players[i];
                let d = ((px - p.x).powi(2) + (py - p.y).powi(2)).sqrt();
                p.desired_dir = dir_to(p.x, p.y, px, py);
                if d < ENTORBDIST {
                    self.enter_orbit(i, k);
                } else {
                    // Slow down on approach so we can drop into orbit.
                    let s = p.stats();
                    let decel = WARP1 * 1000.0 / s.dec as f64;
                    let mut safe = 1;
                    while safe < s.max_speed && (safe * safe) as f64 * decel * 0.5 + ENTORBDIST < d {
                        safe += 1;
                    }
                    if p.desired_speed > safe {
                        p.desired_speed = safe;
                    } else if p.desired_speed == 0 {
                        p.desired_speed = safe.min(p.max_speed_now());
                    }
                }
            }
            Lock::Player(t) => {
                let t = t as usize;
                if self.players[t].alive() {
                    let (tx, ty) = (self.players[t].x, self.players[t].y);
                    let p = &mut self.players[i];
                    p.desired_dir = dir_to(p.x, p.y, tx, ty);
                } else {
                    self.players[i].lock = Lock::None;
                }
            }
            Lock::None => {}
        }

        let planet_under = self.players[i].orbiting.map(|k| (k, self.planets[k].owner, self.planets[k].flags));
        let p = &mut self.players[i];
        let s = p.stats();

        // Speed (with damage-limited maximum).
        let max = p.max_speed_now();
        if p.e_overheat > 0 {
            p.desired_speed = 0;
        }
        if p.desired_speed > max {
            p.desired_speed = max;
        }
        if p.speed < p.desired_speed {
            p.sub_speed += s.acc;
            while p.sub_speed >= 1000 && p.speed < p.desired_speed {
                p.speed += 1;
                p.sub_speed -= 1000;
            }
        } else if p.speed > p.desired_speed {
            p.sub_speed += s.dec;
            while p.sub_speed >= 1000 && p.speed > p.desired_speed {
                p.speed -= 1;
                p.sub_speed -= 1000;
            }
        } else {
            p.sub_speed = 0;
        }

        // Movement.
        if let Some((k, _, _)) = planet_under {
            let (px, py) = (self.planets[k].x, self.planets[k].y);
            let p = &mut self.players[i];
            p.dir = (p.dir + 2.0).rem_euclid(256.0);
            p.desired_dir = p.dir;
            let (rx, ry) = dir_vec(p.dir - 64.0);
            p.x = px + rx * ORBDIST;
            p.y = py + ry * ORBDIST;
        } else {
            // Turning, "new-style" turn rates: faster ships turn slower.
            if p.speed == 0 {
                p.dir = p.desired_dir;
                p.sub_dir = 0.0;
            } else {
                p.sub_dir += s.turns / (p.speed * p.speed) as f64;
                let steps = (p.sub_dir / 1000.0).floor();
                p.sub_dir -= steps * 1000.0;
                let diff = dir_diff(p.dir, p.desired_dir);
                if diff.abs() <= steps {
                    p.dir = p.desired_dir;
                } else {
                    p.dir = (p.dir + steps * diff.signum()).rem_euclid(256.0);
                }
            }
            let (vx, vy) = dir_vec(p.dir);
            p.x += vx * p.speed as f64 * WARP1;
            p.y += vy * p.speed as f64 * WARP1;
            // Bounce off the edge of the galaxy.
            if p.x < 0.0 || p.x > GWIDTH {
                p.x = p.x.clamp(0.0, GWIDTH);
                p.dir = (256.0 - p.dir).rem_euclid(256.0);
                p.desired_dir = p.dir;
            }
            if p.y < 0.0 || p.y > GWIDTH {
                p.y = p.y.clamp(0.0, GWIDTH);
                p.dir = (128.0 - p.dir).rem_euclid(256.0);
                p.desired_dir = p.dir;
            }
        }

        let own_planet = planet_under.filter(|&(_, owner, _)| owner == self.players[i].team);
        let p = &mut self.players[i];

        // The whale probe's call drains all power.
        let powerless = self.tick < p.powerless_until;
        if powerless {
            p.desired_speed = p.desired_speed.min(1);
            p.shields_up = false;
            p.cloaked = false;
        }

        // Fuel.
        if !powerless {
            p.fuel += 2.0 * s.recharge;
        }
        if let Some((_, _, flags)) = own_planet {
            if flags & PL_FUEL != 0 && !powerless {
                p.fuel += 6.0 * s.recharge;
            }
        }
        p.fuel -= s.warp_cost * p.speed as f64;
        if p.shields_up {
            p.fuel -= s.shield_cost;
        }
        if p.cloaked {
            p.fuel -= s.cloak_cost;
        }
        if p.fuel < 0.0 {
            p.fuel = 0.0;
            p.shields_up = false;
            p.cloaked = false;
            p.desired_speed = p.desired_speed.min(2);
        }
        p.fuel = p.fuel.min(s.max_fuel);

        // Heat.
        p.etemp = (p.etemp + p.speed as f64 - s.egn_cool).max(0.0);
        p.wtemp = (p.wtemp - s.wpn_cool).max(0.0);
        if p.w_overheat > 0 {
            p.w_overheat -= 1;
        } else if p.wtemp > s.max_wtemp {
            p.w_overheat = rng.gen_range(40..90);
            let id = p.id;
            self.warn(id, "Weapons overheated!");
        }
        let p = &mut self.players[i];
        if p.e_overheat > 0 {
            p.e_overheat -= 1;
        } else if p.etemp > s.max_etemp {
            p.e_overheat = rng.gen_range(60..160);
            p.desired_speed = 0;
            let id = p.id;
            self.warn(id, "Engines overheated! Stopping to cool down");
        }
        let p = &mut self.players[i];

        // Repairs.
        let at_repair = own_planet.map_or(false, |(_, _, f)| f & PL_REPAIR != 0);
        let mut smul = if p.repair_mode { 4.0 } else { 2.0 };
        let mut dmul = if p.repair_mode { 2.0 } else { 1.0 };
        if at_repair {
            smul += 2.0;
            dmul += 1.0;
        }
        if p.shield < s.max_shield {
            p.shield = (p.shield + s.repair * smul / 1000.0).min(s.max_shield);
        }
        if p.damage > 0.0 {
            p.damage = (p.damage - s.repair * dmul / 1000.0).max(0.0);
        }
        if p.repair_mode && p.speed > 0 {
            p.desired_speed = 0;
        }
        if p.phaser_timer > 0 {
            p.phaser_timer -= 1;
        }

        // Bombing and beaming, one action every few updates.
        if let Some((k, owner, _)) = planet_under {
            let p = &mut self.players[i];
            p.action_timer += 1;
            if p.bombing && p.action_timer >= 5 {
                p.action_timer = 0;
                if owner == p.team || self.planets[k].armies <= 4 {
                    p.bombing = false;
                    self.warn(id, "Bombing stopped: planet down to 4 armies");
                } else if rng.gen_bool(0.6) {
                    let n = if p.ship == ShipType::Assault { 2 } else { 1 };
                    let n = n.min(self.planets[k].armies - 4);
                    self.planets[k].armies -= n;
                    p.kills += 0.02 * n as f64;
                    p.total_kills += 0.02 * n as f64;
                }
            } else if p.beam_up && p.action_timer >= 8 {
                p.action_timer = 0;
                if owner != p.team || self.planets[k].armies <= 1 {
                    p.beam_up = false;
                    self.warn(id, "No more armies can be beamed up");
                } else if p.armies >= p.max_armies_now() {
                    p.beam_up = false;
                    self.warn(id, "Army capacity full");
                } else {
                    p.armies += 1;
                    self.planets[k].armies -= 1;
                }
            } else if p.beam_down && p.action_timer >= 8 {
                p.action_timer = 0;
                if p.armies == 0 {
                    p.beam_down = false;
                } else {
                    p.armies -= 1;
                    let team = p.team;
                    self.beam_army_down(i, k, team);
                }
            }
        }

        // Tractor costs.
        let p = &mut self.players[i];
        if p.tractor.is_some() {
            p.fuel -= TRACTCOST;
            p.etemp += TRACTEHEAT;
        }
    }

    fn beam_army_down(&mut self, i: usize, k: usize, team: Team) {
        let label = self.players[i].label();
        let pl = &mut self.planets[k];
        if pl.owner == team {
            pl.armies += 1;
            return;
        }
        if pl.armies > 0 {
            pl.armies -= 1;
            self.players[i].kills += 0.02;
            self.players[i].total_kills += 0.02;
            if pl.armies == 0 {
                let (name, old) = (pl.name, pl.owner);
                pl.owner = Team::Ind;
                // Khan's stronghold and Terran Empire conquests are freed.
                if pl.alien != Some(Faction::Doomsday) {
                    pl.alien = None;
                }
                self.god(format!("{} ({}) destroyed by {}", name, old.letter(), label));
                self.check_genocide(old, team);
            }
            return;
        }
        // Undefended: it's ours. Even a devoured world can be resettled
        // (as bare rock, with no repair, fuel or farming).
        let (name, old) = (pl.name, pl.owner);
        pl.owner = team;
        pl.alien = None;
        pl.armies = 1;
        pl.known[team.idx()] = true;
        self.god(format!("{} taken over by {}", name, label));
        self.team_msg(team, format!("We now hold {}", name));
        self.check_genocide(old, team);
    }

    pub fn check_genocide(&mut self, loser: Team, winner: Team) {
        if loser == Team::Ind || self.team_planet_count(loser) > 0 {
            return;
        }
        if winner == Team::Ind {
            self.god(format!("The {} have been wiped out by alien invaders!", loser.plural()));
        } else {
            self.god(format!("The {} have been genocided by the {}!", loser.plural(), winner.plural()));
        }
        for i in 0..MAXPLAYER {
            if self.players[i].alive() && self.players[i].team == loser {
                self.kill(i, None, "was lost with the fall of their empire".into());
            }
        }
        // Conquest: only one empire still holds planets.
        let holders: Vec<Team> = Team::PLAYABLE.into_iter().filter(|&t| self.team_planet_count(t) > 0).collect();
        if holders.len() == 1 {
            let msg = format!("The galaxy has been conquered by the {}!", holders[0].plural());
            self.god(msg.clone());
            self.banner = Some(msg);
            self.reset_timer = 15 * UPS as i32;
        }
    }

    fn update_tractors(&mut self) {
        for i in 0..MAXPLAYER {
            if !self.players[i].alive() {
                continue;
            }
            let Some((t, pressor)) = self.players[i].tractor else { continue };
            let ti = t as usize;
            let (a, b) = (&self.players[i], &self.players[ti]);
            let range = TRACTDIST * a.stats().tract_range;
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let d = (dx * dx + dy * dy).sqrt();
            if !b.alive() || d > range || a.fuel < TRACTCOST {
                self.players[i].tractor = None;
                continue;
            }
            if d < 1.0 {
                continue;
            }
            let force = WARP1 * a.stats().tract_str;
            let sign = if pressor { -1.0 } else { 1.0 };
            let (ux, uy) = (dx / d * sign, dy / d * sign);
            let (ma, mb) = (a.stats().mass, b.stats().mass);
            let a = &mut self.players[i];
            if a.orbiting.is_none() {
                a.x = (a.x + ux * force / ma).clamp(0.0, GWIDTH);
                a.y = (a.y + uy * force / ma).clamp(0.0, GWIDTH);
            }
            let b = &mut self.players[ti];
            b.leave_orbit();
            b.x = (b.x - ux * force / mb).clamp(0.0, GWIDTH);
            b.y = (b.y - uy * force / mb).clamp(0.0, GWIDTH);
        }
    }

    fn update_torps(&mut self) {
        let mut hits: Vec<(usize, f64, u8, TorpKind)> = Vec::new();
        for t in self.torps.iter_mut() {
            if t.explode > 0 {
                t.explode += 1;
                continue;
            }
            // Plasma homes in on the nearest enemy.
            if t.kind == TorpKind::Plasma {
                let target = self
                    .players
                    .iter()
                    .filter(|p| p.alive() && p.team != t.team && !p.cloaked)
                    .map(|p| (p, (p.x - t.x).powi(2) + (p.y - t.y).powi(2)))
                    .filter(|(_, d2)| *d2 < 15000.0f64.powi(2))
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((p, _)) = target {
                    let want = dir_to(t.x, t.y, p.x, p.y);
                    let diff = dir_diff(t.dir, want);
                    t.dir = (t.dir + diff.clamp(-2.0, 2.0)).rem_euclid(256.0);
                }
            }
            let (vx, vy) = dir_vec(t.dir);
            t.x += vx * t.speed;
            t.y += vy * t.speed;
            t.fuse -= 1;
            let mut boom = t.fuse <= 0 || t.x < 0.0 || t.y < 0.0 || t.x > GWIDTH || t.y > GWIDTH;
            if !boom {
                let owner_fac = self.players.get(t.owner as usize).and_then(|o| if o.in_use { o.faction } else { None });
                let hostile = |p: &Player| {
                    p.team != t.team || matches!((owner_fac, p.faction), (Some(x), Some(y)) if x.at_war_with(y))
                };
                boom = self.players.iter().any(|p| {
                    let r = p.ship.hit_radius();
                    p.alive()
                        && hostile(p)
                        && (p.x - t.x).abs() < r
                        && (p.y - t.y).abs() < r
                        && (p.x - t.x).powi(2) + (p.y - t.y).powi(2) < r * r
                });
            }
            if boom {
                t.explode = 1;
                let damdist = if t.kind == TorpKind::Plasma { PLASDAMDIST } else { DAMDIST };
                let owner_fac = self.players.get(t.owner as usize).and_then(|o| if o.in_use { o.faction } else { None });
                for (j, p) in self.players.iter().enumerate() {
                    let hostile = p.team != t.team || matches!((owner_fac, p.faction), (Some(x), Some(y)) if x.at_war_with(y));
                    if !p.alive() || !hostile {
                        continue;
                    }
                    // Measure from the hull, so big monsters take full hits.
                    let rim = p.ship.hit_radius() - EXPDIST;
                    let d = (((p.x - t.x).powi(2) + (p.y - t.y).powi(2)).sqrt() - rim).max(0.0);
                    if d > damdist {
                        continue;
                    }
                    let dmg = if d <= EXPDIST { t.damage } else { t.damage * (damdist - d) / (damdist - EXPDIST) };
                    hits.push((j, dmg, t.owner, t.kind));
                }
            }
        }
        self.torps.retain(|t| t.explode < 6);
        for (j, dmg, owner, kind) in hits {
            let what = if kind == TorpKind::Plasma { "plasma" } else { "torp" };
            let tag = self.players[owner as usize].tag();
            self.hit_kind = if kind == TorpKind::Plasma { HitKind::Plasma } else { HitKind::Photon };
            self.inflict(j, dmg, Some(owner), format!("killed by {} from {}", what, tag));
        }
    }

    fn update_webs(&mut self) {
        if self.webs.is_empty() {
            return;
        }
        let mut hits: Vec<(usize, u8)> = Vec::new();
        for w in &self.webs {
            let (dx, dy) = (w.x2 - w.x1, w.y2 - w.y1);
            let len2 = (dx * dx + dy * dy).max(1.0);
            for (j, p) in self.players.iter().enumerate() {
                if !p.alive() || p.faction == Some(Faction::Tholian) {
                    continue;
                }
                let t = (((p.x - w.x1) * dx + (p.y - w.y1) * dy) / len2).clamp(0.0, 1.0);
                let (qx, qy) = (w.x1 + t * dx - p.x, w.y1 + t * dy - p.y);
                if qx * qx + qy * qy < WEB_REACH * WEB_REACH && !hits.iter().any(|h| h.0 == j) {
                    hits.push((j, w.owner));
                }
            }
        }
        for (j, owner) in hits {
            let killer = self.players[owner as usize].in_use.then_some(owner);
            self.inflict(j, WEB_DAMAGE, killer, "was caught in a Tholian web".into());
        }
        for w in self.webs.iter_mut() {
            w.ttl -= 1;
        }
        self.webs.retain(|w| w.ttl > 0);
    }

    fn planet_fire(&mut self) {
        for k in 0..self.planets.len() {
            let (px, py, owner, armies, name) = {
                let pl = &self.planets[k];
                (pl.x, pl.y, pl.owner, pl.armies, pl.name)
            };
            if armies == 0 {
                continue;
            }
            for i in 0..MAXPLAYER {
                let p = &self.players[i];
                if !p.alive() || p.team == owner {
                    continue;
                }
                if (p.x - px).powi(2) + (p.y - py).powi(2) > PFIREDIST * PFIREDIST {
                    continue;
                }
                let dmg = (armies / 10 + 2) as f64;
                self.inflict(i, dmg, None, format!("killed by {} ({})", name, owner.letter()));
            }
        }
    }

    fn planet_growth(&mut self) {
        let mut rng = rand::thread_rng();
        let tick = self.tick;
        for pl in self.planets.iter_mut() {
            if pl.owner == Team::Ind || pl.armies >= 60 || tick < pl.silenced_until {
                continue;
            }
            let chance = if pl.flags & PL_AGRI != 0 { 1.0 / 12.0 } else { 1.0 / 30.0 };
            if rng.gen_bool(chance) {
                pl.armies += if pl.flags & PL_AGRI != 0 && pl.armies >= 4 { 2 } else { 1 };
            }
        }
    }

    fn update_scouting(&mut self) {
        for p in self.players.iter().filter(|p| p.alive()) {
            for pl in self.planets.iter_mut() {
                if (pl.x - p.x).abs() < 6000.0 && (pl.y - p.y).abs() < 6000.0 {
                    pl.known[p.team.idx()] = true;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // views

    pub fn frame_for(&self, me: u8) -> Frame {
        let mp = &self.players[me as usize];
        let my_team = mp.team;
        let mut rng = rand::thread_rng();
        let players = self
            .players
            .iter()
            .filter(|p| p.in_use)
            .map(|p| {
                let friendly = p.team == my_team || p.id == me;
                let fuzzy = p.cloaked && !friendly;
                let (x, y) = if fuzzy {
                    (p.x + rng.gen_range(-4000.0..4000.0), p.y + rng.gen_range(-4000.0..4000.0))
                } else {
                    (p.x, p.y)
                };
                let mut flags = 0u16;
                let set = |flags: &mut u16, cond: bool, f: u16| {
                    if cond {
                        *flags |= f
                    }
                };
                set(&mut flags, p.shields_up, pf::SHIELD);
                set(&mut flags, p.cloaked, pf::CLOAK);
                set(&mut flags, p.orbiting.is_some(), pf::ORBIT);
                set(&mut flags, p.bombing, pf::BOMB);
                set(&mut flags, p.beam_up, pf::BEAMUP);
                set(&mut flags, p.beam_down, pf::BEAMDOWN);
                set(&mut flags, p.repair_mode, pf::REPAIR);
                set(&mut flags, matches!(p.tractor, Some((_, false))), pf::TRACTOR);
                set(&mut flags, matches!(p.tractor, Some((_, true))), pf::PRESSOR);
                set(&mut flags, p.robot, pf::ROBOT);
                set(&mut flags, p.w_overheat > 0, pf::WEAPON_HOT);
                set(&mut flags, p.e_overheat > 0, pf::ENGINE_HOT);
                PlayerInfo {
                    id: p.id,
                    name: p.name.clone(),
                    team: p.team,
                    ship: p.ship,
                    state: p.state,
                    x: x as i32,
                    y: y as i32,
                    dir: p.dir as u8,
                    speed: p.speed as u8,
                    flags,
                    kills: p.kills as f32,
                    armies: if friendly { p.armies as u8 } else { 0 },
                    tractor_target: if fuzzy { None } else { p.tractor.map(|t| t.0) },
                    fuzzy,
                    explode_frame: if p.state == PState::Exploding { (11 - p.state_timer).max(1) as u8 } else { 0 },
                    faction: p.faction,
                }
            })
            .collect();
        let torps = self
            .torps
            .iter()
            .map(|t| TorpInfo { owner: t.owner, team: t.team, kind: t.kind, x: t.x as i32, y: t.y as i32, explode: t.explode })
            .collect();
        let phasers = self.phasers.iter().map(|p| p.info.clone()).collect();
        let planets = self
            .planets
            .iter()
            .map(|pl| {
                let known = my_team != Team::Ind && pl.known[my_team.idx()];
                PlanetInfo {
                    owner: if known { pl.owner } else { Team::Ind },
                    armies: if known { pl.armies as u16 } else { 0 },
                    flags: if known { pl.flags } else { 0 },
                    known,
                    alien: if known { pl.alien } else { None },
                }
            })
            .collect();
        let s = mp.stats();
        let me_info = SelfInfo {
            fuel: mp.fuel as u32,
            shield: mp.shield as u32,
            damage: mp.damage.ceil() as u32,
            wtemp: (mp.wtemp / s.max_wtemp * 100.0) as u32,
            etemp: (mp.etemp / s.max_etemp * 100.0) as u32,
            armies: mp.armies as u8,
            max_armies_now: mp.max_armies_now() as u8,
            kills: mp.kills as f32,
            speed: mp.speed as u8,
            desired_speed: mp.desired_speed as u8,
            max_speed_now: mp.max_speed_now() as u8,
            torps_out: self.torps.iter().filter(|t| t.owner == me && t.kind == TorpKind::Photon).count() as u8,
            lock: match mp.lock {
                Lock::None => None,
                Lock::Planet(k) => Some(self.planets[k].name.to_string()),
                Lock::Player(t) => Some(self.players[t as usize].tag()),
            },
            orbiting: mp.orbiting.map(|k| k as u8),
            deaths: mp.deaths,
            total_kills: mp.total_kills as f32,
        };
        Frame {
            tick: self.tick,
            me,
            me_info,
            players,
            torps,
            phasers,
            planets,
            webs: self.webs.iter().map(|w| WebInfo { x1: w.x1 as i32, y1: w.y1 as i32, x2: w.x2 as i32, y2: w.y2 as i32 }).collect(),
            open_teams: self.open_teams(),
            team_planets: [Team::Fed, Team::Rom, Team::Kli, Team::Ori].map(|t| self.team_planet_count(t) as u8),
            starbase_teams: Team::PLAYABLE.into_iter().filter(|&t| self.has_starbase(t, me)).collect(),
            banner: self.banner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Beam armies onto a planet from a ship in orbit, one army at a time.
    fn beam_down(w: &mut World, id: u8, k: usize, n: u32) {
        let team = w.players[id as usize].team;
        for _ in 0..n {
            w.beam_army_down(id as usize, k, team);
        }
    }

    /// Retaking alien-held and devoured planets clears the alien mark.
    #[test]
    fn retaking_alien_planets() {
        let mut w = World::new();
        let id = w.add_player("Kirk", false).unwrap();
        w.join(id, Team::Fed, ShipType::Cruiser).unwrap();

        // Khan's stronghold with 3 armies left after bombing.
        let k = 5;
        w.planets[k].owner = Team::Ind;
        w.planets[k].alien = Some(Faction::Khan);
        w.planets[k].armies = 3;
        beam_down(&mut w, id, k, 3);
        assert_eq!(w.planets[k].owner, Team::Ind, "defenders gone: planet is neutral");
        assert_eq!(w.planets[k].alien, None, "Khan's hold is broken");
        beam_down(&mut w, id, k, 1);
        assert_eq!((w.planets[k].owner, w.planets[k].armies), (Team::Fed, 1));

        // A world devoured by the planet killer can be resettled.
        let d = 6;
        w.planets[d].owner = Team::Ind;
        w.planets[d].alien = Some(Faction::Doomsday);
        w.planets[d].armies = 0;
        w.planets[d].flags = 0;
        beam_down(&mut w, id, d, 1);
        assert_eq!(w.planets[d].owner, Team::Fed);
        assert_eq!(w.planets[d].alien, None);
        assert_eq!(w.planets[d].flags, 0, "still bare rock");
    }
}
