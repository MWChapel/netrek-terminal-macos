//! Wire protocol: length-prefixed bincode frames over TCP.

use crate::consts::{Faction, ShipType, Team};
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
    pub const SHIELD: u16 = 1;
    pub const CLOAK: u16 = 2;
    pub const ORBIT: u16 = 4;
    pub const BOMB: u16 = 8;
    pub const BEAMUP: u16 = 16;
    pub const BEAMDOWN: u16 = 32;
    pub const REPAIR: u16 = 64;
    pub const TRACTOR: u16 = 128;
    pub const PRESSOR: u16 = 256;
    pub const ROBOT: u16 = 512;
    pub const WEAPON_HOT: u16 = 1024;
    pub const ENGINE_HOT: u16 = 2048;
    /// Marked as prey by the Hirogen.
    pub const HUNTED: u16 = 4096;
    /// Carrying tribbles.
    pub const TRIBBLES: u16 = 8192;
    /// Hidden from sensors (nebula or ion storm) — only set on your own ship.
    pub const HIDDEN: u16 = 16384;
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
    pub flags: u16,
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
