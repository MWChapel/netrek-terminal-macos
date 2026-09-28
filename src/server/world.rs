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
    /// Carrying tribbles (they drain fuel and infest planets we orbit).
    pub tribbles: bool,
    /// Marked as prey by the Hirogen.
    pub marked: bool,
    /// Chang's Bird-of-Prey: its exhaust is visible until this tick.
    pub revealed_until: u32,
    /// Q's champion: only this empire's weapons can hurt it.
    pub only_hurt_by: Option<Team>,
    /// Career rank (with --ranks; humans only).
    pub rank: Option<u8>,
    /// Current orders from command, as shown to the player (with --orders).
    pub order: Option<String>,
    /// One-line service record (with --ranks).
    pub service: Option<String>,
    /// Supply convoy freighter: supplies aboard.
    pub cargo: u32,
    /// Terrain: hidden from distant sensors (nebula or ion storm).
    pub hidden: bool,
    /// Terrain: inside an ion storm (phasers knocked out).
    pub in_storm: bool,
    /// Terrain: cloak exposed by a tachyon detection grid.
    pub detected: bool,
    /// Terrain: ticks spent salvaging a derelict.
    pub salvage: i32,
    /// Terrain: no wormhole transit until this tick.
    pub wormhole_until: u32,
    /// Overwatch: fire automatically at enemies that come into range.
    pub overwatch: bool,
    /// Advanced tech aboard (senior officers, with --ranks).
    pub techs: Vec<Tech>,
    /// Ablative armor left.
    pub armor: f64,
    /// Tick of the last hit taken (regenerative shields).
    pub last_hit: u32,
    /// Ticks when the v / e / j techs are ready again.
    pub tech_ready: [u32; 3],
    /// Tachyon sweep: cloaked and hidden ships near us show until this tick.
    pub sweep_until: u32,
    /// Phase cloak: untouchable (and unable to fire) until this tick.
    pub phased_until: u32,
    /// Transwarp: the jump happens at this tick.
    pub jump_at: Option<u32>,
    /// Graviton pulse: shields can't be raised until this tick.
    pub jammed_until: u32,
    /// Holographic decoy: vanishes at this tick.
    pub decoy_until: Option<u32>,
    /// Galactic scan (starbase): every contact shows for our team until this tick.
    pub scan_until: u32,
    /// Tractor net (starbase): held almost dead in space until this tick.
    pub netted_until: u32,
    /// Fighter (starbase): launched by this ship, returns at this tick.
    pub fighter_of: Option<(u8, u32)>,
    /// Relic ships: the Iconian gateway or the Kazon ram is ready at this tick.
    pub relic_ready: u32,
    /// Pinned to the rim of the Tempest's web.
    pub trapped: bool,
    /// Has used the Superzapper against this Tempest.
    pub zapped: bool,
    /// Armus: how far the slick has swollen from being shot at.
    pub swell: f64,
    /// Infected with nanites.
    pub nanites: bool,
    /// Health of each ship system, 0 (out) to 100 (with --subsystems).
    pub systems: [f64; 8],
    /// The system damage control is fixing first (/fix).
    pub fix_first: Option<System>,
    /// Crew left to fight off boarders.
    pub crew: i32,
    /// Towing a captured prize (top speed 6).
    pub towing: bool,
    /// Where the ship was last destroyed (to launch from the nearest shipyard).
    pub last_death: Option<(f64, f64)>,
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
            tribbles: false,
            marked: false,
            revealed_until: 0,
            only_hurt_by: None,
            rank: None,
            order: None,
            service: None,
            cargo: 0,
            hidden: false,
            in_storm: false,
            detected: false,
            salvage: 0,
            wormhole_until: 0,
            overwatch: false,
            techs: Vec::new(),
            armor: 0.0,
            last_hit: 0,
            tech_ready: [0; 3],
            sweep_until: 0,
            phased_until: 0,
            jump_at: None,
            jammed_until: 0,
            decoy_until: None,
            scan_until: 0,
            netted_until: 0,
            fighter_of: None,
            relic_ready: 0,
            trapped: false,
            zapped: false,
            swell: 0.0,
            nanites: false,
            systems: [100.0; 8],
            fix_first: None,
            crew: 4,
            towing: false,
            last_death: None,
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
        let m = (m as i32).clamp(0, s.max_speed);
        // Towing a prize holds you to warp 6.
        let m = if self.towing { m.min(6) } else { m };
        // A damaged warp drive can't reach top speed; with it out, impulse only.
        match self.sys(System::Warp) {
            w if w <= 0.0 => m.min(3),
            w => ((m as f64 * (0.5 + 0.5 * w)).round() as i32).clamp(m.min(1), m),
        }
    }

    /// How well a system is working, 0.0 (out) to 1.0.
    pub fn sys(&self, s: System) -> f64 {
        self.systems[s as usize] / 100.0
    }

    pub fn sys_out(&self, s: System) -> bool {
        self.systems[s as usize] <= 0.0
    }

    pub fn max_armies_now(&self) -> u32 {
        let s = self.stats();
        let per_kill = match self.ship {
            ShipType::VothCityShip => 4.0,
            ShipType::Assault | ShipType::Corsair => 3.0,
            _ => 2.0,
        };
        ((self.kills * per_kill) as u32).min(s.max_armies)
    }

    pub fn leave_orbit_pub(&mut self) {
        self.leave_orbit();
    }

    fn leave_orbit(&mut self) {
        self.orbiting = None;
        self.bombing = false;
        self.beam_up = false;
        self.beam_down = false;
    }
}

#[derive(Clone)]
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
    /// A quantum torpedo (Captain's tech).
    pub quantum: bool,
    /// Already met a Preserver deflector (it gets one chance per torpedo).
    pub deflect_tried: bool,
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
    /// Infested with tribbles: no army growth.
    pub tribbles: bool,
    /// Supplies waiting for a convoy (with --supply).
    pub supply: u32,
    /// An outpost, and the empire that built it (lost if the planet changes hands).
    pub outpost: Option<(Outpost, Team)>,
}

/// The Tempest's web (an alien incursion): ships that touch its rim are
/// held there until the Tempest dies or dissolves.
#[derive(Clone, Debug)]
pub struct TempestWeb {
    pub x: f64,
    pub y: f64,
    pub r_in: f64,
    pub r_out: f64,
    pub shape: TempestShape,
    pub lanes: u8,
    pub exposed: bool,
    pub level: u8,
    /// Tick the last Superzapper went off (zapped tankers don't split).
    pub zapped_at: u32,
}

/// How far inside the rim a climber at the top of its lane sits.
pub const RIM_INSET: f64 = 450.0;

impl TempestWeb {
    /// The lane (0..lanes) that (x, y) is in, seen from the centre.
    pub fn lane_of(&self, x: f64, y: f64) -> i32 {
        let (_, s) = self.shape.rim_toward(self.x, self.y, self.r_out, self.lanes, x, y);
        (s.round() as i32).rem_euclid(self.lanes as i32)
    }

    /// Where the rim is, in the direction of (x, y) from the centre.
    pub fn rim_toward(&self, x: f64, y: f64) -> (f64, f64) {
        self.shape.rim_toward(self.x, self.y, self.r_out, self.lanes, x, y).0
    }

    /// Centre-line point of `lane`, `prog` of the way from core (0) to rim (1).
    pub fn lane_point(&self, lane: i32, prog: f64) -> (f64, f64) {
        self.web_point(lane as f64, prog)
    }

    /// A point on the web: `lane` may be fractional (mid-flip, or on a
    /// spoke at .5). The core ring is the rim's shape, scaled down, and
    /// climbers at the top sit just inside the rim, in their lane.
    pub fn web_point(&self, lane: f64, prog: f64) -> (f64, f64) {
        let (rx, ry) = self.shape.rim(self.x, self.y, self.r_out, self.lanes, lane);
        let (vx, vy) = (rx - self.x, ry - self.y);
        let len = vx.hypot(vy).max(1.0);
        let inner = self.r_in / self.r_out;
        let outer = (len - RIM_INSET) / len;
        let k = inner + (outer - inner) * prog.clamp(0.0, 1.0);
        (self.x + vx * k, self.y + vy * k)
    }
}

/// Things that climb the Tempest's web.
/// The Ringworld: a band around a planet, lined with sections that are
/// planets in their own right.
#[derive(Clone, Debug)]
pub struct Ringworld {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub sections: Vec<RingSection>,
    /// Which section is Kzin.
    pub kzin: usize,
}

#[derive(Clone, Debug)]
pub struct RingSection {
    pub name: &'static str,
    pub x: f64,
    pub y: f64,
    pub flags: u8,
    /// Its native population (armies) when the ring arrives.
    pub armies: i32,
}

/// Kzin's garrison when the Ringworld arrives (and after a galaxy reset).
pub const KZIN_ARMIES: i32 = 12;

/// An outpost under construction.
#[derive(Clone, Copy, Debug)]
pub struct Build {
    pub builder: u8,
    pub planet: usize,
    pub kind: Outpost,
    pub done_at: u32,
}

/// How close a ship must be to send a boarding party.
pub const BOARD_RANGE: f64 = 1500.0;
/// Ticks between rounds of a boarding action.
pub const BOARD_ROUND: u32 = 8;
/// Marines the transporters send across each round.
pub const BOARD_WAVE: u32 = 2;
/// How far behind its captor a prize is towed.
pub const PRIZE_TETHER: f64 = 1100.0;

/// Marines fighting aboard an enemy ship.
#[derive(Clone, Copy, Debug)]
pub struct Boarding {
    pub attacker: u8,
    pub target: u8,
    pub troops: i32,
    /// Tick of the next round.
    pub next: u32,
}

/// A captured ship under tow.
#[derive(Clone, Debug)]
pub struct Prize {
    pub x: f64,
    pub y: f64,
    pub dir: f64,
    pub ship: ShipType,
    /// The empire it was taken from.
    pub from: Team,
    pub captor: u8,
    pub captor_team: Team,
}

/// The biggest the Armus slick can swell to (added to its base size).
pub const ARMUS_MAX_SWELL: f64 = 6000.0;

/// Names Changelings go by while posing as empire ships.
const CHANGELING_GUISES: [&str; 8] = ["Laas", "Ensign", "Tomas", "Rella", "Brunt", "Vash", "Kell", "Marta"];

pub fn is_tempest_minion(s: ShipType) -> bool {
    matches!(s, ShipType::Flipper | ShipType::Tanker | ShipType::Pulsar | ShipType::Fuseball)
}

/// Optional rules, switched on by server options.
#[derive(Clone, Copy, Default, Debug)]
pub struct Features {
    pub ranks: bool,
    pub orders: bool,
    pub diplomacy: bool,
    pub terrain: bool,
    pub supply: bool,
    /// Senior officers (Captain and up) get advanced tech (with ranks).
    pub rank_tech: bool,
    /// Hits can knock out a ship's systems (warp, phasers, shields...).
    pub subsystems: bool,
    /// Boarding parties: capture ships with the armies you carry.
    pub boarding: bool,
    /// Build defence outposts, shipyards and sensor arrays on your planets.
    pub outposts: bool,
}

/// Things that happened this tick, for the career and orders systems.
#[derive(Clone, Debug)]
pub enum GameEvent {
    Kill { killer: Option<u8>, victim: u8, credit: f64 },
    PlanetTaken { player: u8, planet: usize },
    Bombed { player: u8, planet: usize, armies: i32 },
    /// Armies beamed down onto one of the player's own planets.
    Reinforced { player: u8, planet: usize },
    Salvaged { player: u8 },
    Honour { player: u8, text: String },
    OrderDone { player: u8 },
}

/// An empire's supply stockpile and upgrade levels (with --supply).
#[derive(Clone, Copy, Default, Debug)]
pub struct TeamSupply {
    pub stock: u32,
    pub levels: [u8; 5],
}

/// Armies spilled by a destroyed Ferengi marauder, drifting in space until
/// someone flies over them.
pub struct Loot {
    pub x: f64,
    pub y: f64,
    pub armies: u32,
    pub ttl: i32,
}

pub const LOOT_REACH: f64 = 900.0;

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
    /// Overcharged phasers: a quarter of the damage goes through shields.
    Overcharge,
}

pub const WEB_REACH: f64 = 300.0;
pub const ABLATIVE_ARMOR: f64 = 40.0;
pub const TRICOBALT_COST: f64 = 3000.0;
pub const TRICOBALT_DAMAGE: f64 = 150.0;
pub const TRICOBALT_BLAST: f64 = 3500.0;
pub const SWEEP_RANGE: f64 = 12_000.0;
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
    pub loot: Vec<Loot>,
    pub hit_kind: HitKind,
    pub features: Features,
    /// Drained each tick by the career and orders systems.
    pub events: Vec<GameEvent>,
    /// Slash commands (e.g. /record) for systems outside the world.
    pub commands: Vec<(u8, String)>,
    /// Allied empires (with --diplomacy), each pair sorted.
    pub treaties: Vec<(Team, Team)>,
    /// Treaty offers: (from, to, expiry tick).
    pub proposals: Vec<(Team, Team, u32)>,
    /// Treaties being broken: (breaker, other, tick it ends).
    pub breaking: Vec<(Team, Team, u32)>,
    /// Per-empire supplies and upgrades, indexed by `Team::idx`.
    pub supply: [TeamSupply; 5],
    pub terrain: Vec<super::terrain::Terrain>,
    /// Top careers (with --ranks), kept up to date by the career system.
    pub leaders: Vec<LeaderInfo>,
    /// The Tempest's web, while that incursion is on.
    pub tempest: Option<TempestWeb>,
    /// Boarding actions under way.
    pub boardings: Vec<Boarding>,
    /// Captured ships being towed home.
    pub prizes: Vec<Prize>,
    /// Outposts under construction.
    pub builds: Vec<Build>,
    /// Patches of space marked out by alien incursions (redrawn every tick).
    pub zones: Vec<ZoneInfo>,
    /// Chat sent this tick (Nomad listens for it), cleared by `tick`.
    pub chatter: Vec<(u8, String)>,
    /// The Kzinti's Ringworld: once it arrives, it's here for good.
    pub ring: Option<Ringworld>,
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
            loot: Vec::new(),
            hit_kind: HitKind::Other,
            features: Features::default(),
            events: Vec::new(),
            commands: Vec::new(),
            treaties: Vec::new(),
            proposals: Vec::new(),
            breaking: Vec::new(),
            supply: [TeamSupply::default(); 5],
            terrain: Vec::new(),
            leaders: Vec::new(),
            tempest: None,
            boardings: Vec::new(),
            prizes: Vec::new(),
            builds: Vec::new(),
            zones: Vec::new(),
            chatter: Vec::new(),
            ring: None,
        };
        w.reset_galaxy();
        w
    }

    pub fn with_features(features: Features) -> World {
        let mut w = World::new();
        w.features = features;
        w.reset_galaxy();
        w
    }

    /// Whether two empires are allied by treaty.
    pub fn allied(&self, a: Team, b: Team) -> bool {
        a != b && self.treaties.iter().any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
    }

    /// Whether ships of these teams fight: different teams and not allied.
    pub fn hostile(&self, a: Team, b: Team) -> bool {
        a != b && !self.allied(a, b)
    }

    /// An empire's level in a supply upgrade (0 when supply is off).
    pub fn upgrade(&self, team: Team, u: usize) -> f64 {
        if self.features.supply {
            self.supply[team.idx()].levels[u] as f64
        } else {
            0.0
        }
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
                tribbles: false,
                supply: 0,
                outpost: None,
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
        self.loot.clear();
        self.banner = None;
        self.reset_timer = 0;
        self.treaties.clear();
        self.proposals.clear();
        self.breaking.clear();
        self.supply = [TeamSupply::default(); 5];
        self.place_ring();
        self.terrain.clear();
        if self.features.terrain {
            super::terrain::generate(self);
        }
    }

    /// Add the Ringworld's sections to the galaxy as planets (after the
    /// fixed 40), fresh: Kzin to the Kzinti, the rest unclaimed.
    pub fn place_ring(&mut self) {
        let Some(ring) = &self.ring else { return };
        self.planets.truncate(PLANETS.len());
        for (k, s) in ring.sections.iter().enumerate() {
            let kzin = k == ring.kzin;
            self.planets.push(Planet {
                name: s.name,
                x: s.x,
                y: s.y,
                owner: Team::Ind,
                armies: if kzin { KZIN_ARMIES } else { s.armies },
                flags: s.flags,
                // The Ringworld is too big to miss: everyone knows it.
                known: [true; 5],
                alien: kzin.then_some(Faction::Kzinti),
                silenced_until: 0,
                tribbles: false,
                supply: 0,
                outpost: None,
            });
        }
    }

    /// The planet index of Kzin, the Kzinti homeworld (with the Ringworld here).
    pub fn kzin(&self) -> Option<usize> {
        self.ring.as_ref().map(|r| PLANETS.len() + r.kzin)
    }

    // ------------------------------------------------------------------
    // messaging helpers

    pub fn god(&mut self, text: impl Into<String>) {
        self.outbox.push(Outgoing {
            dest: Dest::All,
            msg: ChatMsg { kind: MsgKind::System, from: "GOD".into(), text: text.into() },
        });
    }

    pub fn team_msg(&mut self, team: Team, text: impl Into<String>) {
        self.outbox.push(Outgoing {
            dest: Dest::Team(team),
            msg: ChatMsg { kind: MsgKind::System, from: team.letter().to_string(), text: text.into() },
        });
    }

    pub fn warn(&mut self, id: u8, text: impl Into<String>) {
        self.warnings.push((id, text.into()));
    }

    /// An alien-incursion bulletin for everyone.
    pub fn alert(&mut self, text: impl Into<String>) {
        self.outbox.push(Outgoing {
            dest: Dest::All,
            msg: ChatMsg { kind: MsgKind::System, from: "ALERT".into(), text: text.into() },
        });
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
        let ship = self.hull_for(i, team, ship)?;
        if ship == ShipType::Starbase && self.has_starbase(team, id) {
            return Err("Your team already has a starbase".into());
        }
        if ship == ShipType::Starbase && self.features.ranks && !self.players[i].robot {
            if self.players[i].rank.unwrap_or(0) < STARBASE_RANK {
                return Err(format!("Starbases need the rank of {}", RANKS[STARBASE_RANK as usize].0));
            }
        }
        let mut rng = rand::thread_rng();
        // Spawn near home, or near any planet the team still owns.
        let home = team.home_planet();
        let start = if self.planets[home].owner == team {
            home
        } else {
            self.planets.iter().position(|p| p.owner == team).unwrap_or(home)
        };
        // With shipyards, launch from the one nearest where you were lost.
        let (dx, dy) = self.players[i].last_death.unwrap_or((self.planets[start].x, self.planets[start].y));
        let yard = (0..self.planets.len())
            .filter(|&k| self.planets[k].owner == team && self.planets[k].outpost == Some((Outpost::Shipyard, team)))
            .min_by(|&a, &b| {
                let (pa, pb) = (&self.planets[a], &self.planets[b]);
                ((pa.x - dx).powi(2) + (pa.y - dy).powi(2)).total_cmp(&((pb.x - dx).powi(2) + (pb.y - dy).powi(2)))
            });
        let (start, spread) = match yard {
            Some(k) => (k, 1500.0),
            None => (start, 5000.0),
        };
        let (px, py) = (self.planets[start].x, self.planets[start].y);
        let s = ship.stats();
        let p = &mut self.players[i];
        p.team = team;
        p.ship = ship;
        p.state = PState::Alive;
        p.x = (px + rng.gen_range(-spread..spread)).clamp(1000.0, GWIDTH - 1000.0);
        p.y = (py + rng.gen_range(-spread..spread)).clamp(1000.0, GWIDTH - 1000.0);
        p.dir = rng.gen_range(0.0..256.0);
        p.desired_dir = p.dir;
        p.speed = 0;
        p.desired_speed = 0;
        p.sub_speed = 0;
        p.fuel = s.max_fuel;
        p.shield = s.max_shield;
        p.damage = 0.0;
        // A new ship: every system working, a full crew.
        p.systems = [100.0; 8];
        p.fix_first = None;
        p.crew = ship.crew();
        p.towing = false;
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
        p.tribbles = false;
        p.marked = false;
        p.jump_at = None;
        p.phased_until = 0;
        p.adapt = 1.0;
        self.fit_tech(i);
        let p = &mut self.players[i];
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
        p.crew = ship.crew();
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
            if text.trim_start().starts_with('/') {
                self.command(id, text.trim());
            } else {
                self.chat(id, *to, text);
            }
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
                if !self.players[i].shields_up && self.tick < self.players[i].jammed_until {
                    return self.warn(id, "Shields jammed by a graviton pulse!");
                }
                if !self.players[i].shields_up && self.players[i].sys_out(System::Shields) {
                    return self.warn(id, "Shield generators are out! (/fix shields)");
                }
                let p = &mut self.players[i];
                p.shields_up = !p.shields_up;
                if p.shields_up {
                    p.repair_mode = false;
                }
            }
            ClientMsg::Cloak => {
                if !self.players[i].cloaked && self.players[i].sys_out(System::Cloak) {
                    return self.warn(id, "The cloaking device is out! (/fix cloak)");
                }
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
            ClientMsg::Overwatch => self.toggle_overwatch(i),
            ClientMsg::Tech { slot, dir } => self.use_tech(i, slot as usize, dir as f64),
            ClientMsg::Board(t) => self.board(i, t as usize),
            ClientMsg::DetOwn => {
                for t in self.torps.iter_mut() {
                    if t.owner == id && t.explode == 0 && t.kind == TorpKind::Photon {
                        t.fuse = 0;
                    }
                }
            }
            ClientMsg::LockPlanet(pl) if self.iconian_gateway(i, pl as usize) => {}
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
            // Connection-level messages (joining, observing) are handled by the server loop.
            ClientMsg::Hello { .. }
            | ClientMsg::Join { .. }
            | ClientMsg::Message { .. }
            | ClientMsg::Observe { .. }
            | ClientMsg::Watch
            | ClientMsg::Follow(_)
            | ClientMsg::Play => {}
        }
    }

    fn chat(&mut self, id: u8, to: MsgTarget, text: &str) {
        let text: String = text.chars().filter(|c| !c.is_control()).take(MAX_MESSAGE).collect();
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
        self.chatter.push((id, text.clone()));
        let msg = ChatMsg { kind, from: format!("{}->{}", from, to_label), text };
        // The sender sees their own individual messages too.
        if let Dest::Player(p) = dest {
            if p != id {
                self.outbox.push(Outgoing { dest: Dest::Player(id), msg: msg.clone() });
            }
        }
        self.outbox.push(Outgoing { dest, msg });
    }

    /// An observer says something to everyone.
    pub fn observer_says(&mut self, name: &str, text: &str) {
        let text: String = text.chars().filter(|c| !c.is_control()).take(MAX_MESSAGE).collect();
        if text.trim().is_empty() {
            return;
        }
        self.outbox.push(Outgoing { dest: Dest::All, msg: ChatMsg { kind: MsgKind::All, from: format!("{} (obs)->ALL", name), text } });
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
        // Chang's Bird-of-Prey is the one ship that can fire while cloaked.
        if p.cloaked && p.ship != ShipType::BirdOfPrey {
            return self.warn(id, "Weapons disabled while cloaked");
        }
        if self.tick < p.phased_until {
            return self.warn(id, "Out of phase: weapons can't fire");
        }
        if p.w_overheat > 0 {
            return self.warn(id, "Weapons overheated!");
        }
        if self.kelvan_field(i) {
            return self.warn(id, "Kelvan neural field: your torpedo crews are paralysed");
        }
        // Damaged tubes misfire; with them out, nothing fires.
        let tubes = p.sys(System::Torpedoes);
        if tubes <= 0.0 {
            return self.warn(id, "Torpedo tubes are out! (/fix torpedoes)");
        }
        if tubes < 1.0 && rand::thread_rng().gen_bool((1.0 - tubes) * 0.5) {
            return self.warn(id, "Torpedo misfire! The tubes are damaged");
        }
        let p = &self.players[i];
        let quantum = kind == TorpKind::Photon && p.techs.contains(&Tech::QuantumTorps);
        let max_torps = p.ship.max_torps();
        let (cost, damage, speed, fuse) = match kind {
            TorpKind::Photon => {
                if s.torp_damage <= 0.0 {
                    return self.warn(id, "This ship has no torpedoes");
                }
                let out = self.torps.iter().filter(|t| t.owner == id && t.kind == TorpKind::Photon).count();
                if out >= max_torps {
                    return self.warn(id, format!("Torps limited to {} at a time", max_torps));
                }
                let mut boost = 1.0 + 0.1 * self.upgrade(p.team, UPGRADE_TORPS);
                let mut speed = s.torp_speed;
                if quantum {
                    boost *= 1.2;
                    speed *= 1.15;
                }
                (s.torp_cost, s.torp_damage * boost, speed, s.torp_fuse)
            }
            TorpKind::Tricobalt => (TRICOBALT_COST, TRICOBALT_DAMAGE, 6.0, 45),
            TorpKind::Plasma => {
                if s.plasma_damage <= 0.0 {
                    return self.warn(id, "This ship has no plasma torpedoes");
                }
                // The Warbird's plasma is ready after a single kill.
                let need = if p.ship == ShipType::Warbird { 1.0 } else { 2.0 };
                if p.kills < need {
                    return self.warn(id, format!("You need {} kills to fire plasma", need));
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
            quantum,
            deflect_tried: false,
        };
        // Photon spread: two more torpedoes fanned out either side.
        let spread = kind == TorpKind::Photon && p.techs.contains(&Tech::PhotonSpread);
        let out = self.torps.iter().filter(|t| t.owner == id && t.kind == TorpKind::Photon).count();
        let mut extra = Vec::new();
        if spread {
            for side in [-6.0, 6.0] {
                if out + 1 + extra.len() < max_torps && p.fuel >= cost * (1.5 + 0.5 * extra.len() as f64) {
                    extra.push(Torp { dir: (dir + side).rem_euclid(256.0), fuse: fuse + rng.gen_range(0..4), ..t.clone() });
                }
            }
        }
        let p = &mut self.players[i];
        p.fuel -= cost * (1.0 + 0.5 * extra.len() as f64);
        p.wtemp += cost / 10.0 * (1.0 + 0.5 * extra.len() as f64);
        p.repair_mode = false;
        self.torps.push(t);
        self.torps.extend(extra);
        if self.features.terrain {
            super::terrain::metreon_ignite(self, i);
        }
    }

    fn fire_phaser(&mut self, i: usize, dir: f64) {
        let id = i as u8;
        let p = &self.players[i];
        let s = p.stats();
        // Chang's Bird-of-Prey is the one ship that can fire while cloaked.
        if p.cloaked && p.ship != ShipType::BirdOfPrey {
            return self.warn(id, "Weapons disabled while cloaked");
        }
        if p.w_overheat > 0 {
            return self.warn(id, "Weapons overheated!");
        }
        if p.phaser_timer > 0 {
            return;
        }
        if p.in_storm {
            return self.warn(id, "Ion interference: phasers are offline in the storm");
        }
        if p.sys_out(System::Phasers) {
            return self.warn(id, "Phasers are out! (/fix phasers)");
        }
        if self.tick < p.phased_until {
            return self.warn(id, "Out of phase: weapons can't fire");
        }
        let overcharge = p.techs.contains(&Tech::PhaserOvercharge);
        if p.fuel < s.phaser_cost {
            return self.warn(id, "Not enough fuel for phaser");
        }
        // Damaged phaser banks fire weaker, shorter beams.
        let phaser_damage = s.phaser_damage * (1.0 + 0.1 * self.upgrade(p.team, UPGRADE_PHASERS)) * (0.5 + 0.5 * p.sys(System::Phasers));
        let range = PHASEDIST * phaser_damage / 100.0 * if overcharge { 1.25 } else { 1.0 };
        let (vx, vy) = dir_vec(dir);
        let (x, y, team) = (p.x, p.y, p.team);
        // Enemy ships close to the beam line, nearest first.
        let mut along_beam: Vec<(usize, f64)> = Vec::new();
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
            if perp < q.ship.hit_radius().max(800.0) {
                along_beam.push((j, along));
            }
        }
        along_beam.sort_by(|a, b| a.1.total_cmp(&b.1));
        // A Xindi particle beam pierces: everything beyond the first is hit too.
        if self.players[i].ship == ShipType::XindiWarship {
            for &(j, dist) in along_beam.iter().skip(1) {
                let label = self.players[i].tag();
                let (tx, ty) = (self.players[j].x, self.players[j].y);
                self.phasers.push(PhaserShot {
                    info: PhaserInfo { owner: id, x1: x as i32, y1: y as i32, x2: tx as i32, y2: ty as i32, hit: true },
                    ticks: 6,
                });
                self.hit_kind = HitKind::Phaser;
                self.inflict(j, phaser_damage * (1.0 - dist / range), Some(id), format!("particle beam from {}", label));
            }
        }
        let best = along_beam.first().copied();
        let (x2, y2, hit) = match best {
            Some((j, dist)) => {
                let dmg = phaser_damage * (1.0 - dist / range);
                let (tx, ty) = (self.players[j].x, self.players[j].y);
                let label = self.players[i].tag();
                self.hit_kind = if self.players[i].faction == Some(Faction::JemHadar) {
                    HitKind::Polaron
                } else if overcharge {
                    HitKind::Overcharge
                } else {
                    HitKind::Phaser
                };
                if self.players[j].ship == ShipType::CrystalEntity {
                    self.resonate(j, id);
                }
                self.inflict(j, dmg, Some(id), format!("phaser from {}", label));
                (tx, ty, true)
            }
            None => (x + vx * range, y + vy * range, false),
        };
        // Phasers also shoot down enemy plasma along the beam. Bioships can't,
        // or plasma (the only thing that hurts them) would never get through.
        let intercepts = self.players[i].ship != ShipType::Bioship;
        for t in self.torps.iter_mut().filter(|_| intercepts) {
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
        // The Defiant's pulse phasers recharge twice as fast.
        p.phaser_timer = if p.ship == ShipType::Defiant { 5 } else { 10 };
        p.repair_mode = false;
        self.phasers.push(PhaserShot {
            info: PhaserInfo { owner: id, x1: x as i32, y1: y as i32, x2: x2 as i32, y2: y2 as i32, hit },
            ticks: 6,
        });
        if self.features.terrain {
            super::terrain::metreon_ignite(self, i);
        }
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

    /// Use the advanced tech on key v (slot 0), e (1) or j (2).
    fn use_tech(&mut self, i: usize, slot: usize, dir: f64) {
        let id = i as u8;
        let tick = self.tick;
        let p = &self.players[i];
        let Some(tech) = p.techs.iter().copied().find(|t| t.slot() == Some(slot)) else {
            let rank = RANKS[(TECH_RANK as usize + 2 + slot).min(RANKS.len() - 1)].0;
            return self.warn(id, format!("No tech on that key yet (it comes with the rank of {})", rank));
        };
        if p.ship == ShipType::Starbase && !tech.starbase_only() {
            return self.warn(id, "Starbases can't use that tech");
        }
        if tick < p.phased_until {
            return self.warn(id, "Out of phase: systems offline");
        }
        if tick < p.tech_ready[slot] {
            let secs = (p.tech_ready[slot] - tick).div_ceil(UPS as u32);
            return self.warn(id, format!("{} ready in {} s", tech.name(), secs));
        }
        let used = match tech {
            Tech::TachyonSweep => {
                self.players[i].sweep_until = tick + 10 * UPS as u32;
                true
            }
            Tech::Decoy => self.launch_decoy(i),
            Tech::GravitonPulse => {
                self.graviton_pulse(i);
                true
            }
            Tech::Tricobalt => {
                let before = self.torps.len();
                self.fire_torp(i, dir, TorpKind::Tricobalt);
                self.torps.len() > before
            }
            Tech::Isokinetic => self.isokinetic(i, dir),
            Tech::Antiproton => self.antiproton(i),
            Tech::Transwarp => {
                self.players[i].jump_at = Some(tick + 2 * UPS as u32);
                true
            }
            Tech::PhaseCloak => {
                self.players[i].phased_until = tick + 6 * UPS as u32;
                for q in self.players.iter_mut() {
                    if matches!(q.tractor, Some((t, _)) if t == id) {
                        q.tractor = None;
                    }
                }
                true
            }
            Tech::FighterWing => self.launch_fighters(i),
            Tech::TractorNet => {
                for (j, _) in self.foes_near(i, 6000.0) {
                    if self.players[j].stats().mass > 50_000.0 {
                        continue;
                    }
                    self.players[j].netted_until = tick + 6 * UPS as u32;
                    let (x1, y1, x2, y2) = (self.players[i].x, self.players[i].y, self.players[j].x, self.players[j].y);
                    self.phasers.push(PhaserShot {
                        info: PhaserInfo { owner: id, x1: x1 as i32, y1: y1 as i32, x2: x2 as i32, y2: y2 as i32, hit: false },
                        ticks: 10,
                    });
                    self.warn(j as u8, "Caught in a starbase tractor net!");
                }
                true
            }
            Tech::GalacticScan => {
                self.players[i].scan_until = tick + 20 * UPS as u32;
                let team = self.players[i].team;
                for pl in self.planets.iter_mut() {
                    pl.known[team.idx()] = true;
                }
                let who = self.players[i].label();
                self.team_msg(team, format!("{} runs a galactic scan: all contacts on screen for 20 seconds", who));
                true
            }
            Tech::EmergencyReserve => {
                let p = &mut self.players[i];
                let s = p.stats();
                p.shield = s.max_shield;
                p.fuel = s.max_fuel;
                (p.wtemp, p.etemp, p.w_overheat, p.e_overheat) = (0.0, 0.0, 0, 0);
                true
            }
            _ => false,
        };
        if used {
            self.players[i].tech_ready[slot] = tick + tech.cooldown() * UPS as u32;
            self.warn(id, format!("{}!", tech.name()));
        }
    }

    /// Enemy ships within `r` of ship `i` that tech can affect.
    fn foes_near(&self, i: usize, r: f64) -> Vec<(usize, f64)> {
        let (x, y) = (self.players[i].x, self.players[i].y);
        (0..MAXPLAYER)
            .filter(|&j| j != i && self.players[j].alive() && self.at_war(i, j))
            .filter(|&j| !matches!(self.players[j].ship, ShipType::QEntity | ShipType::VgerCloud | ShipType::WhaleProbe))
            .map(|j| (j, ((self.players[j].x - x).powi(2) + (self.players[j].y - y).powi(2)).sqrt()))
            .filter(|&(_, d)| d < r)
            .collect()
    }

    /// Starbase: launch three fighters that hunt enemies near the base.
    fn launch_fighters(&mut self, i: usize) -> bool {
        let tick = self.tick;
        let (team, x, y) = (self.players[i].team, self.players[i].x, self.players[i].y);
        let mut launched = 0;
        for k in 0..3 {
            let Some(f) = self.add_player("Fighter", true) else { break };
            let s = ShipType::Scout.stats();
            let a = k as f64 * 85.0;
            let q = &mut self.players[f as usize];
            q.team = team;
            q.ship = ShipType::Scout;
            q.state = PState::Alive;
            let (vx, vy) = dir_vec(a);
            (q.x, q.y, q.dir, q.desired_dir) = (x + vx * 1200.0, y + vy * 1200.0, a, a);
            (q.fuel, q.shield, q.shields_up) = (s.max_fuel, s.max_shield, true);
            q.overwatch = true;
            q.bounty = -8.0;
            q.fighter_of = Some((i as u8, tick + 30 * UPS as u32));
            launched += 1;
        }
        if launched == 0 {
            self.warn(i as u8, "No room to launch fighters");
        }
        launched > 0
    }

    /// A fighter: close on the nearest enemy near its base (overwatch does
    /// the shooting), or fly back to the base; recalled after 30 seconds.
    fn fly_fighter(&mut self, i: usize) {
        let Some((base, until)) = self.players[i].fighter_of else { return };
        let b = base as usize;
        let base_ok = self.players[b].alive() && self.players[b].ship == ShipType::Starbase;
        if self.tick >= until || !base_ok || !self.players[i].alive() {
            if self.players[i].state != PState::Exploding {
                self.remove_player(i as u8);
            }
            return;
        }
        let (bx, by) = (self.players[b].x, self.players[b].y);
        let (x, y) = (self.players[i].x, self.players[i].y);
        let target = self
            .foes_near(b, 15_000.0)
            .into_iter()
            .filter(|&(j, _)| !(self.players[j].cloaked && !self.players[j].detected))
            .map(|(j, _)| (j, (self.players[j].x - x).powi(2) + (self.players[j].y - y).powi(2)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let (tx, ty, speed) = match target {
            Some((j, d2)) => (self.players[j].x, self.players[j].y, if d2 < 3000.0f64.powi(2) { 6 } else { 12 }),
            None => (bx, by, 8),
        };
        let p = &mut self.players[i];
        p.desired_dir = dir_to(x, y, tx, ty);
        p.desired_speed = speed.min(p.stats().max_speed);
    }

    /// The Tempest's web: an empire ship that reaches the rim is pinned
    /// there, free only to slide around the edge.
    fn tempest_trap(&mut self) {
        let Some(web) = self.tempest.clone() else {
            for p in self.players.iter_mut() {
                p.trapped = false;
            }
            return;
        };
        for i in 0..MAXPLAYER {
            let p = &mut self.players[i];
            if !p.alive() || p.faction.is_some() {
                p.trapped = false;
                continue;
            }
            let (rx, ry) = web.rim_toward(p.x, p.y);
            let d = ((p.x - web.x).powi(2) + (p.y - web.y).powi(2)).sqrt();
            let rim = ((rx - web.x).powi(2) + (ry - web.y).powi(2)).sqrt();
            if !p.trapped && d > rim {
                continue;
            }
            if !p.trapped {
                p.trapped = true;
                p.leave_orbit();
                p.lock = Lock::None;
                p.tractor = None;
                let id = p.id;
                self.warn(id, "Caught on the Tempest's web! Slide along the rim and shoot down the climbers. d = SUPERZAPPER (once)");
            }
            let p = &mut self.players[i];
            (p.x, p.y) = (rx, ry);
        }
    }

    /// Whether a Sheliak colony ship hostile to `bomber` is in orbit around
    /// planet `k`, shielding it from bombing.
    fn sheliak_shield(&self, k: usize, bomber: Team) -> bool {
        self.players.iter().any(|p| p.alive() && p.ship == ShipType::SheliakShip && p.orbiting == Some(k) && self.hostile(p.team, bomber))
    }

    /// Whether ship `i` is inside a hostile Kelvan ship's neural field.
    fn kelvan_field(&self, i: usize) -> bool {
        let (x, y) = (self.players[i].x, self.players[i].y);
        (0..MAXPLAYER).any(|j| {
            let q = &self.players[j];
            j != i && q.alive() && q.ship == ShipType::KelvanShip && self.at_war(i, j) && (q.x - x).powi(2) + (q.y - y).powi(2) < 2500.0f64.powi(2)
        })
    }

    /// Kazon raider: at warp 6 or more, slam into an enemy it touches.
    fn kazon_ram(&mut self, i: usize) {
        let p = &self.players[i];
        if !p.alive() || p.speed < 6 || self.tick < p.relic_ready {
            return;
        }
        let Some((j, _)) = self.foes_near(i, 700.0).into_iter().min_by(|a, b| a.1.total_cmp(&b.1)) else { return };
        self.players[i].relic_ready = self.tick + 5 * UPS as u32;
        let (me, them) = (self.players[i].label(), self.players[j].label());
        self.god(format!("{} rams {}!", me, them));
        self.inflict(j, 70.0, Some(i as u8), format!("was rammed by {}", me));
        self.inflict(i, 20.0, None, "broke up ramming an enemy".into());
    }

    /// Iconian gateway ship: orbiting one of our worlds and locking onto
    /// another steps through a gateway straight into orbit there. Returns
    /// whether it happened.
    fn iconian_gateway(&mut self, i: usize, k: usize) -> bool {
        let p = &self.players[i];
        if p.ship != ShipType::IconianShip || k >= self.planets.len() {
            return false;
        }
        let Some(from) = p.orbiting else { return false };
        let team = p.team;
        if from == k || self.planets[from].owner != team || self.planets[k].owner != team {
            return false;
        }
        if self.tick < p.relic_ready {
            let secs = (p.relic_ready - self.tick).div_ceil(UPS as u32);
            self.warn(i as u8, format!("The gateway recharges in {} s", secs));
            return false;
        }
        self.players[i].relic_ready = self.tick + 30 * UPS as u32;
        self.players[i].tractor = None;
        self.enter_orbit(i, k);
        let name = self.planets[k].name;
        self.warn(i as u8, format!("Through the Iconian gateway to {}!", name));
        true
    }

    /// A starbase's passive tech, every few ticks.
    fn starbase_passives(&mut self, i: usize) {
        let tick = self.tick;
        let (team, x, y) = (self.players[i].team, self.players[i].x, self.players[i].y);
        let techs = self.players[i].techs.clone();
        if techs.contains(&Tech::PointDefense) && tick % 3 == 0 {
            let treaties = self.treaties.clone();
            let foe = |t: Team| t != team && !treaties.iter().any(|&(a, b)| (a == t && b == team) || (a == team && b == t));
            let shot = self
                .torps
                .iter_mut()
                .filter(|t| t.explode == 0 && t.kind != TorpKind::Tricobalt && foe(t.team))
                .map(|t| {
                    let d2 = (t.x - x).powi(2) + (t.y - y).powi(2);
                    (d2, t)
                })
                .filter(|(d2, _)| *d2 < 3000.0f64.powi(2))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, t)) = shot {
                t.explode = 1;
                t.damage = 0.0;
                let (tx, ty) = (t.x, t.y);
                self.phasers.push(PhaserShot {
                    info: PhaserInfo { owner: i as u8, x1: x as i32, y1: y as i32, x2: tx as i32, y2: ty as i32, hit: true },
                    ticks: 3,
                });
            }
        }
        let projector = techs.contains(&Tech::ShieldProjector);
        let drydock = techs.contains(&Tech::MobileDrydock);
        if !projector && !drydock {
            return;
        }
        for j in 0..MAXPLAYER {
            let q = &self.players[j];
            if !q.alive() || !(q.team == team || self.allied(q.team, team)) || q.faction.is_some() {
                continue;
            }
            let d2 = (q.x - x).powi(2) + (q.y - y).powi(2);
            let s = q.stats();
            let q = &mut self.players[j];
            if projector && d2 < 6000.0f64.powi(2) && q.shield < s.max_shield {
                q.shield = (q.shield + s.repair * 2.0 / 1000.0).min(s.max_shield);
            }
            if drydock && j != i && d2 < 4000.0f64.powi(2) && q.speed <= 2 {
                q.damage = (q.damage - s.repair * 2.0 / 1000.0).max(0.0);
                q.fuel = (q.fuel + 6.0 * s.recharge).min(s.max_fuel);
            }
        }
    }

    fn launch_decoy(&mut self, i: usize) -> bool {
        let name = self.players[i].name.clone();
        let Some(d) = self.add_player(&name, true) else {
            self.warn(i as u8, "No room for a decoy");
            return false;
        };
        let tick = self.tick;
        let src = &self.players[i];
        let (team, ship, x, y, dir, speed, shields) = (src.team, src.ship, src.x, src.y, src.dir, src.speed.max(4), src.shields_up);
        let s = ship.stats();
        let q = &mut self.players[d as usize];
        q.team = team;
        q.ship = ship;
        q.state = PState::Alive;
        (q.x, q.y, q.dir, q.desired_dir) = (x, y, dir, dir);
        (q.speed, q.desired_speed) = (speed, speed);
        q.fuel = s.max_fuel;
        q.shields_up = shields;
        // A hologram: the first hit dispels it.
        q.damage = s.max_damage - 1.0;
        q.bounty = -10.0;
        q.decoy_until = Some(tick + 15 * UPS as u32);
        true
    }

    fn graviton_pulse(&mut self, i: usize) {
        let (x, y) = (self.players[i].x, self.players[i].y);
        let tick = self.tick;
        for (j, d) in self.foes_near(i, 4000.0) {
            if self.players[j].stats().mass > 50_000.0 {
                continue; // too massive to shove
            }
            let q = &mut self.players[j];
            let (ux, uy) = if d > 1.0 { ((q.x - x) / d, (q.y - y) / d) } else { (1.0, 0.0) };
            q.leave_orbit();
            q.x = (q.x + ux * 2500.0).clamp(0.0, GWIDTH);
            q.y = (q.y + uy * 2500.0).clamp(0.0, GWIDTH);
            q.shields_up = false;
            q.jammed_until = tick + 3 * UPS as u32;
            let (qx, qy) = (q.x, q.y);
            self.phasers.push(PhaserShot {
                info: PhaserInfo { owner: i as u8, x1: x as i32, y1: y as i32, x2: qx as i32, y2: qy as i32, hit: false },
                ticks: 6,
            });
            self.warn(j as u8, "Graviton pulse! Shields jammed");
        }
    }

    /// A long, shield-piercing beam at the first enemy along `dir`.
    fn isokinetic(&mut self, i: usize, dir: f64) -> bool {
        const RANGE: f64 = 9000.0;
        let id = i as u8;
        if self.players[i].fuel < 1500.0 {
            self.warn(id, "Not enough fuel for the isokinetic cannon");
            return false;
        }
        self.players[i].fuel -= 1500.0;
        let (x, y) = (self.players[i].x, self.players[i].y);
        let (vx, vy) = dir_vec(dir);
        let target = self
            .foes_near(i, RANGE)
            .into_iter()
            .filter(|&(j, _)| {
                let q = &self.players[j];
                let (dx, dy) = (q.x - x, q.y - y);
                dx * vx + dy * vy > 0.0 && (dx * vy - dy * vx).abs() < q.ship.hit_radius().max(800.0)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let (x2, y2, hit) = match target {
            Some((j, _)) => {
                let (tx, ty) = (self.players[j].x, self.players[j].y);
                let label = self.players[i].tag();
                self.hit_kind = HitKind::Polaron;
                self.inflict(j, 120.0, Some(id), format!("isokinetic cannon from {}", label));
                (tx, ty, true)
            }
            None => (x + vx * RANGE, y + vy * RANGE, false),
        };
        self.phasers.push(PhaserShot {
            info: PhaserInfo { owner: id, x1: x as i32, y1: y as i32, x2: x2 as i32, y2: y2 as i32, hit },
            ticks: 8,
        });
        true
    }

    /// Strike every enemy within 4,500 at once.
    fn antiproton(&mut self, i: usize) -> bool {
        const RANGE: f64 = 4500.0;
        let id = i as u8;
        if self.players[i].fuel < 2000.0 {
            self.warn(id, "Not enough fuel for an antiproton burst");
            return false;
        }
        self.players[i].fuel -= 2000.0;
        let (x, y) = (self.players[i].x, self.players[i].y);
        let label = self.players[i].tag();
        for (j, d) in self.foes_near(i, RANGE) {
            let q = &self.players[j];
            if q.cloaked && !q.detected {
                continue;
            }
            let (tx, ty) = (q.x, q.y);
            self.phasers.push(PhaserShot {
                info: PhaserInfo { owner: id, x1: x as i32, y1: y as i32, x2: tx as i32, y2: ty as i32, hit: true },
                ticks: 6,
            });
            self.inflict(j, 60.0 * (1.0 - 0.5 * d / RANGE), Some(id), format!("antiproton burst from {}", label));
        }
        true
    }

    fn transwarp(&mut self, i: usize) {
        let id = i as u8;
        let p = &mut self.players[i];
        p.jump_at = None;
        if !p.alive() {
            return;
        }
        let (vx, vy) = dir_vec(p.dir);
        p.leave_orbit();
        p.lock = Lock::None;
        p.tractor = None;
        p.x = (p.x + vx * 15_000.0).clamp(1000.0, GWIDTH - 1000.0);
        p.y = (p.y + vy * 15_000.0).clamp(1000.0, GWIDTH - 1000.0);
        for q in self.players.iter_mut() {
            if matches!(q.tractor, Some((t, _)) if t == id) {
                q.tractor = None;
            }
        }
        self.warn(id, "Transwarp jump!");
    }

    fn toggle_overwatch(&mut self, i: usize) {
        let p = &mut self.players[i];
        if !p.overwatch && p.orbiting.is_none() {
            return self.warn(i as u8, "Overwatch only works in orbit: orbit a planet first");
        }
        p.overwatch = !p.overwatch;
        let text = if p.overwatch {
            "Overwatch ON: firing at any enemy that comes into weapons range (until you leave orbit)"
        } else {
            "Overwatch off"
        };
        self.warn(i as u8, text);
    }

    /// Overwatch: pick the nearest enemy inside weapons range and fire at it,
    /// phasers when close, torpedoes (aimed ahead of it) further out. Holds
    /// fire to keep a fuel and heat reserve, while cloaked, and while
    /// repairing, so it never leaves the ship stranded.
    fn overwatch_fire(&mut self, i: usize) {
        let p = &self.players[i];
        let s = p.stats();
        let tick = self.tick;
        if (p.cloaked && p.ship != ShipType::BirdOfPrey) || p.repair_mode || p.w_overheat > 0 {
            return;
        }
        if p.fuel < s.max_fuel * 0.25 || p.wtemp > s.max_wtemp * 0.7 {
            return;
        }
        let (x, y, team) = (p.x, p.y, p.team);
        let phaser_range = PHASEDIST * s.phaser_damage * (1.0 + 0.1 * self.upgrade(team, UPGRADE_PHASERS)) / 100.0;
        let torp_speed = s.torp_speed * WARP1;
        let torp_range = torp_speed * s.torp_fuse as f64 * 0.8;
        // Special weapons can reach further than the phasers and photons.
        let p = &self.players[i];
        let mut reach = torp_range.max(phaser_range * 0.7);
        if s.plasma_damage > 0.0 {
            reach = reach.max(s.plasma_speed * WARP1 * s.plasma_fuse as f64 * 0.8);
        }
        if p.techs.contains(&Tech::Isokinetic) && tick >= p.tech_ready[1] {
            reach = reach.max(9000.0);
        }
        if p.techs.contains(&Tech::FighterWing) && tick >= p.tech_ready[2] {
            reach = reach.max(15_000.0);
        }
        let target = (0..MAXPLAYER)
            .filter(|&j| j != i && self.at_war(i, j))
            .filter(|&j| {
                let q = &self.players[j];
                let d2 = (q.x - x).powi(2) + (q.y - y).powi(2);
                q.alive()
                    && (!q.cloaked || q.detected)
                    && (!q.hidden || d2 < super::terrain::SENSOR_RANGE.powi(2))
                    && !q.ship.pointless_target()
                    && q.only_hurt_by.map_or(true, |t| t == team)
                    && d2 < reach * reach
            })
            .min_by(|&a, &b| {
                let da = (self.players[a].x - x).powi(2) + (self.players[a].y - y).powi(2);
                let db = (self.players[b].x - x).powi(2) + (self.players[b].y - y).powi(2);
                da.total_cmp(&db)
            });
        let Some(j) = target else { return };
        let q = &self.players[j];
        let d = ((q.x - x).powi(2) + (q.y - y).powi(2)).sqrt();
        let straight = dir_to(x, y, q.x, q.y);
        let (vx, vy) = dir_vec(q.dir);
        let v = q.speed as f64 * WARP1;
        let aim = super::bot::lead(x, y, q.x, q.y, vx * v, vy * v, torp_speed);
        let (qx, qy) = (q.x, q.y);
        let bioship = q.ship == ShipType::Bioship;
        // Special weapons go first, whenever they're ready and it makes sense.
        if self.overwatch_special(i, j, d, straight, aim) {
            return;
        }
        // Plasma next: the Warbird's needs one kill, everyone else's two.
        let p = &self.players[i];
        let plasma_range = s.plasma_speed * WARP1 * s.plasma_fuse as f64 * 0.8;
        let need = if p.ship == ShipType::Warbird { 1.0 } else { 2.0 };
        let can_plasma = s.plasma_damage > 0.0
            && p.kills >= need
            && p.fuel - s.plasma_cost >= s.max_fuel * 0.25
            && !self.torps.iter().any(|t| t.owner == i as u8 && t.kind == TorpKind::Plasma);
        let plasma_aim = dir_to(x, y, qx, qy);
        if can_plasma && d < plasma_range && !self.players[i].sys_out(System::Torpedoes) {
            self.fire_torp(i, plasma_aim, TorpKind::Plasma);
            return;
        }
        // Only plasma hurts Species 8472 bioships; nothing else is worth firing.
        if bioship {
            return;
        }
        let p = &self.players[i];
        if d < phaser_range * 0.7 && p.phaser_timer == 0 && !p.in_storm && s.phaser_damage > 0.0 && !p.sys_out(System::Phasers) {
            self.fire_phaser(i, straight);
            return;
        }
        let out = self.torps.iter().filter(|t| t.owner == i as u8 && t.kind == TorpKind::Photon).count();
        if d < torp_range && s.torp_damage > 0.0 && out < self.players[i].ship.max_torps() && (tick + i as u32) % 5 == 0 && !self.players[i].sys_out(System::Torpedoes) {
            let spread = ((tick % 7) as f64 - 3.0) * 0.8;
            self.fire_torp(i, (aim + spread).rem_euclid(256.0), TorpKind::Photon);
        }
    }

    /// Overwatch: fire a special weapon at target `j` (distance `d`) if one
    /// is ready and worth using. Returns whether it fired.
    fn overwatch_special(&mut self, i: usize, j: usize, d: f64, straight: f64, aim: f64) -> bool {
        let tick = self.tick;
        let p = &self.players[i];
        let s = p.stats();
        let reserve = s.max_fuel * 0.25;
        // Starbase fighters, when an enemy comes near the base.
        if p.ship == ShipType::Starbase && p.techs.contains(&Tech::FighterWing) && tick >= p.tech_ready[2] && d < 15_000.0 {
            self.use_tech(i, 2, straight);
            return self.players[i].tech_ready[2] > tick;
        }
        if p.ship == ShipType::Starbase || tick < p.tech_ready[1] {
            return false;
        }
        let Some(tech) = p.techs.iter().copied().find(|t| t.slot() == Some(1)) else { return false };
        let (tx, ty) = (self.players[j].x, self.players[j].y);
        let team = p.team;
        let usable = match tech {
            Tech::Isokinetic => d < 9000.0 && p.fuel - 1500.0 >= reserve,
            Tech::Antiproton => d < 4500.0 && p.fuel - 2000.0 >= reserve,
            Tech::Tricobalt => {
                // Its blast hurts everyone: only at a safe distance, and never
                // with a friendly ship near the target.
                let friends_near = self.players.iter().any(|q| {
                    q.alive()
                        && q.faction.is_none()
                        && (q.team == team || self.allied(q.team, team))
                        && (q.x - tx).powi(2) + (q.y - ty).powi(2) < 4000.0f64.powi(2)
                });
                (4500.0..5400.0).contains(&d) && !friends_near && p.fuel - TRICOBALT_COST >= reserve
            }
            _ => false,
        };
        if !usable {
            return false;
        }
        let dir = if tech == Tech::Tricobalt { aim } else { straight };
        self.use_tech(i, 1, dir);
        self.players[i].tech_ready[1] > tick
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
        if self.allied(self.planets[k].owner, p.team) {
            return self.warn(id, "That planet belongs to your allies!");
        }
        let floor = if p.ship == ShipType::HusnockWarship { 1 } else { 4 };
        if self.planets[k].armies <= floor {
            return self.warn(id, "Too few armies left to bomb");
        }
        if self.sheliak_shield(k, p.team) {
            return self.warn(id, "A Sheliak shield protects this planet from bombing");
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
        if p.sys_out(System::Transporters) {
            return self.warn(id, "Transporters are out! (/fix transporters)");
        }
        if up {
            if self.planets[k].owner != p.team {
                return self.warn(id, "Can only beam up armies from your own planets");
            }
            if p.max_armies_now() == 0 {
                return self.warn(id, "You need kills to carry armies (2 per kill)");
            }
        } else if p.armies == 0 {
            return self.warn(id, "You have no armies on board");
        } else if self.allied(self.planets[k].owner, p.team) {
            return self.warn(id, "That planet belongs to your allies!");
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
        if self.players[i].sys_out(System::Tractor) {
            return self.warn(id, "The tractor beam emitter is out! (/fix tractor)");
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
        // Trapped on the Tempest's web: the Superzapper, once, clears it.
        if self.players[i].trapped && !self.players[i].zapped {
            self.players[i].zapped = true;
            let tick = self.tick;
            if let Some(t) = self.tempest.as_mut() {
                t.zapped_at = tick;
            }
            let minions: Vec<usize> = (0..MAXPLAYER).filter(|&j| self.players[j].alive() && is_tempest_minion(self.players[j].ship)).collect();
            let who = self.players[i].label();
            self.alert(format!("{} fires the SUPERZAPPER! {} climbers destroyed.", who, minions.len()));
            for j in minions {
                self.kill(j, Some(id), "was superzapped".into());
            }
            return;
        }
        let (x, y, team) = (self.players[i].x, self.players[i].y, self.players[i].team);
        if self.players[i].fuel < 100.0 || self.players[i].w_overheat > 0 {
            return;
        }
        self.players[i].fuel -= 100.0;
        self.players[i].wtemp += 10.0;
        // The blast also shakes off swarm ships.
        let swarm: Vec<usize> = (0..MAXPLAYER)
            .filter(|&j| self.players[j].alive() && self.players[j].ship == ShipType::SwarmShip)
            .filter(|&j| ((self.players[j].x - x).powi(2) + (self.players[j].y - y).powi(2)).sqrt() < DETDIST)
            .collect();
        for j in swarm {
            self.kill(j, Some(id), "was shaken off".into());
        }
        // Fighting off boarders: each blast takes one down.
        for b in self.boardings.iter_mut().filter(|b| b.target == id && b.troops > 0) {
            b.troops -= 1;
        }
        // ...and the shock wave fries nanites, ours and those of ships close by.
        let cured: Vec<usize> = (0..MAXPLAYER)
            .filter(|&j| self.players[j].alive() && self.players[j].nanites)
            .filter(|&j| j == i || ((self.players[j].x - x).powi(2) + (self.players[j].y - y).powi(2)).sqrt() < DETDIST * 2.0)
            .collect();
        for j in cured {
            self.players[j].nanites = false;
            self.warn(j as u8, "The detonation's shock wave burns out the nanites!");
        }
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
            self.planets[k].owner == p.team && (self.planets[k].flags & PL_HOME != 0 || self.planets[k].outpost == Some((Outpost::Shipyard, p.team)))
        });
        if !ok {
            return self.warn(id, "You must orbit your home planet (or a shipyard) to refit");
        }
        if p.armies > 0 {
            return self.warn(id, "Beam your armies down before refitting");
        }
        let ship = match self.hull_for(i, p.team, ship) {
            Ok(s) => s,
            Err(e) => return self.warn(id, e),
        };
        let p = &self.players[i];
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
        self.fit_tech(i);
    }

    /// Check a ship choice: the classics for anyone; an empire's special
    /// ship for Captains and up; a relic (drawn at random) for Commodores
    /// and up. Specials and relics need ranks on.
    fn hull_for(&self, i: usize, team: Team, ship: ShipType) -> Result<ShipType, String> {
        // (The server's own ships, like convoy freighters, fly anything.)
        if !ship.playable() && !self.players[i].robot {
            return Err("That ship can't be flown".into());
        }
        if !ship.is_special() && !ship.is_relic() {
            return Ok(ship);
        }
        let p = &self.players[i];
        if !self.features.ranks || p.robot {
            return Err("Special and relic ships need a server running with ranks".into());
        }
        let rank = p.rank.unwrap_or(0);
        if ship.is_special() {
            if rank < SPECIAL_RANK {
                return Err(format!("Special ships need the rank of {}", RANKS[SPECIAL_RANK as usize].0));
            }
            return ShipType::special_for(team).ok_or_else(|| "No special ship for that empire".into());
        }
        if rank < RELIC_RANK {
            return Err(format!("Relic ships need the rank of {}", RANKS[RELIC_RANK as usize].0));
        }
        Ok(*ShipType::RELICS.choose(&mut rand::thread_rng()).unwrap())
    }

    /// Fit a senior officer's advanced tech (at launch and refit): one tech
    /// from each rank reached, from Captain up. An Admiral in a starbase gets
    /// starbase tech in place of the active techs a starbase can't use.
    fn fit_tech(&mut self, i: usize) {
        let mut rng = rand::thread_rng();
        let (ranks, rank_tech) = (self.features.ranks, self.features.rank_tech);
        let p = &mut self.players[i];
        let id = p.id;
        p.techs.clear();
        p.armor = 0.0;
        let ranked = ranks && !p.robot && p.decoy_until.is_none();
        let rank = if ranked { p.rank.unwrap_or(0) } else { 0 };
        // Senior officers are worth hunting: +0.5 kill credit per rank above Commander.
        p.bounty = if ranked { rank.saturating_sub(STARBASE_RANK) as f64 * 5.0 } else { 0.0 };
        if !ranked || !rank_tech {
            return;
        }
        let starbase = p.ship == ShipType::Starbase;
        let tiers = (rank + 1).saturating_sub(TECH_RANK).min(5) as usize;
        for (t, tier) in Tech::TIERS.iter().take(tiers).enumerate() {
            if !(starbase && t >= 2) {
                p.techs.push(tier[rng.gen_range(0..3)]);
            }
        }
        if starbase && tiers == 5 {
            for set in Tech::STARBASE {
                p.techs.push(set[rng.gen_range(0..3)]);
            }
        }
        if p.techs.contains(&Tech::AblativeArmor) {
            p.armor = ABLATIVE_ARMOR;
        }
        if !p.techs.is_empty() {
            let list: Vec<String> = p
                .techs
                .iter()
                .map(|t| match t.key() {
                    Some(k) => format!("{} [{}]", t.name(), k),
                    None => t.name().to_string(),
                })
                .collect();
            let text = format!("Advanced tech aboard: {}", list.join(", "));
            self.warnings.push((id, text.clone()));
            self.reply(id, text);
        }
    }

    // ------------------------------------------------------------------
    // damage & death

    /// Whether ships `a` and `b` are enemies: different teams, or alien
    /// factions at war with each other (the Borg and Species 8472).
    pub fn at_war(&self, a: usize, b: usize) -> bool {
        let (pa, pb) = (&self.players[a], &self.players[b]);
        self.hostile(pa.team, pb.team) || matches!((pa.faction, pb.faction), (Some(x), Some(y)) if x.at_war_with(y))
    }

    pub fn inflict(&mut self, i: usize, amount: f64, killer: Option<u8>, how: String) {
        let kind = std::mem::replace(&mut self.hit_kind, HitKind::Other);
        let killer_team = killer.map(|k| self.players[k as usize].team);
        let killer_trapped = killer.map_or(false, |k| self.players[k as usize].trapped);
        let tick = self.tick;
        let p = &mut self.players[i];
        if !p.alive() || amount <= 0.0 || tick < p.phased_until {
            return;
        }
        if p.only_hurt_by.is_some() && killer_team != p.only_hurt_by {
            return;
        }
        // A Suliban cell ship's enhanced reflexes: about a third of hits miss.
        if p.ship == ShipType::SulibanCell && rand::thread_rng().gen_bool(0.35) {
            return;
        }
        p.last_hit = tick;
        // Any hit on Chang's Bird-of-Prey lights up its exhaust.
        if p.ship == ShipType::BirdOfPrey {
            let hidden = tick >= p.revealed_until;
            p.revealed_until = tick + 20 * UPS as u32;
            p.cloaked = false;
            if hidden {
                self.alert("Chang's Bird-of-Prey is hit! Its plasma exhaust gives it away. Fire at will!");
            }
        }
        // Any hit makes a Changeling lose its shape for a while.
        let p = &mut self.players[i];
        if p.ship == ShipType::ChangelingShip {
            let hidden = tick >= p.revealed_until;
            p.revealed_until = tick + 30 * UPS as u32;
            if hidden {
                let who = killer.map(|k| self.players[k as usize].label()).unwrap_or_else(|| "planetary defences".into());
                self.alert(format!("A ship hit by {} melts out of shape: it was a Changeling!", who));
            }
        }
        // Armus feeds on violence: every shot makes the slick bigger.
        let p = &mut self.players[i];
        if p.ship == ShipType::ArmusSlick {
            p.swell = (p.swell + amount * 6.0).min(ARMUS_MAX_SWELL);
            return;
        }
        let p = &mut self.players[i];
        let amount = match p.ship {
            // Invulnerable: they have to be dealt with some other way.
            ShipType::VgerCloud | ShipType::WhaleProbe | ShipType::QEntity => return,
            ShipType::NomadProbe | ShipType::MetronPresence | ShipType::DarkMatterAnomaly => return,
            // The Caretaker's shields weaken each time it sends out a wave.
            ShipType::CaretakerArray => amount * p.adapt,
            // The Tempest core is only exposed when its web has been cleared;
            // ships trapped on its rim hit it full on, others at half strength.
            ShipType::TempestCore if !self.tempest.as_ref().map_or(false, |t| t.exposed) => return,
            ShipType::TempestCore if !killer_trapped => amount * 0.5,
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
                    self.alert("The Borg have adapted to your weapons!");
                }
                amount * before
            }
            _ => amount,
        };
        // A Vidiian harvester repairs itself with what it takes from others.
        if let Some(k) = killer.map(|k| k as usize).filter(|&k| k != i && self.players[k].ship == ShipType::VidiianHarvester) {
            let v = &mut self.players[k];
            v.damage = (v.damage - amount * 0.4).max(0.0);
        }
        let absorb = 1.0 - 0.1 * self.upgrade(self.players[i].team, UPGRADE_SHIELDS);
        let p = &mut self.players[i];
        // Polaron beams go straight through shields; overcharged phasers partly.
        let (through, rest) = match kind {
            HitKind::Polaron => (amount, 0.0),
            HitKind::Overcharge => (amount * 0.25, amount * 0.75),
            _ => (0.0, amount),
        };
        let mut hull = through;
        if p.shields_up {
            p.shield -= rest * absorb;
            if p.shield < 0.0 {
                hull -= p.shield;
                p.shield = 0.0;
            }
        } else {
            hull += rest;
        }
        // Ablative armor soaks up hull damage first.
        let soak = hull.min(p.armor);
        p.armor -= soak;
        p.damage += hull - soak;
        p.repair_mode = false;
        // Hits that get through to the hull can knock out a system.
        let hit = hull - soak;
        if self.features.subsystems && hit >= 3.0 && p.faction.is_none() && p.fighter_of.is_none() {
            let mut rng = rand::thread_rng();
            if rng.gen_bool((hit / 30.0).min(0.9)) {
                let sy = System::ALL[rng.gen_range(0..System::ALL.len())];
                self.damage_system(i, sy, hit * 2.5);
            }
        }
        let p = &mut self.players[i];
        if p.damage >= p.stats().max_damage {
            self.kill(i, killer, how);
        }
    }

    /// Start building an outpost on the planet ship `i` is orbiting.
    pub fn start_build(&mut self, i: usize, kind: Outpost) {
        let id = i as u8;
        let p = &self.players[i];
        let Some(k) = p.orbiting.filter(|&k| self.planets[k].owner == p.team) else {
            return self.warn(id, "Orbit a planet you own to build on it");
        };
        if self.planets[k].outpost.map(|o| o.0) == Some(kind) {
            return self.warn(id, format!("{} already has a {}", self.planets[k].name, kind.name()));
        }
        if self.builds.iter().any(|b| b.planet == k) {
            return self.warn(id, format!("Something is already being built on {}", self.planets[k].name));
        }
        if let Err(why) = self.build_cost(i, false) {
            return self.warn(id, why);
        }
        self.builds.retain(|b| b.builder != id);
        self.builds.push(Build { builder: id, planet: k, kind, done_at: self.tick + BUILD_SECS * UPS as u32 });
        let replacing = self.planets[k].outpost.map_or(String::new(), |o| format!(" (replacing the {})", o.0.name()));
        self.warn(id, format!("Building a {} on {}{}: stay in orbit for {} seconds", kind.name(), self.planets[k].name, replacing, BUILD_SECS));
    }

    /// Check (and, with `pay`, take) what an outpost costs ship `i`:
    /// supplies from its empire's stockpile (with --supply), or failing that
    /// armies it carries, who stay on as the builders.
    fn build_cost(&mut self, i: usize, pay: bool) -> Result<(), String> {
        let team = self.players[i].team;
        if self.features.supply && self.supply[team.idx()].stock >= BUILD_SUPPLIES {
            if pay {
                self.supply[team.idx()].stock -= BUILD_SUPPLIES;
            }
            return Ok(());
        }
        let p = &mut self.players[i];
        if p.armies < BUILD_ARMIES {
            return Err(if self.features.supply {
                format!("An outpost takes {} supplies (your empire has {}) or {} armies aboard as builders", BUILD_SUPPLIES, self.supply[team.idx()].stock, BUILD_ARMIES)
            } else {
                format!("An outpost takes {} armies as builders; carry them here", BUILD_ARMIES)
            });
        }
        if pay {
            p.armies -= BUILD_ARMIES;
        }
        Ok(())
    }

    /// Construction (the builder must stay in orbit), and outposts lost
    /// when their planets change hands.
    fn outpost_tick(&mut self) {
        let tick = self.tick;
        let mut k = 0;
        while k < self.builds.len() {
            let b = self.builds[k];
            let p = &self.players[b.builder as usize];
            if !p.alive() || p.orbiting != Some(b.planet) || self.planets[b.planet].owner != p.team {
                self.builds.remove(k);
                if self.players[b.builder as usize].in_use {
                    self.warn(b.builder, format!("Construction of the {} abandoned", b.kind.name()));
                }
                continue;
            }
            if tick < b.done_at {
                k += 1;
                continue;
            }
            self.builds.remove(k);
            let i = b.builder as usize;
            if let Err(why) = self.build_cost(i, true) {
                self.warn(b.builder, why);
                continue;
            }
            let team = self.players[i].team;
            self.planets[b.planet].outpost = Some((b.kind, team));
            let (who, name) = (self.players[i].label(), self.planets[b.planet].name);
            self.team_msg(team, format!("{} has built a {} on {}", who, b.kind.name(), name));
            self.events.push(GameEvent::Honour { player: b.builder, text: format!("Built a {}", b.kind.name()) });
        }
        for pl in self.planets.iter_mut() {
            if let Some((o, t)) = pl.outpost {
                if pl.owner != t {
                    pl.outpost = None;
                    let text = format!("The {} on {} is destroyed", o.name(), pl.name);
                    self.outbox.push(Outgoing { dest: Dest::All, msg: ChatMsg { kind: MsgKind::System, from: "GOD".into(), text } });
                }
            }
        }
    }

    /// Send a boarding party onto ship `t`: its shields must be down, and
    /// ours close, uncloaked, with armies aboard and transporters working.
    fn board(&mut self, i: usize, t: usize) {
        let id = i as u8;
        if !self.features.boarding {
            return self.warn(id, "Boarding parties are off on this server (--boarding)");
        }
        if t >= MAXPLAYER || t == i || !self.players[t].alive() || !self.at_war(i, t) {
            return self.warn(id, "No enemy ship there to board");
        }
        let (p, q) = (&self.players[i], &self.players[t]);
        let d = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt();
        let why = if !q.ship.boardable() {
            Some("That can't be boarded")
        } else if d > BOARD_RANGE {
            Some("Too far to board: get within 1,500")
        } else if p.armies == 0 {
            Some("You need armies aboard to send a boarding party")
        } else if p.cloaked {
            Some("Decloak first")
        } else if p.sys_out(System::Transporters) {
            Some("Transporters are out! (/fix transporters)")
        } else if q.shields_up && q.shield >= q.stats().max_shield * 0.1 {
            Some("Their shields are up: knock them down first")
        } else if self.tick < q.phased_until || self.tick < p.phased_until {
            Some("Out of phase: the transporters can't lock on")
        } else if self.boardings.iter().any(|b| b.attacker == id && b.target == t as u8) {
            Some("Your marines are already aboard")
        } else {
            None
        };
        if let Some(why) = why {
            return self.warn(id, why);
        }
        let (me, them) = (self.players[i].label(), self.players[t].label());
        // Resistance is futile.
        if self.players[t].ship == ShipType::BorgCube {
            let n = self.players[i].armies;
            self.players[i].armies = 0;
            self.warn(id, format!("Your boarding party of {} has been assimilated by the Borg!", n));
            self.alert(format!("{} sends marines aboard a Borg cube. They are assimilated.", me));
            return;
        }
        let n = self.players[i].armies.min(BOARD_WAVE);
        self.players[i].armies -= n;
        self.boardings.push(Boarding { attacker: id, target: t as u8, troops: n as i32, next: self.tick + BOARD_ROUND });
        self.warn(id, format!("Boarding party away! Marines beaming aboard {}", them));
        self.warn(t as u8, format!("Enemy boarders aboard from {}! Raise shields to stop more coming, or detonate (d) to fight them", me));
    }

    /// Boarding actions: reinforcements while the target's shields are down,
    /// then a round of fighting, every BOARD_ROUND ticks. Prizes follow their
    /// captors and are delivered at home.
    fn boarding_tick(&mut self) {
        let tick = self.tick;
        let mut rng = rand::thread_rng();
        let mut k = 0;
        while k < self.boardings.len() {
            let b = self.boardings[k];
            let (a, t) = (b.attacker as usize, b.target as usize);
            if !self.players[t].alive() || b.troops <= 0 {
                self.boardings.remove(k);
                if b.troops <= 0 && self.players[t].alive() {
                    self.warn(b.attacker, format!("Your boarding party aboard {} has been wiped out", self.players[t].label()));
                    self.warn(b.target, "The boarders have been fought off!");
                }
                continue;
            }
            if tick < b.next {
                k += 1;
                continue;
            }
            self.boardings[k].next = tick + BOARD_ROUND;
            // Reinforcements, while the shields stay down.
            let (p, q) = (&self.players[a], &self.players[t]);
            let close = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt() < BOARD_RANGE * 1.5;
            let open = !q.shields_up || q.shield < q.stats().max_shield * 0.1;
            if p.alive() && p.armies > 0 && close && open && !p.sys_out(System::Transporters) {
                let n = p.armies.min(BOARD_WAVE);
                self.players[a].armies -= n;
                self.boardings[k].troops += n as i32;
            }
            // A round of fighting: the armies aboard defend first, then the crew.
            let troops = self.boardings[k].troops;
            let q = &mut self.players[t];
            let defenders = q.crew.max(0) + q.armies as i32;
            if defenders > 0 {
                if rng.gen_bool(troops as f64 / (troops + defenders) as f64) {
                    if q.armies > 0 {
                        q.armies -= 1;
                    } else {
                        q.crew -= 1;
                    }
                } else {
                    self.boardings[k].troops -= 1;
                }
            }
            let q = &self.players[t];
            let (troops, defenders) = (self.boardings[k].troops, q.crew.max(0) + q.armies as i32);
            if defenders <= 0 && troops > 0 {
                self.boardings.remove(k);
                self.capture(a, t);
                continue;
            }
            let name = self.players[t].label();
            self.warn(b.attacker, format!("Boarding {}: {} marines vs {} defenders", name, troops, defenders));
            self.warn(b.target, format!("Boarders: {} enemy marines vs {} of your crew", troops, defenders));
            k += 1;
        }
        // Prizes follow their captors home.
        let mut z = 0;
        while z < self.prizes.len() {
            let pz = self.prizes[z].clone();
            let c = &self.players[pz.captor as usize];
            if !c.alive() || c.team != pz.captor_team {
                self.prizes.remove(z);
                if self.players[pz.captor as usize].in_use {
                    self.players[pz.captor as usize].towing = false;
                }
                self.god(format!("The captured {} is lost", pz.ship.stats().name));
                continue;
            }
            let (vx, vy) = dir_vec(c.dir);
            let (x, y, dir) = (c.x - vx * PRIZE_TETHER, c.y - vy * PRIZE_TETHER, c.dir);
            let home = c.orbiting.filter(|&k| self.planets[k].owner == c.team && self.planets[k].flags & (PL_REPAIR | PL_HOME) != 0);
            let pr = &mut self.prizes[z];
            (pr.x, pr.y, pr.dir) = (x.clamp(0.0, GWIDTH), y.clamp(0.0, GWIDTH), dir);
            let Some(k) = home else {
                z += 1;
                continue;
            };
            // Delivered: the prize crew joins the garrison.
            self.prizes.remove(z);
            let ci = pz.captor as usize;
            let armies = pz.ship.crew();
            self.planets[k].armies += armies;
            let team = self.players[ci].team;
            let supplies = if self.features.supply {
                self.supply[team.idx()].stock += 10;
                " and 10 supplies"
            } else {
                ""
            };
            let p = &mut self.players[ci];
            p.towing = false;
            p.kills += 1.0;
            p.total_kills += 1.0;
            let who = p.label();
            self.events.push(GameEvent::Honour { player: pz.captor, text: "Brought home a prize".into() });
            self.god(format!(
                "{} brings the captured {} home to {}: {} armies{} (+1 kill)",
                who,
                pz.ship.stats().name,
                self.planets[k].name,
                armies,
                supplies
            ));
        }
    }

    /// Boarders have taken ship `t`: its pilot bails out, and (for an empire
    /// ship) the hull becomes a prize to tow home. Aliens are simply taken.
    fn capture(&mut self, a: usize, t: usize) {
        let captor = self.players[a].in_use.then_some(a as u8);
        let (who, victim, ship, from) = (self.players[a].label(), self.players[t].label(), self.players[t].ship, self.players[t].team);
        let (x, y, dir) = (self.players[t].x, self.players[t].y, self.players[t].dir);
        let alien = self.players[t].faction.is_some();
        self.kill(t, captor, format!("was captured by a boarding party from {}", who));
        // Taken intact: no explosion.
        self.players[t].just_exploded = false;
        let Some(c) = captor.map(|c| c as usize) else { return };
        let p = &mut self.players[c];
        p.kills += 1.0;
        p.total_kills += 1.0;
        let honour = if alien { format!("Captured a {}", ship.stats().name) } else { "Captured an enemy ship".to_string() };
        self.events.push(GameEvent::Honour { player: c as u8, text: honour });
        if alien || !self.players[c].alive() || self.players[c].towing {
            self.alert(format!("{}'s boarding party captures {}! (+1 kill)", who, victim));
            return;
        }
        let captor_team = self.players[c].team;
        self.prizes.push(Prize { x, y, dir, ship, from, captor: c as u8, captor_team });
        self.players[c].towing = true;
        self.god(format!("{}'s boarding party captures {}! (+1 kill)", who, victim));
        self.warn(c as u8, format!("Prize taken! Tow the {} to a repair planet or your home world (warp 6 while towing)", ship.stats().name));
    }

    /// Knock `amount` points off one of ship `i`'s systems, with the
    /// consequences if it goes out.
    pub fn damage_system(&mut self, i: usize, sy: System, amount: f64) {
        let id = i as u8;
        let p = &mut self.players[i];
        let was = p.systems[sy as usize];
        if was <= 0.0 {
            return;
        }
        let now = (was - amount).max(0.0);
        p.systems[sy as usize] = now;
        if now > 0.0 {
            if was >= 50.0 && now < 50.0 {
                self.warn(id, format!("{} damaged!", sy.name()));
            }
            return;
        }
        match sy {
            System::Shields => p.shields_up = false,
            System::Cloak => p.cloaked = false,
            System::Tractor => p.tractor = None,
            System::Transporters => (p.beam_up, p.beam_down) = (false, false),
            _ => {}
        }
        self.warn(id, format!("{} knocked out! (/fix {})", sy.name(), sy.abbr().to_ascii_lowercase()));
    }

    pub fn kill(&mut self, i: usize, killer: Option<u8>, how: String) {
        // A holographic decoy just flickers out.
        if self.players[i].decoy_until.is_some() {
            self.remove_player(i as u8);
            return;
        }
        self.hunt_outcome(i, killer);
        let victim_kills = self.players[i].kills + self.players[i].bounty;
        let victim_armies = self.players[i].armies;
        // A marauder's stolen armies spill out for anyone to grab.
        if self.players[i].faction == Some(Faction::Ferengi) && victim_armies > 0 {
            let (x, y) = (self.players[i].x, self.players[i].y);
            self.loot.push(Loot { x, y, armies: victim_armies, ttl: 60 * UPS as i32 });
            let what = if victim_armies == 1 { "army" } else { "armies" };
            self.alert(format!("A Ferengi marauder breaks up and spills {} stolen {} into space!", victim_armies, what));
        }
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
            p.tribbles = false;
            p.marked = false;
            p.trapped = false;
            p.nanites = false;
            p.overwatch = false;
            p.last_death = Some((p.x, p.y));
        }
        let with_armies = if victim_armies > 0 { format!(" (carrying {} armies)", victim_armies) } else { String::new() };
        if self.players[i].ship == ShipType::Freighter && self.players[i].cargo > 0 {
            let (team, cargo) = (self.players[i].team, self.players[i].cargo);
            self.players[i].cargo = 0;
            self.god(format!("The {} convoy is destroyed with {} supplies aboard!", team.name(), cargo));
        }
        let killer_ok = killer.map(|k| k as usize).filter(|&k| k != i && self.players[k].in_use);
        let credit = 1.0 + victim_kills * 0.1 + victim_armies as f64 * 0.1;
        self.events.push(GameEvent::Kill { killer: killer_ok.map(|k| k as u8), victim: i as u8, credit });
        // Destroying one of the great monsters is a career honour.
        if let (Some(k), true) = (killer_ok, self.players[i].faction.is_some() && self.players[i].bounty >= 15.0) {
            let text = format!("Destroyed {}", self.players[i].stats().name);
            self.events.push(GameEvent::Honour { player: k as u8, text });
        }
        match killer_ok {
            Some(k) => {
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

    /// The Hirogen hunt: prey killed by a hunter loses a trophy (half the
    /// kills it made this life come off its career total) and the hunters
    /// patch themselves up; prey that kills a hunter earns an extra kill.
    fn hunt_outcome(&mut self, i: usize, killer: Option<u8>) {
        let Some(k) = killer.map(|k| k as usize).filter(|&k| k != i && self.players[k].in_use) else { return };
        if self.players[i].marked && self.players[k].faction == Some(Faction::Hirogen) {
            let v = &mut self.players[i];
            let lost = v.kills * 0.5;
            v.total_kills = (v.total_kills - lost).max(0.0);
            let who = v.label();
            self.alert(format!("The Hirogen take a trophy from {} ({:.1} career kills)!", who, lost));
            for p in self.players.iter_mut().filter(|p| p.alive() && p.faction == Some(Faction::Hirogen)) {
                p.damage = 0.0;
                p.shield = p.stats().max_shield;
            }
        }
        if self.players[i].faction == Some(Faction::Hirogen) && self.players[k].marked {
            let p = &mut self.players[k];
            p.kills += 1.0;
            p.total_kills += 1.0;
            let who = p.label();
            self.events.push(GameEvent::Honour { player: k as u8, text: "Bit back at the Hirogen".into() });
            self.alert(format!("The prey bites back! {} destroys a Hirogen hunter (+1 kill).", who));
        }
    }

    fn ship_explosion(&mut self, i: usize) {
        let (x, y) = (self.players[i].x, self.players[i].y);
        let base = match self.players[i].ship {
            ShipType::Starbase => 200.0,
            ShipType::Scout => 75.0,
            ShipType::SwarmShip => 5.0,
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
            // Scotty's gambit with the Jenolan: a ship blowing up in a Dyson
            // sphere's doorway wrecks the hatch.
            if self.players[j].ship == ShipType::DysonHatch {
                dmg *= 6.0;
            }
            self.inflict(j, dmg, Some(i as u8), format!("caught in the explosion of {}", tag));
        }
    }

    // ------------------------------------------------------------------
    // the update

    pub fn tick(&mut self) {
        self.tick += 1;
        self.chatter.clear();
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
        self.tempest_trap();
        if self.features.boarding {
            self.boarding_tick();
        }
        if self.features.outposts {
            self.outpost_tick();
        }
        for i in 0..MAXPLAYER {
            if self.players[i].overwatch && self.players[i].alive() {
                // Overwatch is a sentry post: it only holds while in orbit
                // (a starbase's fighters are the exception).
                if self.players[i].orbiting.is_none() && self.players[i].fighter_of.is_none() {
                    self.players[i].overwatch = false;
                    self.warn(i as u8, "Overwatch off: you've left orbit");
                } else {
                    self.overwatch_fire(i);
                }
            }
            if self.players[i].jump_at.map_or(false, |t| self.tick >= t) {
                self.transwarp(i);
            }
            if let Some(until) = self.players[i].decoy_until {
                if self.tick >= until || !self.players[i].alive() {
                    self.remove_player(i as u8);
                }
            }
            if self.players[i].fighter_of.is_some() {
                self.fly_fighter(i);
            }
            if self.players[i].ship == ShipType::KazonRaider {
                self.kazon_ram(i);
            }
            if self.players[i].alive() && self.players[i].ship == ShipType::Starbase && !self.players[i].techs.is_empty() {
                self.starbase_passives(i);
            }
        }
        // Terrain from the --terrain option, or planted by the Sphere Builders.
        if self.features.terrain || !self.terrain.is_empty() {
            super::terrain::tick(self);
        }
        if self.features.diplomacy {
            self.update_treaties();
        }
        self.update_tractors();
        self.update_torps();
        self.update_webs();
        self.update_loot();
        // Nobody may be reading events (e.g. in tests): don't let them pile up.
        if self.events.len() > 2000 {
            self.events.drain(..1000);
        }
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
        // Damaged impulse engines accelerate and turn sluggishly.
        let agile = match p.sys(System::Impulse) {
            m if m <= 0.0 => 0.25,
            m => 0.4 + 0.6 * m,
        };
        if p.speed < p.desired_speed {
            p.sub_speed += (s.acc as f64 * agile) as i32;
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
                p.sub_dir += s.turns * agile / (p.speed * p.speed) as f64;
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
        if self.tick < p.netted_until {
            p.desired_speed = p.desired_speed.min(1);
            p.speed = p.speed.min(1);
        }
        let powerless = self.tick < p.powerless_until;
        if powerless {
            p.desired_speed = p.desired_speed.min(1);
            p.shields_up = false;
            p.cloaked = false;
        }

        // Fuel.
        let engines = 1.0 + 0.2 * if self.features.supply { self.supply[p.team.idx()].levels[UPGRADE_ENGINES] as f64 } else { 0.0 };
        if !powerless {
            p.fuel += 2.0 * s.recharge * engines;
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
        let fix = 1.0 + 0.25 * if self.features.supply { self.supply[p.team.idx()].levels[UPGRADE_REPAIR] as f64 } else { 0.0 };
        let at_repair = own_planet.map_or(false, |(_, _, f)| f & PL_REPAIR != 0);
        let mut smul = if p.repair_mode { 4.0 } else { 2.0 };
        if p.techs.contains(&Tech::RegenShields) && self.tick.saturating_sub(p.last_hit) > 5 * UPS as u32 {
            smul *= 3.0;
        }
        if p.techs.contains(&Tech::AblativeArmor) && (p.repair_mode || at_repair) {
            p.armor = (p.armor + 0.1).min(ABLATIVE_ARMOR);
        }
        let mut dmul = if p.repair_mode { 2.0 } else { 1.0 };
        if at_repair {
            smul += 2.0;
            dmul += 1.0;
        }
        // Damaged shield generators recharge slowly.
        smul *= p.sys(System::Shields);
        if p.shield < s.max_shield {
            p.shield = (p.shield + s.repair * fix * smul / 1000.0).min(s.max_shield);
        }
        // Damage control: systems come back, the /fix one first.
        if self.features.subsystems {
            let first = p.fix_first.filter(|&f| p.systems[f as usize] < 100.0);
            let mut back = Vec::new();
            for sy in System::ALL {
                let h = &mut p.systems[sy as usize];
                if *h >= 100.0 {
                    continue;
                }
                let focus = match first {
                    Some(f) if f == sy => 3.0,
                    Some(_) => 0.5,
                    None => 1.0,
                };
                let was = *h;
                *h = (*h + 0.15 * fix * dmul * focus).min(100.0);
                if was <= 0.0 && *h > 0.0 {
                    back.push(sy);
                }
            }
            if first.is_some() && p.fix_first.map_or(false, |f| p.systems[f as usize] >= 100.0) {
                p.fix_first = None;
            }
            let id = p.id;
            for sy in back {
                self.warn(id, format!("{} back online", sy.name()));
            }
        }
        let p = &mut self.players[i];
        if p.damage > 0.0 {
            p.damage = (p.damage - s.repair * fix * dmul / 1000.0).max(0.0);
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
            // A Husnock warship bombs harder, and all the way down to 1 army.
            let husnock = p.ship == ShipType::HusnockWarship;
            let floor = if husnock { 1 } else { 4 };
            let beam_every = if p.ship == ShipType::VothCityShip { 4 } else { 8 };
            // Damaged transporters beam slowly.
            let beam_every = (beam_every as f64 / p.sys(System::Transporters).max(0.25)).round() as i32;
            let team = p.team;
            let shielded = self.sheliak_shield(k, team);
            let p = &mut self.players[i];
            if p.bombing && p.action_timer >= 5 {
                p.action_timer = 0;
                if owner == p.team || self.planets[k].armies <= floor {
                    p.bombing = false;
                    self.warn(id, format!("Bombing stopped: planet down to {} {}", floor, if floor == 1 { "army" } else { "armies" }));
                } else if shielded {
                    p.bombing = false;
                    self.warn(id, "A Sheliak shield protects this planet from bombing");
                } else if rng.gen_bool(if self.planets[k].outpost.map_or(false, |o| o.0 == Outpost::Defence) { 0.3 } else { 0.6 }) {
                    let n = if husnock { 3 } else if p.ship == ShipType::Assault { 2 } else { 1 };
                    let n = n.min(self.planets[k].armies - floor);
                    self.planets[k].armies -= n;
                    self.planets[k].tribbles = false;
                    self.events.push(GameEvent::Bombed { player: id, planet: k, armies: n });
                    p.kills += 0.02 * n as f64;
                    p.total_kills += 0.02 * n as f64;
                }
            } else if p.beam_up && p.action_timer >= beam_every {
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
            } else if p.beam_down && p.action_timer >= beam_every {
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
            self.events.push(GameEvent::Reinforced { player: i as u8, planet: k });
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
        self.events.push(GameEvent::PlanetTaken { player: i as u8, planet: k });
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
        let treaties = self.treaties.clone();
        let foes = |a: Team, b: Team| a != b && !treaties.iter().any(|&(x, y)| (x == a && y == b) || (x == b && y == a));
        // Preserver obelisks' deflectors, as (x, y, team, id).
        let deflectors: Vec<(f64, f64, Team, u8)> = self
            .players
            .iter()
            .filter(|p| p.alive() && p.ship == ShipType::PreserverObelisk)
            .map(|p| (p.x, p.y, p.team, p.id))
            .collect();
        let tick = self.tick;
        for (n, t) in self.torps.iter_mut().enumerate() {
            if t.explode > 0 {
                t.explode += 1;
                continue;
            }
            // An enemy photon coming within 1,200 of an obelisk is usually
            // turned back the way it came, and becomes the obelisk's.
            if t.kind == TorpKind::Photon && !t.deflect_tried {
                for &(ox, oy, team, oid) in &deflectors {
                    if foes(team, t.team) && (t.x - ox).powi(2) + (t.y - oy).powi(2) < 1200.0f64.powi(2) {
                        t.deflect_tried = true;
                        if (tick as usize + n) % 5 < 3 {
                            t.dir = (t.dir + 128.0).rem_euclid(256.0);
                            t.owner = oid;
                            t.team = team;
                        }
                        break;
                    }
                }
            }
            // Plasma homes in on the nearest enemy.
            if t.kind == TorpKind::Plasma {
                let target = self
                    .players
                    .iter()
                    .filter(|p| p.alive() && foes(p.team, t.team) && (!p.cloaked || p.detected) && !p.hidden)
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
                    foes(p.team, t.team) || matches!((owner_fac, p.faction), (Some(x), Some(y)) if x.at_war_with(y))
                };
                let tick = self.tick;
                boom = self.players.iter().any(|p| {
                    let r = p.ship.hit_radius();
                    p.alive()
                        && tick >= p.phased_until
                        && hostile(p)
                        && (p.x - t.x).abs() < r
                        && (p.y - t.y).abs() < r
                        && (p.x - t.x).powi(2) + (p.y - t.y).powi(2) < r * r
                });
            }
            if boom {
                t.explode = 1;
                let damdist = match t.kind {
                    TorpKind::Plasma => PLASDAMDIST,
                    TorpKind::Tricobalt => TRICOBALT_BLAST,
                    TorpKind::Photon => DAMDIST,
                };
                let owner_fac = self.players.get(t.owner as usize).and_then(|o| if o.in_use { o.faction } else { None });
                for (j, p) in self.players.iter().enumerate() {
                    let hostile = foes(p.team, t.team) || matches!((owner_fac, p.faction), (Some(x), Some(y)) if x.at_war_with(y));
                    // A tricobalt blast hurts everyone in range, friend or foe.
                    if !p.alive() || (!hostile && t.kind != TorpKind::Tricobalt) {
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
            let what = match kind {
                TorpKind::Plasma => "plasma",
                TorpKind::Tricobalt => "a tricobalt device",
                TorpKind::Photon => "torp",
            };
            let tag = self.players[owner as usize].tag();
            self.hit_kind = match kind {
                TorpKind::Plasma => HitKind::Plasma,
                TorpKind::Tricobalt => HitKind::Other,
                TorpKind::Photon => HitKind::Photon,
            };
            self.inflict(j, dmg, Some(owner), format!("killed by {} from {}", what, tag));
            // Breen energy-dampening torpedoes.
            if kind == TorpKind::Photon && self.players[owner as usize].ship == ShipType::BreenWarship && self.players[j].alive() {
                let tick = self.tick;
                let q = &mut self.players[j];
                q.shields_up = false;
                q.jammed_until = tick + 3 * UPS as u32;
                q.fuel = (q.fuel - 500.0).max(0.0);
                self.warn(j as u8, "Breen energy dampener! Shields down, power drained");
            }
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
                // A player's web-spinner spares its own side (and allies).
                let o = &self.players[w.owner as usize];
                if o.in_use && o.faction.is_none() && !self.hostile(o.team, p.team) {
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

    // ------------------------------------------------------------------
    // slash commands and diplomacy

    fn command(&mut self, id: u8, text: &str) {
        let mut words = text.trim_start_matches('/').split_whitespace();
        let verb = words.next().unwrap_or("").to_ascii_lowercase();
        let arg = words.next().unwrap_or("").to_ascii_lowercase();
        let team = self.players[id as usize].team;
        let d = self.features.diplomacy;
        match verb.as_str() {
            "treaty" | "ally" if d => match parse_team_word(&arg) {
                Some(t) => self.propose(team, t, Some(id)),
                None => self.warn(id, "Usage: /treaty fed|rom|kli|ori"),
            },
            "break" if d => self.break_treaty(team, Some(id)),
            "treaties" if d => {
                let list = if self.treaties.is_empty() {
                    "No treaties are in force.".to_string()
                } else {
                    let v: Vec<String> = self.treaties.iter().map(|(a, b)| format!("{} + {}", a.plural(), b.plural())).collect();
                    format!("Treaties: {}", v.join(", "))
                };
                self.reply(id, list);
            }
            "upgrade" | "buy" if self.features.supply => self.buy_upgrade(team, &arg, Some(id)),
            "supplies" if self.features.supply => {
                let s = self.supply[team.idx()];
                let levels: Vec<String> = UPGRADES.iter().zip(s.levels).map(|((n, _), l)| format!("{} {}", n, l)).collect();
                self.reply(id, format!("{} supplies. Upgrades: {}. Buy with /upgrade <name>.", s.stock, levels.join(", ")));
            }
            "overwatch" | "ow" => self.toggle_overwatch(id as usize),
            "build" if self.features.outposts => match Outpost::from_word(&arg) {
                Some(o) => self.start_build(id as usize, o),
                None => self.warn(id, "Usage: /build defence|yard|sensor (while orbiting a planet you own)"),
            },
            "fix" if self.features.subsystems => {
                let p = &mut self.players[id as usize];
                if arg.is_empty() {
                    p.fix_first = None;
                    self.reply(id, "Damage control: repairing all systems evenly.");
                } else if let Some(sy) = System::from_word(&arg) {
                    let health = p.systems[sy as usize].round();
                    if health >= 100.0 {
                        self.reply(id, format!("Damage control: nothing to fix on the {}.", sy.name().to_lowercase()));
                    } else {
                        p.fix_first = Some(sy);
                        self.reply(id, format!("Damage control: {} first ({}%).", sy.name(), health));
                    }
                } else {
                    self.warn(id, "Usage: /fix warp|impulse|phasers|torpedoes|shields|transporters|cloak|tractor");
                }
            }
            "tech" => {
                let p = &self.players[id as usize];
                let text = if p.techs.is_empty() {
                    format!("No advanced tech: it starts at the rank of {} (with ranks on).", RANKS[TECH_RANK as usize].0)
                } else {
                    let v: Vec<String> = p
                        .techs
                        .iter()
                        .map(|t| format!("{}{}: {}", t.name(), t.key().map_or(String::new(), |k| format!(" [{}]", k)), t.blurb()))
                        .collect();
                    v.join(" • ")
                };
                self.reply(id, text);
            }
            "record" | "orders" | "order" => self.commands.push((id, verb)),
            "help" | "" => {
                let mut cmds = vec!["/overwatch", "/tech", "/record", "/orders"];
                if d {
                    cmds.extend(["/treaty <empire>", "/break", "/treaties"]);
                }
                if self.features.supply {
                    cmds.extend(["/supplies", "/upgrade <name>"]);
                }
                if self.features.subsystems {
                    cmds.push("/fix <system>");
                }
                if self.features.outposts {
                    cmds.push("/build defence|yard|sensor");
                }
                self.reply(id, format!("Commands: {}", cmds.join("  ")));
            }
            _ => self.warn(id, "Unknown command. Try /help"),
        }
    }

    /// A private system message to one player.
    pub fn reply(&mut self, id: u8, text: impl Into<String>) {
        self.outbox.push(Outgoing {
            dest: Dest::Player(id),
            msg: ChatMsg { kind: MsgKind::System, from: "COMMAND".into(), text: text.into() },
        });
    }

    fn ally_of(&self, t: Team) -> Option<Team> {
        self.treaties.iter().find_map(|&(a, b)| if a == t { Some(b) } else if b == t { Some(a) } else { None })
    }

    /// Whether an empire has human players in the game.
    fn has_humans(&self, t: Team) -> bool {
        self.players.iter().any(|p| p.in_use && !p.robot && p.team == t && p.state != PState::Outfit)
    }

    /// `from` offers `to` a treaty. A pending offer the other way is
    /// accepted. Empires run by robots decide on the spot.
    pub fn propose(&mut self, from: Team, to: Team, who: Option<u8>) {
        let say = |w: &mut World, text: String| match who {
            Some(id) => w.warn(id, text),
            None => {}
        };
        if from == to || to == Team::Ind || from == Team::Ind {
            return say(self, "Choose another empire".into());
        }
        if self.team_planet_count(to) == 0 {
            return say(self, format!("The {} are no longer in the game", to.plural()));
        }
        if self.allied(from, to) {
            return say(self, format!("You are already allied with the {}", to.plural()));
        }
        if self.ally_of(from).is_some() || self.ally_of(to).is_some() {
            return say(self, "An empire may only have one ally at a time".into());
        }
        let others = Team::PLAYABLE.into_iter().filter(|&t| t != from && t != to && self.team_planet_count(t) > 0).count();
        if others == 0 {
            return say(self, "With no common enemy left, there's nothing to ally against".into());
        }
        let tick = self.tick;
        if self.proposals.iter().any(|&(a, b, until)| a == to && b == from && until > tick) {
            return self.form_treaty(from, to);
        }
        if !self.has_humans(to) {
            // Robot empires join a treaty unless the proposer is running away with the game.
            let leader = Team::PLAYABLE.into_iter().max_by_key(|&t| self.team_planet_count(t));
            let mut rng = rand::thread_rng();
            if leader != Some(from) && rng.gen_bool(0.7) {
                return self.form_treaty(from, to);
            }
            let text = format!("The {} reject the {}' offer of a treaty.", to.plural(), from.plural());
            return self.god(text);
        }
        self.proposals.retain(|&(a, b, _)| !(a == from && b == to));
        self.proposals.push((from, to, tick + 60 * UPS as u32));
        let text = format!(
            "The {} propose a treaty. Any of you can accept within 60 seconds: /treaty {}",
            from.plural(),
            from.abbr().to_ascii_lowercase()
        );
        self.team_msg(to, text);
        self.team_msg(from, format!("Treaty offered to the {}.", to.plural()));
    }

    fn form_treaty(&mut self, a: Team, b: Team) {
        let pair = if a.idx() < b.idx() { (a, b) } else { (b, a) };
        self.treaties.push(pair);
        self.proposals.retain(|&(x, y, _)| !((x == a && y == b) || (x == b && y == a)));
        self.god(format!("The {} and the {} have signed a treaty of alliance!", a.plural(), b.plural()));
    }

    /// Give notice that `breaker` is leaving its treaty (in 10 seconds).
    pub fn break_treaty(&mut self, breaker: Team, who: Option<u8>) {
        let Some(other) = self.ally_of(breaker) else {
            if let Some(id) = who {
                self.warn(id, "You have no treaty to break");
            }
            return;
        };
        if self.breaking.iter().any(|&(a, _, _)| a == breaker) {
            return;
        }
        self.breaking.push((breaker, other, self.tick + 10 * UPS as u32));
        self.god(format!("The {} are breaking their treaty with the {}! Hostilities resume in 10 seconds.", breaker.plural(), other.plural()));
    }

    fn update_treaties(&mut self) {
        let tick = self.tick;
        self.proposals.retain(|&(_, _, until)| until > tick);
        let due: Vec<(Team, Team, u32)> = self.breaking.iter().copied().filter(|&(_, _, at)| at <= tick).collect();
        self.breaking.retain(|&(_, _, at)| at > tick);
        for (a, b, _) in due {
            self.treaties.retain(|&(x, y)| !((x == a && y == b) || (x == b && y == a)));
            self.god(format!("The treaty between the {} and the {} is over.", a.plural(), b.plural()));
        }
        if tick % (10 * UPS as u32) != 0 {
            return;
        }
        // Now and then a robot-run empire looks for an ally against the leader.
        if tick % (60 * UPS as u32) == 0 {
            let mut rng = rand::thread_rng();
            let live: Vec<Team> = Team::PLAYABLE.into_iter().filter(|&t| self.team_planet_count(t) > 0).collect();
            let leader = live.iter().copied().max_by_key(|&t| self.team_planet_count(t));
            let free: Vec<Team> = live.iter().copied().filter(|&t| Some(t) != leader && self.ally_of(t).is_none()).collect();
            let askers: Vec<Team> = free.iter().copied().filter(|&t| !self.has_humans(t)).collect();
            if live.len() >= 3 && rng.gen_bool(0.5) {
                if let Some(&from) = askers.choose(&mut rng) {
                    let partners: Vec<Team> = free.iter().copied().filter(|&t| t != from).collect();
                    if let Some(&to) = partners.choose(&mut rng) {
                        self.propose(from, to, None);
                    }
                }
            }
        }
        for (a, b) in self.treaties.clone() {
            // With no common enemy left, the alliance lapses (so someone can win).
            let others = Team::PLAYABLE.into_iter().filter(|&t| t != a && t != b && self.team_planet_count(t) > 0).count();
            if others == 0 {
                self.treaties.retain(|&p| p != (a, b));
                self.god(format!("With no common enemy left, the {}-{} alliance dissolves.", a.abbr(), b.abbr()));
                continue;
            }
            // Robot empires turn on an ally that grows far stronger than they are.
            for (me, them) in [(a, b), (b, a)] {
                if !self.has_humans(me) && self.team_planet_count(them) >= self.team_planet_count(me) + 8 {
                    self.break_treaty(me, None);
                    break;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // supplies

    /// Spend supplies on the next level of an upgrade (by name).
    pub fn buy_upgrade(&mut self, team: Team, name: &str, who: Option<u8>) {
        let Some(u) = UPGRADES.iter().position(|(n, _)| n.starts_with(name) && !name.is_empty()) else {
            if let Some(id) = who {
                let names: Vec<&str> = UPGRADES.iter().map(|(n, _)| *n).collect();
                self.warn(id, format!("Upgrades: {}", names.join(", ")));
            }
            return;
        };
        let s = &mut self.supply[team.idx()];
        let level = s.levels[u];
        if level >= MAX_UPGRADE {
            if let Some(id) = who {
                self.warn(id, format!("{} are already fully upgraded", UPGRADES[u].0));
            }
            return;
        }
        let cost = upgrade_cost(level);
        if s.stock < cost {
            if let Some(id) = who {
                let have = s.stock;
                self.warn(id, format!("{} level {} costs {} supplies; you have {}", UPGRADES[u].0, level + 1, cost, have));
            }
            return;
        }
        s.stock -= cost;
        s.levels[u] += 1;
        let text = format!("Upgrade: {} level {} ({}).", UPGRADES[u].0, level + 1, UPGRADES[u].1);
        self.team_msg(team, text);
    }

    /// Spilled armies go to the first empire ship to fly over them.
    fn update_loot(&mut self) {
        for n in 0..self.loot.len() {
            let (x, y) = (self.loot[n].x, self.loot[n].y);
            let taker = (0..MAXPLAYER).find(|&j| {
                let p = &self.players[j];
                p.alive()
                    && p.faction.is_none()
                    && p.armies < p.stats().max_armies
                    && (p.x - x).powi(2) + (p.y - y).powi(2) < LOOT_REACH * LOOT_REACH
            });
            if let Some(j) = taker {
                let p = &mut self.players[j];
                let n_taken = self.loot[n].armies.min(p.stats().max_armies - p.armies);
                p.armies += n_taken;
                self.loot[n].armies -= n_taken;
                let who = p.label();
                let what = if n_taken == 1 { "army" } else { "armies" };
                self.god(format!("{} recovers {} {} from the Ferengi wreckage", who, n_taken, what));
            }
            self.loot[n].ttl -= 1;
        }
        self.loot.retain(|l| l.armies > 0 && l.ttl > 0);
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
            // A defence outpost fires further and harder.
            let def = if self.planets[k].outpost.map_or(false, |o| o.0 == Outpost::Defence) { 1.5 } else { 1.0 };
            for i in 0..MAXPLAYER {
                let p = &self.players[i];
                if !p.alive() || !self.hostile(p.team, owner) {
                    continue;
                }
                if (p.x - px).powi(2) + (p.y - py).powi(2) > (PFIREDIST * def).powi(2) {
                    continue;
                }
                let dmg = (armies / 10 + 2) as f64 * def;
                self.inflict(i, dmg, None, format!("killed by {} ({})", name, owner.letter()));
            }
        }
    }

    fn planet_growth(&mut self) {
        let mut rng = rand::thread_rng();
        let tick = self.tick;
        for pl in self.planets.iter_mut() {
            // Independent worlds don't grow (the Kzinti's are grown by their own rules).
            if pl.owner == Team::Ind || pl.armies >= 60 || tick < pl.silenced_until || pl.tribbles {
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
            let ally = self.treaties.iter().find_map(|&(a, b)| if a == p.team { Some(b) } else if b == p.team { Some(a) } else { None });
            for pl in self.planets.iter_mut() {
                if (pl.x - p.x).abs() < 6000.0 && (pl.y - p.y).abs() < 6000.0 {
                    pl.known[p.team.idx()] = true;
                    if let Some(a) = ally {
                        pl.known[a.idx()] = true;
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // views

    pub fn frame_for(&self, me: u8) -> Frame {
        self.frame_view(me, false)
    }

    /// An observer's view: the whole galaxy with nothing hidden or
    /// disguised, centred on the ship being followed (if any).
    pub fn frame_for_observer(&self, follow: Option<u8>) -> Frame {
        let follow = follow.filter(|&f| (f as usize) < MAXPLAYER && self.players[f as usize].in_use);
        let mut f = self.frame_view(follow.unwrap_or(0), true);
        if follow.is_none() {
            f.me = u8::MAX;
            f.me_info = SelfInfo::default();
        }
        f
    }

    fn frame_view(&self, me: u8, omniscient: bool) -> Frame {
        let mp = &self.players[me as usize];
        let my_team = mp.team;
        let mut rng = rand::thread_rng();
        let players = self
            .players
            .iter()
            .filter(|p| p.in_use)
            .map(|p| {
                let friendly = omniscient || p.team == my_team || p.id == me || self.allied(p.team, my_team);
                // Terrain: nebulae and ion storms hide ships from all but close range.
                let far = (p.x - mp.x).powi(2) + (p.y - mp.y).powi(2) > 3000.0 * 3000.0;
                // A tachyon sweep by us (or an ally) shows everything near the sweeper.
                let swept = self.players.iter().any(|s| {
                    s.alive()
                        && (s.team == my_team || self.allied(s.team, my_team))
                        && (s.scan_until > self.tick
                            || (s.sweep_until > self.tick
                                && (s.x - p.x).powi(2) + (s.y - p.y).powi(2) < SWEEP_RANGE * SWEEP_RANGE))
                });
                // Our sensor arrays see through cloaks and nebulae nearby.
                let arrayed = self.planets.iter().any(|pl| {
                    matches!(pl.outpost, Some((Outpost::Sensor, t)) if t == pl.owner && (t == my_team || self.allied(t, my_team)))
                        && (pl.x - p.x).powi(2) + (pl.y - p.y).powi(2) < SENSOR_ARRAY_RANGE * SENSOR_ARRAY_RANGE
                });
                let fuzzy = ((p.cloaked && !p.detected) || (p.hidden && far)) && !friendly && !swept && !arrayed && !omniscient;
                let illusion = p.ship == ShipType::TalosianShip
                    && !friendly
                    && !omniscient
                    && p.alive()
                    && (p.x - mp.x).powi(2) + (p.y - mp.y).powi(2) > 2000.0f64.powi(2);
                let (x, y) = if fuzzy {
                    (p.x + rng.gen_range(-4000.0..4000.0), p.y + rng.gen_range(-4000.0..4000.0))
                } else if illusion {
                    // A Talosian illusion: to distant enemies it appears 1,500 to
                    // 2,500 from where it really is, drifting every few seconds.
                    let phase = self.tick as f64 / (4.0 * UPS as f64) + p.id as f64 * 1.7;
                    let a = phase * 1.3 + (phase * 0.37).sin() * 3.0;
                    let r = 1500.0 + 1000.0 * (phase * 0.71).sin().abs();
                    (p.x + a.cos() * r, p.y + a.sin() * r)
                } else {
                    (p.x, p.y)
                };
                let mut flags = 0u32;
                let set = |flags: &mut u32, cond: bool, f: u32| {
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
                set(&mut flags, p.marked, pf::HUNTED);
                set(&mut flags, p.tribbles, pf::TRIBBLES);
                set(&mut flags, p.hidden && p.id == me, pf::HIDDEN);
                set(&mut flags, p.overwatch && friendly, pf::OVERWATCH);
                set(&mut flags, p.jump_at.is_some(), pf::CHARGING);
                set(&mut flags, self.tick < p.phased_until, pf::PHASED);
                set(&mut flags, p.trapped, pf::TRAPPED);
                set(&mut flags, p.trapped && !p.zapped && friendly, pf::ZAPPER);
                set(&mut flags, p.nanites, pf::NANITES);
                set(&mut flags, self.boardings.iter().any(|b| b.attacker == p.id), pf::BOARDING);
                set(&mut flags, self.boardings.iter().any(|b| b.target == p.id), pf::BOARDED);
                set(&mut flags, p.towing, pf::TOWING);
                // An Excalbian shapeshifter looks like one of your own cruisers from afar.
                let disguised = p.ship == ShipType::ExcalbianShip
                    && !friendly
                    && p.alive()
                    && (p.x - mp.x).powi(2) + (p.y - mp.y).powi(2) > 3000.0f64.powi(2);
                // A Changeling looks like one of your own ships until a hit exposes it.
                let changeling = p.ship == ShipType::ChangelingShip && p.alive() && self.tick >= p.revealed_until && my_team != Team::Ind && !omniscient;
                let disguised = disguised || changeling;
                PlayerInfo {
                    id: p.id,
                    name: if changeling { CHANGELING_GUISES[p.id as usize % CHANGELING_GUISES.len()].to_string() } else { p.name.clone() },
                    team: if disguised { my_team } else { p.team },
                    ship: if disguised { ShipType::Cruiser } else { p.ship },
                    state: p.state,
                    x: x as i32,
                    y: y as i32,
                    dir: p.dir as u8,
                    speed: p.speed as u8,
                    flags,
                    kills: p.kills as f32,
                    // A freighter's "armies" are the supplies in its hold.
                    armies: match (friendly, p.ship) {
                        (false, _) => 0,
                        (true, ShipType::Freighter) => p.cargo.min(255) as u8,
                        (true, _) => p.armies as u8,
                    },
                    tractor_target: if fuzzy { None } else { p.tractor.map(|t| t.0) },
                    fuzzy,
                    explode_frame: if p.state == PState::Exploding { (11 - p.state_timer).max(1) as u8 } else { 0 },
                    faction: if changeling { None } else { p.faction },
                    rank: p.rank,
                }
            })
            .collect();
        let torps = self
            .torps
            .iter()
            .map(|t| TorpInfo { owner: t.owner, team: t.team, kind: t.kind, x: t.x as i32, y: t.y as i32, explode: t.explode, quantum: t.quantum })
            .collect();
        let phasers = self.phasers.iter().map(|p| p.info.clone()).collect();
        let planets = self
            .planets
            .iter()
            .map(|pl| {
                let known = omniscient || (my_team != Team::Ind && pl.known[my_team.idx()]);
                PlanetInfo {
                    owner: if known { pl.owner } else { Team::Ind },
                    armies: if known { pl.armies as u16 } else { 0 },
                    flags: if known { pl.flags } else { 0 },
                    known,
                    alien: if known { pl.alien } else { None },
                    tribbles: known && pl.tribbles,
                    outpost: if known { pl.outpost.map(|o| o.0) } else { None },
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
            order: mp.order.clone(),
            supply: (self.features.supply && my_team != Team::Ind)
                .then(|| (self.supply[my_team.idx()].stock, self.supply[my_team.idx()].levels)),
            service: mp.service.clone(),
            techs: mp
                .techs
                .iter()
                .map(|&t| {
                    let left = t.slot().map_or(0, |s| mp.tech_ready[s].saturating_sub(self.tick) / UPS as u32);
                    (t, left.min(u16::MAX as u32) as u16)
                })
                .collect(),
            armor: mp.armor.ceil() as u16,
            systems: self.features.subsystems.then(|| mp.systems.map(|h| h.ceil() as u8)),
            fix_first: mp.fix_first,
            building: self
                .builds
                .iter()
                .find(|b| b.builder == mp.id)
                .map(|b| (b.kind, b.planet as u8, (b.done_at.saturating_sub(self.tick) / UPS as u32).min(u16::MAX as u32) as u16)),
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
            loot: self.loot.iter().map(|l| LootInfo { x: l.x as i32, y: l.y as i32, armies: l.armies.min(255) as u8 }).collect(),
            terrain: self.terrain.iter().filter(|t| t.visible()).map(|t| t.info()).collect(),
            treaties: self.treaties.clone(),
            leaders: self.leaders.clone(),
            tempest: self.tempest.as_ref().map(|t| TempestInfo {
                x: t.x as i32,
                y: t.y as i32,
                r_in: t.r_in as i32,
                r_out: t.r_out as i32,
                shape: t.shape,
                lanes: t.lanes,
                exposed: t.exposed,
                level: t.level,
            }),
            zones: self.zones.clone(),
            ring: self.ring.as_ref().map(|r| RingInfo {
                x: r.x as i32,
                y: r.y as i32,
                r: r.r as i32,
                sections: r.sections.iter().map(|s| RingSectionInfo { name: s.name.to_string(), x: s.x as i32, y: s.y as i32 }).collect(),
                kzin: r.kzin as u8,
            }),
            observers: Vec::new(),
            prizes: self
                .prizes
                .iter()
                .map(|z| PrizeInfo { x: z.x as i32, y: z.y as i32, dir: z.dir as u8, ship: z.ship, team: z.from, captor: z.captor })
                .collect(),
            open_teams: self.open_teams(),
            team_planets: [Team::Fed, Team::Rom, Team::Kli, Team::Ori].map(|t| self.team_planet_count(t) as u8),
            starbase_teams: Team::PLAYABLE.into_iter().filter(|&t| self.has_starbase(t, me)).collect(),
            banner: self.banner.clone(),
        }
    }
}

/// "fed", "federation", "f" ... to a team.
fn parse_team_word(s: &str) -> Option<Team> {
    Team::PLAYABLE
        .into_iter()
        .find(|t| !s.is_empty() && (t.abbr().eq_ignore_ascii_case(s) || t.plural().to_ascii_lowercase().starts_with(s) || t.name().to_ascii_lowercase().starts_with(s)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::TerrainKind;

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

    /// Put ship `i` in orbit where it is (a spare Federation planet is moved
    /// in underneath it), as overwatch needs.
    fn park_in_orbit(w: &mut World, i: usize) {
        let k = 5;
        (w.planets[k].x, w.planets[k].y) = (w.players[i].x, w.players[i].y - ORBDIST);
        w.planets[k].owner = w.players[i].team;
        w.players[i].orbiting = Some(k);
        (w.players[i].speed, w.players[i].desired_speed) = (0, 0);
        // Orbiting ships sit a quarter turn from their heading: keep it where it is.
        (w.players[i].dir, w.players[i].desired_dir) = (190.0, 190.0);
    }

    fn pilot(w: &mut World, name: &str, team: Team, x: f64, y: f64) -> u8 {
        let id = w.add_player(name, false).unwrap();
        w.join(id, team, ShipType::Cruiser).unwrap();
        (w.players[id as usize].x, w.players[id as usize].y) = (x, y);
        id
    }

    /// Two human empires sign a treaty, can't hurt each other, and the
    /// break takes ten seconds' notice.
    #[test]
    fn treaties_between_players() {
        let mut w = World::new();
        w.features.diplomacy = true;
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0);
        let tal = pilot(&mut w, "Tal", Team::Rom, 52_000.0, 50_000.0);
        w.handle(kirk, ClientMsg::Message { to: MsgTarget::All, text: "/treaty rom".into() });
        assert!(w.treaties.is_empty(), "needs the Romulans to agree");
        w.handle(tal, ClientMsg::Message { to: MsgTarget::All, text: "/treaty fed".into() });
        assert!(w.allied(Team::Fed, Team::Rom));
        // Phasers, torpedoes and planets leave allies alone.
        let dir = dir_to(50_000.0, 50_000.0, 52_000.0, 50_000.0);
        w.handle(kirk, ClientMsg::Phaser(dir as u8));
        w.handle(kirk, ClientMsg::Torp(dir as u8));
        for _ in 0..30 {
            w.tick();
        }
        assert_eq!(w.players[tal as usize].damage, 0.0);
        assert_eq!(w.players[tal as usize].shield, ShipType::Cruiser.stats().max_shield);
        // Breaking it: still allied for ten seconds, then at war.
        w.handle(tal, ClientMsg::Message { to: MsgTarget::All, text: "/break".into() });
        assert!(w.allied(Team::Fed, Team::Rom));
        for _ in 0..(10 * UPS as u32 + 1) {
            w.tick();
        }
        assert!(!w.allied(Team::Fed, Team::Rom));
    }

    #[test]
    fn one_ally_at_a_time_and_robots_decide() {
        let mut w = World::new();
        w.features.diplomacy = true;
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0);
        // The Klingons have no players: they answer at once.
        for _ in 0..20 {
            if w.allied(Team::Fed, Team::Kli) {
                break;
            }
            w.handle(kirk, ClientMsg::Message { to: MsgTarget::All, text: "/treaty kli".into() });
        }
        assert!(w.allied(Team::Fed, Team::Kli), "robots accept most offers");
        w.handle(kirk, ClientMsg::Message { to: MsgTarget::All, text: "/treaty ori".into() });
        assert!(!w.allied(Team::Fed, Team::Ori), "only one ally");
    }

    /// Supply upgrades change the numbers.
    #[test]
    fn upgrades_boost_torpedoes() {
        let mut w = World::new();
        w.features.supply = true;
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0);
        w.supply[Team::Fed.idx()].stock = 10;
        w.handle(kirk, ClientMsg::Message { to: MsgTarget::All, text: "/upgrade torps".into() });
        assert_eq!(w.supply[Team::Fed.idx()].levels[UPGRADE_TORPS], 1);
        assert_eq!(w.supply[Team::Fed.idx()].stock, 0);
        w.handle(kirk, ClientMsg::Torp(0));
        let base = ShipType::Cruiser.stats().torp_damage;
        assert!((w.torps[0].damage - base * 1.1).abs() < 1e-9);
    }

    /// With ranks on, starbases need a Commander.
    #[test]
    fn starbase_needs_rank() {
        let mut w = World::new();
        w.features.ranks = true;
        let id = w.add_player("Cadet", false).unwrap();
        w.players[id as usize].rank = Some(1);
        assert!(w.join(id, Team::Fed, ShipType::Starbase).is_err());
        w.players[id as usize].rank = Some(STARBASE_RANK);
        assert!(w.join(id, Team::Fed, ShipType::Starbase).is_ok());
    }

    /// Knocked-out systems stop what they do until damage control gets them
    /// back, and /fix puts one first.
    #[test]
    fn subsystems_knocked_out_and_repaired() {
        let mut w = World::with_features(Features { subsystems: true, ..Features::default() });
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0);
        let tal = pilot(&mut w, "Tal", Team::Rom, 52_000.0, 50_000.0);
        let k = kirk as usize;
        let full = w.players[k].max_speed_now();
        // Warp out: impulse only. Impulse out: sluggish, but still moving.
        w.damage_system(k, System::Warp, 100.0);
        assert_eq!(w.players[k].max_speed_now(), 3.min(full));
        // Weapons out.
        w.damage_system(k, System::Phasers, 100.0);
        w.damage_system(k, System::Torpedoes, 100.0);
        let dir = dir_to(50_000.0, 50_000.0, 52_000.0, 50_000.0) as u8;
        w.handle(kirk, ClientMsg::Phaser(dir));
        w.handle(kirk, ClientMsg::Torp(dir));
        assert!(w.phasers.is_empty() && w.torps.is_empty(), "nothing fires");
        // Shields drop and won't come back up; no cloak, no tractor.
        assert!(w.players[k].shields_up);
        w.damage_system(k, System::Shields, 100.0);
        assert!(!w.players[k].shields_up);
        w.handle(kirk, ClientMsg::Shields);
        assert!(!w.players[k].shields_up);
        w.damage_system(k, System::Cloak, 100.0);
        w.handle(kirk, ClientMsg::Cloak);
        assert!(!w.players[k].cloaked);
        w.damage_system(k, System::Tractor, 100.0);
        w.handle(kirk, ClientMsg::Tractor { target: Some(tal), pressor: false });
        assert!(w.players[k].tractor.is_none());
        // The frame reports it all.
        let f = w.frame_for(kirk);
        let sys = f.me_info.systems.unwrap();
        assert_eq!(sys[System::Phasers as usize], 0);
        assert_eq!(sys[System::Impulse as usize], 100);
        // Damage control, phasers first.
        w.handle(kirk, ClientMsg::Message { to: MsgTarget::All, text: "/fix phasers".into() });
        assert_eq!(w.players[k].fix_first, Some(System::Phasers));
        w.players[k].repair_mode = true;
        for _ in 0..200 {
            w.players[k].repair_mode = true;
            w.tick();
        }
        let (pha, tor) = (w.players[k].systems[System::Phasers as usize], w.players[k].systems[System::Torpedoes as usize]);
        assert!(pha == 100.0 && tor < 100.0, "phasers first: {} vs {}", pha, tor);
        assert!(!w.players[k].sys_out(System::Phasers), "phasers back online");
        // A new ship starts with everything working.
        w.kill(k, None, "test".into());
        for _ in 0..20 {
            w.tick();
        }
        w.players[k].state = PState::Outfit;
        w.join(kirk, Team::Fed, ShipType::Cruiser).unwrap();
        assert!(w.players[k].systems.iter().all(|&h| h == 100.0));
    }

    /// Hull hits knock out systems only with --subsystems on.
    #[test]
    fn hull_hits_damage_systems() {
        for on in [true, false] {
            let mut w = World::with_features(Features { subsystems: on, ..Features::default() });
            let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0) as usize;
            w.players[kirk].shields_up = false;
            for _ in 0..40 {
                w.inflict(kirk, 10.0, None, "test".into());
                w.players[kirk].damage = 0.0;
            }
            let hurt = w.players[kirk].systems.iter().any(|&h| h < 100.0);
            assert_eq!(hurt, on, "subsystems {}", on);
            assert_eq!(w.frame_for(kirk as u8).me_info.systems.is_some(), on);
        }
    }

    /// A boarding party takes a shieldless ship; the prize is towed home.
    #[test]
    fn boarding_party_captures_a_ship_and_tows_it_home() {
        let mut w = World::with_features(Features { boarding: true, ..Features::default() });
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0);
        let tal = pilot(&mut w, "Tal", Team::Rom, 51_000.0, 50_000.0);
        let (k, t) = (kirk as usize, tal as usize);
        w.players[k].armies = 12;
        // Shields up: no way aboard.
        w.handle(kirk, ClientMsg::Board(tal));
        assert!(w.boardings.is_empty(), "their shields are up");
        w.players[t].shields_up = false;
        w.handle(kirk, ClientMsg::Board(tal));
        assert_eq!(w.boardings.len(), 1);
        assert!(w.frame_for(tal).players.iter().find(|p| p.id == tal).unwrap().flags & pf::BOARDED != 0);
        let kills = w.players[k].kills;
        for _ in 0..600 {
            (w.players[k].x, w.players[k].y) = (50_000.0, 50_000.0);
            // Beaten back? Send another party while there are armies left.
            if w.boardings.is_empty() && w.players[t].alive() && w.players[k].armies > 0 {
                w.handle(kirk, ClientMsg::Board(tal));
            }
            w.tick();
            if !w.players[t].alive() {
                break;
            }
        }
        assert!(!w.players[t].alive(), "captured");
        assert!(w.players[k].alive(), "no explosion hurt the boarding ship");
        assert!(w.players[k].kills >= kills + 2.0, "kill plus the capture: {}", w.players[k].kills);
        assert_eq!(w.prizes.len(), 1);
        assert!(w.players[k].towing && w.players[k].max_speed_now() <= 6);
        // Tow it home to Earth (a repair world).
        let earth = Team::Fed.home_planet();
        let armies = w.planets[earth].armies;
        (w.players[k].x, w.players[k].y) = (w.planets[earth].x, w.planets[earth].y + ORBDIST);
        w.players[k].orbiting = Some(earth);
        w.tick();
        assert!(w.prizes.is_empty(), "delivered");
        assert!(!w.players[k].towing);
        assert!(w.planets[earth].armies >= armies + ShipType::Cruiser.crew(), "the prize crew joins the garrison");
    }

    /// Detonating fights boarders off; the Borg assimilate them; and with
    /// the option off there's no boarding at all.
    #[test]
    fn boarders_can_be_repelled() {
        let mut w = World::with_features(Features { boarding: true, ..Features::default() });
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0);
        let tal = pilot(&mut w, "Tal", Team::Rom, 51_000.0, 50_000.0);
        let (k, t) = (kirk as usize, tal as usize);
        w.players[k].armies = 2;
        w.players[t].shields_up = false;
        w.handle(kirk, ClientMsg::Board(tal));
        assert_eq!(w.boardings.len(), 1);
        w.handle(tal, ClientMsg::DetEnemy);
        w.players[t].fuel = 5000.0;
        w.handle(tal, ClientMsg::DetEnemy);
        w.tick();
        assert!(w.boardings.is_empty(), "fought off");
        assert!(w.players[t].alive());
        // The Borg.
        let b = w.spawn_alien("Borg", Faction::Borg, ShipType::BorgCube, 51_000.0, 49_000.0, 30.0).unwrap();
        w.players[b as usize].shields_up = false;
        w.players[k].armies = 5;
        w.handle(kirk, ClientMsg::Board(b));
        assert!(w.boardings.is_empty() && w.players[k].armies == 0, "assimilated");
        // Off.
        let mut w = World::new();
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0);
        let tal = pilot(&mut w, "Tal", Team::Rom, 51_000.0, 50_000.0);
        w.players[kirk as usize].armies = 3;
        w.players[tal as usize].shields_up = false;
        w.handle(kirk, ClientMsg::Board(tal));
        assert!(w.boardings.is_empty());
    }

    /// Put ship `i` in orbit of planet `k`.
    fn orbit_at(w: &mut World, i: usize, k: usize) {
        (w.players[i].x, w.players[i].y) = (w.planets[k].x, w.planets[k].y + ORBDIST);
        w.players[i].orbiting = Some(k);
        (w.players[i].speed, w.players[i].desired_speed) = (0, 0);
    }

    fn build(w: &mut World, id: u8, what: &str) {
        w.handle(id, ClientMsg::Message { to: MsgTarget::All, text: format!("/build {}", what) });
        for _ in 0..(BUILD_SECS * UPS as u32 + 2) {
            w.tick();
        }
    }

    /// Outposts: built in orbit, paid for in armies (or supplies), and each
    /// does its job: a shipyard to launch and refit from, a defence outpost
    /// that shoots further, a sensor array that sees cloaked ships.
    #[test]
    fn outposts_are_built_and_do_their_jobs() {
        let mut w = World::with_features(Features { outposts: true, ..Features::default() });
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 0.0, 0.0);
        let k = kirk as usize;
        let colony = Team::Fed.home_planet() + 4;
        w.planets[colony].owner = Team::Fed;
        orbit_at(&mut w, k, colony);
        // Builders needed.
        build(&mut w, kirk, "yard");
        assert!(w.planets[colony].outpost.is_none(), "no armies aboard, no outpost");
        w.players[k].armies = 5;
        build(&mut w, kirk, "yard");
        assert_eq!(w.planets[colony].outpost, Some((Outpost::Shipyard, Team::Fed)));
        assert_eq!(w.players[k].armies, 5 - BUILD_ARMIES);
        // Lost near the yard: relaunch there, not at home.
        (w.players[k].x, w.players[k].y) = (w.planets[colony].x + 3000.0, w.planets[colony].y);
        w.players[k].orbiting = None;
        w.kill(k, None, "test".into());
        for _ in 0..20 {
            w.tick();
        }
        w.players[k].state = PState::Outfit;
        w.join(kirk, Team::Fed, ShipType::Cruiser).unwrap();
        let (px, py) = (w.planets[colony].x, w.planets[colony].y);
        assert!(((w.players[k].x - px).powi(2) + (w.players[k].y - py).powi(2)).sqrt() < 2500.0, "launched from the shipyard");
        // Refit there too.
        orbit_at(&mut w, k, colony);
        w.handle(kirk, ClientMsg::Refit(ShipType::Destroyer));
        assert_eq!(w.players[k].ship, ShipType::Destroyer, "refit at the shipyard");
        // A defence outpost reaches further.
        w.players[k].armies = 3;
        build(&mut w, kirk, "defence");
        assert_eq!(w.planets[colony].outpost, Some((Outpost::Defence, Team::Fed)), "replaces the yard");
        w.planets[colony].armies = 20;
        let tal = pilot(&mut w, "Tal", Team::Rom, px + PFIREDIST * 1.3, py);
        w.players[tal as usize].shields_up = false;
        for _ in 0..20 {
            (w.players[tal as usize].x, w.players[tal as usize].y) = (px + PFIREDIST * 1.3, py);
            w.tick();
        }
        assert!(w.players[tal as usize].damage > 0.0, "fired on from beyond normal range");
        // A sensor array sees cloaked ships near it, even from far away.
        w.players[k].armies = 3;
        build(&mut w, kirk, "sensor");
        w.players[tal as usize].cloaked = true;
        w.players[k].orbiting = None;
        (w.players[k].x, w.players[k].y) = (px + 40_000.0, py);
        let f = w.frame_for(kirk);
        let t = f.players.iter().find(|p| p.id == tal).unwrap();
        assert!(!t.fuzzy, "the array sees the cloaked warbird");
        assert_eq!(f.planets[colony].outpost, Some(Outpost::Sensor));
        // Lose the planet and the outpost goes with it.
        w.planets[colony].owner = Team::Rom;
        w.tick();
        assert!(w.planets[colony].outpost.is_none());
    }

    /// Leaving orbit abandons a build; with --supply it costs supplies.
    #[test]
    fn outpost_builds_need_the_builder_in_orbit() {
        let mut w = World::with_features(Features { outposts: true, supply: true, ..Features::default() });
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 0.0, 0.0);
        let k = kirk as usize;
        let colony = Team::Fed.home_planet() + 4;
        w.planets[colony].owner = Team::Fed;
        orbit_at(&mut w, k, colony);
        w.supply[Team::Fed.idx()].stock = 20;
        w.handle(kirk, ClientMsg::Message { to: MsgTarget::All, text: "/build sensor".into() });
        assert_eq!(w.builds.len(), 1);
        w.handle(kirk, ClientMsg::Course(0));
        w.tick();
        assert!(w.builds.is_empty(), "abandoned");
        orbit_at(&mut w, k, colony);
        build(&mut w, kirk, "sensor");
        assert_eq!(w.planets[colony].outpost, Some((Outpost::Sensor, Team::Fed)));
        assert_eq!(w.supply[Team::Fed.idx()].stock, 20 - BUILD_SUPPLIES);
    }

    /// An observer's view hides nothing: cloaked ships, disguises, unscouted
    /// planets and armies aboard are all shown as they are.
    #[test]
    fn observers_see_everything() {
        let mut w = World::new();
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0);
        let tal = pilot(&mut w, "Tal", Team::Rom, 80_000.0, 50_000.0);
        w.players[tal as usize].cloaked = true;
        w.players[tal as usize].armies = 3;
        let c = w.spawn_alien("Changeling", Faction::Changeling, ShipType::ChangelingShip, 60_000.0, 50_000.0, 8.0).unwrap();
        // A player sees a blur, a friendly cruiser, and planets they haven't scouted.
        let pf = w.frame_for(kirk);
        assert!(pf.players.iter().find(|p| p.id == tal).unwrap().fuzzy);
        assert_eq!(pf.players.iter().find(|p| p.id == c).unwrap().faction, None);
        assert!(pf.planets.iter().any(|p| !p.known));
        // An observer sees it all.
        let of = w.frame_for_observer(None);
        assert_eq!(of.me, u8::MAX);
        let t = of.players.iter().find(|p| p.id == tal).unwrap();
        assert!(!t.fuzzy && t.armies == 3 && t.x == 80_000);
        assert_eq!(of.players.iter().find(|p| p.id == c).unwrap().faction, Some(Faction::Changeling));
        assert!(of.planets.iter().all(|p| p.known));
        // Following a ship gives its gauges.
        let of = w.frame_for_observer(Some(kirk));
        assert_eq!(of.me, kirk);
        assert_eq!(of.me_info.fuel, w.players[kirk as usize].fuel as u32);
    }

    /// Overwatch fires at enemies that come into range, and only them.
    #[test]
    fn overwatch_fires_at_enemies_in_range() {
        let mut w = World::new();
        w.features.diplomacy = true;
        let kirk = pilot(&mut w, "Kirk", Team::Fed, 50_000.0, 50_000.0);
        let tal = pilot(&mut w, "Tal", Team::Rom, 80_000.0, 50_000.0);
        // Only in orbit.
        w.handle(kirk, ClientMsg::Overwatch);
        assert!(!w.players[kirk as usize].overwatch, "not in open space");
        park_in_orbit(&mut w, kirk as usize);
        w.handle(kirk, ClientMsg::Overwatch);
        assert!(w.players[kirk as usize].overwatch);
        let run = |w: &mut World| {
            for _ in 0..10 {
                w.tick();
            }
        };
        // Far away: holds fire.
        run(&mut w);
        assert!(w.torps.is_empty() && w.phasers.is_empty());
        // In torpedo range: fires.
        w.players[tal as usize].x = 56_000.0;
        run(&mut w);
        assert!(w.torps.iter().any(|t| t.owner == kirk), "no torpedoes");
        // Close in: phasers.
        w.torps.clear();
        w.players[tal as usize].x = 52_500.0;
        let mut phasered = false;
        for _ in 0..10 {
            w.tick();
            phasered |= w.phasers.iter().any(|p| p.info.owner == kirk);
        }
        assert!(phasered, "no phasers at close range");
        // Allies are left alone.
        w.treaties.push((Team::Fed, Team::Rom));
        w.torps.clear();
        w.phasers.clear();
        w.players[kirk as usize].phaser_timer = 0;
        run(&mut w);
        assert!(!w.torps.iter().any(|t| t.owner == kirk) && !w.phasers.iter().any(|p| p.info.owner == kirk));
        // A low tank holds fire, keeping a reserve.
        w.treaties.clear();
        w.players[kirk as usize].fuel = 100.0;
        w.torps.clear();
        run(&mut w);
        assert!(!w.torps.iter().any(|t| t.owner == kirk));
        // And off again.
        w.handle(kirk, ClientMsg::Message { to: MsgTarget::All, text: "/overwatch".into() });
        assert!(!w.players[kirk as usize].overwatch);
        // Leaving orbit switches it off.
        w.handle(kirk, ClientMsg::Overwatch);
        assert!(w.players[kirk as usize].overwatch);
        w.handle(kirk, ClientMsg::Course(64));
        w.tick();
        assert!(!w.players[kirk as usize].overwatch, "off once out of orbit");
    }

    /// A ranked officer at (x, y) with exactly these techs.
    fn officer(w: &mut World, team: Team, x: f64, y: f64, techs: &[Tech]) -> usize {
        let id = pilot(w, "Officer", team, x, y) as usize;
        w.players[id].techs = techs.to_vec();
        if techs.contains(&Tech::AblativeArmor) {
            w.players[id].armor = ABLATIVE_ARMOR;
        }
        id
    }

    #[test]
    fn tech_comes_with_rank() {
        let mut w = World::new();
        w.features.ranks = true;
        w.features.rank_tech = true;
        for (rank, expect) in [(3u8, 0usize), (4, 1), (6, 3), (8, 5)] {
            let id = w.add_player("Officer", false).unwrap();
            w.players[id as usize].rank = Some(rank);
            w.join(id, Team::Fed, ShipType::Cruiser).unwrap();
            let techs = &w.players[id as usize].techs;
            assert_eq!(techs.len(), expect, "rank {}", rank);
            for (tier, t) in techs.iter().enumerate() {
                assert_eq!(t.tier(), tier, "one from each tier, in order");
            }
            // Senior officers carry a bounty: +0.5 kill credit per rank above Commander.
            assert_eq!(w.players[id as usize].bounty, rank.saturating_sub(3) as f64 * 5.0);
            w.remove_player(id);
        }
        w.features.rank_tech = false;
        let id = w.add_player("Officer", false).unwrap();
        w.players[id as usize].rank = Some(8);
        w.join(id, Team::Fed, ShipType::Cruiser).unwrap();
        assert!(w.players[id as usize].techs.is_empty(), "--no-rank-tech");
    }

    #[test]
    fn captain_weapons() {
        let mut w = World::new();
        let q = officer(&mut w, Team::Fed, 50_000.0, 50_000.0, &[Tech::QuantumTorps]);
        w.handle(q as u8, ClientMsg::Torp(0));
        let s = ShipType::Cruiser.stats();
        assert!((w.torps[0].damage - s.torp_damage * 1.2).abs() < 1e-9 && w.torps[0].quantum);
        let sp = officer(&mut w, Team::Fed, 50_000.0, 60_000.0, &[Tech::PhotonSpread]);
        w.handle(sp as u8, ClientMsg::Torp(0));
        assert_eq!(w.torps.iter().filter(|t| t.owner == sp as u8).count(), 3, "a fan of three");
        // Overcharged phasers reach farther, and some damage gets through shields.
        let oc = officer(&mut w, Team::Fed, 20_000.0, 20_000.0, &[Tech::PhaserOvercharge]);
        let tal = pilot(&mut w, "Tal", Team::Rom, 20_000.0 + PHASEDIST * 1.1, 20_000.0);
        w.handle(oc as u8, ClientMsg::Phaser(64));
        assert!(w.players[tal as usize].damage > 0.0, "reached past normal range and pierced shields");
    }

    #[test]
    fn fleet_captain_defences() {
        let mut w = World::new();
        let a = officer(&mut w, Team::Fed, 50_000.0, 50_000.0, &[Tech::AblativeArmor]);
        w.players[a].shields_up = false;
        w.inflict(a, 30.0, None, "test".into());
        assert_eq!((w.players[a].damage, w.players[a].armor), (0.0, 10.0), "armor soaks it");
        w.inflict(a, 30.0, None, "test".into());
        assert_eq!(w.players[a].damage, 20.0);
        // Regenerative shields: much faster once you've been left alone.
        let r = officer(&mut w, Team::Fed, 60_000.0, 50_000.0, &[Tech::RegenShields]);
        let n = pilot(&mut w, "Plain", Team::Fed, 70_000.0, 50_000.0) as usize;
        w.tick = 1000;
        for id in [r, n] {
            w.players[id].shield = 0.0;
            w.players[id].last_hit = 0;
        }
        w.tick();
        assert!(w.players[r].shield > w.players[n].shield * 2.5);
        // Metaphasic shields shrug off the star's core.
        w.features.terrain = true;
        let m = officer(&mut w, Team::Fed, 30_000.0, 30_000.0, &[Tech::MetaphasicShields]);
        w.terrain.push(super::super::terrain::tests_make(TerrainKind::Star, 30_000.0, 30_000.0, 5000.0));
        w.players[m].shields_up = false;
        super::super::terrain::tick(&mut w);
        assert_eq!(w.players[m].damage, 0.0);
    }

    #[test]
    fn commodore_tricks() {
        let mut w = World::new();
        let c = officer(&mut w, Team::Fed, 50_000.0, 50_000.0, &[Tech::TachyonSweep]);
        let tal = pilot(&mut w, "Tal", Team::Rom, 58_000.0, 50_000.0);
        w.players[tal as usize].cloaked = true;
        let seen = |w: &World| !w.frame_for(c as u8).players.iter().find(|p| p.id == tal).unwrap().fuzzy;
        assert!(!seen(&w));
        w.handle(c as u8, ClientMsg::Tech { slot: 0, dir: 0 });
        assert!(seen(&w), "swept");
        // Cooldown.
        let until = w.players[c].sweep_until;
        w.players[c].sweep_until = 0;
        w.handle(c as u8, ClientMsg::Tech { slot: 0, dir: 0 });
        assert_eq!(w.players[c].sweep_until, 0, "still cooling down");
        assert!(until > 0);
        // Decoy: a copy that vanishes quietly when hit.
        w.players[c].techs = vec![Tech::Decoy];
        w.players[c].tech_ready = [0; 3];
        let before = w.players.iter().filter(|p| p.in_use).count();
        w.handle(c as u8, ClientMsg::Tech { slot: 0, dir: 0 });
        let d = w.players.iter().position(|p| p.decoy_until.is_some()).unwrap();
        assert_eq!(w.players.iter().filter(|p| p.in_use).count(), before + 1);
        w.outbox.clear();
        w.inflict(d, 5.0, Some(tal), "test".into());
        assert!(!w.players[d].in_use && w.outbox.is_empty(), "no kill, no message");
        // Graviton pulse: shove and jam.
        w.players[c].techs = vec![Tech::GravitonPulse];
        w.players[c].tech_ready = [0; 3];
        (w.players[tal as usize].x, w.players[tal as usize].cloaked) = (52_000.0, false);
        w.handle(c as u8, ClientMsg::Tech { slot: 0, dir: 0 });
        assert!(w.players[tal as usize].x > 54_000.0 && !w.players[tal as usize].shields_up);
        w.handle(tal, ClientMsg::Shields);
        assert!(!w.players[tal as usize].shields_up, "jammed");
    }

    #[test]
    fn rear_admiral_weapons() {
        let mut w = World::new();
        let r = officer(&mut w, Team::Fed, 50_000.0, 50_000.0, &[Tech::Isokinetic]);
        let tal = pilot(&mut w, "Tal", Team::Rom, 58_000.0, 50_000.0) as usize;
        w.handle(r as u8, ClientMsg::Tech { slot: 1, dir: 64 });
        assert_eq!(w.players[tal].damage, 120.0, "straight through the shields at 8,000");
        // Antiproton burst hits everyone close.
        let a = officer(&mut w, Team::Fed, 20_000.0, 20_000.0, &[Tech::Antiproton]);
        let x = pilot(&mut w, "X", Team::Rom, 22_000.0, 20_000.0) as usize;
        let y = pilot(&mut w, "Y", Team::Rom, 20_000.0, 23_000.0) as usize;
        w.handle(a as u8, ClientMsg::Tech { slot: 1, dir: 0 });
        assert!(w.players[x].shield < 100.0 && w.players[y].shield < 100.0);
        // Tricobalt: hurts everyone in the blast, its owner too.
        let t = officer(&mut w, Team::Fed, 80_000.0, 80_000.0, &[Tech::Tricobalt]);
        w.players[t].shields_up = false;
        w.handle(t as u8, ClientMsg::Tech { slot: 1, dir: 0 });
        assert!(w.torps.iter().any(|q| q.kind == TorpKind::Tricobalt));
        w.torps.iter_mut().for_each(|q| q.fuse = 1);
        w.tick();
        w.tick();
        assert!(w.players[t].damage > 0.0, "caught in its own blast");
    }

    #[test]
    fn admiral_tech() {
        let mut w = World::new();
        let a = officer(&mut w, Team::Fed, 50_000.0, 50_000.0, &[Tech::Transwarp]);
        (w.players[a].dir, w.players[a].desired_dir) = (64.0, 64.0);
        w.handle(a as u8, ClientMsg::Tech { slot: 2, dir: 0 });
        assert!(w.frame_for(a as u8).players.iter().find(|p| p.id == a as u8).unwrap().flags & pf::CHARGING != 0);
        for _ in 0..(2 * UPS as u32 + 1) {
            w.tick();
        }
        assert!(w.players[a].x > 64_000.0, "jumped 15,000 east");
        // Phase cloak: untouchable and can't shoot.
        let p = officer(&mut w, Team::Fed, 20_000.0, 20_000.0, &[Tech::PhaseCloak]);
        w.handle(p as u8, ClientMsg::Tech { slot: 2, dir: 0 });
        w.inflict(p, 50.0, None, "test".into());
        assert_eq!(w.players[p].damage, 0.0);
        let shield = w.players[p].shield;
        assert_eq!(shield, 100.0);
        w.handle(p as u8, ClientMsg::Torp(0));
        assert!(!w.torps.iter().any(|t| t.owner == p as u8));
        // Emergency reserve.
        let e = officer(&mut w, Team::Fed, 80_000.0, 20_000.0, &[Tech::EmergencyReserve]);
        (w.players[e].fuel, w.players[e].shield, w.players[e].wtemp) = (10.0, 5.0, 900.0);
        w.handle(e as u8, ClientMsg::Tech { slot: 2, dir: 0 });
        let s = ShipType::Cruiser.stats();
        assert_eq!((w.players[e].fuel, w.players[e].shield, w.players[e].wtemp), (s.max_fuel, s.max_shield, 0.0));
    }

    #[test]
    fn admiral_starbase_tech() {
        let mut w = World::new();
        w.features.ranks = true;
        w.features.rank_tech = true;
        // Only an Admiral in a starbase: Captain and Fleet Captain passives,
        // plus one starbase passive and one starbase active.
        let id = w.add_player("Janeway", false).unwrap();
        w.players[id as usize].rank = Some(8);
        w.join(id, Team::Fed, ShipType::Starbase).unwrap();
        let t = w.players[id as usize].techs.clone();
        let tiers: Vec<usize> = t.iter().map(|t| t.tier()).collect();
        assert_eq!(tiers, vec![0, 1, 5, 6], "{:?}", t);
        assert_eq!(t[3].key(), Some('j'));
        // A Rear Admiral's starbase gets none of it.
        let ra = w.add_player("Nechayev", false).unwrap();
        w.players[ra as usize].rank = Some(7);
        w.join(ra, Team::Rom, ShipType::Starbase).unwrap();
        assert!(w.players[ra as usize].techs.iter().all(|t| !t.starbase_only()));
        // Refitting into a starbase refits the tech.
        let cr = w.add_player("Paris", false).unwrap();
        w.players[cr as usize].rank = Some(8);
        w.join(cr, Team::Kli, ShipType::Cruiser).unwrap();
        assert_eq!(w.players[cr as usize].techs.len(), 5);
        w.players[cr as usize].orbiting = Some(Team::Kli.home_planet());
        w.handle(cr, ClientMsg::Refit(ShipType::Starbase));
        assert!(w.players[cr as usize].techs.iter().any(|t| t.starbase_only()));
    }

    #[test]
    fn starbase_tech_works() {
        let mut w = World::new();
        let sb = pilot(&mut w, "Base", Team::Fed, 50_000.0, 50_000.0) as usize;
        w.players[sb].ship = ShipType::Starbase;
        let tal = pilot(&mut w, "Tal", Team::Rom, 54_000.0, 50_000.0) as usize;
        // Point defense: enemy torpedoes near the base are shot down.
        w.players[sb].techs = vec![Tech::PointDefense];
        let dir = dir_to(54_000.0, 50_000.0, 50_000.0, 50_000.0);
        w.handle(tal as u8, ClientMsg::Torp(dir as u8));
        w.players[sb].shields_up = false;
        for _ in 0..30 {
            w.tick();
        }
        assert_eq!(w.players[sb].damage, 0.0, "torpedo shot down");
        // Shield projector and drydock look after friends nearby.
        let friend = pilot(&mut w, "Friend", Team::Fed, 52_000.0, 52_000.0) as usize;
        w.players[sb].techs = vec![Tech::ShieldProjector, Tech::MobileDrydock];
        (w.players[friend].shield, w.players[friend].damage, w.players[friend].fuel) = (10.0, 50.0, 100.0);
        w.players[tal].x = 90_000.0;
        w.tick();
        // (Normal recharge alone gives about +24 fuel a tick; the drydock adds 72.)
        assert!(w.players[friend].shield > 10.2 && w.players[friend].damage < 49.8 && w.players[friend].fuel > 160.0);
        // Tractor net.
        w.players[sb].techs = vec![Tech::TractorNet];
        w.players[tal].x = 54_000.0;
        w.players[tal].desired_speed = 9;
        w.handle(sb as u8, ClientMsg::Tech { slot: 2, dir: 0 });
        w.tick();
        assert!(w.players[tal].speed <= 1 && w.players[tal].desired_speed <= 1);
        // Galactic scan: a cloaked ship across the galaxy shows up.
        w.players[sb].techs = vec![Tech::GalacticScan];
        w.players[sb].tech_ready = [0; 3];
        (w.players[tal].x, w.players[tal].cloaked) = (95_000.0, true);
        w.handle(sb as u8, ClientMsg::Tech { slot: 2, dir: 0 });
        assert!(!w.frame_for(friend as u8).players.iter().find(|p| p.id == tal as u8).unwrap().fuzzy);
        assert!(w.planets.iter().all(|pl| pl.known[Team::Fed.idx()]));
        // Fighter wing: three fighters that go home after 30 seconds.
        w.players[sb].techs = vec![Tech::FighterWing];
        w.players[sb].tech_ready = [0; 3];
        w.handle(sb as u8, ClientMsg::Tech { slot: 2, dir: 0 });
        let fighters = |w: &World| w.players.iter().filter(|p| p.in_use && p.fighter_of.is_some()).count();
        assert_eq!(fighters(&w), 3);
        for _ in 0..(30 * UPS as u32 + 2) {
            w.tick();
        }
        assert_eq!(fighters(&w), 0);
    }

    /// A ranked human launching in `ship`.
    fn flyer(w: &mut World, rank: u8, team: Team, ship: ShipType, x: f64, y: f64) -> usize {
        let id = w.add_player("Flyer", false).unwrap();
        w.players[id as usize].rank = Some(rank);
        w.join(id, team, ship).unwrap();
        (w.players[id as usize].x, w.players[id as usize].y) = (x, y);
        id as usize
    }

    #[test]
    fn special_and_relic_ships_need_rank() {
        let mut w = World::new();
        let id = w.add_player("Cadet", false).unwrap();
        w.players[id as usize].rank = Some(8);
        assert!(w.join(id, Team::Fed, ShipType::Defiant).is_err(), "needs ranks on");
        w.features.ranks = true;
        w.players[id as usize].rank = Some(3);
        assert!(w.join(id, Team::Fed, ShipType::Defiant).is_err(), "needs Captain");
        w.players[id as usize].rank = Some(4);
        // Asking for any special gets your own empire's.
        w.join(id, Team::Kli, ShipType::Defiant).unwrap();
        assert_eq!(w.players[id as usize].ship, ShipType::NeghVar);
        let r = w.add_player("Ensign", false).unwrap();
        w.players[r as usize].rank = Some(5);
        assert!(w.join(r, Team::Rom, ShipType::IconianShip).is_err(), "relics need Commodore");
        w.players[r as usize].rank = Some(6);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            w.players[r as usize].state = PState::Outfit;
            w.join(r, Team::Rom, ShipType::IconianShip).unwrap();
            assert!(w.players[r as usize].ship.is_relic());
            seen.insert(w.players[r as usize].ship);
        }
        assert_eq!(seen.len(), ShipType::RELICS.len(), "all the relics turn up");
    }

    #[test]
    fn special_ship_traits() {
        let mut w = World::new();
        w.features.ranks = true;
        // Defiant: pulse phasers.
        let d = flyer(&mut w, 4, Team::Fed, ShipType::Defiant, 50_000.0, 50_000.0);
        w.handle(d as u8, ClientMsg::Phaser(0));
        assert_eq!(w.players[d].phaser_timer, 5);
        // Warbird: plasma after one kill.
        let wb = flyer(&mut w, 4, Team::Rom, ShipType::Warbird, 10_000.0, 10_000.0);
        w.players[wb].kills = 1.0;
        w.handle(wb as u8, ClientMsg::Plasma(0));
        assert!(w.torps.iter().any(|t| t.owner == wb as u8 && t.kind == TorpKind::Plasma));
        // Negh'Var: twelve torpedoes.
        let nv = flyer(&mut w, 4, Team::Kli, ShipType::NeghVar, 90_000.0, 10_000.0);
        for _ in 0..15 {
            w.handle(nv as u8, ClientMsg::Torp(0));
            w.players[nv].wtemp = 0.0;
        }
        assert_eq!(w.torps.iter().filter(|t| t.owner == nv as u8).count(), 12);
        // Corsair: three armies a kill.
        let oc = flyer(&mut w, 4, Team::Ori, ShipType::Corsair, 90_000.0, 90_000.0);
        w.players[oc].kills = 2.0;
        assert_eq!(w.players[oc].max_armies_now(), 6);
    }

    #[test]
    fn relic_traits() {
        let mut w = World::new();
        w.features.ranks = true;
        let relic = |w: &mut World, ship: ShipType, team: Team, x: f64, y: f64| {
            let i = flyer(w, 6, team, ShipType::IconianShip, x, y);
            w.players[i].ship = ship;
            let s = ship.stats();
            (w.players[i].shield, w.players[i].fuel) = (s.max_shield, s.max_fuel);
            i
        };
        // Iconian: gateway between our worlds.
        let ic = relic(&mut w, ShipType::IconianShip, Team::Fed, 0.0, 0.0);
        let (a, b) = (1, 8);
        w.players[ic].orbiting = Some(a);
        w.handle(ic as u8, ClientMsg::LockPlanet(b as u8));
        assert_eq!(w.players[ic].orbiting, Some(b), "stepped through to {}", w.planets[b].name);
        w.handle(ic as u8, ClientMsg::LockPlanet(a as u8));
        assert_ne!(w.players[ic].orbiting, Some(a), "the gateway needs to recharge");
        // Breen: dampening torpedoes.
        let br = relic(&mut w, ShipType::BreenWarship, Team::Fed, 50_000.0, 50_000.0);
        let tal = pilot(&mut w, "Tal", Team::Rom, 53_000.0, 50_000.0) as usize;
        w.handle(br as u8, ClientMsg::Torp(64));
        let mut hit = false;
        for _ in 0..20 {
            let fuel = w.players[tal].fuel;
            w.tick();
            if w.players[tal].jammed_until > w.tick {
                assert!(!w.players[tal].shields_up && w.players[tal].fuel < fuel - 400.0);
                hit = true;
                break;
            }
        }
        assert!(hit, "the Breen torpedo never hit");
        w.handle(tal as u8, ClientMsg::Shields);
        assert!(!w.players[tal].shields_up, "jammed");
        // Vidiian: damage dealt repairs the harvester.
        let vh = relic(&mut w, ShipType::VidiianHarvester, Team::Fed, 20_000.0, 20_000.0);
        w.players[vh].damage = 50.0;
        w.inflict(tal, 50.0, Some(vh as u8), "test".into());
        assert_eq!(w.players[vh].damage, 30.0);
        // Xindi: the beam pierces every enemy along it.
        let xr = relic(&mut w, ShipType::XindiWarship, Team::Fed, 20_000.0, 80_000.0);
        let e1 = pilot(&mut w, "E1", Team::Rom, 22_000.0, 80_000.0) as usize;
        let e2 = pilot(&mut w, "E2", Team::Rom, 24_000.0, 80_000.0) as usize;
        w.handle(xr as u8, ClientMsg::Phaser(64));
        assert!(w.players[e1].shield < 100.0 && w.players[e2].shield < 100.0);
        // Preserver: enemy torpedoes are turned back, most of the time.
        let po = relic(&mut w, ShipType::PreserverObelisk, Team::Fed, 80_000.0, 20_000.0);
        let k = pilot(&mut w, "Kor", Team::Rom, 84_000.0, 20_000.0) as usize;
        let mut turned = 0;
        for _ in 0..10 {
            w.torps.clear();
            w.players[k].wtemp = 0.0;
            w.handle(k as u8, ClientMsg::Torp(192));
            let mut this = false;
            for _ in 0..25 {
                w.tick();
                this |= w.torps.iter().any(|t| t.owner == po as u8);
            }
            turned += usize::from(this);
        }
        assert!((3..=9).contains(&turned), "{} of 10 torpedoes turned", turned);
        // Talosian: to distant enemies it shows up where it isn't.
        let ti = relic(&mut w, ShipType::TalosianShip, Team::Fed, 50_000.0, 90_000.0);
        let far = pilot(&mut w, "Far", Team::Rom, 60_000.0, 90_000.0);
        let seen = w.frame_for(far).players.iter().find(|p| p.id == ti as u8).cloned().unwrap();
        let off = ((seen.x as f64 - 50_000.0).powi(2) + (seen.y as f64 - 90_000.0).powi(2)).sqrt();
        assert!((1400.0..=2600.0).contains(&off) && !seen.fuzzy, "offset {}", off);
        let friend = pilot(&mut w, "Friend", Team::Fed, 60_000.0, 90_000.0);
        let seen = w.frame_for(friend).players.iter().find(|p| p.id == ti as u8).cloned().unwrap();
        assert_eq!((seen.x, seen.y), (50_000, 90_000), "friends see the truth");
    }

    #[test]
    fn more_relic_traits() {
        let mut w = World::new();
        w.features.ranks = true;
        let relic = |w: &mut World, ship: ShipType, team: Team, x: f64, y: f64| {
            let i = flyer(w, 6, team, ShipType::IconianShip, x, y);
            w.players[i].ship = ship;
            let s = ship.stats();
            (w.players[i].shield, w.players[i].fuel) = (s.max_shield, s.max_fuel);
            i
        };
        // Voth: four armies a kill.
        let vc = relic(&mut w, ShipType::VothCityShip, Team::Fed, 10_000.0, 10_000.0);
        w.players[vc].kills = 3.0;
        assert_eq!(w.players[vc].max_armies_now(), 12);
        // Kazon: rams what it touches at speed.
        let kz = relic(&mut w, ShipType::KazonRaider, Team::Fed, 30_000.0, 30_000.0);
        let tal = pilot(&mut w, "Tal", Team::Rom, 30_500.0, 30_000.0) as usize;
        w.players[kz].speed = 8;
        w.players[kz].desired_speed = 8;
        w.players[tal].shields_up = false;
        w.tick();
        assert!(w.players[tal].damage >= 70.0, "rammed for {}", w.players[tal].damage);
        // Sheliak: its planet can't be bombed.
        let k = Team::Fed.home_planet() + 3;
        let sh = relic(&mut w, ShipType::SheliakShip, Team::Fed, 0.0, 0.0);
        w.planets[k].owner = Team::Fed;
        w.players[sh].orbiting = Some(k);
        let bomber = pilot(&mut w, "Kor", Team::Rom, 0.0, 0.0) as usize;
        w.players[bomber].orbiting = Some(k);
        w.handle(bomber as u8, ClientMsg::Bomb);
        assert!(!w.players[bomber].bombing, "shielded");
        // Kelvan: no torpedoes near it.
        let kv = relic(&mut w, ShipType::KelvanShip, Team::Fed, 70_000.0, 70_000.0);
        let near = pilot(&mut w, "Near", Team::Rom, 71_500.0, 70_000.0);
        w.handle(near, ClientMsg::Torp(0));
        assert!(!w.torps.iter().any(|t| t.owner == near));
        w.players[kv].x = 90_000.0;
        w.handle(near, ClientMsg::Torp(0));
        assert!(w.torps.iter().any(|t| t.owner == near), "out of the field it fires");
        // Husnock: bombs three at a time, down to one.
        let hn = relic(&mut w, ShipType::HusnockWarship, Team::Fed, 0.0, 0.0);
        let target = Team::Rom.home_planet() + 2;
        w.planets[target].armies = 5;
        w.players[hn].orbiting = Some(target);
        w.handle(hn as u8, ClientMsg::Bomb);
        assert!(w.players[hn].bombing);
        let mut lowest = w.planets[target].armies;
        for _ in 0..100 {
            w.tick();
            w.players[hn].orbiting = Some(target);
            lowest = lowest.min(w.planets[target].armies);
        }
        assert_eq!(lowest, 1);
        // Suliban: a good share of hits miss.
        let sc = relic(&mut w, ShipType::SulibanCell, Team::Fed, 50_000.0, 20_000.0);
        w.players[sc].shields_up = false;
        let mut missed = 0;
        for _ in 0..40 {
            let before = w.players[sc].damage;
            w.inflict(sc, 1.0, None, "test".into());
            missed += usize::from(w.players[sc].damage == before);
        }
        assert!((5..=25).contains(&missed), "{} of 40 missed", missed);
        // Excalbian: from afar, enemies see one of their own cruisers.
        let ex = relic(&mut w, ShipType::ExcalbianShip, Team::Fed, 50_000.0, 90_000.0);
        let far = pilot(&mut w, "Far", Team::Rom, 60_000.0, 90_000.0);
        let seen = w.frame_for(far).players.iter().find(|p| p.id == ex as u8).cloned().unwrap();
        assert_eq!((seen.team, seen.ship), (Team::Rom, ShipType::Cruiser));
        let friend = pilot(&mut w, "Friend", Team::Fed, 60_000.0, 90_000.0);
        let seen = w.frame_for(friend).players.iter().find(|p| p.id == ex as u8).cloned().unwrap();
        assert_eq!((seen.team, seen.ship), (Team::Fed, ShipType::ExcalbianShip));
    }

    /// The ranks are minimums: every rank from Captain up can fly a special
    /// ship, and every rank from Commodore up can fly a relic (so an
    /// Admiral can fly either).
    #[test]
    fn senior_ranks_can_fly_both() {
        let mut w = World::new();
        w.features.ranks = true;
        for rank in 0..RANKS.len() as u8 {
            let id = w.add_player("Officer", false).unwrap();
            w.players[id as usize].rank = Some(rank);
            assert_eq!(w.join(id, Team::Fed, ShipType::Defiant).is_ok(), rank >= SPECIAL_RANK, "special at rank {}", rank);
            w.players[id as usize].state = PState::Outfit;
            assert_eq!(w.join(id, Team::Fed, ShipType::IconianShip).is_ok(), rank >= RELIC_RANK, "relic at rank {}", rank);
            w.remove_player(id);
        }
    }

    /// Overwatch reaches for special weapons first.
    #[test]
    fn overwatch_fires_special_weapons_first() {
        let fire = |techs: &[Tech], range: f64| {
            let mut w = World::new();
            let me = officer(&mut w, Team::Fed, 50_000.0, 50_000.0, techs);
            let tal = pilot(&mut w, "Tal", Team::Rom, 50_000.0 + range, 50_000.0) as usize;
            park_in_orbit(&mut w, me);
            w.players[me].overwatch = true;
            for _ in 0..3 {
                w.tick();
            }
            (w, me, tal)
        };
        // Isokinetic cannon at 8,000: out of phaser range, but not its range.
        let (w, me, tal) = fire(&[Tech::Isokinetic], 8000.0);
        assert!(w.players[me].tech_ready[1] > 0, "fired the cannon");
        assert_eq!(w.players[tal].damage, 120.0, "straight through the shields");
        // Antiproton burst up close.
        let (w, me, _) = fire(&[Tech::Antiproton], 3000.0);
        assert!(w.players[me].tech_ready[1] > 0);
        // Tricobalt: fired at a safe distance...
        let (w, me, _) = fire(&[Tech::Tricobalt], 5000.0);
        assert!(w.torps.iter().any(|t| t.owner == me as u8 && t.kind == TorpKind::Tricobalt));
        // ...but never with a friend near the target.
        let mut w = World::new();
        let me = officer(&mut w, Team::Fed, 50_000.0, 50_000.0, &[Tech::Tricobalt]);
        pilot(&mut w, "Tal", Team::Rom, 55_000.0, 50_000.0);
        pilot(&mut w, "Friend", Team::Fed, 56_000.0, 51_000.0);
        park_in_orbit(&mut w, me);
        w.players[me].overwatch = true;
        for _ in 0..3 {
            w.tick();
        }
        assert!(!w.torps.iter().any(|t| t.kind == TorpKind::Tricobalt), "held fire");
        // Plasma before photons, when the ship has it and the kills.
        let (mut w, me, _) = fire(&[], 50_000.0);
        let tal = pilot(&mut w, "Kor", Team::Rom, 56_000.0, 50_000.0);
        let _ = tal;
        w.players[me].kills = 2.0;
        for _ in 0..3 {
            w.tick();
        }
        assert!(w.torps.iter().any(|t| t.owner == me as u8 && t.kind == TorpKind::Plasma), "plasma first");
        // A starbase launches its fighters.
        let mut w = World::new();
        let sb = officer(&mut w, Team::Fed, 50_000.0, 50_000.0, &[Tech::FighterWing]);
        w.players[sb].ship = ShipType::Starbase;
        w.players[sb].fuel = ShipType::Starbase.stats().max_fuel;
        pilot(&mut w, "Tal", Team::Rom, 60_000.0, 50_000.0);
        park_in_orbit(&mut w, sb);
        w.players[sb].overwatch = true;
        w.tick();
        assert!(w.players.iter().any(|p| p.fighter_of.is_some()), "fighters out");
    }
}
