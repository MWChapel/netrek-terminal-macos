//! Everything that draws to the screen.

use super::canvas::{Braille, Screen};
use super::{App, Layout, Mode, Popup, Rect};
use crate::consts::*;
use crate::proto::*;
use crossterm::style::Color;
use std::time::Duration;

const BORDER: Color = Color::DarkCyan;
const DIM: Color = Color::DarkGrey;
const TAC_WIDTH: f64 = 20_000.0; // galaxy units across the tactical view at zoom 1

pub fn team_color(t: Team) -> Color {
    match t {
        Team::Fed => Color::Yellow,
        Team::Rom => Color::Red,
        Team::Kli => Color::Green,
        Team::Ori => Color::Cyan,
        Team::Ind => Color::Grey,
    }
}

/// Outline of each ship class, nose pointing up, in units of the ship radius.
fn ship_shape(s: ShipType) -> &'static [(f64, f64)] {
    match s {
        ShipType::Scout => &[(0.0, -1.0), (0.55, 0.8), (0.0, 0.35), (-0.55, 0.8)],
        ShipType::Destroyer => &[(0.0, -1.0), (0.35, 0.1), (0.85, 0.9), (0.0, 0.5), (-0.85, 0.9), (-0.35, 0.1)],
        ShipType::Cruiser => &[
            (0.0, -1.0), (0.6, -0.55), (0.25, 0.0), (0.8, 0.95), (0.3, 0.6), (-0.3, 0.6), (-0.8, 0.95), (-0.25, 0.0), (-0.6, -0.55),
        ],
        ShipType::Battleship => &[(0.0, -1.0), (0.75, -0.35), (0.75, 0.95), (-0.75, 0.95), (-0.75, -0.35)],
        ShipType::Assault => &[(0.0, -0.85), (0.95, 0.15), (0.55, 0.95), (-0.55, 0.95), (-0.95, 0.15)],
        ShipType::Starbase | ShipType::Fesarius | ShipType::Amoeba => &[
            (0.0, -1.0), (0.7, -0.7), (1.0, 0.0), (0.7, 0.7), (0.0, 1.0), (-0.7, 0.7), (-1.0, 0.0), (-0.7, -0.7),
        ],
        ShipType::BorgCube => &[(-0.75, -0.75), (0.75, -0.75), (0.75, 0.75), (-0.75, 0.75)],
        ShipType::PlanetKiller => &[(-0.5, -1.0), (0.5, -1.0), (0.26, 1.0), (-0.26, 1.0)],
        ShipType::TholianVessel => &[(0.0, -1.0), (0.55, 0.8), (0.0, 0.45), (-0.55, 0.8)],
        ShipType::Augment | ShipType::GornRaider => &[
            (0.0, -1.0), (0.6, -0.55), (0.25, 0.0), (0.8, 0.95), (0.3, 0.6), (-0.3, 0.6), (-0.8, 0.95), (-0.25, 0.0), (-0.6, -0.55),
        ],
    }
}

fn level_color(frac: f64, good_high: bool) -> Color {
    let f = if good_high { frac } else { 1.0 - frac };
    if f > 0.6 {
        Color::Green
    } else if f > 0.3 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Cheap deterministic hash for the star field.
fn hash(x: i64, y: i64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 32)
}

const LOGO: [&str; 6] = [
    "███╗   ██╗███████╗████████╗██████╗ ███████╗██╗  ██╗",
    "████╗  ██║██╔════╝╚══██╔══╝██╔══██╗██╔════╝██║ ██╔╝",
    "██╔██╗ ██║█████╗     ██║   ██████╔╝█████╗  █████╔╝ ",
    "██║╚██╗██║██╔══╝     ██║   ██╔══██╗██╔══╝  ██╔═██╗ ",
    "██║ ╚████║███████╗   ██║   ██║  ██║███████╗██║  ██╗",
    "╚═╝  ╚═══╝╚══════╝   ╚═╝   ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝",
];

impl App {
    /// Terminal colour for a ship: alien faction colour or empire colour.
    fn player_color(&self, p: &PlayerInfo) -> Color {
        match p.faction {
            Some(f) => super::pixels::to_color(super::render_px::faction_rgb(f), self.truecolor),
            None => team_color(p.team),
        }
    }

    fn planet_color(&self, info: &PlanetInfo) -> Color {
        match info.alien {
            Some(_) => super::pixels::to_color(super::render_px::planet_rgb(info), self.truecolor),
            None => team_color(info.owner),
        }
    }

    pub(super) fn draw(&mut self, scr: &mut Screen) {
        scr.clear();
        let (w, h) = (scr.w as i32, scr.h as i32);
        if w < 80 || h < 24 {
            scr.text(0, 0, "Netrek needs a terminal of at least 80x24.", Color::Yellow, true);
            scr.text(0, 1, &format!("Current size: {}x{} — please enlarge the window.", w, h), Color::Grey, false);
            return;
        }
        if self.frame.is_none() {
            scr.text(2, 1, "Waiting for the server...", Color::Grey, false);
            return;
        }
        if self.my_state() == PState::Outfit {
            self.draw_title(scr);
            self.draw_outfit(scr);
            let (y, maxx) = (scr.h as i32 - 1, scr.w as i32 - 1);
            self.draw_input_line(scr, 1, y, maxx);
        } else {
            self.draw_game(scr);
        }
        if self.popup != Popup::None {
            self.draw_popup(scr);
        }
    }

    fn draw_title(&self, scr: &mut Screen) {
        let f = self.frame.as_ref().unwrap();
        scr.fill(0, 0, scr.w as i32, 1, ' ', Color::Reset);
        scr.text(1, 0, "NETREK", Color::White, true);
        let mut x = 9;
        let host = format!("{}:{}", self.cfg.host, self.cfg.port);
        scr.text(x, 0, &host, DIM, false);
        x += host.len() as i32 + 2;
        if let Some(me) = self.me() {
            let who = format!("{}{} {}", me.team.letter(), slot_char(me.id), me.name);
            let c = if me.state == PState::Outfit { Color::Grey } else { team_color(me.team) };
            scr.text(x, 0, &who, c, true);
            x += who.chars().count() as i32 + 2;
        }
        if scr.w as i32 - x < 38 {
            return;
        }
        // Planet counts per empire, right-aligned.
        let mut right = scr.w as i32 - 1;
        for t in Team::PLAYABLE.iter().rev() {
            let n = f.team_planets[t.idx() - 1];
            let s = format!("{} {:>2}", t.abbr(), n);
            right -= s.len() as i32 + 2;
            scr.text(right, 0, &s, team_color(*t), true);
        }
        scr.text(right - 9, 0, "planets:", DIM, false);
    }

    /// Classic Netrek layout: tactical and galactic as two equal squares across
    /// the top; gauges below the tactical, warnings, talk line and messages
    /// below the galactic, and the player list in a side column when the
    /// window is wide enough. Sized in real pixels so the maps stay square.
    fn draw_game(&mut self, scr: &mut Screen) {
        use super::Gfx;
        let (w, h) = (scr.w as i32, scr.h as i32);
        let (cw, ch) = self.cell_px;
        let bottom_min = 7;
        let max_h = ((h - bottom_min - 2).max(6)) as f64 * ch;
        let full = max_h.min(((w - 4) / 2) as f64 * cw);
        // Prefer keeping a player-list column if the maps shrink only a little.
        let with_list = max_h.min(((w - 4 - 34) / 2) as f64 * cw);
        let side_px = if with_list >= full * 0.8 { with_list } else { full };
        let sc = ((side_px / cw).floor() as i32).max(8);
        let sr = ((side_px / ch).floor() as i32).max(4);
        let tac = Rect { x: 1, y: 1, w: sc, h: sr };
        let gx = sc + 2;
        let gal = Rect { x: gx + 1, y: 1, w: sc, h: sr };
        let maps_right = 2 * sc + 4;
        let side_col = w - maps_right >= 24;

        let f = self.frame.as_ref().unwrap();
        let me = self.me().cloned();
        let center = match &me {
            Some(p) if p.state != PState::Outfit => (p.x as f64, p.y as f64),
            _ => self.last_center,
        };
        let vector = self.gfx == Gfx::Vector && self.popup == Popup::None;
        let cpx = match self.gfx {
            Gfx::Vector => self.cell_px,
            Gfx::Blocks => (2.0, self.logical_per_row()),
            Gfx::Braille => (2.0, 4.0),
        };
        let (pw, ph) = ((tac.w as f64 * cpx.0) as i32, (tac.h as f64 * cpx.1) as i32);
        let upd = TAC_WIDTH * self.zoom / pw as f64;
        self.layout = Layout { tac, gal, center, upd, cpx };

        // Tactical border shows alert status, like the original client.
        let alert = match &me {
            Some(p) if p.state == PState::Alive => {
                let near = f
                    .players
                    .iter()
                    .filter(|q| q.state == PState::Alive && q.team != p.team && !q.fuzzy)
                    .map(|q| (((q.x - p.x) as f64).powi(2) + ((q.y - p.y) as f64).powi(2)).sqrt())
                    .fold(f64::MAX, f64::min);
                if near < 7000.0 {
                    Color::Red
                } else if near < 14000.0 {
                    Color::Yellow
                } else {
                    Color::Green
                }
            }
            _ => Color::DarkRed,
        };
        let tac_title = if self.zoom != 1.0 { format!("×{:.1}", 1.0 / self.zoom) } else { String::new() };
        scr.frame(0, 0, sc + 2, sr + 2, &tac_title, alert);
        scr.frame(gx, 0, sc + 2, sr + 2, "", Color::White);
        let (vw, vh) = (upd * pw as f64, upd * ph as f64);
        if vector {
            let mut img = self.draw_tactical_vec(pw, ph, center, upd);
            if let Some(b) = &f.banner {
                let fs = (ch as f32 * 0.8).clamp(10.0, 18.0);
                img.text_centered(&self.text, pw as f32 / 2.0, ph as f32 / 2.0, b, fs, [255.0, 255.0, 255.0]);
            }
            self.images.push(super::Image { x: tac.x, y: tac.y, slot: 0, data: super::sixel::encode_canvas(&img) });
            scr.masks.push((tac.x, tac.y, tac.w, tac.h));
            // The galaxy map changes slowly; refresh it a few times a second.
            if self.sent[1].is_none() || self.last_gal.elapsed() >= Duration::from_millis(500) {
                let (gw, gh) = ((gal.w as f64 * cw) as i32, (gal.h as f64 * ch) as i32);
                let img = self.draw_galactic_vec(gw, gh, center, vw, vh);
                self.images.push(super::Image { x: gal.x, y: gal.y, slot: 1, data: super::sixel::encode_canvas(&img) });
                self.last_gal = std::time::Instant::now();
            }
            scr.masks.push((gal.x, gal.y, gal.w, gal.h));
        } else {
            self.sent = [None; 3];
            if self.gfx == Gfx::Braille {
                self.draw_tactical(scr, tac, center, upd);
                self.draw_galactic(scr, gal, center, vw, vh);
            } else {
                self.draw_tactical_px(scr, tac, center, upd);
                self.draw_galactic_px(scr, gal, center, vw, vh);
            }
            if let Some(b) = &f.banner {
                let bx = tac.x + (tac.w - b.chars().count() as i32) / 2;
                scr.text(bx.max(tac.x), tac.y + tac.h / 2, b, Color::White, true);
            }
        }

        // Bottom left: gauges (and the player list if there's no side column).
        let by = sr + 2;
        let bh = h - by;
        scr.frame(0, by, sc + 2, bh, "", Color::White);
        let inner = Rect { x: 1, y: by + 1, w: sc, h: bh - 2 };
        let list_top;
        if vector {
            // Controls drawn as a vector image; leave room for the list if needed.
            let dash_rows = if side_col { inner.h } else { inner.h.min(((5.5 * 1.75 * 0.62 * ch) / ch).ceil() as i32 + 1) };
            let img = self.draw_dashboard_vec((inner.w as f64 * cw) as i32, (dash_rows as f64 * ch) as i32);
            self.images.push(super::Image { x: inner.x, y: inner.y, slot: 2, data: super::sixel::encode_canvas(&img) });
            scr.masks.push((inner.x, inner.y, inner.w, dash_rows));
            list_top = inner.y + dash_rows;
        } else {
            self.draw_gauges(scr, inner);
            list_top = by + 5;
        }
        if !side_col && by + bh - 1 - list_top > 1 {
            self.draw_player_list(scr, Rect { x: 1, y: list_top, w: sc, h: by + bh - 1 - list_top }, true);
        }

        // Bottom right: warning line, talk line, message log.
        let right_end = if side_col { maps_right } else { w };
        scr.frame(gx, by, right_end - gx, bh, "", Color::White);
        let rx = gx + 1;
        let rw = right_end - gx - 2;
        if let Some((text, t)) = &self.warning {
            if t.elapsed() < Duration::from_secs(if self.tmux_hint { 12 } else { 5 }) {
                scr.text_clip(rx, by + 1, text, Color::Yellow, true, rx + rw - 1);
            }
        }
        self.draw_input_line(scr, rx, by + 2, rx + rw - 1);
        for x in rx..rx + rw {
            scr.put(x, by + 3, '─', DIM, false);
        }
        self.draw_messages(scr, Rect { x: rx, y: by + 4, w: rw, h: bh - 5 });

        // Side column: the player list, full height.
        if side_col {
            scr.frame(maps_right, 0, w - maps_right, h, "", Color::White);
            self.draw_player_list(scr, Rect { x: maps_right + 1, y: 1, w: w - maps_right - 2, h: h - 2 }, true);
        }
    }

    fn draw_tactical(&self, scr: &mut Screen, r: Rect, center: (f64, f64), upd: f64) {
        let f = self.frame.as_ref().unwrap();
        let mut b = Braille::new(r.x, r.y, r.w, r.h);
        let (dw, dh) = b.dots();
        let (cx, cy) = (dw as f64 / 2.0, dh as f64 / 2.0);
        let to_dot = |x: f64, y: f64| (cx + (x - center.0) / upd, cy + (y - center.1) / upd);
        let half_w = dw as f64 / 2.0 * upd;
        let half_h = dh as f64 / 2.0 * upd;
        let visible = |x: f64, y: f64, margin: f64| {
            (x - center.0).abs() < half_w + margin && (y - center.1).abs() < half_h + margin
        };
        let mut labels: Vec<(i32, i32, String, Color, bool)> = Vec::new();

        // Star field, fixed in galaxy space so you can feel your motion.
        let cell = 1600.0;
        let (x0, x1) = (((center.0 - half_w) / cell).floor() as i64, ((center.0 + half_w) / cell).ceil() as i64);
        let (y0, y1) = (((center.1 - half_h) / cell).floor() as i64, ((center.1 + half_h) / cell).ceil() as i64);
        for gy in y0..=y1 {
            for gx in x0..=x1 {
                let hv = hash(gx, gy);
                if hv % 3 != 0 {
                    continue;
                }
                let sx = gx as f64 * cell + (hv >> 8) as f64 % cell;
                let sy = gy as f64 * cell + (hv >> 24) as f64 % cell;
                if !(0.0..=GWIDTH).contains(&sx) || !(0.0..=GWIDTH).contains(&sy) {
                    continue;
                }
                let (dx, dy) = to_dot(sx, sy);
                b.dotf(dx, dy, DIM, 0);
            }
        }

        // Edge of the galaxy.
        for (ax, ay, bx, by) in [
            (0.0, 0.0, GWIDTH, 0.0),
            (GWIDTH, 0.0, GWIDTH, GWIDTH),
            (GWIDTH, GWIDTH, 0.0, GWIDTH),
            (0.0, GWIDTH, 0.0, 0.0),
        ] {
            let (p0, p1) = (to_dot(ax, ay), to_dot(bx, by));
            let (p0x, p0y) = (p0.0.clamp(-5.0, dw as f64 + 5.0), p0.1.clamp(-5.0, dh as f64 + 5.0));
            let (p1x, p1y) = (p1.0.clamp(-5.0, dw as f64 + 5.0), p1.1.clamp(-5.0, dh as f64 + 5.0));
            b.line_pattern(p0x, p0y, p1x, p1y, Color::DarkRed, 1, 2);
        }

        // Planets.
        let pr = (700.0 / upd).max(2.5);
        for (k, def) in PLANETS.iter().enumerate() {
            if !visible(def.x, def.y, 3000.0) {
                continue;
            }
            let info = &f.planets[k];
            let col = if info.known { self.planet_color(info) } else { DIM };
            let (px, py) = to_dot(def.x, def.y);
            b.circle(px, py, pr, col, 2);
            if info.flags & PL_HOME != 0 {
                b.circle(px, py, pr * 0.55, col, 2);
            }
            if info.known && info.flags & PL_REPAIR != 0 {
                b.line(px - pr * 0.4, py, px + pr * 0.4, py, col, 2);
                b.line(px, py - pr * 0.4, px, py + pr * 0.4, col, 2);
            }
            let lx = (px / 2.0) as i32;
            let ly = ((py + pr) / 4.0) as i32 + 1;
            labels.push((lx - def.name.len() as i32 / 2, ly, def.name.to_string(), col, info.known && info.armies > 4));
            if info.known {
                let mut tag = format!("{}", info.armies);
                if info.flags & PL_REPAIR != 0 {
                    tag.push('R');
                }
                if info.flags & PL_FUEL != 0 {
                    tag.push('F');
                }
                if info.flags & PL_AGRI != 0 {
                    tag.push('A');
                }
                labels.push((lx - tag.len() as i32 / 2, ly + 1, tag, DIM, false));
            }
        }

        // Tholian webs.
        for w in &f.webs {
            let (ax, ay) = to_dot(w.x1 as f64, w.y1 as f64);
            let (bx, by) = to_dot(w.x2 as f64, w.y2 as f64);
            b.line(ax, ay, bx, by, Color::DarkYellow, 3);
        }

        // Phasers.
        for ph in &f.phasers {
            let owner = f.players.iter().find(|p| p.id == ph.owner);
            let col = if ph.owner == self.slot { Color::White } else { owner.map_or(Color::White, |p| self.player_color(p)) };
            let (ax, ay) = to_dot(ph.x1 as f64, ph.y1 as f64);
            let (bx, by) = to_dot(ph.x2 as f64, ph.y2 as f64);
            b.line_pattern(ax, ay, bx, by, col, 6, if ph.hit { 1 } else { 2 });
        }

        // Tractor and pressor beams.
        for p in f.players.iter().filter(|p| p.state == PState::Alive) {
            if let Some(t) = p.tractor_target.and_then(|t| f.players.iter().find(|q| q.id == t)) {
                let (ax, ay) = to_dot(p.x as f64, p.y as f64);
                let (bx, by) = to_dot(t.x as f64, t.y as f64);
                let col = if p.flags & pf::PRESSOR != 0 { Color::Magenta } else { Color::Green };
                b.line_pattern(ax, ay, bx, by, col, 3, 3);
            }
        }

        // Torpedoes.
        for t in &f.torps {
            if !visible(t.x as f64, t.y as f64, 500.0) {
                continue;
            }
            let (x, y) = to_dot(t.x as f64, t.y as f64);
            let mine = t.owner == self.slot;
            let col = if mine { Color::White } else { f.players.iter().find(|p| p.id == t.owner).map_or(team_color(t.team), |p| self.player_color(p)) };
            if t.explode > 0 {
                let r = t.explode as f64 * 0.9 * (DAMDIST / 2000.0) / (upd / 140.0).max(0.5);
                b.arc(x, y, r.min(10.0), if t.explode % 2 == 0 { Color::Yellow } else { Color::Red }, 5, 2);
            } else if t.kind == TorpKind::Plasma {
                b.disc(x, y, 1.5, col, 5);
                b.circle(x, y, 3.0, col, 5);
            } else {
                b.dotf(x, y, col, 4);
                b.dotf(x + 1.0, y, col, 4);
                b.dotf(x - 1.0, y, col, 4);
                b.dotf(x, y + 1.0, col, 4);
                b.dotf(x, y - 1.0, col, 4);
            }
        }

        // Ships.
        let sr = (4.5 / self.zoom.sqrt()).clamp(2.5, 7.0);
        for p in f.players.iter() {
            if p.state == PState::Outfit || p.state == PState::Dead || p.fuzzy {
                continue;
            }
            if !visible(p.x as f64, p.y as f64, 1000.0) {
                continue;
            }
            let (x, y) = to_dot(p.x as f64, p.y as f64);
            let is_me = p.id == self.slot;
            if p.state == PState::Exploding {
                let fr = p.explode_frame as f64;
                let col = if p.explode_frame % 2 == 0 { Color::Yellow } else { Color::Red };
                b.arc(x, y, fr * 1.6, col, 7, 1);
                b.arc(x, y, fr * 0.8, Color::White, 7, 2);
                for k in 0..10 {
                    let a = k as f64 * 0.63 + fr * 0.2;
                    b.dotf(x + a.cos() * fr * 2.4, y + a.sin() * fr * 2.4, col, 7);
                }
                continue;
            }
            let col = if is_me { Color::White } else { self.player_color(p) };
            let cloaked = p.flags & pf::CLOAK != 0;
            let a = p.dir as f64 * std::f64::consts::TAU / 256.0;
            let (sa, ca) = (a.sin(), a.cos());
            let pts: Vec<(f64, f64)> = ship_shape(p.ship)
                .iter()
                .map(|&(px, py)| (x + (px * ca - py * sa) * sr, y + (px * sa + py * ca) * sr))
                .collect();
            let prio = if is_me { 8 } else { 6 };
            let (scol, step) = if cloaked { (DIM, 2) } else { (col, 1) };
            for k in 0..pts.len() {
                let (p0, p1) = (pts[k], pts[(k + 1) % pts.len()]);
                b.line_pattern(p0.0, p0.1, p1.0, p1.1, scol, prio, step);
            }
            if p.ship == ShipType::Starbase {
                b.circle(x, y, sr * 0.4, scol, prio);
            }
            if p.flags & pf::SHIELD != 0 && !cloaked {
                let shield_col = if is_me {
                    let frac = f.me_info.shield as f64 / p.ship.stats().max_shield;
                    match level_color(frac, true) {
                        Color::Green => Color::Blue,
                        c => c,
                    }
                } else {
                    col
                };
                b.arc(x, y, sr + 2.5, shield_col, prio - 1, 1);
            }
            let tag = super::render_px::callsign(p);
            labels.push((((x + sr + 3.0) / 2.0) as i32, (y / 4.0) as i32, tag, if cloaked { DIM } else { self.player_color(p) }, is_me));
        }

        b.blit(scr);
        for (lx, ly, text, col, bold) in labels {
            for (k, ch) in text.chars().enumerate() {
                let xx = lx + k as i32;
                if xx >= 0 && xx < r.w && ly >= 0 && ly < r.h {
                    scr.put(r.x + xx, r.y + ly, ch, col, bold);
                }
            }
        }

        // Mouse pointer crosshair.
        if let Some((px, py)) = self.pointer {
            if r.contains(px, py) {
                scr.put(px, py, '+', Color::Magenta, true);
            }
        }
    }

    fn draw_galactic(&self, scr: &mut Screen, r: Rect, center: (f64, f64), view_w: f64, view_h: f64) {
        let f = self.frame.as_ref().unwrap();
        let mut b = Braille::new(r.x, r.y, r.w, r.h);
        let (dw, dh) = b.dots();
        let sx = dw as f64 / GWIDTH;
        let sy = dh as f64 / GWIDTH;
        let to_dot = |x: f64, y: f64| (x * sx, y * sy);

        // Quadrant guides.
        b.line_pattern(dw as f64 / 2.0, 0.0, dw as f64 / 2.0, dh as f64, DIM, 0, 6);
        b.line_pattern(0.0, dh as f64 / 2.0, dw as f64, dh as f64 / 2.0, DIM, 0, 6);

        // Tactical window outline.
        let (ax, ay) = to_dot(center.0 - view_w / 2.0, center.1 - view_h / 2.0);
        let (bx, by) = to_dot(center.0 + view_w / 2.0, center.1 + view_h / 2.0);
        b.line_pattern(ax, ay, bx, ay, DIM, 1, 2);
        b.line_pattern(bx, ay, bx, by, DIM, 1, 2);
        b.line_pattern(bx, by, ax, by, DIM, 1, 2);
        b.line_pattern(ax, by, ax, ay, DIM, 1, 2);

        let mut labels: Vec<(i32, i32, String, Color, bool)> = Vec::new();
        for (k, def) in PLANETS.iter().enumerate() {
            let info = &f.planets[k];
            let col = if info.known { self.planet_color(info) } else { DIM };
            let (x, y) = to_dot(def.x, def.y);
            b.disc(x, y, 0.8, col, 2);
            let abbr: String = def.name.chars().take(3).collect();
            let cx = (x / 2.0) as i32;
            let cy = (y / 4.0) as i32;
            labels.push((cx + 1, cy, abbr, col, info.known && info.armies > 4));
        }
        for p in f.players.iter() {
            if p.state != PState::Alive {
                continue;
            }
            let (x, y) = to_dot(p.x as f64, p.y as f64);
            let (cx, cy) = ((x / 2.0) as i32, (y / 4.0) as i32);
            let is_me = p.id == self.slot;
            let (text, col) = if p.fuzzy {
                ("??".to_string(), DIM)
            } else {
                (super::render_px::callsign(p), if is_me { Color::White } else { self.player_color(p) })
            };
            labels.push((cx, cy, text, col, true));
        }
        b.blit(scr);
        for (lx, ly, text, col, bold) in labels {
            let lx = lx.clamp(0, (r.w - text.chars().count() as i32).max(0));
            for (k, ch) in text.chars().enumerate() {
                let xx = lx + k as i32;
                if xx < r.w && ly >= 0 && ly < r.h {
                    scr.put(r.x + xx, r.y + ly, ch, col, bold);
                }
            }
        }
        if let Some((px, py)) = self.pointer {
            if r.contains(px, py) {
                scr.put(px, py, '+', Color::Magenta, true);
            }
        }
    }

    /// The classic dashboard: a row of ramp gauges, then weapons and status.
    fn draw_gauges(&self, scr: &mut Screen, r: Rect) {
        let f = self.frame.as_ref().unwrap();
        let Some(me) = self.me() else { return };
        let s = me.ship.stats();
        let mi = &f.me_info;
        let maxx = r.x + r.w - 1;
        let gw = ((r.w - 4) / 5).clamp(8, 22);
        let gauges: [(&str, f64, u32, u32); 5] = [
            ("Spd", mi.speed as f64 / s.max_speed as f64, mi.speed as u32, mi.max_speed_now as u32),
            ("Ful", mi.fuel as f64 / s.max_fuel, mi.fuel, s.max_fuel as u32),
            ("Shl", mi.shield as f64 / s.max_shield, mi.shield, s.max_shield as u32),
            ("Dam", mi.damage as f64 / s.max_damage, mi.damage, s.max_damage as u32),
            ("Egn", mi.etemp as f64 / 100.0, mi.etemp, 100),
        ];
        for (k, (label, frac, val, max)) in gauges.iter().enumerate() {
            let gx = r.x + k as i32 * (gw + 1);
            if gx + gw > maxx + 1 {
                break;
            }
            let v = val.to_string();
            scr.text(gx, r.y, &v, Color::White, true);
            scr.text_clip(gx + v.len() as i32, r.y, &format!("/{}", max), DIM, false, gx + gw - 1);
            self.draw_ramp(scr, gx, r.y + 1, gw, *frac, label);
        }
        let shields = if me.flags & pf::SHIELD != 0 { "shields UP" } else { "shields down" };
        let wpn = if me.flags & pf::WEAPON_HOT != 0 { "Wpn: OVERHEATED".to_string() } else { format!("Wpn: {}/100", mi.wtemp) };
        let mut line = format!(
            "{}  {}  kills {:.2}  armies {}/{}  torps {}/8  {}{}",
            wpn,
            shields,
            mi.kills,
            mi.armies,
            mi.max_armies_now,
            mi.torps_out,
            s.abbr,
            if me.flags & pf::CLOAK != 0 { "  CLOAKED" } else { "" }
        );
        if let Some(k) = mi.orbiting {
            line += &format!("  orbiting {}", PLANETS[k as usize].name);
        }
        if let Some(l) = &mi.lock {
            line += &format!("  lock {}", l);
        }
        for (flag, name) in [
            (pf::REPAIR, "REPAIR"),
            (pf::BOMB, "BOMB"),
            (pf::BEAMUP, "BEAM-UP"),
            (pf::BEAMDOWN, "BEAM-DOWN"),
            (pf::TRACTOR, "TRACTOR"),
            (pf::PRESSOR, "PRESSOR"),
        ] {
            if me.flags & flag != 0 {
                line += "  ";
                line += name;
            }
        }
        scr.text_clip(r.x, r.y + 3, &line, Color::Grey, false, maxx);
        let secs = f.tick / UPS as u32;
        let clock = format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
        let cx = r.x + 5 * (gw + 1);
        if cx + clock.len() as i32 <= maxx {
            scr.text(cx, r.y + 1, &clock, Color::White, false);
        }
    }

    /// A triangular ramp gauge (green → yellow → red), two rows tall.
    fn draw_ramp(&self, scr: &mut Screen, x: i32, y: i32, w: i32, frac: f64, label: &str) {
        use super::pixels::{mix, rgb, to_color, Pixels};
        let (pw, ph) = (w * 2, 8);
        let mut px = Pixels::new(pw, ph);
        let tc = self.truecolor;
        let (green, yellow, red) = (rgb(0x20c040), rgb(0xf0e020), rgb(0xe02020));
        let frac = frac.clamp(0.0, 1.0) as f32;
        px.fill_with(|xx, yy| {
            let t = xx as f32 / pw as f32;
            let top = ph as f32 * (1.0 - t * 0.9) - 0.5;
            if (yy as f32 - 0.5) < top {
                return [0.0, 0.0, 0.0];
            }
            if t <= frac {
                if t < 0.5 {
                    mix(green, yellow, t * 2.0)
                } else {
                    mix(yellow, red, (t - 0.5) * 2.0)
                }
            } else {
                [40.0, 42.0, 48.0]
            }
        });
        px.blit(scr, x, y, tc);
        let lx = x + w - label.len() as i32 - 1;
        for (k, ch) in label.chars().enumerate() {
            let bg = px.cell_color((lx - x) + k as i32, 1);
            scr.put_cell(lx + k as i32, y + 1, ch, to_color([255.0, 255.0, 255.0], tc), to_color(bg, tc), true);
        }
    }

    fn draw_player_list(&self, scr: &mut Screen, r: Rect, _wide: bool) {
        let f = self.frame.as_ref().unwrap();
        let mut ps: Vec<&PlayerInfo> = f.players.iter().filter(|p| p.state != PState::Outfit).collect();
        ps.sort_by_key(|p| (p.team.idx(), p.id));
        let maxx = r.x + r.w - 1;
        if r.h < 1 {
            return;
        }
        let header = format!("{:<4}{:<3}{:<16}{:>7}{:>6}  {}", "No", "Ty", "Name", "Kills", "Arm", "");
        scr.text_clip(r.x, r.y, &header, Color::White, true, maxx);
        for (k, p) in ps.iter().enumerate() {
            let y = r.y + 1 + k as i32;
            if y >= r.y + r.h {
                break;
            }
            let arm = if p.armies > 0 { p.armies.to_string() } else { String::new() };
            let status = match p.state {
                PState::Alive if p.flags & pf::ROBOT != 0 => "robot",
                PState::Alive => "",
                _ => "dead",
            };
            let line = format!(
                "{:<4}{:<3}{:<16.16}{:>7.2}{:>6}  {}",
                super::render_px::callsign(p),
                p.ship.stats().abbr,
                p.name,
                p.kills,
                arm,
                status
            );
            let col = if p.state == PState::Alive { self.player_color(p) } else { DIM };
            scr.text_clip(r.x, y, &line, col, p.id == self.slot, maxx);
        }
    }

    fn draw_messages(&self, scr: &mut Screen, r: Rect) {
        let my_team = self.me().map(|p| p.team).unwrap_or(Team::Ind);
        let n = r.h as usize;
        let start = self.msgs.len().saturating_sub(n);
        for (k, m) in self.msgs.iter().skip(start).enumerate() {
            let col = match m.kind {
                MsgKind::System if m.from == "ALERT" => Color::Magenta,
                MsgKind::System => Color::Grey,
                MsgKind::All => Color::White,
                MsgKind::Team => team_color(my_team),
                MsgKind::Indiv => Color::Cyan,
            };
            let line = format!("{:<8} {}", m.from, m.text);
            scr.text_clip(r.x, r.y + k as i32, &line, col, m.kind != MsgKind::System || m.from == "ALERT", r.x + r.w - 1);
        }
    }

    fn draw_input_line(&self, scr: &mut Screen, x: i32, y: i32, maxx: i32) {
        let in_game = self.my_state() != PState::Outfit;
        match &self.mode {
            Mode::Compose { target: None, .. } => {
                scr.text_clip(x, y, "Send to: [A]ll [T]eam [F/R/K/O] a team [0-9a-v] a player (Esc cancels)", Color::Yellow, true, maxx);
            }
            Mode::Compose { target: Some(t), text } => {
                let to = match t {
                    MsgTarget::All => "ALL".to_string(),
                    MsgTarget::Team(t) => t.abbr().to_string(),
                    MsgTarget::Player(p) => format!("player {}", slot_char(*p)),
                };
                scr.text_clip(x, y, &format!("To {}> {}█", to, text), Color::White, true, maxx);
            }
            Mode::Refit => {
                scr.text_clip(x, y, "Refit to: [s]cout [d]estroyer [c]ruiser [b]attleship [a]ssault [x] starbase", Color::Yellow, true, maxx);
            }
            Mode::ConfirmQuit => {
                scr.text_clip(x, y, "Really quit Netrek? (y/n)", Color::Red, true, maxx);
            }
            Mode::Play if in_game => {
                scr.text_clip(x, y, "Talk to everyone: press m, then A and type. Press ? for help.", Color::Red, false, maxx);
            }
            Mode::Play => {
                if let Some((w, t)) = &self.warning {
                    if t.elapsed() < Duration::from_secs(5) {
                        scr.text_clip(x, y, w, Color::Yellow, true, maxx);
                        return;
                    }
                }
                let hint = "f/r/k/o team • s/d/c/b/a/x ship • Enter launch • m message • ? help • q quit";
                scr.text_clip(x, y, hint, DIM, false, maxx);
            }
        }
    }

    fn draw_outfit(&mut self, scr: &mut Screen) {
        let (w, h) = (scr.w as i32, scr.h as i32);
        let f = self.frame.as_ref().unwrap();
        self.layout = Layout::default();
        let mut y = 2;
        let logo_w = LOGO[0].chars().count() as i32;
        if h >= 34 && w >= logo_w + 2 {
            for (k, line) in LOGO.iter().enumerate() {
                let col = [Color::Yellow, Color::Red, Color::Green, Color::Cyan][k % 4];
                scr.text((w - logo_w) / 2, y, line, col, true);
                y += 1;
            }
        } else {
            let t = "N E T R E K";
            scr.text((w - t.len() as i32) / 2, y, t, Color::White, true);
            y += 1;
        }
        let sub = "a multi-player space battle game — 1988 to forever";
        scr.text((w - sub.len() as i32) / 2, y, sub, DIM, false);
        y += 2;

        // Team cards.
        let card_w = ((w - 4) / 4).min(26);
        let total_w = card_w * 4;
        let x0 = (w - total_w) / 2;
        let default_team = self.outfit_team;
        for (k, t) in Team::PLAYABLE.iter().enumerate() {
            let x = x0 + k as i32 * card_w;
            let open = f.open_teams.contains(t);
            let chosen = default_team == Some(*t);
            let col = if !open { DIM } else { team_color(*t) };
            let title = format!("{}{}", if chosen { "▶ " } else { "" }, t.name());
            scr.frame(x, y, card_w - 1, 6, &title, col);
            let players = f.players.iter().filter(|p| p.team == *t && p.state != PState::Outfit).count();
            let planets = f.team_planets[t.idx() - 1];
            scr.text(x + 2, y + 1, &format!("key: {}", t.letter().to_ascii_lowercase()), col, chosen);
            scr.text(x + 2, y + 2, &format!("players: {}", players), col, false);
            scr.text(x + 2, y + 3, &format!("planets: {}", planets), col, false);
            scr.text(x + 2, y + 4, if open { "open" } else { "closed" }, col, false);
        }
        y += 7;

        // Ship table.
        let header = format!(
            "   {:<12}{:>6}{:>9}{:>6}{:>7}{:>8}{:>7}{:>8}",
            "ship", "speed", "shields", "hull", "fuel", "armies", "torp", "phaser"
        );
        let tx = (w - header.len() as i32) / 2;
        scr.text(tx, y, &header, Color::Grey, true);
        y += 1;
        for st in ShipType::ALL {
            let s = st.stats();
            let chosen = st == self.outfit_ship;
            let taken = st == ShipType::Starbase
                && default_team.map_or(false, |t| f.starbase_teams.contains(&t));
            let line = format!(
                "{} {} {:<10}{:>6}{:>9}{:>6}{:>7}{:>8}{:>7}{:>8}",
                if chosen { "▶" } else { " " },
                st.key(),
                s.name,
                s.max_speed,
                s.max_shield,
                s.max_damage,
                s.max_fuel,
                s.max_armies,
                s.torp_damage,
                s.phaser_damage
            );
            let col = if taken { DIM } else if chosen { Color::White } else { Color::Grey };
            scr.text(tx, y, &line, col, chosen);
            y += 1;
        }
        y += 1;
        let help = "Choose a team (f r k o) and a ship (s d c b a x), then press Enter to launch.";
        scr.text((w - help.len() as i32).max(0) / 2, y, help, Color::White, true);
        y += 1;
        for m in &self.motd {
            if y >= h - 7 {
                break;
            }
            scr.text((w - m.len() as i32).max(0) / 2, y, m, DIM, false);
            y += 1;
        }
        // Recent messages at the bottom.
        let msg_h = 4;
        let my = h - msg_h - 3;
        if my > y {
            scr.frame(0, my, w, msg_h + 2, "Messages", BORDER);
            self.draw_messages(scr, Rect { x: 1, y: my + 1, w: w - 2, h: msg_h });
        }
    }

    fn popup_box(&self, scr: &mut Screen, want_w: i32, want_h: i32, title: &str) -> Rect {
        let (w, h) = (scr.w as i32, scr.h as i32);
        let bw = want_w.min(w - 2);
        let bh = want_h.min(h - 2);
        let x = (w - bw) / 2;
        let y = (h - bh) / 2;
        scr.fill(x, y, bw, bh, ' ', Color::Reset);
        scr.frame(x, y, bw, bh, title, Color::White);
        Rect { x: x + 2, y: y + 1, w: bw - 4, h: bh - 2 }
    }

    fn draw_popup(&self, scr: &mut Screen) {
        match self.popup {
            Popup::Help => {
                let lines = HELP;
                let r = self.popup_box(scr, 76, lines.len() as i32 + 3, "Help — press ? or Esc to close");
                for (k, (keys, what)) in lines.iter().enumerate() {
                    if k as i32 >= r.h {
                        break;
                    }
                    scr.text(r.x, r.y + k as i32, keys, Color::Yellow, true);
                    scr.text_clip(r.x + 18, r.y + k as i32, what, Color::Grey, false, r.x + r.w - 1);
                }
            }
            Popup::Players => {
                let n = self.frame.as_ref().map_or(0, |f| f.players.iter().filter(|p| p.state != PState::Outfit).count());
                let r = self.popup_box(scr, 64, n as i32 + 2, "Players — L or Esc to close");
                self.draw_player_list(scr, r, true);
            }
            Popup::Planets => {
                let f = self.frame.as_ref().unwrap();
                let r = self.popup_box(scr, 72, 45, "Planets — P or Esc to close");
                let cols = if r.w >= 68 { 2 } else { 1 };
                let per_col = (40 + cols - 1) / cols;
                for (k, def) in PLANETS.iter().enumerate() {
                    let (c, row) = (k as i32 / per_col, k as i32 % per_col);
                    if row >= r.h {
                        continue;
                    }
                    let info = &f.planets[k];
                    let line = if info.known {
                        format!(
                            "{:<15}{} {:>3} {}{}{}",
                            def.name,
                            info.owner.letter(),
                            info.armies,
                            if info.flags & PL_REPAIR != 0 { 'R' } else { ' ' },
                            if info.flags & PL_FUEL != 0 { 'F' } else { ' ' },
                            if info.flags & PL_AGRI != 0 { 'A' } else { ' ' }
                        )
                    } else {
                        format!("{:<15}?   ?", def.name)
                    };
                    let col = if info.known { self.planet_color(info) } else { DIM };
                    scr.text_clip(r.x + c * 34, r.y + row, &line, col, false, r.x + r.w - 1);
                }
            }
            Popup::None => {}
        }
    }
}

const HELP: &[(&str, &str)] = &[
    ("mouse", "move to aim; left click torp, right click steer, middle click phaser"),
    ("k", "steer toward the mouse pointer (arrows ← → also turn)"),
    ("0-9 ) ! @", "set speed 0-9, 10, 11, 12   % max speed   # half speed   ↑ ↓ adjust"),
    ("t / p / f", "fire photon torpedo / phaser / plasma toward the pointer"),
    ("s  (or u)", "toggle shields"),
    ("c", "toggle cloak (costs fuel, can't fire while cloaked)"),
    ("o", "orbit the planet you're next to (warp 2 or less)"),
    ("l", "lock on to the planet or ship nearest the pointer (auto-pilot)"),
    ("b", "bomb enemy armies on the planet you orbit"),
    ("z / x", "beam armies up from a friendly planet / down onto a planet"),
    ("R", "repair mode (stop, shields down, repair faster)"),
    ("T / y", "tractor / pressor beam on the ship nearest the pointer"),
    ("d / D", "detonate nearby enemy torps / your own torps"),
    ("r", "refit to another ship (orbiting your home planet)"),
    ("i", "info on the thing nearest the pointer"),
    ("m", "send a message (then A, T, F/R/K/O or a player slot)"),
    ("L / P", "player list / planet list"),
    ("+ / -", "zoom tactical view (or mouse wheel)"),
    ("g", "cycle graphics: vector / color blocks / braille"),
    ("S", "sound effects on / off"),
    ("Ctrl-L", "redraw screen"),
    ("q", "quit"),
    ("", ""),
    ("How to win", "Kill enemies to earn kills. Kills let you carry armies (2 per kill)."),
    ("", "Bomb enemy planets down to 4 armies, pick up armies from your own"),
    ("", "planets, then beam them down to take enemy planets. Capture every"),
    ("", "planet of the enemy empire to genocide it and conquer the galaxy."),
];
