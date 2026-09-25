mod client;
mod consts;
mod proto;
mod server;

use clap::{Parser, Subcommand};
use consts::{ShipType, Team, DEFAULT_PORT};

#[derive(Parser)]
#[command(name = "netrek", version, about = "Netrek — the classic multiplayer space battle game, in your terminal")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a game server
    Server {
        /// Port to listen on
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        /// Address to bind
        #[arg(short, long, default_value = "0.0.0.0")]
        bind: String,
        /// Number of robot players to keep in the game
        #[arg(long, default_value_t = 6)]
        bots: usize,
        /// Empires the robots play for: "all", or a list like "fed,rom,kli"
        #[arg(short, long, default_value = "fed,rom", value_parser = parse_empires)]
        empires: Empires,
    },
    /// Connect to a server and play
    Play {
        /// Server host name or address
        #[arg(default_value = "localhost")]
        host: String,
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[command(flatten)]
        who: Who,
    },
    /// Start a private local server with robots and play on it
    Solo {
        /// Number of robot players
        #[arg(long, default_value_t = 7)]
        bots: usize,
        /// Empires the robots play for: "all", or a list like "fed,rom,kli"
        #[arg(short, long, default_value = "fed,rom", value_parser = parse_empires)]
        empires: Empires,
        #[command(flatten)]
        who: Who,
    },
}

#[derive(clap::Args)]
struct Who {
    /// Your callsign
    #[arg(short, long)]
    name: Option<String>,
    /// Preferred team: fed, rom, kli or ori
    #[arg(short, long, value_parser = parse_team)]
    team: Option<Team>,
    /// Preferred ship: SC, DD, CA, BB, AS or SB
    #[arg(short, long, default_value = "CA", value_parser = parse_ship)]
    ship: ShipType,
    /// Graphics for the maps: auto, vector (sixel images), blocks or braille
    #[arg(short, long, default_value = "auto", value_parser = ["auto", "vector", "sixel", "blocks", "braille"])]
    gfx: String,
    /// Start with sound effects turned off (toggle in game with S)
    #[arg(long)]
    mute: bool,
}

#[derive(Clone)]
struct Empires(Vec<Team>);

fn parse_empires(s: &str) -> Result<Empires, String> {
    if s.eq_ignore_ascii_case("all") {
        return Ok(Empires(Team::PLAYABLE.to_vec()));
    }
    let mut teams = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let t = parse_team(part)?;
        if !teams.contains(&t) {
            teams.push(t);
        }
    }
    if teams.is_empty() {
        return Err("give at least one empire, or \"all\"".into());
    }
    Ok(Empires(teams))
}

fn parse_team(s: &str) -> Result<Team, String> {
    s.chars().next().and_then(Team::from_char).ok_or_else(|| "team must be fed, rom, kli or ori".into())
}

fn parse_ship(s: &str) -> Result<ShipType, String> {
    ShipType::from_abbr(s).ok_or_else(|| "ship must be one of SC DD CA BB AS SB".into())
}

fn parse_gfx(s: &str) -> Option<client::Gfx> {
    match s {
        "vector" | "sixel" => Some(client::Gfx::Vector),
        "blocks" => Some(client::Gfx::Blocks),
        "braille" => Some(client::Gfx::Braille),
        _ => None,
    }
}

fn player_name(who: &Who) -> String {
    who.name
        .clone()
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "guest".into())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.cmd {
        Cmd::Server { port, bind, bots, empires } => {
            server::run(server::ServerConfig { bind, port, bots, empires: empires.0, quiet: false })
        }
        Cmd::Play { host, port, who } => client::run(client::ClientConfig {
            host,
            port,
            name: player_name(&who),
            team: who.team,
            ship: who.ship,
            gfx: parse_gfx(&who.gfx),
            mute: who.mute,
        }),
        Cmd::Solo { bots, empires, who } => {
            let cfg = server::ServerConfig { bind: "127.0.0.1".into(), port: 0, bots, empires: empires.0, quiet: true };
            server::spawn_background(cfg).and_then(|port| {
                client::run(client::ClientConfig {
                    host: "127.0.0.1".into(),
                    port,
                    name: player_name(&who),
                    team: who.team.or(Some(Team::Fed)),
                    ship: who.ship,
                    gfx: parse_gfx(&who.gfx),
                    mute: who.mute,
                })
            })
        }
    };
    if let Err(e) = result {
        eprintln!("netrek: {}", e);
        std::process::exit(1);
    }
}
