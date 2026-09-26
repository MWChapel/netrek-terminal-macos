//! Colours shared by the renderers: RGB helpers, conversion to terminal
//! colours, and the palette for empires, aliens and planets.

use crate::consts::*;
use crate::proto::*;
use crossterm::style::Color;

pub type Rgb = [f32; 3];

pub const fn rgb(hex: u32) -> Rgb {
    [((hex >> 16) & 0xff) as f32, ((hex >> 8) & 0xff) as f32, (hex & 0xff) as f32]
}

pub fn scale(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

pub fn to_color(c: Rgb, truecolor: bool) -> Color {
    let q = |v: f32| ((v.clamp(0.0, 255.0) / 4.0).round() * 4.0).min(255.0) as u8;
    let (r, g, b) = (q(c[0]), q(c[1]), q(c[2]));
    if truecolor {
        return Color::Rgb { r, g, b };
    }
    // Nearest xterm-256 color from the 6x6x6 cube or the grey ramp.
    const LV: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let near = |v: u8| (0..6).min_by_key(|&i| (LV[i] - v as i32).abs()).unwrap();
    let (ri, gi, bi) = (near(r), near(g), near(b));
    let cube = 16 + 36 * ri + 6 * gi + bi;
    let cube_err = (LV[ri] - r as i32).pow(2) + (LV[gi] - g as i32).pow(2) + (LV[bi] - b as i32).pow(2);
    let avg = (r as i32 + g as i32 + b as i32) / 3;
    let gi2 = ((avg - 8).max(0) / 10).min(23);
    let gv = 8 + gi2 * 10;
    let grey_err = (gv - r as i32).pow(2) + (gv - g as i32).pow(2) + (gv - b as i32).pow(2);
    if grey_err < cube_err {
        Color::AnsiValue(232 + gi2 as u8)
    } else {
        Color::AnsiValue(cube as u8)
    }
}

pub fn truecolor_supported() -> bool {
    let ct = std::env::var("COLORTERM").unwrap_or_default().to_lowercase();
    if ct.contains("truecolor") || ct.contains("24bit") {
        return true;
    }
    matches!(
        std::env::var("TERM_PROGRAM").unwrap_or_default().as_str(),
        "iTerm.app" | "WezTerm" | "ghostty" | "vscode"
    )
}

pub fn team_rgb(t: Team) -> Rgb {
    match t {
        Team::Fed => rgb(0xf2c94c),
        Team::Rom => rgb(0xff5a4f),
        Team::Kli => rgb(0x4fd46a),
        Team::Ori => rgb(0x3fc5f0),
        Team::Ind => rgb(0xa0a4ab),
    }
}

pub fn faction_rgb(f: Faction) -> Rgb {
    match f {
        Faction::Khan => rgb(0xe060e0),
        Faction::Gorn => rgb(0xb4c43c),
        Faction::Tholian => rgb(0xffa040),
        Faction::Fesarius => rgb(0x80c8ff),
        Faction::Mirror => rgb(0xc8d0dc),
        Faction::Doomsday => rgb(0xa8b0bc),
        Faction::Amoeba => rgb(0x50e8b0),
        Faction::Borg => rgb(0x39ff14),
        Faction::Vger => rgb(0x6aa8ff),
        Faction::Crystal => rgb(0xd8f6ff),
        Faction::Probe => rgb(0xb8a888),
        Faction::Species8472 => rgb(0xff7fa0),
        Faction::JemHadar => rgb(0xa070ff),
        Faction::Tribbles => rgb(0xe8c89a),
        Faction::Chang => rgb(0xc8504a),
        Faction::Hirogen => rgb(0x8aa0a8),
        Faction::Q => rgb(0xfffbe0),
        Faction::Ferengi => rgb(0xc8843c),
        Faction::Swarm => rgb(0xe0ff70),
    }
}

/// A ship's colour: its alien faction's, or its empire's.
pub fn player_rgb(p: &PlayerInfo) -> Rgb {
    p.faction.map_or(team_rgb(p.team), faction_rgb)
}

/// A planet's colour; devoured planets are dead grey rock.
pub fn planet_rgb(info: &PlanetInfo) -> Rgb {
    match info.alien {
        Some(Faction::Doomsday | Faction::Species8472) => rgb(0x55595f),
        Some(f) => faction_rgb(f),
        None => team_rgb(info.owner),
    }
}

/// Torpedoes take their owner's colour (aliens included).
pub fn torp_rgb(f: &Frame, t: &TorpInfo) -> Rgb {
    f.players.iter().find(|p| p.id == t.owner).map_or(team_rgb(t.team), player_rgb)
}

/// "F3" for empire ships, "Borg3" style for aliens.
pub fn callsign(p: &PlayerInfo) -> String {
    match p.faction {
        Some(f) => format!("{}{}", f.short(), slot_char(p.id)),
        None => format!("{}{}", p.team.letter(), slot_char(p.id)),
    }
}

/// How big to draw a ship, in galaxy units (monsters are huge).
pub fn ship_size_units(s: ShipType) -> f64 {
    if s == ShipType::VgerCloud {
        6000.0
    } else if s.hit_radius() > EXPDIST {
        s.hit_radius() * 1.05
    } else {
        0.0
    }
}

/// Armies spilled from a Ferengi wreck.
pub const LOOT: Rgb = rgb(0xffd84a);
