//! Terminal client: connects to a server, reads keyboard + mouse, and draws
//! the tactical and galactic displays with braille graphics.

mod canvas;
mod pixels;
mod render;
mod render_px;
mod render_vec;
mod shipart;
mod sixel;
mod sound;
mod vg;

use crate::consts::*;
use crate::proto::*;
use canvas::Screen;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::{cursor, execute, terminal};
use std::collections::VecDeque;
use std::io::{self, BufReader, BufWriter};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

/// Galaxy units across the tactical view at zoom 1 (used for sound distance).
const TAC_VIEW: f64 = 20_000.0;

pub struct ClientConfig {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub team: Option<Team>,
    pub ship: ShipType,
    /// Graphics style; None picks the best the terminal supports.
    pub gfx: Option<Gfx>,
    /// Start with sound effects off.
    pub mute: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Gfx {
    /// Real pixel images (SIXEL) with thin anti-aliased vector drawing.
    Vector,
    /// Color block glyphs, 2x4 pixels per cell.
    Blocks,
    /// Braille line art.
    Braille,
}

/// An encoded image waiting to be sent after the text layer.
struct Image {
    x: i32,
    y: i32,
    slot: usize,
    data: Vec<u8>,
}

/// What we found out about the terminal's image support.
pub struct SixelCheck {
    pub supported: bool,
    /// Advice to show the player when images aren't available.
    pub hint: Option<&'static str>,
}

/// Terminal apps known not to display SIXEL images.
const NO_SIXEL_APPS: [&str; 4] = ["Terminal.app", "Alacritty", "kitty", "Ghostty"];

/// Inside tmux, find the terminal app hosting the tmux client that shows our
/// pane, by walking up its process tree (e.g. tmux -> zsh -> login -> Terminal).
fn tmux_outer_app() -> Option<String> {
    let out = std::process::Command::new("tmux").args(["display", "-p", "#{client_pid}"]).output().ok()?;
    let mut pid: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    for _ in 0..12 {
        let out = std::process::Command::new("ps").args(["-o", "ppid=,comm=", "-p", &pid.to_string()]).output().ok()?;
        let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let (ppid, comm) = line.split_once(char::is_whitespace)?;
        let comm = comm.trim();
        if comm.contains(".app/") || ["wezterm-gui", "alacritty", "kitty", "ghostty", "foot"].iter().any(|n| comm.to_lowercase().ends_with(n)) {
            return Some(comm.to_string());
        }
        pid = ppid.trim().parse().ok()?;
        if pid <= 1 {
            break;
        }
    }
    None
}

/// Whether the terminal (and tmux, if we're inside it) can show SIXEL images.
pub fn sixel_check() -> SixelCheck {
    if std::env::var_os("TMUX").is_some() {
        if let Some(app) = tmux_outer_app() {
            if NO_SIXEL_APPS.iter().any(|n| app.contains(n)) {
                return SixelCheck {
                    supported: false,
                    hint: Some("This terminal app can't show images; attach tmux from iTerm2 for vector graphics"),
                };
            }
        }
        let tmux_sixel = std::process::Command::new("tmux")
            .args(["display", "-p", "#{client_termfeatures}"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("sixel"))
            .unwrap_or(false);
        if !tmux_sixel {
            return SixelCheck {
                supported: false,
                hint: Some("Tip: for vector graphics in tmux add  set -as terminal-features ',xterm*:sixel'  to ~/.tmux.conf and reattach"),
            };
        }
        return SixelCheck { supported: true, hint: None };
    }
    let prog = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let term = std::env::var("TERM").unwrap_or_default();
    if matches!(prog.as_str(), "iTerm.app" | "WezTerm") || ["foot", "mlterm", "contour"].iter().any(|t| term.contains(t)) {
        return SixelCheck { supported: true, hint: None };
    }
    let hint = (prog == "Apple_Terminal").then_some("Terminal.app can't show images; run in iTerm2 for vector graphics");
    SixelCheck { supported: false, hint }
}

enum NetEvent {
    Msg(ServerMsg),
    Closed(String),
}

#[derive(Clone, PartialEq)]
enum Mode {
    Play,
    Compose { target: Option<MsgTarget>, text: String },
    Refit,
    ConfirmQuit,
}

#[derive(Clone, Copy, PartialEq)]
enum Popup {
    None,
    Help,
    Players,
    Planets,
}

#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// Where things were drawn last frame, so mouse clicks map back to the galaxy.
#[derive(Clone, Copy, Default)]
struct Layout {
    tac: Rect,
    gal: Rect,
    center: (f64, f64),
    upd: f64,
    /// Tactical-view pixels per character cell (x, y).
    cpx: (f64, f64),
}

pub struct App {
    cfg: ClientConfig,
    writer: BufWriter<TcpStream>,
    slot: u8,
    frame: Option<Frame>,
    msgs: VecDeque<ChatMsg>,
    warning: Option<(String, Instant)>,
    pointer: Option<(i32, i32)>,
    zoom: f64,
    mode: Mode,
    popup: Popup,
    outfit_team: Option<Team>,
    outfit_ship: ShipType,
    layout: Layout,
    last_center: (f64, f64),
    quit: bool,
    closed: Option<String>,
    motd: Vec<String>,
    redraw: bool,
    gfx: Gfx,
    truecolor: bool,
    text: sixel::TextRenderer,
    /// Size of one character cell in screen pixels.
    cell_px: (f64, f64),
    /// Blocks mode: physical subpixels per cell (square, matching the cell shape).
    ss: (i32, i32),
    images: Vec<Image>,
    sent: [Option<u64>; 3],
    last_gal: Instant,
    tmux_hint: bool,
    sound: sound::Sound,
}

pub fn run(cfg: ClientConfig) -> io::Result<()> {
    let stream = TcpStream::connect((cfg.host.as_str(), cfg.port))
        .map_err(|e| io::Error::new(e.kind(), format!("cannot connect to {}:{}: {}", cfg.host, cfg.port, e)))?;
    stream.set_nodelay(true)?;
    let mut writer = BufWriter::new(stream.try_clone()?);
    write_msg(&mut writer, &ClientMsg::Hello { name: cfg.name.clone(), version: PROTOCOL_VERSION })?;

    let mut reader = BufReader::new(stream);
    let (slot, motd) = match read_msg::<ServerMsg, _>(&mut reader)? {
        ServerMsg::Welcome { slot, motd } => (slot, motd),
        ServerMsg::Reject(why) => return Err(io::Error::new(io::ErrorKind::Other, why)),
        _ => return Err(io::Error::new(io::ErrorKind::Other, "unexpected reply from server")),
    };

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || loop {
        match read_msg::<ServerMsg, _>(&mut reader) {
            Ok(m) => {
                if tx.send(NetEvent::Msg(m)).is_err() {
                    break;
                }
            }
            Err(e) => {
                let _ = tx.send(NetEvent::Closed(e.to_string()));
                break;
            }
        }
    });

    let outfit_team = cfg.team;
    let outfit_ship = cfg.ship;
    let cfg_mute = cfg.mute;
    let check = sixel_check();
    let gfx = cfg.gfx.unwrap_or(if check.supported { Gfx::Vector } else { Gfx::Blocks });
    let hint = if cfg.gfx.is_none() { check.hint } else { None };
    let tmux_hint = hint.is_some();
    let mut app = App {
        cfg,
        writer,
        slot,
        frame: None,
        msgs: VecDeque::new(),
        warning: None,
        pointer: None,
        zoom: 1.0,
        mode: Mode::Play,
        popup: Popup::None,
        outfit_team,
        outfit_ship,
        layout: Layout::default(),
        last_center: (GWIDTH / 2.0, GWIDTH / 2.0),
        quit: false,
        closed: None,
        motd,
        redraw: false,
        gfx,
        truecolor: pixels::truecolor_supported(),
        text: sixel::TextRenderer::load(),
        cell_px: (8.0, 16.0),
        ss: (6, 12),
        images: Vec::new(),
        sent: [None; 3],
        last_gal: Instant::now(),
        tmux_hint,
        sound: sound::Sound::new(!cfg_mute),
    };
    if let Some(h) = hint {
        app.warn(h);
    }

    let _guard = TermGuard::enter()?;
    let result = app.main_loop(rx);
    drop(_guard);
    if let Some(why) = &app.closed {
        eprintln!("Disconnected from server: {}", why);
    }
    result
}

struct TermGuard;

impl TermGuard {
    fn enter() -> io::Result<TermGuard> {
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(out, terminal::EnterAlternateScreen, cursor::Hide, EnableMouseCapture, terminal::Clear(terminal::ClearType::All))?;
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            TermGuard::restore();
            prev(info);
        }));
        Ok(TermGuard)
    }

    fn restore() {
        let mut out = io::stdout();
        let _ = execute!(out, DisableMouseCapture, cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        TermGuard::restore();
    }
}

impl App {
    fn main_loop(&mut self, rx: Receiver<NetEvent>) -> io::Result<()> {
        let (w, h) = terminal::size()?;
        let mut screen = Screen::new(w, h);
        self.measure_cells();
        let mut out = BufWriter::with_capacity(1 << 16, io::stdout());
        let mut dirty = true;
        let mut last_draw = Instant::now() - Duration::from_secs(1);
        let mut had_images = false;
        while !self.quit {
            loop {
                match rx.try_recv() {
                    Ok(NetEvent::Msg(m)) => {
                        self.on_server(m);
                        dirty = true;
                    }
                    Ok(NetEvent::Closed(why)) => {
                        self.closed = Some(why);
                        self.quit = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
            if event::poll(Duration::from_millis(10))? {
                loop {
                    match event::read()? {
                        Event::Key(k) if k.kind != KeyEventKind::Release => self.on_key(k),
                        Event::Mouse(m) => self.on_mouse(m),
                        Event::Resize(w, h) => {
                            screen.resize(w, h);
                            self.measure_cells();
                            self.sent = [None; 3];
                        }
                        _ => {}
                    }
                    dirty = true;
                    if !event::poll(Duration::ZERO)? {
                        break;
                    }
                }
            }
            if self.redraw {
                self.redraw = false;
                let (w, h) = terminal::size()?;
                execute!(out, terminal::Clear(terminal::ClearType::All))?;
                screen.resize(w, h);
                self.measure_cells();
                self.sent = [None; 3];
            }
            if dirty && last_draw.elapsed() >= Duration::from_millis(33) {
                self.draw(&mut screen);
                // Images stay painted on the terminal until cleared, so when a
                // frame stops using them (death, outfit screen, popups, mode
                // change), wipe the screen and repaint everything.
                let has_images = !screen.masks.is_empty();
                if had_images && !has_images {
                    let (w, h) = terminal::size()?;
                    execute!(out, terminal::Clear(terminal::ClearType::All))?;
                    screen.resize(w, h);
                    self.sent = [None; 3];
                    self.draw(&mut screen);
                }
                had_images = has_images;
                execute!(out, terminal::BeginSynchronizedUpdate)?;
                screen.flush(&mut out)?;
                self.send_images(&mut out)?;
                execute!(out, terminal::EndSynchronizedUpdate)?;
                dirty = false;
                last_draw = Instant::now();
            }
        }
        Ok(())
    }

    fn measure_cells(&mut self) {
        if let Ok(ws) = terminal::window_size() {
            if ws.width > 0 && ws.height > 0 && ws.columns > 0 && ws.rows > 0 {
                self.cell_px = (ws.width as f64 / ws.columns as f64, ws.height as f64 / ws.rows as f64);
            }
        }
        let ratio = (self.cell_px.1 / self.cell_px.0).clamp(1.2, 3.0);
        self.ss = (6, ((6.0 * ratio).round() as i32).clamp(8, 18));
    }

    fn send_images(&mut self, out: &mut impl io::Write) -> io::Result<()> {
        use std::hash::{Hash, Hasher};
        for img in std::mem::take(&mut self.images) {
            let mut hs = std::collections::hash_map::DefaultHasher::new();
            img.data.hash(&mut hs);
            let h = hs.finish();
            if self.sent[img.slot] == Some(h) {
                continue;
            }
            crossterm::queue!(out, cursor::MoveTo(img.x as u16, img.y as u16))?;
            out.write_all(&img.data)?;
            self.sent[img.slot] = Some(h);
        }
        out.flush()
    }

    fn send(&mut self, msg: ClientMsg) -> io::Result<()> {
        write_msg(&mut self.writer, &msg)
    }

    fn cmd(&mut self, msg: ClientMsg) {
        if let Err(e) = self.send(msg) {
            self.closed = Some(e.to_string());
            self.quit = true;
        }
    }

    fn warn(&mut self, s: impl Into<String>) {
        self.warning = Some((s.into(), Instant::now()));
    }

    fn on_server(&mut self, m: ServerMsg) {
        match m {
            ServerMsg::Frame(f) => {
                if let Some(me) = f.players.iter().find(|p| p.id == f.me) {
                    if me.state != PState::Outfit {
                        self.last_center = (me.x as f64, me.y as f64);
                    }
                    if me.state == PState::Outfit && self.mode == Mode::Refit {
                        self.mode = Mode::Play;
                    }
                }
                let old = self.frame.take();
                if let Some(old) = &old {
                    self.frame_sounds(old, &f);
                }
                self.frame = Some(*f);
            }
            ServerMsg::Msg(m) => {
                match m.kind {
                    MsgKind::System if m.text.contains("taken over by") => self.sound.play(sound::Sfx::Capture, 0.5),
                    MsgKind::System => {}
                    _ => self.sound.play(sound::Sfx::Message, 0.6),
                }
                self.msgs.push_back(m);
                while self.msgs.len() > 200 {
                    self.msgs.pop_front();
                }
            }
            ServerMsg::Warning(w) => {
                self.sound.play(sound::Sfx::Warning, 0.4);
                self.warn(w)
            }
            ServerMsg::Reject(why) => {
                self.closed = Some(why);
                self.quit = true;
            }
            ServerMsg::Welcome { .. } => {}
        }
    }

    /// Work out which sound effects the latest frame calls for.
    fn frame_sounds(&mut self, old: &Frame, new: &Frame) {
        use sound::Sfx;
        let me = self.slot;
        let find = |f: &Frame, id: u8| f.players.iter().find(|p| p.id == id).cloned();
        let (Some(o), Some(n)) = (find(old, me), find(new, me)) else { return };
        let (mx, my) = (n.x as f64, n.y as f64);
        let dist = |x: i32, y: i32| ((x as f64 - mx).powi(2) + (y as f64 - my).powi(2)).sqrt();
        // Louder when closer; silent beyond about one and a half screens.
        let near = |d: f64| (1.0 - d / (TAC_VIEW * 1.5)).clamp(0.0, 1.0) as f32;
        let mut plays: Vec<(Sfx, f32)> = Vec::new();

        let count = |f: &Frame, kind: TorpKind| {
            f.torps.iter().filter(|t| t.owner == me && t.kind == kind && t.explode == 0).count()
        };
        if count(new, TorpKind::Photon) > count(old, TorpKind::Photon) {
            plays.push((Sfx::Torp, 0.55));
        }
        if count(new, TorpKind::Plasma) > count(old, TorpKind::Plasma) {
            plays.push((Sfx::Plasma, 0.7));
        }
        let key = |p: &PhaserInfo| (p.owner, p.x1, p.y1, p.x2, p.y2);
        for ph in &new.phasers {
            if !old.phasers.iter().any(|q| key(q) == key(ph)) {
                let v = if ph.owner == me { 0.6 } else { near(dist(ph.x1, ph.y1)) * 0.4 };
                plays.push((Sfx::Phaser, v));
            }
        }
        // Torpedoes bursting nearby.
        if let Some(t) = new.torps.iter().filter(|t| t.explode == 1).min_by_key(|t| dist(t.x, t.y) as i64) {
            plays.push((Sfx::TorpBurst, near(dist(t.x, t.y)) * 0.7));
        }
        // Ships blowing up.
        for p in &new.players {
            let was = old.players.iter().find(|q| q.id == p.id).map(|q| q.state);
            if p.state == PState::Exploding && was == Some(PState::Alive) {
                if p.id == me {
                    plays.push((Sfx::Death, 1.0));
                } else {
                    plays.push((Sfx::Explosion, near(dist(p.x, p.y))));
                }
            }
        }
        if n.state == PState::Alive && o.state == PState::Alive {
            let (om, nm) = (&old.me_info, &new.me_info);
            if nm.damage > om.damage || nm.shield + 2 < om.shield {
                plays.push((Sfx::Hit, 0.8));
            }
            let flip = |flag: u16| (o.flags & flag != 0, n.flags & flag != 0);
            match flip(pf::SHIELD) {
                (false, true) => plays.push((Sfx::ShieldUp, 0.5)),
                (true, false) => plays.push((Sfx::ShieldDown, 0.5)),
                _ => {}
            }
            if flip(pf::CLOAK).0 != flip(pf::CLOAK).1 {
                plays.push((Sfx::Cloak, 0.6));
            }
            if nm.orbiting.is_some() && om.orbiting.is_none() {
                plays.push((Sfx::Orbit, 0.5));
            }
            // Red alert: an enemy just came close.
            let enemy_near = |f: &Frame, x: i32, y: i32| {
                f.players.iter().any(|q| {
                    q.state == PState::Alive && q.team != n.team && !q.fuzzy && {
                        let d = (((q.x - x) as f64).powi(2) + ((q.y - y) as f64).powi(2)).sqrt();
                        d < 7000.0
                    }
                })
            };
            if enemy_near(new, n.x, n.y) && !enemy_near(old, o.x, o.y) {
                plays.push((Sfx::Alert, 0.35));
            }
        }
        for (fx, v) in plays {
            self.sound.play(fx, v);
        }
    }

    fn me(&self) -> Option<&PlayerInfo> {
        let f = self.frame.as_ref()?;
        f.players.iter().find(|p| p.id == self.slot)
    }

    fn my_state(&self) -> PState {
        self.me().map(|p| p.state).unwrap_or(PState::Outfit)
    }

    /// Galaxy coordinates under the mouse pointer, if it is over a map.
    fn pointer_world(&self) -> Option<(f64, f64)> {
        let (col, row) = self.pointer?;
        let l = &self.layout;
        if l.tac.contains(col, row) {
            let dx = ((col - l.tac.x) as f64 + 0.5) * l.cpx.0 - l.tac.w as f64 * l.cpx.0 / 2.0;
            let dy = ((row - l.tac.y) as f64 + 0.5) * l.cpx.1 - l.tac.h as f64 * l.cpx.1 / 2.0;
            return Some((l.center.0 + dx * l.upd, l.center.1 + dy * l.upd));
        }
        if l.gal.contains(col, row) {
            let gx = ((col - l.gal.x) as f64 + 0.5) / l.gal.w as f64 * GWIDTH;
            let gy = ((row - l.gal.y) as f64 + 0.5) / l.gal.h as f64 * GWIDTH;
            return Some((gx, gy));
        }
        None
    }

    /// Pointer position in image pixels inside the tactical (or galactic) map.
    fn pointer_px(&self, tactical: bool) -> Option<(f32, f32)> {
        let (col, row) = self.pointer?;
        let r = if tactical { self.layout.tac } else { self.layout.gal };
        if !r.contains(col, row) {
            return None;
        }
        let (cw, ch) = self.cell_px;
        Some((((col - r.x) as f64 + 0.5) as f32 * cw as f32, ((row - r.y) as f64 + 0.5) as f32 * ch as f32))
    }

    /// Direction from my ship to the pointer (or my heading if no pointer).
    fn aim(&self) -> u8 {
        let Some(me) = self.me() else { return 0 };
        match self.pointer_world() {
            Some((x, y)) => dir_to(me.x as f64, me.y as f64, x, y) as u8,
            None => me.dir,
        }
    }

    fn nearest_player_to_pointer(&self) -> Option<&PlayerInfo> {
        let (x, y) = self.pointer_world()?;
        let f = self.frame.as_ref()?;
        f.players
            .iter()
            .filter(|p| p.id != self.slot && p.state == PState::Alive && !p.fuzzy)
            .min_by(|a, b| {
                let da = (a.x as f64 - x).powi(2) + (a.y as f64 - y).powi(2);
                let db = (b.x as f64 - x).powi(2) + (b.y as f64 - y).powi(2);
                da.total_cmp(&db)
            })
    }

    fn nearest_planet_to_pointer(&self) -> Option<(usize, f64)> {
        let (x, y) = self.pointer_world()?;
        PLANETS
            .iter()
            .enumerate()
            .map(|(k, p)| (k, ((p.x - x).powi(2) + (p.y - y).powi(2)).sqrt()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        self.pointer = Some((m.column as i32, m.row as i32));
        if self.my_state() != PState::Alive || self.mode != Mode::Play {
            return;
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let d = self.aim();
                self.cmd(ClientMsg::Torp(d));
            }
            MouseEventKind::Down(MouseButton::Right) => {
                let d = self.aim();
                self.cmd(ClientMsg::Course(d));
            }
            MouseEventKind::Down(MouseButton::Middle) => {
                let d = self.aim();
                self.cmd(ClientMsg::Phaser(d));
            }
            MouseEventKind::ScrollUp => self.zoom = (self.zoom / 1.25).max(0.4),
            MouseEventKind::ScrollDown => self.zoom = (self.zoom * 1.25).min(4.0),
            _ => {}
        }
    }

    fn on_key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('d')) {
            self.quit = true;
            return;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('l') {
            self.redraw = true;
            return;
        }
        match self.mode.clone() {
            Mode::ConfirmQuit => {
                self.mode = Mode::Play;
                if matches!(k.code, KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('q')) {
                    self.quit = true;
                }
                return;
            }
            Mode::Compose { target, text } => return self.compose_key(k, target, text),
            Mode::Refit => {
                self.mode = Mode::Play;
                if let KeyCode::Char(c) = k.code {
                    if let Some(s) = ShipType::from_key(c) {
                        self.cmd(ClientMsg::Refit(s));
                    }
                }
                return;
            }
            Mode::Play => {}
        }
        if self.popup != Popup::None && k.code == KeyCode::Esc {
            self.popup = Popup::None;
            return;
        }
        if self.my_state() == PState::Outfit {
            return self.outfit_key(k);
        }
        let KeyCode::Char(c) = k.code else {
            return self.special_key(k.code);
        };
        let alive = self.my_state() == PState::Alive;
        match c {
            '0'..='9' if alive => self.cmd(ClientMsg::Speed(c as u8 - b'0')),
            ')' => self.cmd(ClientMsg::Speed(10)),
            '!' => self.cmd(ClientMsg::Speed(11)),
            '@' => self.cmd(ClientMsg::Speed(12)),
            '%' => self.cmd(ClientMsg::Speed(99)),
            '#' => {
                let half = self.me().map(|p| p.ship.stats().max_speed / 2).unwrap_or(4);
                self.cmd(ClientMsg::Speed(half as u8));
            }
            'k' => {
                let d = self.aim();
                self.cmd(ClientMsg::Course(d));
            }
            't' => {
                let d = self.aim();
                self.cmd(ClientMsg::Torp(d));
            }
            'p' => {
                let d = self.aim();
                self.cmd(ClientMsg::Phaser(d));
            }
            'f' => {
                let d = self.aim();
                self.cmd(ClientMsg::Plasma(d));
            }
            's' | 'u' => self.cmd(ClientMsg::Shields),
            'c' => self.cmd(ClientMsg::Cloak),
            'o' => self.cmd(ClientMsg::Orbit),
            'b' => self.cmd(ClientMsg::Bomb),
            'z' => self.cmd(ClientMsg::BeamUp),
            'x' => self.cmd(ClientMsg::BeamDown),
            'R' => self.cmd(ClientMsg::Repair),
            'd' => self.cmd(ClientMsg::DetEnemy),
            'D' => self.cmd(ClientMsg::DetOwn),
            'T' | 'y' => {
                let pressor = c == 'y';
                let tractoring = self.me().map_or(false, |p| p.flags & (pf::TRACTOR | pf::PRESSOR) != 0);
                if tractoring {
                    self.cmd(ClientMsg::Tractor { target: None, pressor });
                } else if let Some(t) = self.nearest_player_to_pointer().map(|p| p.id) {
                    self.cmd(ClientMsg::Tractor { target: Some(t), pressor });
                }
            }
            'l' => self.lock_pointer(),
            'r' => {
                self.mode = Mode::Refit;
            }
            'i' => self.info_pointer(),
            'm' => self.mode = Mode::Compose { target: None, text: String::new() },
            'g' => {
                self.gfx = match self.gfx {
                    Gfx::Vector => Gfx::Blocks,
                    Gfx::Blocks => Gfx::Braille,
                    Gfx::Braille => Gfx::Vector,
                };
                self.redraw = true;
                self.warn(match self.gfx {
                    Gfx::Vector => "Graphics: vector (sixel images)",
                    Gfx::Blocks => "Graphics: color blocks",
                    Gfx::Braille => "Graphics: braille line art",
                });
            }
            'S' => {
                self.sound.enabled = !self.sound.enabled && self.sound.available();
                self.warn(if self.sound.enabled { "Sound on" } else if self.sound.available() { "Sound off" } else { "No audio player found (afplay)" });
            }
            'L' => self.toggle_popup(Popup::Players),
            'P' => self.toggle_popup(Popup::Planets),
            '?' | 'h' => self.toggle_popup(Popup::Help),
            '+' | '=' => self.zoom = (self.zoom / 1.25).max(0.4),
            '-' | '_' => self.zoom = (self.zoom * 1.25).min(4.0),
            'q' | 'Q' => self.mode = Mode::ConfirmQuit,
            _ => {}
        }
    }

    fn special_key(&mut self, code: KeyCode) {
        let Some(me) = self.me() else { return };
        let (dir, speed) = (me.dir as i32, me.speed as i32);
        let desired = self.frame.as_ref().map(|f| f.me_info.desired_speed as i32).unwrap_or(speed);
        match code {
            KeyCode::Left => self.cmd(ClientMsg::Course((dir - 16).rem_euclid(256) as u8)),
            KeyCode::Right => self.cmd(ClientMsg::Course((dir + 16).rem_euclid(256) as u8)),
            KeyCode::Up => self.cmd(ClientMsg::Speed((desired + 1).min(12) as u8)),
            KeyCode::Down => self.cmd(ClientMsg::Speed((desired - 1).max(0) as u8)),
            KeyCode::Esc => self.popup = Popup::None,
            _ => {}
        }
    }

    fn toggle_popup(&mut self, p: Popup) {
        self.popup = if self.popup == p { Popup::None } else { p };
    }

    fn lock_pointer(&mut self) {
        let player = self.nearest_player_to_pointer().map(|p| (p.id, p.x as f64, p.y as f64));
        let planet = self.nearest_planet_to_pointer();
        let Some((px, py)) = self.pointer_world() else {
            return self.warn("Point at a planet or ship to lock on");
        };
        let pd = player.map(|(_, x, y)| ((x - px).powi(2) + (y - py).powi(2)).sqrt());
        match (player, pd, planet) {
            (Some((id, _, _)), Some(d), Some((_, pld))) if d < pld * 0.5 => self.cmd(ClientMsg::LockPlayer(id)),
            (_, _, Some((k, _))) => self.cmd(ClientMsg::LockPlanet(k as u8)),
            _ => {}
        }
    }

    fn info_pointer(&mut self) {
        let Some((px, py)) = self.pointer_world() else { return };
        let Some(f) = self.frame.as_ref() else { return };
        let player = self
            .nearest_player_to_pointer()
            .map(|p| (p.clone(), ((p.x as f64 - px).powi(2) + (p.y as f64 - py).powi(2)).sqrt()));
        let planet = self.nearest_planet_to_pointer();
        let text = match (player, planet) {
            (Some((p, d)), Some((_, pd))) if d < pd => {
                let s = p.ship.stats();
                format!(
                    "{}{} {} — {} {} • speed {} • kills {:.2}{}",
                    p.team.letter(),
                    slot_char(p.id),
                    p.name,
                    p.team.name(),
                    s.name,
                    p.speed,
                    p.kills,
                    if p.flags & pf::ROBOT != 0 { " • robot" } else { "" }
                )
            }
            (_, Some((k, _))) => {
                let pl = &f.planets[k];
                let def = &PLANETS[k];
                if pl.known {
                    format!(
                        "{} — {} • {} armies{}{}{}{}",
                        def.name,
                        pl.owner.name(),
                        pl.armies,
                        if pl.flags & PL_HOME != 0 { " • HOME" } else { "" },
                        if pl.flags & PL_REPAIR != 0 { " • REPAIR" } else { "" },
                        if pl.flags & PL_FUEL != 0 { " • FUEL" } else { "" },
                        if pl.flags & PL_AGRI != 0 { " • AGRI" } else { "" },
                    )
                } else {
                    format!("{} — not yet scouted", def.name)
                }
            }
            _ => return,
        };
        self.warn(text);
    }

    fn outfit_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.launch(),
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => self.quit = true,
            KeyCode::Char('m') => self.mode = Mode::Compose { target: None, text: String::new() },
            KeyCode::Char('?') | KeyCode::Char('h') => self.toggle_popup(Popup::Help),
            KeyCode::Left | KeyCode::Right => {
                let cur = self.outfit_team.unwrap_or(Team::Fed);
                let i = Team::PLAYABLE.iter().position(|&t| t == cur).unwrap_or(0) as i32;
                let d = if k.code == KeyCode::Left { -1 } else { 1 };
                self.outfit_team = Some(Team::PLAYABLE[(i + d).rem_euclid(4) as usize]);
            }
            KeyCode::Up | KeyCode::Down => {
                let i = ShipType::ALL.iter().position(|&s| s == self.outfit_ship).unwrap_or(2) as i32;
                let d = if k.code == KeyCode::Up { -1 } else { 1 };
                self.outfit_ship = ShipType::ALL[(i + d).rem_euclid(6) as usize];
            }
            KeyCode::Char(c) => {
                if let Some(t) = Team::from_char(c) {
                    self.outfit_team = Some(t);
                } else if let Some(s) = ShipType::from_key(c) {
                    self.outfit_ship = s;
                }
            }
            _ => {}
        }
    }

    fn launch(&mut self) {
        let open = self.frame.as_ref().map(|f| f.open_teams.clone()).unwrap_or_default();
        let team = match self.outfit_team {
            Some(t) if open.contains(&t) => t,
            Some(t) => return self.warn(format!("The {} are not accepting recruits", t.plural())),
            None => {
                // Default: join the smallest empire that already has ships in play.
                let f = self.frame.as_ref();
                let size = |t: Team| {
                    f.map(|f| f.players.iter().filter(|p| p.team == t && p.state != PState::Outfit).count())
                        .unwrap_or(0)
                };
                let active: Vec<Team> = open.iter().copied().filter(|&t| size(t) > 0).collect();
                let pool = if active.is_empty() { open.clone() } else { active };
                match pool.into_iter().min_by_key(|&t| size(t)) {
                    Some(t) => t,
                    None => return self.warn("No teams are open right now"),
                }
            }
        };
        self.outfit_team = Some(team);
        let ship = self.outfit_ship;
        self.cmd(ClientMsg::Join { team, ship });
    }

    fn compose_key(&mut self, k: KeyEvent, target: Option<MsgTarget>, mut text: String) {
        match (target, k.code) {
            (_, KeyCode::Esc) => self.mode = Mode::Play,
            (None, KeyCode::Char(c)) => {
                let my_team = self.me().map(|p| p.team).unwrap_or(Team::Ind);
                let t = match c {
                    'A' => Some(MsgTarget::All),
                    'T' => Some(MsgTarget::Team(my_team)),
                    'F' | 'R' | 'K' | 'O' => Team::from_char(c).map(MsgTarget::Team),
                    _ => slot_from_char(c).map(MsgTarget::Player),
                };
                match t {
                    Some(t) => self.mode = Mode::Compose { target: Some(t), text },
                    None => self.mode = Mode::Play,
                }
            }
            (Some(t), KeyCode::Enter) => {
                self.mode = Mode::Play;
                if !text.trim().is_empty() {
                    self.cmd(ClientMsg::Message { to: t, text });
                }
            }
            (Some(t), KeyCode::Backspace) => {
                text.pop();
                self.mode = Mode::Compose { target: Some(t), text };
            }
            (Some(t), KeyCode::Char(c)) => {
                if text.chars().count() < 79 {
                    text.push(c);
                }
                self.mode = Mode::Compose { target: Some(t), text };
            }
            _ => {}
        }
    }
}
