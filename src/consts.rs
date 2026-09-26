//! Game constants, taken from the classic Vanilla Netrek server
//! (include/defs.h, ntserv/getship.c, ntserv/planet.c).

use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: u16 = 2592; // the traditional Netrek port
pub const PROTOCOL_VERSION: u32 = 5;

pub const UPS: u64 = 10; // server updates per second, like the original
pub const GWIDTH: f64 = 100_000.0;
pub const WARP1: f64 = 20.0; // units moved per update at warp 1
pub const MAXPLAYER: usize = 32;
pub const MAXTORP: usize = 8;

pub const EXPDIST: f64 = 350.0; // torp explodes at this range
pub const DAMDIST: f64 = 2000.0; // torp does damage within this range
pub const PLASDAMDIST: f64 = 2500.0;
pub const SHIPDAMDIST: f64 = 3000.0; // exploding ship damage radius
pub const DETDIST: f64 = 1700.0; // enemy torps within this can be detonated
pub const PHASEDIST: f64 = 6000.0;
pub const ENTORBDIST: f64 = 900.0;
pub const ORBDIST: f64 = 800.0;
pub const ORBSPEED: i32 = 2;
pub const PFIREDIST: f64 = 1500.0;
pub const TRACTDIST: f64 = 6000.0;
pub const TRACTCOST: f64 = 20.0;
pub const TRACTEHEAT: f64 = 5.0;
pub const START_ARMIES: i32 = 17;
pub const HOME_ARMIES: i32 = 30;

/// Player slot letters: 0-9 then a-v, exactly like Netrek.
pub const SLOT_CHARS: &[u8; 32] = b"0123456789abcdefghijklmnopqrstuv";

pub fn slot_char(id: u8) -> char {
    SLOT_CHARS.get(id as usize).map(|&c| c as char).unwrap_or('?')
}

pub fn slot_from_char(c: char) -> Option<u8> {
    SLOT_CHARS.iter().position(|&s| s as char == c).map(|p| p as u8)
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Team {
    Ind,
    Fed,
    Rom,
    Kli,
    Ori,
}

impl Team {
    pub const PLAYABLE: [Team; 4] = [Team::Fed, Team::Rom, Team::Kli, Team::Ori];

    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn letter(self) -> char {
        match self {
            Team::Ind => 'I',
            Team::Fed => 'F',
            Team::Rom => 'R',
            Team::Kli => 'K',
            Team::Ori => 'O',
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Team::Ind => "Independent",
            Team::Fed => "Federation",
            Team::Rom => "Romulan",
            Team::Kli => "Klingon",
            Team::Ori => "Orion",
        }
    }
    pub fn abbr(self) -> &'static str {
        match self {
            Team::Ind => "IND",
            Team::Fed => "FED",
            Team::Rom => "ROM",
            Team::Kli => "KLI",
            Team::Ori => "ORI",
        }
    }
    pub fn plural(self) -> &'static str {
        match self {
            Team::Ind => "Independents",
            Team::Fed => "Federation",
            Team::Rom => "Romulans",
            Team::Kli => "Klingons",
            Team::Ori => "Orions",
        }
    }
    pub fn home_planet(self) -> usize {
        match self {
            Team::Fed => 0,
            Team::Rom => 10,
            Team::Kli => 20,
            Team::Ori => 30,
            Team::Ind => 0,
        }
    }
    pub fn from_char(c: char) -> Option<Team> {
        match c.to_ascii_lowercase() {
            'f' => Some(Team::Fed),
            'r' => Some(Team::Rom),
            'k' => Some(Team::Kli),
            'o' => Some(Team::Ori),
            _ => None,
        }
    }
}

/// Alien incursions (the --aliens option). Aliens fly as `Team::Ind`, so
/// they are hostile to all four empires.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Faction {
    Khan,
    Gorn,
    Tholian,
    Fesarius,
    Mirror,
    Doomsday,
    Amoeba,
    Borg,
    Vger,
    Crystal,
    Probe,
    Species8472,
    JemHadar,
    Tribbles,
    Chang,
    Hirogen,
    Q,
    Ferengi,
    Swarm,
}

impl Faction {
    pub const ALL: [Faction; 19] = [
        Faction::Khan,
        Faction::Gorn,
        Faction::Tholian,
        Faction::Fesarius,
        Faction::Mirror,
        Faction::Doomsday,
        Faction::Amoeba,
        Faction::Borg,
        Faction::Vger,
        Faction::Crystal,
        Faction::Probe,
        Faction::Species8472,
        Faction::JemHadar,
        Faction::Tribbles,
        Faction::Chang,
        Faction::Hirogen,
        Faction::Q,
        Faction::Ferengi,
        Faction::Swarm,
    ];

    /// Name used on the command line.
    pub fn key(self) -> &'static str {
        match self {
            Faction::Khan => "khan",
            Faction::Gorn => "gorn",
            Faction::Tholian => "tholian",
            Faction::Fesarius => "fesarius",
            Faction::Mirror => "mirror",
            Faction::Doomsday => "doomsday",
            Faction::Amoeba => "amoeba",
            Faction::Borg => "borg",
            Faction::Vger => "vger",
            Faction::Crystal => "crystal",
            Faction::Probe => "probe",
            Faction::Species8472 => "8472",
            Faction::JemHadar => "jemhadar",
            Faction::Tribbles => "tribbles",
            Faction::Chang => "chang",
            Faction::Hirogen => "hirogen",
            Faction::Q => "q",
            Faction::Ferengi => "ferengi",
            Faction::Swarm => "swarm",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Faction::Khan => "Khan's augments",
            Faction::Gorn => "Gorn raiders",
            Faction::Tholian => "Tholians",
            Faction::Fesarius => "the Fesarius",
            Faction::Mirror => "the Terran Empire",
            Faction::Doomsday => "the planet killer",
            Faction::Amoeba => "the space amoeba",
            Faction::Borg => "the Borg",
            Faction::Vger => "V'Ger",
            Faction::Crystal => "the Crystalline Entity",
            Faction::Probe => "the whale probe",
            Faction::Species8472 => "Species 8472",
            Faction::JemHadar => "the Jem'Hadar",
            Faction::Tribbles => "the tribbles",
            Faction::Chang => "General Chang",
            Faction::Hirogen => "the Hirogen",
            Faction::Q => "Q",
            Faction::Ferengi => "the Ferengi",
            Faction::Swarm => "the Swarm",
        }
    }

    /// Short callsign shown next to the ship.
    pub fn tag(self) -> &'static str {
        match self {
            Faction::Khan => "Khan",
            Faction::Gorn => "Gorn",
            Faction::Tholian => "Tholian",
            Faction::Fesarius => "Fesarius",
            Faction::Mirror => "ISS",
            Faction::Doomsday => "Planet Killer",
            Faction::Amoeba => "Amoeba",
            Faction::Borg => "Borg",
            Faction::Vger => "V'Ger",
            Faction::Crystal => "Crystalline Entity",
            Faction::Probe => "Probe",
            Faction::Species8472 => "8472",
            Faction::JemHadar => "Jem'Hadar",
            Faction::Tribbles => "Tribbles",
            Faction::Chang => "Chang",
            Faction::Hirogen => "Hirogen",
            Faction::Q => "Q",
            Faction::Ferengi => "Ferengi",
            Faction::Swarm => "Swarm",
        }
    }

    /// Two-letter tag for the galaxy map and player list.
    pub fn short(self) -> &'static str {
        match self {
            Faction::Khan => "KH",
            Faction::Gorn => "GN",
            Faction::Tholian => "TH",
            Faction::Fesarius => "FS",
            Faction::Mirror => "MU",
            Faction::Doomsday => "PK",
            Faction::Amoeba => "AM",
            Faction::Borg => "BG",
            Faction::Vger => "VG",
            Faction::Crystal => "CE",
            Faction::Probe => "WP",
            Faction::Species8472 => "85",
            Faction::JemHadar => "JH",
            Faction::Tribbles => "TB",
            Faction::Chang => "CH",
            Faction::Hirogen => "HG",
            Faction::Q => "QQ",
            Faction::Ferengi => "FE",
            Faction::Swarm => "SW",
        }
    }

    /// Species 8472 and the Borg are at war with each other as well as
    /// with everyone else.
    pub fn at_war_with(self, other: Faction) -> bool {
        matches!((self, other), (Faction::Borg, Faction::Species8472) | (Faction::Species8472, Faction::Borg))
    }

    pub fn from_key(s: &str) -> Option<Faction> {
        Faction::ALL.into_iter().find(|f| f.key().eq_ignore_ascii_case(s.trim()))
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ShipType {
    Scout,
    Destroyer,
    Cruiser,
    Battleship,
    Assault,
    Starbase,
    // Alien vessels (never flown by players).
    Augment,
    GornRaider,
    TholianVessel,
    Fesarius,
    PlanetKiller,
    Amoeba,
    BorgCube,
    VgerCloud,
    CrystalEntity,
    WhaleProbe,
    Bioship,
    JemHadarFighter,
    BirdOfPrey,
    HirogenHunter,
    QEntity,
    QChampion,
    FerengiMarauder,
    SwarmShip,
    /// Supply convoy freighter (the --supply option); flown by the server.
    Freighter,
}

impl ShipType {
    pub const ALL: [ShipType; 6] = [
        ShipType::Scout,
        ShipType::Destroyer,
        ShipType::Cruiser,
        ShipType::Battleship,
        ShipType::Assault,
        ShipType::Starbase,
    ];
    /// Key used to choose this ship on the outfit screen (Netrek keys).
    pub fn key(self) -> char {
        match self {
            ShipType::Scout => 's',
            ShipType::Destroyer => 'd',
            ShipType::Cruiser => 'c',
            ShipType::Battleship => 'b',
            ShipType::Assault => 'a',
            ShipType::Starbase => 'x',
            _ => '?',
        }
    }
    pub fn from_key(c: char) -> Option<ShipType> {
        ShipType::ALL.into_iter().find(|s| s.key() == c.to_ascii_lowercase())
    }
    pub fn from_abbr(s: &str) -> Option<ShipType> {
        let s = s.to_ascii_uppercase();
        ShipType::ALL.into_iter().find(|t| t.stats().abbr == s)
    }
    pub fn stats(self) -> &'static ShipStats {
        &SHIPS[self as usize]
    }

    /// How close a torpedo must get to explode on this ship (big monsters
    /// have big hulls).
    pub fn hit_radius(self) -> f64 {
        match self {
            ShipType::Fesarius => 1800.0,
            ShipType::PlanetKiller => 1500.0,
            ShipType::Amoeba => 1400.0,
            ShipType::BorgCube => 1000.0,
            // Only the core: torpedoes fired inside the cloud fly on.
            ShipType::VgerCloud => 1200.0,
            ShipType::CrystalEntity => 1500.0,
            ShipType::WhaleProbe => 1200.0,
            ShipType::QEntity => 600.0,
            _ => EXPDIST,
        }
    }

    pub fn is_alien(self) -> bool {
        !ShipType::ALL.contains(&self)
    }
}

pub struct ShipStats {
    pub name: &'static str,
    pub abbr: &'static str,
    pub max_speed: i32,
    pub max_shield: f64,
    pub max_damage: f64,
    pub max_fuel: f64,
    pub max_armies: u32,
    pub torp_damage: f64,
    pub torp_speed: f64,
    pub torp_fuse: i32,
    pub torp_cost: f64,
    pub phaser_damage: f64,
    pub phaser_cost: f64,
    pub plasma_damage: f64, // 0 = no plasma
    pub plasma_speed: f64,
    pub plasma_fuse: i32,
    pub plasma_cost: f64,
    pub recharge: f64,
    pub repair: f64,
    pub warp_cost: f64,
    pub cloak_cost: f64,
    pub shield_cost: f64,
    pub turns: f64,
    pub acc: i32,
    pub dec: i32,
    pub wpn_cool: f64,
    pub egn_cool: f64,
    pub max_etemp: f64,
    pub max_wtemp: f64,
    pub mass: f64,
    pub tract_range: f64,
    pub tract_str: f64,
}

pub static SHIPS: [ShipStats; 25] = [
    ShipStats {
        name: "Scout", abbr: "SC", max_speed: 12, max_shield: 75.0, max_damage: 75.0,
        max_fuel: 5000.0, max_armies: 2, torp_damage: 25.0, torp_speed: 16.0, torp_fuse: 16,
        torp_cost: 7.0 * 25.0, phaser_damage: 75.0, phaser_cost: 7.0 * 75.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 8.0, repair: 80.0, warp_cost: 2.0, cloak_cost: 17.0, shield_cost: 2.0,
        turns: 570_000.0, acc: 200, dec: 270, wpn_cool: 3.0, egn_cool: 8.0,
        max_etemp: 1000.0, max_wtemp: 1000.0, mass: 1500.0, tract_range: 0.7, tract_str: 2000.0,
    },
    ShipStats {
        name: "Destroyer", abbr: "DD", max_speed: 10, max_shield: 85.0, max_damage: 85.0,
        max_fuel: 7000.0, max_armies: 5, torp_damage: 30.0, torp_speed: 14.0, torp_fuse: 30,
        torp_cost: 7.0 * 30.0, phaser_damage: 85.0, phaser_cost: 7.0 * 85.0,
        plasma_damage: 75.0, plasma_speed: 15.0, plasma_fuse: 30, plasma_cost: 30.0 * 75.0,
        recharge: 11.0, repair: 100.0, warp_cost: 3.0, cloak_cost: 21.0, shield_cost: 3.0,
        turns: 310_000.0, acc: 200, dec: 300, wpn_cool: 2.0, egn_cool: 7.0,
        max_etemp: 1000.0, max_wtemp: 1000.0, mass: 1800.0, tract_range: 0.9, tract_str: 2500.0,
    },
    ShipStats {
        name: "Cruiser", abbr: "CA", max_speed: 9, max_shield: 100.0, max_damage: 100.0,
        max_fuel: 10000.0, max_armies: 10, torp_damage: 40.0, torp_speed: 12.0, torp_fuse: 40,
        torp_cost: 7.0 * 40.0, phaser_damage: 100.0, phaser_cost: 7.0 * 100.0,
        plasma_damage: 100.0, plasma_speed: 15.0, plasma_fuse: 35, plasma_cost: 30.0 * 100.0,
        recharge: 12.0, repair: 110.0, warp_cost: 4.0, cloak_cost: 26.0, shield_cost: 3.0,
        turns: 170_000.0, acc: 150, dec: 200, wpn_cool: 2.0, egn_cool: 6.0,
        max_etemp: 1000.0, max_wtemp: 1000.0, mass: 2000.0, tract_range: 1.0, tract_str: 3000.0,
    },
    ShipStats {
        name: "Battleship", abbr: "BB", max_speed: 8, max_shield: 130.0, max_damage: 130.0,
        max_fuel: 14000.0, max_armies: 6, torp_damage: 40.0, torp_speed: 12.0, torp_fuse: 40,
        torp_cost: 9.0 * 40.0, phaser_damage: 105.0, phaser_cost: 10.0 * 105.0,
        plasma_damage: 130.0, plasma_speed: 15.0, plasma_fuse: 35, plasma_cost: 30.0 * 130.0,
        recharge: 14.0, repair: 125.0, warp_cost: 6.0, cloak_cost: 30.0, shield_cost: 3.0,
        turns: 75_000.0, acc: 80, dec: 180, wpn_cool: 3.0, egn_cool: 6.0,
        max_etemp: 1000.0, max_wtemp: 1000.0, mass: 2300.0, tract_range: 1.2, tract_str: 3700.0,
    },
    ShipStats {
        name: "Assault", abbr: "AS", max_speed: 8, max_shield: 80.0, max_damage: 200.0,
        max_fuel: 6000.0, max_armies: 20, torp_damage: 30.0, torp_speed: 16.0, torp_fuse: 30,
        torp_cost: 9.0 * 30.0, phaser_damage: 80.0, phaser_cost: 7.0 * 80.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 10.0, repair: 120.0, warp_cost: 3.0, cloak_cost: 17.0, shield_cost: 3.0,
        turns: 120_000.0, acc: 100, dec: 200, wpn_cool: 2.0, egn_cool: 6.0,
        max_etemp: 1200.0, max_wtemp: 1000.0, mass: 2300.0, tract_range: 0.7, tract_str: 2500.0,
    },
    ShipStats {
        name: "Starbase", abbr: "SB", max_speed: 2, max_shield: 500.0, max_damage: 600.0,
        max_fuel: 60000.0, max_armies: 25, torp_damage: 30.0, torp_speed: 14.0, torp_fuse: 30,
        torp_cost: 10.0 * 30.0, phaser_damage: 120.0, phaser_cost: 8.0 * 120.0,
        plasma_damage: 150.0, plasma_speed: 15.0, plasma_fuse: 25, plasma_cost: 25.0 * 150.0,
        recharge: 35.0, repair: 140.0, warp_cost: 10.0, cloak_cost: 75.0, shield_cost: 6.0,
        turns: 50_000.0, acc: 100, dec: 200, wpn_cool: 4.0, egn_cool: 4.0,
        max_etemp: 1000.0, max_wtemp: 1300.0, mass: 5000.0, tract_range: 1.5, tract_str: 8000.0,
    },
    // Khan's augment ships: Reliant-style starships, faster and tougher than stock.
    ShipStats {
        name: "Augment ship", abbr: "KH", max_speed: 10, max_shield: 150.0, max_damage: 150.0,
        max_fuel: 15000.0, max_armies: 0, torp_damage: 50.0, torp_speed: 13.0, torp_fuse: 40,
        torp_cost: 5.0 * 50.0, phaser_damage: 120.0, phaser_cost: 5.0 * 120.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 20.0, repair: 180.0, warp_cost: 3.0, cloak_cost: 26.0, shield_cost: 2.0,
        turns: 260_000.0, acc: 200, dec: 250, wpn_cool: 4.0, egn_cool: 10.0,
        max_etemp: 1500.0, max_wtemp: 1400.0, mass: 2200.0, tract_range: 1.0, tract_str: 3000.0,
    },
    ShipStats {
        name: "Gorn raider", abbr: "GN", max_speed: 7, max_shield: 120.0, max_damage: 170.0,
        max_fuel: 14000.0, max_armies: 0, torp_damage: 45.0, torp_speed: 11.0, torp_fuse: 40,
        torp_cost: 5.0 * 45.0, phaser_damage: 90.0, phaser_cost: 5.0 * 90.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 18.0, repair: 150.0, warp_cost: 3.0, cloak_cost: 26.0, shield_cost: 2.0,
        turns: 150_000.0, acc: 120, dec: 200, wpn_cool: 4.0, egn_cool: 10.0,
        max_etemp: 1500.0, max_wtemp: 1400.0, mass: 2600.0, tract_range: 1.0, tract_str: 3000.0,
    },
    ShipStats {
        name: "Tholian vessel", abbr: "TH", max_speed: 8, max_shield: 70.0, max_damage: 80.0,
        max_fuel: 20000.0, max_armies: 0, torp_damage: 20.0, torp_speed: 14.0, torp_fuse: 20,
        torp_cost: 5.0 * 20.0, phaser_damage: 70.0, phaser_cost: 5.0 * 70.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 25.0, repair: 120.0, warp_cost: 1.0, cloak_cost: 20.0, shield_cost: 1.0,
        turns: 400_000.0, acc: 250, dec: 300, wpn_cool: 4.0, egn_cool: 12.0,
        max_etemp: 2000.0, max_wtemp: 1400.0, mass: 1500.0, tract_range: 0.7, tract_str: 2000.0,
    },
    ShipStats {
        name: "Fesarius", abbr: "FS", max_speed: 3, max_shield: 1500.0, max_damage: 1500.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 140.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 400.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 60_000.0, acc: 80, dec: 150, wpn_cool: 40.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 50_000.0, tract_range: 1.2, tract_str: 9000.0,
    },
    ShipStats {
        name: "Planet killer", abbr: "PK", max_speed: 2, max_shield: 1000.0, max_damage: 2500.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 0.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 150.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 20_000.0, acc: 60, dec: 150, wpn_cool: 50.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 100_000.0, tract_range: 1.0, tract_str: 5000.0,
    },
    ShipStats {
        name: "Space amoeba", abbr: "AM", max_speed: 3, max_shield: 0.0, max_damage: 1400.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 0.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 600.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 40_000.0, acc: 60, dec: 150, wpn_cool: 50.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 30_000.0, tract_range: 0.8, tract_str: 4000.0,
    },
    ShipStats {
        name: "Borg cube", abbr: "BC", max_speed: 6, max_shield: 2000.0, max_damage: 3000.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 60.0, torp_speed: 12.0, torp_fuse: 40,
        torp_cost: 0.0, phaser_damage: 120.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 700.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 40_000.0, acc: 100, dec: 200, wpn_cool: 40.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 60_000.0, tract_range: 1.3, tract_str: 12000.0,
    },
    ShipStats {
        name: "V'Ger", abbr: "VG", max_speed: 2, max_shield: 100_000.0, max_damage: 100_000.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 0.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 10_000.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 20_000.0, acc: 60, dec: 150, wpn_cool: 50.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 1_000_000.0, tract_range: 1.0, tract_str: 1.0,
    },
    ShipStats {
        name: "Crystalline Entity", abbr: "CE", max_speed: 4, max_shield: 0.0, max_damage: 600.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 0.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 2000.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 60_000.0, acc: 80, dec: 150, wpn_cool: 50.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 40_000.0, tract_range: 1.0, tract_str: 1.0,
    },
    ShipStats {
        name: "Whale probe", abbr: "WP", max_speed: 3, max_shield: 100_000.0, max_damage: 100_000.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 0.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 10_000.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 40_000.0, acc: 60, dec: 150, wpn_cool: 50.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 200_000.0, tract_range: 1.0, tract_str: 1.0,
    },
    ShipStats {
        name: "Bioship", abbr: "85", max_speed: 11, max_shield: 0.0, max_damage: 300.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 110.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 100.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 300_000.0, acc: 250, dec: 300, wpn_cool: 40.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 3000.0, tract_range: 1.0, tract_str: 3000.0,
    },
    ShipStats {
        name: "Jem'Hadar fighter", abbr: "JH", max_speed: 11, max_shield: 80.0, max_damage: 90.0,
        max_fuel: 12000.0, max_armies: 0, torp_damage: 30.0, torp_speed: 15.0, torp_fuse: 25,
        torp_cost: 4.0 * 30.0, phaser_damage: 90.0, phaser_cost: 4.0 * 90.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 20.0, repair: 60.0, warp_cost: 2.0, cloak_cost: 20.0, shield_cost: 2.0,
        turns: 420_000.0, acc: 250, dec: 300, wpn_cool: 4.0, egn_cool: 12.0,
        max_etemp: 2000.0, max_wtemp: 1400.0, mass: 1400.0, tract_range: 0.7, tract_str: 2000.0,
    },
    ShipStats {
        name: "Bird-of-Prey", abbr: "CH", max_speed: 9, max_shield: 80.0, max_damage: 110.0,
        max_fuel: 20000.0, max_armies: 0, torp_damage: 40.0, torp_speed: 12.0, torp_fuse: 35,
        torp_cost: 4.0 * 40.0, phaser_damage: 70.0, phaser_cost: 4.0 * 70.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 20.0, repair: 90.0, warp_cost: 2.0, cloak_cost: 0.0, shield_cost: 2.0,
        turns: 250_000.0, acc: 200, dec: 300, wpn_cool: 4.0, egn_cool: 12.0,
        max_etemp: 2000.0, max_wtemp: 1400.0, mass: 1500.0, tract_range: 0.7, tract_str: 2000.0,
    },
    ShipStats {
        name: "Hirogen hunter", abbr: "HG", max_speed: 10, max_shield: 110.0, max_damage: 130.0,
        max_fuel: 20000.0, max_armies: 0, torp_damage: 35.0, torp_speed: 13.0, torp_fuse: 35,
        torp_cost: 4.0 * 35.0, phaser_damage: 95.0, phaser_cost: 4.0 * 95.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 20.0, repair: 80.0, warp_cost: 2.0, cloak_cost: 20.0, shield_cost: 2.0,
        turns: 250_000.0, acc: 200, dec: 300, wpn_cool: 4.0, egn_cool: 12.0,
        max_etemp: 2000.0, max_wtemp: 1400.0, mass: 1500.0, tract_range: 0.7, tract_str: 2000.0,
    },
    ShipStats {
        name: "Q", abbr: "QQ", max_speed: 4, max_shield: 100_000.0, max_damage: 100_000.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 0.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 10_000.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 400_000.0, acc: 300, dec: 300, wpn_cool: 50.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 200_000.0, tract_range: 1.0, tract_str: 1.0,
    },
    ShipStats {
        name: "Q's champion", abbr: "QC", max_speed: 9, max_shield: 200.0, max_damage: 250.0,
        max_fuel: 30000.0, max_armies: 0, torp_damage: 50.0, torp_speed: 13.0, torp_fuse: 35,
        torp_cost: 3.0 * 50.0, phaser_damage: 110.0, phaser_cost: 3.0 * 110.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 30.0, repair: 60.0, warp_cost: 2.0, cloak_cost: 20.0, shield_cost: 2.0,
        turns: 200_000.0, acc: 200, dec: 300, wpn_cool: 5.0, egn_cool: 12.0,
        max_etemp: 3000.0, max_wtemp: 2000.0, mass: 3000.0, tract_range: 1.0, tract_str: 3000.0,
    },
    ShipStats {
        name: "Ferengi marauder", abbr: "FE", max_speed: 10, max_shield: 90.0, max_damage: 110.0,
        max_fuel: 20000.0, max_armies: 6, torp_damage: 25.0, torp_speed: 12.0, torp_fuse: 30,
        torp_cost: 4.0 * 25.0, phaser_damage: 70.0, phaser_cost: 4.0 * 70.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 20.0, repair: 80.0, warp_cost: 2.0, cloak_cost: 20.0, shield_cost: 2.0,
        turns: 250_000.0, acc: 250, dec: 300, wpn_cool: 4.0, egn_cool: 12.0,
        max_etemp: 2000.0, max_wtemp: 1400.0, mass: 1600.0, tract_range: 0.7, tract_str: 1500.0,
    },
    ShipStats {
        name: "Swarm ship", abbr: "SW", max_speed: 12, max_shield: 0.0, max_damage: 20.0,
        max_fuel: 1_000_000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 0.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 500.0, repair: 0.0, warp_cost: 0.0, cloak_cost: 0.0, shield_cost: 0.0,
        turns: 600_000.0, acc: 400, dec: 400, wpn_cool: 50.0, egn_cool: 50.0,
        max_etemp: 100_000.0, max_wtemp: 100_000.0, mass: 300.0, tract_range: 1.0, tract_str: 1.0,
    },
    ShipStats {
        name: "Freighter", abbr: "FR", max_speed: 7, max_shield: 250.0, max_damage: 300.0,
        max_fuel: 20000.0, max_armies: 0, torp_damage: 0.0, torp_speed: 10.0, torp_fuse: 30,
        torp_cost: 0.0, phaser_damage: 0.0, phaser_cost: 0.0,
        plasma_damage: 0.0, plasma_speed: 0.0, plasma_fuse: 0, plasma_cost: 0.0,
        recharge: 25.0, repair: 150.0, warp_cost: 1.0, cloak_cost: 20.0, shield_cost: 1.0,
        turns: 150_000.0, acc: 150, dec: 200, wpn_cool: 4.0, egn_cool: 20.0,
        max_etemp: 3000.0, max_wtemp: 1000.0, mass: 5000.0, tract_range: 0.5, tract_str: 1000.0,
    },
];

/// Career ranks (the --ranks option), from Netrek's classic ladder.
pub const RANKS: [(&str, &str, f64); 9] = [
    ("Ensign", "Ens", 0.0),
    ("Lieutenant", "Lt", 3.0),
    ("Lieutenant Commander", "LCdr", 8.0),
    ("Commander", "Cdr", 15.0),
    ("Captain", "Capt", 25.0),
    ("Fleet Captain", "FCpt", 40.0),
    ("Commodore", "Cdre", 60.0),
    ("Rear Admiral", "RAdm", 90.0),
    ("Admiral", "Adm", 130.0),
];

/// Rank needed to fly a starbase when ranks are on (Commander).
pub const STARBASE_RANK: u8 = 3;

/// Team upgrades bought with supplies (the --supply option).
pub const UPGRADES: [(&str, &str); 5] = [
    ("shields", "shields absorb 10% more per level"),
    ("repair", "25% faster repairs per level"),
    ("torps", "10% more torpedo damage per level"),
    ("phasers", "10% more phaser damage and range per level"),
    ("engines", "20% faster fuel recharge per level"),
];
pub const UPGRADE_SHIELDS: usize = 0;
pub const UPGRADE_REPAIR: usize = 1;
pub const UPGRADE_TORPS: usize = 2;
pub const UPGRADE_PHASERS: usize = 3;
pub const UPGRADE_ENGINES: usize = 4;
pub const MAX_UPGRADE: u8 = 3;

/// Supplies needed to buy the next level of an upgrade.
pub fn upgrade_cost(level: u8) -> u32 {
    (level as u32 + 1) * 10
}

pub const PL_HOME: u8 = 1;
pub const PL_REPAIR: u8 = 2;
pub const PL_FUEL: u8 = 4;
pub const PL_AGRI: u8 = 8;

pub struct PlanetDef {
    pub name: &'static str,
    pub team: Team,
    pub x: f64,
    pub y: f64,
}

macro_rules! pl {
    ($n:expr, $t:ident, $x:expr, $y:expr) => {
        PlanetDef { name: $n, team: Team::$t, x: $x as f64, y: $y as f64 }
    };
}

/// The classic 40-planet galaxy.
pub static PLANETS: [PlanetDef; 40] = [
    pl!("Earth", Fed, 20000, 80000),
    pl!("Rigel", Fed, 10000, 60000),
    pl!("Canopus", Fed, 25000, 60000),
    pl!("Beta Crucis", Fed, 44000, 81000),
    pl!("Organia", Fed, 39000, 55000),
    pl!("Deneb", Fed, 30000, 90000),
    pl!("Ceti Alpha V", Fed, 45000, 66000),
    pl!("Altair", Fed, 11000, 75000),
    pl!("Vega", Fed, 8000, 93000),
    pl!("Alpha Centauri", Fed, 32000, 74000),
    pl!("Romulus", Rom, 20000, 20000),
    pl!("Eridani", Rom, 45000, 7000),
    pl!("Aldeberan", Rom, 4000, 12000),
    pl!("Regulus", Rom, 42000, 44000),
    pl!("Capella", Rom, 13000, 45000),
    pl!("Tauri", Rom, 28000, 8000),
    pl!("Draconis", Rom, 28000, 23000),
    pl!("Sirius", Rom, 40000, 25000),
    pl!("Indi", Rom, 25000, 44000),
    pl!("Hydrae", Rom, 8000, 29000),
    pl!("Klingus", Kli, 80000, 20000),
    pl!("Pliedes V", Kli, 70000, 40000),
    pl!("Andromeda", Kli, 60000, 10000),
    pl!("Lalande", Kli, 56400, 38200),
    pl!("Praxis", Kli, 91120, 9320),
    pl!("Lyrae", Kli, 89960, 31760),
    pl!("Scorpii", Kli, 70720, 26320),
    pl!("Mira", Kli, 83600, 45400),
    pl!("Cygni", Kli, 54600, 22600),
    pl!("Achernar", Kli, 73080, 6640),
    pl!("Orion", Ori, 80000, 80000),
    pl!("Cassiopeia", Ori, 91200, 56600),
    pl!("El Nath", Ori, 70800, 54200),
    pl!("Spica", Ori, 57400, 62600),
    pl!("Procyon", Ori, 72720, 70880),
    pl!("Polaris", Ori, 61400, 77000),
    pl!("Arcturus", Ori, 55600, 89000),
    pl!("Ursae Majoris", Ori, 91000, 94000),
    pl!("Herculis", Ori, 70000, 93000),
    pl!("Antares", Ori, 86920, 68920),
];

/// Netrek direction (0..256, 0 = north, clockwise) to a unit vector
/// in galaxy coordinates (y grows downward).
pub fn dir_vec(dir: f64) -> (f64, f64) {
    let a = dir * std::f64::consts::TAU / 256.0;
    (a.sin(), -a.cos())
}

/// Direction from (x1,y1) toward (x2,y2) in Netrek units (0..256).
pub fn dir_to(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let a = (x2 - x1).atan2(-(y2 - y1));
    let d = a * 256.0 / std::f64::consts::TAU;
    d.rem_euclid(256.0)
}

/// Signed shortest angular difference b - a, in -128..128.
pub fn dir_diff(a: f64, b: f64) -> f64 {
    let mut d = (b - a).rem_euclid(256.0);
    if d > 128.0 {
        d -= 256.0;
    }
    d
}
