//! Careers (the --ranks option): a service record for every callsign that
//! survives between games, a rank from Ensign to Admiral, honours for great
//! deeds, and a leaderboard. Records are kept in a small tab-separated file.

use super::world::{GameEvent, World};
use crate::consts::*;
use crate::proto::LeaderInfo;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Record {
    pub name: String,
    pub kills: f64,
    pub deaths: u32,
    pub planets: u32,
    pub bombed: u32,
    pub orders: u32,
    pub salvaged: u32,
    pub seconds: u64,
    pub honours: Vec<String>,
}

impl Record {
    /// Career points: kills, half a point per planet taken, a point per
    /// order carried out, and two per honour.
    pub fn points(&self) -> f64 {
        self.kills + self.planets as f64 * 0.5 + self.orders as f64 + self.honours.len() as f64 * 2.0
    }

    pub fn rank(&self) -> u8 {
        let pts = self.points();
        RANKS.iter().rposition(|r| pts >= r.2).unwrap_or(0) as u8
    }

    pub fn summary(&self) -> String {
        format!(
            "{} • {:.1} points • {:.1} kills, {} deaths • {} planets • {} orders • {} honours",
            RANKS[self.rank() as usize].0,
            self.points(),
            self.kills,
            self.deaths,
            self.planets,
            self.orders,
            self.honours.len()
        )
    }

    fn to_line(&self) -> String {
        let clean = |s: &str| s.replace(['\t', '\n', '|'], " ");
        format!(
            "{}\t{:.2}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            clean(&self.name),
            self.kills,
            self.deaths,
            self.planets,
            self.bombed,
            self.orders,
            self.salvaged,
            self.seconds,
            self.honours.iter().map(|h| clean(h)).collect::<Vec<_>>().join("|")
        )
    }

    fn from_line(line: &str) -> Option<Record> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 8 || f[0].is_empty() {
            return None;
        }
        Some(Record {
            name: f[0].to_string(),
            kills: f[1].parse().ok()?,
            deaths: f[2].parse().ok()?,
            planets: f[3].parse().ok()?,
            bombed: f[4].parse().ok()?,
            orders: f[5].parse().ok()?,
            salvaged: f[6].parse().ok()?,
            seconds: f[7].parse().ok()?,
            honours: f.get(8).map_or(Vec::new(), |h| h.split('|').filter(|s| !s.is_empty()).map(String::from).collect()),
        })
    }
}

/// Default place for the records: ~/.netrek/service-records.tsv
pub fn default_path() -> PathBuf {
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    home.join(".netrek").join("service-records.tsv")
}

pub struct Careers {
    path: Option<PathBuf>,
    records: HashMap<String, Record>,
    /// Slot -> record key, for the humans in the game.
    slots: HashMap<u8, String>,
    dirty: bool,
}

fn key(name: &str) -> String {
    name.trim().to_lowercase()
}

impl Careers {
    /// Load the records from `path` (None keeps them in memory only).
    pub fn new(path: Option<PathBuf>) -> Careers {
        let mut records = HashMap::new();
        if let Some(text) = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            for r in text.lines().filter_map(Record::from_line) {
                records.insert(key(&r.name), r);
            }
        }
        Careers { path, records, slots: HashMap::new(), dirty: false }
    }

    #[cfg(test)]
    pub fn record(&self, name: &str) -> Option<&Record> {
        self.records.get(&key(name))
    }

    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let Some(path) = &self.path else { return };
        let mut rows: Vec<&Record> = self.records.values().collect();
        rows.sort_by(|a, b| b.points().total_cmp(&a.points()));
        let mut out = String::from("# name\tkills\tdeaths\tplanets\tbombed\torders\tsalvaged\tseconds\thonours\n");
        for r in rows {
            out += &r.to_line();
            out.push('\n');
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, out).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }

    pub fn tick(&mut self, world: &mut World, events: &[GameEvent], commands: &[(u8, String)]) {
        let tick = world.tick;
        // Humans coming and going.
        let before = self.slots.len();
        self.slots.retain(|&id, k| {
            let p = &world.players[id as usize];
            p.in_use && !p.robot && key(&p.name) == *k
        });
        if self.slots.len() != before {
            self.save();
        }
        let arrivals: Vec<(u8, String)> = world
            .players
            .iter()
            .filter(|p| p.in_use && !p.robot && !self.slots.contains_key(&p.id))
            .map(|p| (p.id, p.name.clone()))
            .collect();
        for (id, name) in arrivals {
            let k = key(&name);
            self.slots.insert(id, k.clone());
            let r = self.records.entry(k).or_insert_with(|| Record { name: name.clone(), ..Record::default() });
            let rank = RANKS[r.rank() as usize].0;
            let text = if r.seconds == 0 && r.points() == 0.0 {
                format!("Welcome aboard, {} {}. Your service record starts today.", rank, name)
            } else {
                format!("Welcome back, {} {}. {}", rank, name, r.summary())
            };
            world.reply(id, text);
        }

        // What happened this tick.
        let mut ranks_before: HashMap<u8, u8> = HashMap::new();
        for (&id, k) in &self.slots {
            ranks_before.insert(id, self.records[k].rank());
        }
        for e in events {
            let (who, f): (Option<u8>, Box<dyn Fn(&mut Record)>) = match e.clone() {
                GameEvent::Kill { killer, victim, credit } => {
                    if let Some(k) = self.slots.get(&victim).cloned() {
                        self.records.get_mut(&k).unwrap().deaths += 1;
                        self.dirty = true;
                    }
                    (killer, Box::new(move |r: &mut Record| r.kills += credit))
                }
                GameEvent::PlanetTaken { player, .. } => (Some(player), Box::new(|r: &mut Record| r.planets += 1)),
                GameEvent::Bombed { player, armies, .. } => (Some(player), Box::new(move |r: &mut Record| r.bombed += armies as u32)),
                GameEvent::OrderDone { player } => (Some(player), Box::new(|r: &mut Record| r.orders += 1)),
                GameEvent::Salvaged { player } => (Some(player), Box::new(|r: &mut Record| r.salvaged += 1)),
                GameEvent::Honour { player, text } => {
                    let fresh = self.slots.get(&player).map_or(false, |k| !self.records[k].honours.contains(&text));
                    if fresh {
                        world.reply(player, format!("Honour added to your service record: {}", text));
                    }
                    (Some(player), Box::new(move |r: &mut Record| {
                        if !r.honours.contains(&text) {
                            r.honours.push(text.clone());
                        }
                    }))
                }
                GameEvent::Reinforced { .. } => (None, Box::new(|_: &mut Record| {})),
            };
            if let Some(k) = who.and_then(|id| self.slots.get(&id)) {
                f(self.records.get_mut(k).unwrap());
                self.dirty = true;
            }
        }
        if tick % UPS as u32 == 0 {
            for (&id, k) in &self.slots {
                if world.players[id as usize].alive() {
                    self.records.get_mut(k).unwrap().seconds += 1;
                }
            }
        }

        // Promotions.
        for (&id, k) in &self.slots {
            let r = &self.records[k];
            let now = r.rank();
            if ranks_before.get(&id).map_or(false, |&b| now > b) {
                let who = world.players[id as usize].label();
                world.god(format!("{} is promoted to {}!", who, RANKS[now as usize].0));
            }
            let p = &mut world.players[id as usize];
            p.rank = Some(now);
            p.service = Some(r.summary());
        }
        for (id, cmd) in commands {
            if cmd == "record" {
                let text = match self.slots.get(id).map(|k| &self.records[k]) {
                    Some(r) if r.honours.is_empty() => r.summary(),
                    Some(r) => format!("{} • Honours: {}", r.summary(), r.honours.join(", ")),
                    None => "Robots don't keep service records.".into(),
                };
                world.reply(*id, text);
            }
        }
        if tick % (UPS as u32 * 5) == 0 {
            let mut top: Vec<&Record> = self.records.values().filter(|r| r.points() > 0.0).collect();
            top.sort_by(|a, b| b.points().total_cmp(&a.points()));
            world.leaders = top.iter().take(5).map(|r| LeaderInfo { name: r.name.clone(), rank: r.rank(), points: r.points() as f32 }).collect();
        }
        if tick % (UPS as u32 * 30) == 0 {
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records build up from events, promotions are announced, honours
    /// stick, and it all survives a save and reload.
    #[test]
    fn careers_persist_and_promote() {
        let path = std::env::temp_dir().join(format!("netrek-records-{}.tsv", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut w = World::new();
        let id = w.add_player("Chappie", false).unwrap();
        w.join(id, Team::Fed, ShipType::Cruiser).unwrap();
        let mut c = Careers::new(Some(path.clone()));
        c.tick(&mut w, &[], &[]);
        assert_eq!(w.players[id as usize].rank, Some(0));
        let events = vec![
            GameEvent::Kill { killer: Some(id), victim: 30, credit: 2.5 },
            GameEvent::PlanetTaken { player: id, planet: 12 },
            GameEvent::Honour { player: id, text: "Joined with V'Ger".into() },
        ];
        w.outbox.clear();
        c.tick(&mut w, &events, &[]);
        let r = c.record("chappie").unwrap().clone();
        assert_eq!((r.kills, r.planets, r.honours.len()), (2.5, 1, 1));
        assert_eq!(r.points(), 2.5 + 0.5 + 2.0);
        assert_eq!(w.players[id as usize].rank, Some(1), "promoted to Lieutenant");
        assert!(w.outbox.iter().any(|o| o.msg.text.contains("promoted to Lieutenant")));
        c.dirty = true;
        c.save();
        let again = Careers::new(Some(path.clone()));
        assert_eq!(again.record("CHAPPIE"), Some(&r));
        let _ = std::fs::remove_file(&path);
    }
}
