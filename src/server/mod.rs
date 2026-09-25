//! TCP game server: one thread runs the simulation at 10 updates/second,
//! each connection gets a reader thread and a writer thread.

pub mod bot;
pub mod world;

use crate::consts::*;
use crate::proto::*;
use std::collections::HashMap;
use std::io::{self, BufReader, BufWriter, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use world::{Dest, World};

#[derive(Clone)]
pub struct ServerConfig {
    pub bind: String,
    pub port: u16,
    pub bots: usize,
    pub quiet: bool,
}

enum Event {
    Connect { conn: u64, name: String, out: SyncSender<Arc<Vec<u8>>> },
    Cmd { conn: u64, msg: ClientMsg },
    Disconnect { conn: u64 },
}

struct Conn {
    slot: u8,
    out: SyncSender<Arc<Vec<u8>>>,
}

/// Bind and run the server on the current thread (blocks forever).
pub fn run(cfg: ServerConfig) -> io::Result<()> {
    let listener = TcpListener::bind((cfg.bind.as_str(), cfg.port))?;
    serve(listener, cfg)
}

/// Bind and run the server on background threads; returns the bound port.
pub fn spawn_background(cfg: ServerConfig) -> io::Result<u16> {
    let listener = TcpListener::bind((cfg.bind.as_str(), cfg.port))?;
    let port = listener.local_addr()?.port();
    thread::spawn(move || {
        let _ = serve(listener, cfg);
    });
    Ok(port)
}

fn serve(listener: TcpListener, cfg: ServerConfig) -> io::Result<()> {
    let (tx, rx) = mpsc::channel::<Event>();
    let quiet = cfg.quiet;
    if !quiet {
        println!("netrek server listening on {}", listener.local_addr()?);
        println!("{} robots will be playing", cfg.bots);
    }
    let game_cfg = cfg.clone();
    thread::spawn(move || game_loop(rx, game_cfg));
    let mut next_conn = 1u64;
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let conn = next_conn;
        next_conn += 1;
        let tx = tx.clone();
        if !quiet {
            if let Ok(addr) = stream.peer_addr() {
                println!("connection {} from {}", conn, addr);
            }
        }
        thread::spawn(move || handle_conn(conn, stream, tx));
    }
    Ok(())
}

fn handle_conn(conn: u64, stream: TcpStream, tx: Sender<Event>) {
    let _ = stream.set_nodelay(true);
    let Ok(write_half) = stream.try_clone() else { return };
    let mut reader = BufReader::new(stream);
    let _ = reader.get_ref().set_read_timeout(Some(Duration::from_secs(15)));
    let name = match read_msg::<ClientMsg, _>(&mut reader) {
        Ok(ClientMsg::Hello { name, version }) if version == PROTOCOL_VERSION => name,
        Ok(ClientMsg::Hello { .. }) => {
            let mut w = write_half;
            let _ = write_msg(&mut w, &ServerMsg::Reject("Client version mismatch".into()));
            return;
        }
        _ => return,
    };
    let _ = reader.get_ref().set_read_timeout(None);

    // Writer: frames queue up here; if the client can't keep up we drop frames.
    let (out_tx, out_rx) = mpsc::sync_channel::<Arc<Vec<u8>>>(16);
    let shutdown_half = write_half.try_clone().ok();
    thread::spawn(move || {
        let mut w = BufWriter::new(write_half);
        while let Ok(buf) = out_rx.recv() {
            if w.write_all(&buf).and_then(|_| w.flush()).is_err() {
                break;
            }
        }
        if let Some(s) = shutdown_half {
            let _ = s.shutdown(Shutdown::Both);
        }
    });

    if tx.send(Event::Connect { conn, name, out: out_tx }).is_err() {
        return;
    }
    loop {
        match read_msg::<ClientMsg, _>(&mut reader) {
            Ok(msg) => {
                if tx.send(Event::Cmd { conn, msg }).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = tx.send(Event::Disconnect { conn });
}

fn send(c: &Conn, msg: &ServerMsg) -> bool {
    !matches!(c.out.try_send(Arc::new(encode(msg))), Err(TrySendError::Disconnected(_)))
}

fn game_loop(rx: Receiver<Event>, cfg: ServerConfig) {
    let mut world = World::new();
    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut bots: Vec<bot::Bot> = Vec::new();
    let tick_len = Duration::from_millis(1000 / UPS);
    let mut next = Instant::now();
    let log = |s: String| {
        if !cfg.quiet {
            println!("{}", s);
        }
    };

    loop {
        // Network events.
        while let Ok(ev) = rx.try_recv() {
            match ev {
                Event::Connect { conn, name, out } => {
                    // Make room by retiring a robot if the galaxy is full.
                    if world.players.iter().all(|p| p.in_use) {
                        if let Some(b) = bots.pop() {
                            world.remove_player(b.id);
                        }
                    }
                    match world.add_player(&name, false) {
                        Some(slot) => {
                            let c = Conn { slot, out };
                            send(
                                &c,
                                &ServerMsg::Welcome {
                                    slot,
                                    motd: vec![
                                        "Welcome to Netrek!".into(),
                                        "Conquer the galaxy for your empire.".into(),
                                    ],
                                },
                            );
                            log(format!("{} joined as slot {}", name, slot_char(slot)));
                            conns.insert(conn, c);
                        }
                        None => {
                            let _ = out.try_send(Arc::new(encode(&ServerMsg::Reject("Galaxy is full".into()))));
                        }
                    }
                }
                Event::Cmd { conn, msg } => {
                    if let Some(c) = conns.get(&conn) {
                        world.handle(c.slot, msg);
                    }
                }
                Event::Disconnect { conn } => {
                    if let Some(c) = conns.remove(&conn) {
                        log(format!("slot {} disconnected", slot_char(c.slot)));
                        world.remove_player(c.slot);
                    }
                }
            }
        }

        if world.tick % UPS as u32 == 0 {
            balance_bots(&mut world, &mut bots, cfg.bots);
        }
        for b in bots.iter_mut() {
            b.think(&mut world);
        }
        world.tick();

        // Deliver chat and warnings.
        for out in world.outbox.drain(..) {
            let msg = ServerMsg::Msg(out.msg);
            for c in conns.values() {
                let p = &world.players[c.slot as usize];
                let deliver = match out.dest {
                    Dest::All => true,
                    Dest::Team(t) => p.team == t && p.state != PState::Outfit,
                    Dest::Player(id) => id == c.slot,
                };
                if deliver {
                    send(c, &msg);
                }
            }
            if let ServerMsg::Msg(m) = &msg {
                if matches!(out.dest, Dest::All) {
                    log(format!("[{}] {}", m.from, m.text));
                }
            }
        }
        let warnings: Vec<(u8, String)> = world.warnings.drain(..).collect();
        for (id, text) in warnings {
            for c in conns.values().filter(|c| c.slot == id) {
                send(c, &ServerMsg::Warning(text.clone()));
            }
        }

        // Send each client its view of the world.
        conns.retain(|_, c| send(c, &ServerMsg::Frame(Box::new(world.frame_for(c.slot)))));

        next += tick_len;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}

/// Keep `total` robots in play, split so the Federation and Romulans
/// (the classic "T-mode" pairing) end up with even team sizes.
fn balance_bots(world: &mut World, bots: &mut Vec<bot::Bot>, total: usize) {
    let humans = |t: Team| {
        world
            .players
            .iter()
            .filter(|p| p.in_use && !p.robot && p.team == t && p.state != PState::Outfit)
            .count()
    };
    let (hf, hr) = (humans(Team::Fed), humans(Team::Rom));
    let sum = total + hf + hr;
    let fed_size = sum / 2;
    for team in [Team::Fed, Team::Rom] {
        let want = if team == Team::Fed { fed_size.saturating_sub(hf) } else { (sum - fed_size).saturating_sub(hr) };
        let have = bots.iter().filter(|b| b.team == team).count();
        if have < want {
            for _ in have..want {
                match bot::spawn(world, team) {
                    Some(b) => bots.push(b),
                    None => break,
                }
            }
        } else if have > want {
            let mut extra = have - want;
            bots.retain(|b| {
                if extra > 0 && b.team == team {
                    extra -= 1;
                    world.remove_player(b.id);
                    false
                } else {
                    true
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Let robots play a long game and make sure the rules engine holds up.
    #[test]
    fn robots_play_a_game() {
        let mut world = World::new();
        let mut bots = Vec::new();
        balance_bots(&mut world, &mut bots, 10);
        assert_eq!(bots.len(), 10);
        let mut log = Vec::new();
        for _ in 0..(UPS as u32 * 60 * 30) {
            for b in bots.iter_mut() {
                b.think(&mut world);
            }
            world.tick();
            for f in world.players.iter().filter(|p| p.in_use).map(|p| p.id).collect::<Vec<_>>() {
                let _ = encode(&ServerMsg::Frame(Box::new(world.frame_for(f))));
            }
            log.extend(world.outbox.drain(..).map(|o| o.msg.text));
            world.warnings.clear();
        }
        let kills = log.iter().filter(|m| m.contains("was kill")).count();
        let taken = log.iter().filter(|m| m.contains("taken over")).count();
        let bombed: i32 = world.planets.iter().map(|p| (START_ARMIES - p.armies).max(0)).sum();
        println!("armies below start across galaxy: {}", bombed);
        let deaths = log.len();
        for m in log.iter().filter(|m| !m.contains("was kill")).take(40) {
            println!("{}", m);
        }
        println!("messages={} kills={} planets taken={}", deaths, kills, taken);
        for t in Team::PLAYABLE {
            println!("{:?}: {} planets", t, world.team_planet_count(t));
        }
        assert!(kills > 0, "robots never killed anyone");
    }
}
