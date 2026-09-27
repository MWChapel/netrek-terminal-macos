//! Wire protocol: length-prefixed bincode frames over TCP.

use crate::consts::{Faction, ShipType, Team, Tech};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{self, Read, Write};

const MAX_FRAME: usize = 1 << 20;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgTarget {
    All,
    Team(Team),
    Player(u8),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientMsg {
    Hello { name: String, version: u32 },
    Join { team: Team, ship: ShipType },
    Course(u8),
    Speed(u8),
    Torp(u8),
    Phaser(u8),
    Plasma(u8),
    Shields,
    Cloak,
    Orbit,
    Bomb,
    BeamUp,
    BeamDown,
    Repair,
    /// Tractor (pressor=false) or pressor (pressor=true) on a player; None releases.
    Tractor { target: Option<u8>, pressor: bool },
    DetEnemy,
    DetOwn,
    /// Toggle overwatch: fire automatically at enemies that come into range.
    Overwatch,
    /// Use the advanced tech in slot 0 (v), 1 (e) or 2 (j), aimed at `dir`.
    Tech { slot: u8, dir: u8 },
    LockPlanet(u8),
    LockPlayer(u8),
    Refit(ShipType),
    Message { to: MsgTarget, text: String },
    Quit,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgKind {
    All,
    Team,
    Indiv,
    System,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMsg {
    pub kind: MsgKind,
    pub from: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PState {
    Outfit,
    Alive,
    Exploding,
    Dead,
}

pub mod pf {
    pub const SHIELD: u32 = 1;
    pub const CLOAK: u32 = 2;
    pub const ORBIT: u32 = 4;
    pub const BOMB: u32 = 8;
    pub const BEAMUP: u32 = 16;
    pub const BEAMDOWN: u32 = 32;
    pub const REPAIR: u32 = 64;
    pub const TRACTOR: u32 = 128;
    pub const PRESSOR: u32 = 256;
    pub const ROBOT: u32 = 512;
    pub const WEAPON_HOT: u32 = 1024;
    pub const ENGINE_HOT: u32 = 2048;
    /// Marked as prey by the Hirogen.
    pub const HUNTED: u32 = 4096;
    /// Carrying tribbles.
    pub const TRIBBLES: u32 = 8192;
    /// Hidden from sensors (nebula or ion storm) — only set on your own ship.
    pub const HIDDEN: u32 = 16384;
    /// Overwatch is on: firing automatically at enemies in range.
    pub const OVERWATCH: u32 = 32768;
    /// Charging a transwarp jump.
    pub const CHARGING: u32 = 65536;
    /// Out of phase (phase cloak): untouchable, can't fire.
    pub const PHASED: u32 = 131072;
    /// Trapped on the rim of the Tempest's web.
    pub const TRAPPED: u32 = 262144;
    /// Superzapper ready (trapped, and not used yet).
    pub const ZAPPER: u32 = 524288;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerInfo {
    pub id: u8,
    pub name: String,
    pub team: Team,
    pub ship: ShipType,
    pub state: PState,
    pub x: i32,
    pub y: i32,
    pub dir: u8,
    pub speed: u8,
    pub flags: u32,
    pub kills: f32,
    /// Armies carried (only revealed to teammates; 0 otherwise).
    pub armies: u8,
    pub tractor_target: Option<u8>,
    /// True when this is a cloaked enemy whose position is only approximate.
    pub fuzzy: bool,
    pub explode_frame: u8,
    /// Set for alien ships (the --aliens incursions).
    pub faction: Option<Faction>,
    /// Career rank index into `RANKS` (with --ranks; humans only).
    pub rank: Option<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorpKind {
    Photon,
    Plasma,
    /// The Rear Admiral's tricobalt device: slow, huge blast.
    Tricobalt,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TorpInfo {
    pub owner: u8,
    pub team: Team,
    pub kind: TorpKind,
    pub x: i32,
    pub y: i32,
    /// 0 = in flight, >0 = explosion animation frame.
    pub explode: u8,
    /// A quantum torpedo (Captain's tech).
    pub quantum: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PhaserInfo {
    pub owner: u8,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub hit: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlanetInfo {
    pub owner: Team,
    pub armies: u16,
    pub flags: u8,
    /// Whether our team has scouted this planet (otherwise owner/armies are stale).
    pub known: bool,
    /// Held by an alien power (Khan's stronghold, Terran Empire conquests),
    /// or `Some(Doomsday)` when the planet killer has devoured it.
    pub alien: Option<Faction>,
    /// Infested with tribbles.
    pub tribbles: bool,
}

/// Space terrain (the --terrain option).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainKind {
    /// Hides ships from distant sensors; no shields, warp 6 at most.
    Nebula,
    /// A drifting storm: scrambles sensors, knocks out phasers, throws lightning.
    IonStorm,
    /// Rocks: fast ships take hull damage, torpedoes are soaked up.
    Asteroids,
    /// Pulls ships and torpedoes in; the event horizon destroys them.
    BlackHole,
    /// Sweeps its surroundings with a radiation pulse every ten seconds.
    Pulsar,
    /// A pair of linked mouths (x, y) and (x2, y2).
    Wormhole,
    /// A wreck to salvage for fuel, repairs or stranded colonists.
    Derelict,
    /// A subspace slipstream from (x, y) to (x2, y2): +3 warp, no fuel cost.
    Corridor,
    /// A star: its corona refuels ships but heats them up; the core burns.
    Star,
    /// Crosses the galaxy; its tail refuels, its head hurts.
    Comet,
    /// Reveals cloaked ships inside it.
    TachyonGrid,
    /// Hidden mines: touch one and it blows (a new one is laid elsewhere).
    Minefield,
    /// Time runs slow: ships and torpedoes inside move at half speed.
    ChronitonField,
    /// A whirlpool that sweeps ships around its centre.
    GravitonEddy,
    /// Tractors fail and torpedoes curve; the core crushes.
    Magnetar,
    /// Volatile gas: firing a weapon inside ignites it.
    MetreonCloud,
    /// Strips shields and stops them recharging.
    TetryonField,
    /// Solid: ships can't pass through it, and it stops torpedoes.
    Planetoid,
    /// A wall from (x, y) to (x2, y2): crossing costs fuel and hull.
    GalacticBarrier,
    /// Flings ships to a random spot in the galaxy.
    FluidicRift,
    /// A neutral outpost that repairs and refuels anyone holding beside it.
    AbandonedStation,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TerrainInfo {
    pub kind: TerrainKind,
    pub x: i32,
    pub y: i32,
    pub r: i32,
    /// Second point: the other wormhole mouth, the corridor's far end, or
    /// the end of a comet's tail.
    pub x2: i32,
    pub y2: i32,
    /// Animation state (pulsar: ticks until the next pulse).
    pub phase: u8,
    pub name: String,
}

/// The shape of the Tempest's web (it changes each level, as in the arcade).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempestShape {
    Circle,
    Square,
    Triangle,
}

impl TempestShape {
    pub const ALL: [TempestShape; 3] = [TempestShape::Circle, TempestShape::Square, TempestShape::Triangle];

    pub fn lanes(self) -> u8 {
        match self {
            TempestShape::Circle | TempestShape::Square => 16,
            TempestShape::Triangle => 15,
        }
    }

    pub fn next(self) -> TempestShape {
        match self {
            TempestShape::Circle => TempestShape::Square,
            TempestShape::Square => TempestShape::Triangle,
            TempestShape::Triangle => TempestShape::Circle,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            TempestShape::Circle => "circle",
            TempestShape::Square => "square",
            TempestShape::Triangle => "triangle",
        }
    }

    /// Polygon corners (none for the circle), `r` from the centre: the
    /// square sits flat, the triangle points up the screen.
    fn corners(self, cx: f64, cy: f64, r: f64) -> Vec<(f64, f64)> {
        let (n, start) = match self {
            TempestShape::Circle => return Vec::new(),
            TempestShape::Square => (4, std::f64::consts::FRAC_PI_4),
            TempestShape::Triangle => (3, -std::f64::consts::FRAC_PI_2),
        };
        (0..n)
            .map(|k| {
                let a = start + k as f64 * std::f64::consts::TAU / n as f64;
                (cx + a.cos() * r, cy + a.sin() * r)
            })
            .collect()
    }

    /// The rim point at lane coordinate `s`: lane centres are at whole
    /// numbers and the lane edges (spokes) halfway between.
    pub fn rim(self, cx: f64, cy: f64, r: f64, lanes: u8, s: f64) -> (f64, f64) {
        let lanes = lanes.max(1) as f64;
        let corners = self.corners(cx, cy, r);
        if corners.is_empty() {
            let a = s / lanes * std::f64::consts::TAU;
            return (cx + a.cos() * r, cy + a.sin() * r);
        }
        let n = corners.len() as f64;
        let t = ((s + 0.5) / lanes).rem_euclid(1.0) * n;
        let j = (t.floor() as usize).min(corners.len() - 1);
        let u = t - j as f64;
        let (a, b) = (corners[j], corners[(j + 1) % corners.len()]);
        (a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u)
    }

    /// Where the rim crosses the line from the centre toward (x, y), and the
    /// lane coordinate there.
    pub fn rim_toward(self, cx: f64, cy: f64, r: f64, lanes: u8, x: f64, y: f64) -> ((f64, f64), f64) {
        let (dx, dy) = (x - cx, y - cy);
        let len = dx.hypot(dy).max(1e-9);
        let (ux, uy) = (dx / len, dy / len);
        let corners = self.corners(cx, cy, r);
        let lanes_f = lanes.max(1) as f64;
        if corners.is_empty() {
            let a = uy.atan2(ux).rem_euclid(std::f64::consts::TAU);
            return ((cx + ux * r, cy + uy * r), a / std::f64::consts::TAU * lanes_f);
        }
        let n = corners.len();
        for j in 0..n {
            let (a, b) = (corners[j], corners[(j + 1) % n]);
            // Solve centre + k*u = a + m*(b - a) for k >= 0, 0 <= m <= 1.
            let (ex, ey) = (b.0 - a.0, b.1 - a.1);
            let den = ux * ey - uy * ex;
            if den.abs() < 1e-12 {
                continue;
            }
            let (wx, wy) = (a.0 - cx, a.1 - cy);
            let k = (wx * ey - wy * ex) / den;
            let m = (wx * uy - wy * ux) / den;
            if k >= 0.0 && (-1e-9..=1.0 + 1e-9).contains(&m) {
                let s = ((j as f64 + m.clamp(0.0, 1.0)) / n as f64 * lanes_f - 0.5).rem_euclid(lanes_f);
                return ((cx + ux * k, cy + uy * k), s);
            }
        }
        ((cx + ux * r, cy + uy * r), 0.0)
    }
}

/// The Tempest's web: a tube of `lanes` lanes between two rings.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TempestInfo {
    pub x: i32,
    pub y: i32,
    pub r_in: i32,
    pub r_out: i32,
    pub shape: TempestShape,
    pub lanes: u8,
    /// The core is exposed (the web has been cleared).
    pub exposed: bool,
    pub level: u8,
}

/// A career on the leaderboard (with --ranks).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LeaderInfo {
    pub name: String,
    pub rank: u8,
    pub points: f32,
}

/// Armies dropped by a destroyed Ferengi marauder, free for anyone to pick up.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LootInfo {
    pub x: i32,
    pub y: i32,
    pub armies: u8,
}

/// One strand of a Tholian web.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebInfo {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SelfInfo {
    pub fuel: u32,
    pub shield: u32,
    pub damage: u32,
    pub wtemp: u32,
    pub etemp: u32,
    pub armies: u8,
    pub max_armies_now: u8,
    pub kills: f32,
    pub speed: u8,
    pub desired_speed: u8,
    pub max_speed_now: u8,
    pub torps_out: u8,
    pub lock: Option<String>,
    pub orbiting: Option<u8>,
    pub deaths: u32,
    pub total_kills: f32,
    /// Current orders from command (with --orders).
    pub order: Option<String>,
    /// Your empire's supply stockpile and upgrade levels (with --supply).
    pub supply: Option<(u32, [u8; 5])>,
    /// One-line service record (with --ranks).
    pub service: Option<String>,
    /// Advanced tech aboard, with seconds until each active one is ready.
    pub techs: Vec<(Tech, u16)>,
    /// Ablative armor left (with that tech).
    pub armor: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Frame {
    pub tick: u32,
    pub me: u8,
    pub me_info: SelfInfo,
    pub players: Vec<PlayerInfo>,
    pub torps: Vec<TorpInfo>,
    pub phasers: Vec<PhaserInfo>,
    pub planets: Vec<PlanetInfo>,
    pub webs: Vec<WebInfo>,
    pub loot: Vec<LootInfo>,
    pub terrain: Vec<TerrainInfo>,
    /// Allied empires (with --diplomacy).
    pub treaties: Vec<(Team, Team)>,
    /// Top careers (with --ranks).
    pub leaders: Vec<LeaderInfo>,
    /// The Tempest's web, while it's in the galaxy.
    pub tempest: Option<TempestInfo>,
    /// Teams that are currently allowed to be joined.
    pub open_teams: Vec<Team>,
    /// Planets held by Fed, Rom, Kli, Ori (public knowledge, like the team window).
    pub team_planets: [u8; 4],
    /// Teams that already have a starbase in play.
    pub starbase_teams: Vec<Team>,
    /// Banner shown across the screen (e.g. galaxy conquered).
    pub banner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerMsg {
    Welcome { slot: u8, motd: Vec<String> },
    Reject(String),
    Frame(Box<Frame>),
    Msg(ChatMsg),
    /// Result of a join/refit attempt that failed.
    Warning(String),
}

pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    let body = bincode::serialize(msg).expect("serialize");
    let mut out = Vec::with_capacity(body.len() + 4);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

pub fn write_msg<T: Serialize, W: Write>(w: &mut W, msg: &T) -> io::Result<()> {
    w.write_all(&encode(msg))?;
    w.flush()
}

pub fn read_msg<T: DeserializeOwned, R: Read>(r: &mut R) -> io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    bincode::deserialize(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
