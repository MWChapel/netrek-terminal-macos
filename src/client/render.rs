//! Everything that draws to the screen.

use super::canvas::Screen;
use super::{App, Layout, Mode, Popup, Rect, IMAGE_SLOTS, POPUP_SLOT, VIEW_SLOT};
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
            Some(f) => super::palette::to_color(super::palette::faction_rgb(f), self.truecolor),
            None => team_color(p.team),
        }
    }

    fn planet_color(&self, info: &PlanetInfo) -> Color {
        match info.alien {
            Some(_) => super::palette::to_color(super::palette::planet_rgb(info), self.truecolor),
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
        let in_game = self.observer || self.my_state() != PState::Outfit;
        if in_game {
            self.draw_game(scr);
        } else {
            self.draw_title(scr);
            self.draw_outfit(scr);
            let (y, maxx) = (scr.h as i32 - 1, scr.w as i32 - 1);
            self.draw_input_line(scr, 1, y, maxx);
        }
        if self.popup != Popup::None {
            if in_game {
                // Keep the maps as images and float the popup over them.
                self.draw_popup_image(scr);
            } else {
                self.draw_popup(scr);
            }
        } else if self.sent[POPUP_SLOT].is_some() {
            // The popup just closed: repaint everything it was covering.
            self.sent = [None; IMAGE_SLOTS];
            scr.repaint();
        }
    }

    /// Size (in cells) and title of the open popup.
    fn popup_size(&self) -> (i32, i32, &'static str) {
        match self.popup {
            Popup::Help => (76, HELP.len() as i32 + 3, "Help — press ? or Esc to close"),
            Popup::Players => {
                let n = self.frame.as_ref().map_or(0, |f| f.players.iter().filter(|p| p.state != PState::Outfit).count());
                let watching = self.frame.as_ref().map_or(false, |f| !f.observers.is_empty());
                (64, n as i32 + 3 + i32::from(watching), "Players — L or Esc to close")
            }
            Popup::Planets => {
                let n = self.frame.as_ref().map_or(40, |f| f.planets.len());
                (72, (n as i32 + 1) / 2 + 4, "Planets — P or Esc to close")
            }
            Popup::None => (0, 0, ""),
        }
    }

    /// In game: the popup as an image floating over the maps.
    fn draw_popup_image(&mut self, scr: &mut Screen) {
        let (want_w, want_h, title) = self.popup_size();
        let (w, h) = (scr.w as i32, scr.h as i32);
        let (bw, bh) = (want_w.min(w - 2), want_h.min(h - 2));
        let (x, y) = ((w - bw) / 2, (h - bh) / 2);
        let (cw, ch) = self.cell_px;
        let img = self.draw_popup_vec((bw as f64 * cw) as i32, (bh as f64 * ch) as i32, title);
        self.images.push(super::Image { x, y, slot: POPUP_SLOT, data: super::sixel::encode_canvas(&img) });
        scr.masks.push((x, y, bw, bh));
    }

    /// Whether two empires are allied (from the server's treaty list).
    pub(super) fn allied(&self, a: Team, b: Team) -> bool {
        self.frame.as_ref().map_or(false, |f| f.treaties.iter().any(|&(x, y)| (x == a && y == b) || (x == b && y == a)))
    }

    /// Extra status rows for the message panel: current orders, and your
    /// alliance and supplies (only when the server has those options on).
    pub(super) fn status_lines(&self) -> Vec<(String, [f32; 3])> {
        let mut out = Vec::new();
        let Some(f) = self.frame.as_ref() else { return out };
        if self.observer {
            let who = match self.me() {
                Some(p) => format!("Observing {} ({})", p.name, super::palette::callsign(p)),
                None => "Observing: free camera".to_string(),
            };
            out.push((format!("{} • Tab / Shift-Tab: follow a ship • arrows: pan • J: join", who), [150.0, 220.0, 255.0]));
        }
        let mi = &f.me_info;
        if let Some((kind, k, secs)) = mi.building {
            out.push((format!("Building a {} on {}: {}s (stay in orbit)", kind.name(), super::planet_name(f, k as usize), secs), [255.0, 200.0, 90.0]));
        }
        if let Some(o) = &mi.order {
            out.push((format!("Orders: {}", o), [255.0, 200.0, 90.0]));
        }
        let mut info = Vec::new();
        if let Some(me) = self.me() {
            if let Some(&(a, b)) = f.treaties.iter().find(|&&(a, b)| a == me.team || b == me.team) {
                let ally = if a == me.team { b } else { a };
                info.push(format!("Allied with the {}", ally.plural()));
            }
        }
        if let Some((stock, levels)) = mi.supply {
            let lv: Vec<String> = UPGRADES.iter().zip(levels).map(|((n, _), l)| format!("{}{}", &n[..1].to_uppercase(), l)).collect();
            info.push(format!("Supplies {} • upgrades {}", stock, lv.join(" ")));
        }
        if !info.is_empty() {
            out.push((info.join("  •  "), [140.0, 210.0, 255.0]));
        }
        if !self.tech_in_controls {
            let ship = self.ship_lines();
            if let [(name, _), (what, _)] = ship.as_slice() {
                out.push((format!("{} ({})", name, what), SHIP_COLOR));
            }
        }
        if !mi.techs.is_empty() && !self.tech_in_controls {
            let v: Vec<String> = self.tech_lines().into_iter().map(|(t, _)| t).collect();
            out.push((format!("Tech: {}", v.join(" • ")), TECH_COLOR));
        }
        out
    }

    /// Your special or relic ship, if you're flying one: its name, then what
    /// makes it special.
    pub(super) fn ship_lines(&self) -> Vec<(String, [f32; 3])> {
        let Some(me) = self.me() else { return Vec::new() };
        let kind = if me.ship.is_relic() {
            "Relic ship"
        } else if me.ship.is_special() {
            "Special ship"
        } else {
            return Vec::new();
        };
        vec![
            (format!("{}: {}", kind, me.ship.stats().name), SHIP_COLOR),
            (me.ship.trait_text().to_string(), [185.0, 190.0, 200.0]),
        ]
    }

    /// Your advanced tech, one line each: the key for active ones, and
    /// whether it's ready (or seconds to go); armor left for ablative armor.
    pub(super) fn tech_lines(&self) -> Vec<(String, [f32; 3])> {
        let Some(f) = self.frame.as_ref() else { return Vec::new() };
        let mi = &f.me_info;
        mi.techs
            .iter()
            .map(|&(t, left)| match t.key() {
                Some(k) if left == 0 => (format!("[{}] {} ready", k, t.name()), [140.0, 230.0, 150.0]),
                Some(k) => (format!("[{}] {} {}s", k, t.name(), left), [150.0, 150.0, 165.0]),
                None if t == Tech::AblativeArmor => (format!("{} {}/{}", t.name(), mi.armor, 40), TECH_COLOR),
                None => (t.name().to_string(), TECH_COLOR),
            })
            .collect()
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

    /// Classic Netrek layout: tactical and galactic as two equal maps across
    /// the top; gauges below the tactical, warnings, talk line and messages
    /// below the galactic, and the player list in a side column when the
    /// window is wide enough. Sized in real pixels so shapes stay true.
    fn draw_game(&mut self, scr: &mut Screen) {
        let (w, h) = (scr.w as i32, scr.h as i32);
        let (cw, ch) = self.cell_px;
        // The controls and messages keep a fixed height; only the maps grow
        // (or shrink) with the window.
        let max_h = ((h - BOTTOM_ROWS - 2).max(6)) as f64 * ch;
        let full = max_h.min(((w - 4) / 2) as f64 * cw);
        // Prefer a player-list column (wide enough for names and ranks) if the
        // maps only shrink a little for it.
        let with_list = max_h.min(((w - 4 - LIST_COLS) / 2) as f64 * cw);
        let side_px = if with_list >= full * 0.75 { with_list } else { full };
        let sc = ((side_px / cw).floor() as i32).max(8);
        // The maps are as wide as fits; their height takes whatever the
        // panels below don't need (up to 1.6x their width), so a tall window
        // shows more of space instead of leaving the panels half empty.
        let square = ((side_px / ch).floor() as i32).max(4);
        let tallest = ((side_px * 1.6 / ch).floor() as i32).max(square);
        let sr = (h - BOTTOM_ROWS - 2).clamp(square, tallest);
        let tac = Rect { x: 1, y: 1, w: sc, h: sr };
        let gx = sc + 2;
        let gal = Rect { x: gx + 1, y: 1, w: sc, h: sr };
        let maps_right = 2 * sc + 4;
        let side_col = w - maps_right >= 24;
        self.tech_in_controls = side_col;

        let f = self.frame.as_ref().unwrap();
        let me = self.me().cloned();
        let center = match &me {
            Some(p) if p.state != PState::Outfit => (p.x as f64, p.y as f64),
            _ => self.last_center,
        };
        let (pw, ph) = ((tac.w as f64 * cw) as i32, (tac.h as f64 * ch) as i32);
        let upd = TAC_WIDTH * self.zoom / pw as f64;
        self.layout = Layout { tac, gal, center, upd, cpx: self.cell_px };

        // Tactical border shows alert status, like the original client.
        let alert = match &me {
            Some(p) if p.state == PState::Alive => {
                let near = f
                    .players
                    .iter()
                    .filter(|q| q.state == PState::Alive && q.team != p.team && !self.allied(q.team, p.team) && !q.fuzzy)
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

        // Bottom left: gauges (and the player list if there's no side column).
        // The panels always have room for their contents; any height the
        // (square) maps can't use goes to them too, as more message history,
        // with the text staying the same size.
        let by = sr + 2;
        let bh = h - by;
        scr.frame(0, by, sc + 2, bh, "", Color::White);
        let inner = Rect { x: 1, y: by + 1, w: sc, h: bh - 2 };
        // The controls, with room left for the player list if it has no column.
        let dash_rows = if side_col { inner.h } else { inner.h.min(DASH_ROWS) };
        let img = self.draw_dashboard_vec((inner.w as f64 * cw) as i32, (dash_rows as f64 * ch) as i32);
        self.images.push(super::Image { x: inner.x, y: inner.y, slot: 2, data: super::sixel::encode_canvas(&img) });
        scr.masks.push((inner.x, inner.y, inner.w, dash_rows));
        let list_top = inner.y + dash_rows;
        if !side_col && by + bh - 1 - list_top > 1 {
            let lh = by + bh - 1 - list_top;
            let img = self.draw_side_list_vec((sc as f64 * cw) as i32, (lh as f64 * ch) as i32);
            self.images.push(super::Image { x: 1, y: list_top, slot: 4, data: super::sixel::encode_canvas(&img) });
            scr.masks.push((1, list_top, sc, lh));
        }

        // Bottom right: warning line, talk line, message log. With a side
        // column the viewscreen sits under the galaxy map and the messages
        // under the player list; otherwise the messages run the full width
        // under the galaxy map.
        let viewscreen = side_col;
        let comms_x = if viewscreen { maps_right } else { gx };
        scr.frame(comms_x, by, w - comms_x, bh, "", Color::White);
        let rx = comms_x + 1;
        let rw = w - comms_x - 2;
        let img = self.draw_comms_vec((rw as f64 * cw) as i32, ((bh - 2) as f64 * ch) as i32);
        self.images.push(super::Image { x: rx, y: by + 1, slot: 3, data: super::sixel::encode_canvas(&img) });
        scr.masks.push((rx, by + 1, rw, bh - 2));
        if viewscreen {
            scr.frame(gx, by, maps_right - gx, bh, "", Color::White);
            let (vw, vh) = (maps_right - gx - 2, bh - 2);
            let img = self.draw_viewscreen_vec((vw as f64 * cw) as i32, (vh as f64 * ch) as i32);
            self.images.push(super::Image { x: gx + 1, y: by + 1, slot: VIEW_SLOT, data: super::sixel::encode_canvas(&img) });
            scr.masks.push((gx + 1, by + 1, vw, vh));
        }
        if side_col {
            scr.frame(maps_right, 0, w - maps_right, sr + 2, "", Color::White);
            let (lw, lh) = (w - maps_right - 2, sr);
            let img = self.draw_side_list_vec((lw as f64 * cw) as i32, (lh as f64 * ch) as i32);
            self.images.push(super::Image { x: maps_right + 1, y: 1, slot: 4, data: super::sixel::encode_canvas(&img) });
            scr.masks.push((maps_right + 1, 1, lw, lh));
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
        // With ranks on (server --ranks), a Rank column after the name.
        let ranks = f.players.iter().any(|p| p.rank.is_some());
        let rank_col = |s: &str| if ranks { format!("{:<5}", s) } else { String::new() };
        let header = format!("{:<4}{:<3}{:<16}{}{:>7}{:>6}  {}", "No", "Ty", "Name", rank_col("Rank"), "Kills", "Arm", "");
        scr.text_clip(r.x, r.y, &header, Color::White, true, maxx);
        for (k, p) in ps.iter().enumerate() {
            let y = r.y + 1 + k as i32;
            if y >= r.y + r.h {
                break;
            }
            let arm = if p.armies > 0 { p.armies.to_string() } else { String::new() };
            let status = match p.state {
                PState::Alive if p.ship == ShipType::Freighter => "convoy",
                PState::Alive if p.flags & pf::ROBOT != 0 => "robot",
                PState::Alive => "",
                _ => "dead",
            };
            let rank = rank_col(p.rank.map_or("", |r| RANKS[r as usize].1));
            let mut line = format!(
                "{:<4}{:<3}{:<16.16}{}{:>7.2}{:>6}",
                super::palette::callsign(p),
                p.ship.stats().abbr,
                p.name,
                rank,
                p.kills,
                arm
            );
            // The robot/convoy note only if it fits whole.
            if !status.is_empty() && line.chars().count() + 2 + status.len() <= r.w as usize {
                line += "  ";
                line += status;
            }
            let col = if p.state == PState::Alive { self.player_color(p) } else { DIM };
            scr.text_clip(r.x, y, &line, col, p.id == self.slot, maxx);
        }
        let y = r.y + 1 + ps.len() as i32;
        if !f.observers.is_empty() && y < r.y + r.h {
            scr.text_clip(r.x, y, &format!("Observers: {}", f.observers.join(", ")), DIM, false, maxx);
        }
    }

    fn draw_messages(&self, scr: &mut Screen, r: Rect) {
        let my_team = self.me().map(|p| p.team).unwrap_or(Team::Ind);
        let rows = r.h.max(0) as usize;
        // Newest at the bottom; long messages wrap under the sender's name.
        let indent = 9usize;
        let mut lines: Vec<(String, Color, bool)> = Vec::new();
        for m in self.msgs.iter().rev() {
            let col = match m.kind {
                MsgKind::System if m.from == "ALERT" => Color::Magenta,
                MsgKind::System => Color::Grey,
                MsgKind::All => Color::White,
                MsgKind::Team => team_color(my_team),
                MsgKind::Indiv => Color::Cyan,
            };
            let bold = m.kind != MsgKind::System || m.from == "ALERT";
            let parts = wrap_chars(&m.text, (r.w as usize).saturating_sub(indent).max(10));
            for (k, part) in parts.iter().enumerate().rev() {
                let head = if k == 0 { format!("{:<8} ", m.from) } else { " ".repeat(indent) };
                lines.push((format!("{}{}", head, part), col, bold));
            }
            if lines.len() >= rows {
                break;
            }
        }
        for (k, (line, col, bold)) in lines.into_iter().take(rows).collect::<Vec<_>>().into_iter().rev().enumerate() {
            scr.text_clip(r.x, r.y + k as i32, &line, col, bold, r.x + r.w - 1);
        }
    }

    /// The current warning, if it hasn't timed out.
    pub(super) fn warning_text(&self) -> Option<(String, [f32; 3])> {
        let (text, t) = self.warning.as_ref()?;
        (t.elapsed() < Duration::from_secs(5)).then(|| (text.clone(), [255.0, 220.0, 80.0]))
    }

    /// The talk/typing line in game, as text and colour (for vector mode).
    pub(super) fn input_line_text(&self) -> (String, [f32; 3]) {
        let (yellow, white, red) = ([255.0, 220.0, 80.0], [255.0, 255.0, 255.0], [240.0, 90.0, 80.0]);
        match &self.mode {
            Mode::Compose { target: None, .. } => {
                ("Send to: [A]ll [T]eam [F/R/K/O] a team [0-9a-v] a player (Esc cancels)".into(), yellow)
            }
            Mode::Compose { target: Some(t), text } => {
                let to = match t {
                    MsgTarget::All => "ALL".to_string(),
                    MsgTarget::Team(t) => t.abbr().to_string(),
                    MsgTarget::Player(p) => format!("player {}", slot_char(*p)),
                };
                (format!("To {}> {}\u{2588}", to, text), white)
            }
            Mode::Refit => ("Refit to: [s]cout [d]estroyer [c]ruiser [b]attleship [a]ssault [x] starbase [e] special [u] relic".into(), yellow),
            Mode::ConfirmQuit => ("Really quit Netrek? (y/n)".into(), red),
            Mode::Play if self.observer => ("Observing. Press m to talk to everyone, J to join the game, ? for help.".into(), red),
            Mode::Play => ("Talk to everyone: press m, then A and type. Press ? for help.".into(), red),
        }
    }

    fn draw_input_line(&self, scr: &mut Screen, x: i32, y: i32, maxx: i32) {
        let in_game = self.observer || self.my_state() != PState::Outfit;
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
                // Keep the end of a long message (where you're typing) in view.
                let prompt = format!("To {}> ", to);
                let room = ((maxx - x + 1) as usize).saturating_sub(prompt.chars().count() + 2).max(8);
                let n = text.chars().count();
                let shown: String = if n > room {
                    format!("…{}", text.chars().skip(n - room + 1).collect::<String>())
                } else {
                    text.clone()
                };
                scr.text_clip(x, y, &format!("{}{}█", prompt, shown), Color::White, true, maxx);
            }
            Mode::Refit => {
                scr.text_clip(x, y, "Refit to: [s]cout [d]estroyer [c]ruiser [b]attleship [a]ssault [x] starbase [e] special [u] relic", Color::Yellow, true, maxx);
            }
            Mode::ConfirmQuit => {
                scr.text_clip(x, y, "Really quit Netrek? (y/n)", Color::Red, true, maxx);
            }
            Mode::Play if self.observer => {
                scr.text_clip(x, y, "Observing. Press m to talk to everyone, J to join the game, ? for help.", Color::Red, false, maxx);
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
                let hint = "f/r/k/o team • s/d/c/b/a/x ship • Enter launch • W watch • m message • ? help • q quit";
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
        // Senior officers: the empire's special ship and alien relics.
        let my_rank = f.players.iter().find(|p| p.id == self.slot).and_then(|p| p.rank);
        let team_now = default_team.unwrap_or(Team::Fed);
        if let Some(sp) = ShipType::special_for(team_now) {
            let s = sp.stats();
            let ok = my_rank.map_or(false, |r| r >= SPECIAL_RANK);
            let chosen = self.outfit_ship.is_special();
            let line = format!(
                "{} e {:<10}{:>6}{:>9}{:>6}{:>7}{:>8}{:>7}{:>8}  {}",
                if chosen { "▶" } else { " " },
                s.name.chars().take(10).collect::<String>(),
                s.max_speed,
                s.max_shield,
                s.max_damage,
                s.max_fuel,
                s.max_armies,
                s.torp_damage,
                s.phaser_damage,
                if ok { "special" } else { "needs Captain + ranks" }
            );
            scr.text(tx, y, &line, if !ok { DIM } else if chosen { Color::White } else { Color::Grey }, chosen);
            y += 1;
        }
        {
            let ok = my_rank.map_or(false, |r| r >= RELIC_RANK);
            let chosen = self.outfit_ship.is_relic();
            let line = format!(
                "{} u {:<10}{:>6}{:>9}{:>6}{:>7}{:>8}{:>7}{:>8}  {}",
                if chosen { "▶" } else { " " },
                "Relic", "?", "?", "?", "?", "?", "?", "?",
                if ok { "one of 13 alien relics, at random" } else { "needs Commodore + ranks" }
            );
            scr.text(tx, y, &line, if !ok { DIM } else if chosen { Color::White } else { Color::Grey }, chosen);
            y += 1;
        }
        let pick = if self.outfit_ship.is_special() { ShipType::special_for(team_now) } else { Some(self.outfit_ship) };
        if let Some(t) = pick.filter(|t| t.is_special()) {
            let line = format!("{}: {}", t.stats().name, t.trait_text());
            scr.text((w - line.chars().count() as i32).max(0) / 2, y, &line, Color::Cyan, false);
            y += 1;
        } else if self.outfit_ship.is_relic() {
            let names: Vec<&str> = ShipType::RELICS.iter().map(|r| r.stats().name).collect();
            let line = format!("Relics: {}", names.join(", "));
            scr.text((w - line.chars().count() as i32).max(0) / 2, y, &line, Color::Cyan, false);
            y += 1;
        }
        y += 1;
        let help = "Choose a team (f r k o) and a ship (s d c b a x, e special, u relic), then press Enter (or W to watch).";
        scr.text((w - help.len() as i32).max(0) / 2, y, help, Color::White, true);
        y += 1;
        if let Some(sv) = &f.me_info.service {
            let line = format!("Service record: {}", sv);
            scr.text((w - line.chars().count() as i32).max(0) / 2, y, &line, Color::Cyan, false);
            y += 1;
        }
        if !f.leaders.is_empty() {
            let top: Vec<String> = f
                .leaders
                .iter()
                .enumerate()
                .map(|(k, l)| format!("{}. {} {} {:.0}", k + 1, RANKS[l.rank as usize].1, l.name, l.points))
                .collect();
            let line = format!("Top careers: {}", top.join("   "));
            scr.text((w - line.chars().count() as i32).max(0) / 2, y, &line, DIM, false);
            y += 1;
        }
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
                let (bw, bh, title) = self.popup_size();
                let r = self.popup_box(scr, bw, bh, title);
                for (k, (keys, what)) in lines.iter().enumerate() {
                    if k as i32 >= r.h {
                        break;
                    }
                    scr.text(r.x, r.y + k as i32, keys, Color::Yellow, true);
                    scr.text_clip(r.x + 18, r.y + k as i32, what, Color::Grey, false, r.x + r.w - 1);
                }
            }
            Popup::Players => {
                let (bw, bh, title) = self.popup_size();
                let r = self.popup_box(scr, bw, bh, title);
                self.draw_player_list(scr, r, true);
            }
            Popup::Planets => {
                let f = self.frame.as_ref().unwrap();
                let (bw, bh, title) = self.popup_size();
                let r = self.popup_box(scr, bw, bh, title);
                let cols = if r.w >= 68 { 2 } else { 1 };
                let per_col = (f.planets.len() as i32 + cols - 1) / cols;
                for (k, def) in super::planet_defs(f).iter().enumerate() {
                    let (c, row) = (k as i32 / per_col, k as i32 % per_col);
                    if row >= r.h {
                        continue;
                    }
                    let info = &f.planets[k];
                    let line = if info.known {
                        format!(
                            "{:<15}{} {:>3} {}{}{}{}",
                            def.name,
                            info.owner.letter(),
                            info.armies,
                            if info.flags & PL_REPAIR != 0 { 'R' } else { ' ' },
                            if info.flags & PL_FUEL != 0 { 'F' } else { ' ' },
                            if info.flags & PL_AGRI != 0 { 'A' } else { ' ' },
                            info.outpost.map_or(' ', |o| o.letter())
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

/// Break `text` into lines of at most `width` characters, at spaces where
/// possible.
pub(super) fn wrap_chars(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        let len = |s: &str| s.chars().count();
        if !cur.is_empty() && len(&cur) + 1 + len(word) > width {
            lines.push(std::mem::take(&mut cur));
        }
        let mut word = word.to_string();
        while len(&word) > width {
            let head: String = word.chars().take(width).collect();
            word = word.chars().skip(width).collect();
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            lines.push(head);
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(&word);
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// The colour of advanced tech in the panels.
pub(super) const TECH_COLOR: [f32; 3] = [200.0, 170.0, 255.0];
/// The colour of your special or relic ship's name in the panels.
pub(super) const SHIP_COLOR: [f32; 3] = [255.0, 200.0, 90.0];

/// Rows (frames included) the controls and message panels always get: room
/// for the warning and talk lines, orders, supplies, two lines of tech and
/// four messages. Taller windows give them more (as message history).
pub(super) const BOTTOM_ROWS: i32 = 15;
/// Columns for the player list beside the maps (names, ranks and kills).
const LIST_COLS: i32 = 46;
/// Rows the controls need when the player list shares their panel.
const DASH_ROWS: i32 = 8;

pub(super) const HELP: &[(&str, &str)] = &[
    ("mouse", "move to aim; on a map: left click torp, right click steer, middle click phaser; wheel zooms"),
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
    ("B", "board the enemy ship nearest the pointer (armies aboard, its shields down; with --boarding)"),
    ("w", "overwatch (in orbit): auto-fire at any enemy that comes into weapons range"),
    ("v / e / j", "advanced tech (Commodore / Rear Admiral / Admiral, with ranks); /tech lists yours"),
    ("r", "refit to another ship (orbiting your home planet)"),
    ("i", "info on the thing nearest the pointer"),
    ("m", "send a message (then A, T, F/R/K/O or a player slot)"),
    ("L / P", "player list / planet list"),
    ("+ / -", "zoom tactical view (or mouse wheel)"),
    ("S", "sound effects on / off"),
    ("/", "server command, e.g. /record /orders /treaty rom /upgrade torps /fix warp /build yard (when enabled)"),
    ("Ctrl-L", "redraw screen"),
    ("q", "quit"),
    ("G", "field manual in place of the player list: [ ] page, { } chapter"),
    ("W", "on the outfit screen: watch as an observer instead of flying"),
    ("Tab", "observers: follow the next ship (Shift-Tab previous, click a ship, arrows or Esc for a free camera, J to join)"),
    ("", ""),
    ("How to win", "Kill enemies to earn kills. Kills let you carry armies (2 per kill)."),
    ("", "Bomb enemy planets down to 4 armies, pick up armies from your own"),
    ("", "planets, then beam them down to take enemy planets. Capture every"),
    ("", "planet of the enemy empire to genocide it and conquer the galaxy."),
];
