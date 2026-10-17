//! Making sure the terminal can show the client's SIXEL graphics before the
//! game starts. The client only runs with images; other terminals get an
//! error explaining what to use instead.

use std::io::{self, IsTerminal, Write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

/// What the terminal told us about itself.
#[derive(Default, Debug, PartialEq)]
pub struct Answers {
    /// Its device attributes list SIXEL graphics (feature 4).
    pub sixel: bool,
    /// The size of a character cell in pixels (width, height), if it said.
    pub cell_px: Option<(f64, f64)>,
}

/// What the environment says about image support.
enum Known {
    Yes,
    No(&'static str),
    Unknown,
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

/// Whether the terminal (and tmux, if we're inside it) is known to show
/// SIXEL images, known not to, or neither.
fn known_support() -> Known {
    if std::env::var_os("TMUX").is_some() {
        if let Some(app) = tmux_outer_app() {
            if NO_SIXEL_APPS.iter().any(|n| app.contains(n)) {
                return Known::No("tmux is attached from a terminal app that can't show images; attach it from iTerm2 (or WezTerm, foot) instead.");
            }
        }
        let tmux_sixel = std::process::Command::new("tmux")
            .args(["display", "-p", "#{client_termfeatures}"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("sixel"))
            .unwrap_or(false);
        if !tmux_sixel {
            return Known::No(
                "tmux isn't passing SIXEL images through. Add  set -as terminal-features ',xterm*:sixel'  to ~/.tmux.conf (tmux 3.4 or newer) and reattach.",
            );
        }
        return Known::Yes;
    }
    let prog = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let term = std::env::var("TERM").unwrap_or_default();
    if matches!(prog.as_str(), "iTerm.app" | "WezTerm") || ["foot", "mlterm", "contour"].iter().any(|t| term.contains(t)) {
        return Known::Yes;
    }
    if prog == "Apple_Terminal" {
        return Known::No("Terminal.app can't show images; run Netrek in iTerm2 (or WezTerm, foot) instead.");
    }
    if prog.eq_ignore_ascii_case("ghostty") || term.contains("kitty") || term.contains("alacritty") || term.contains("ghostty") {
        return Known::No("this terminal can't show SIXEL images; run Netrek in iTerm2 (or WezTerm, foot) instead.");
    }
    Known::Unknown
}

/// Check the terminal can run the client, before anything starts. On
/// success returns what the terminal reported about itself.
pub fn require_vector() -> Result<Answers, String> {
    const NEED: &str = "Netrek needs a terminal that can show SIXEL images";
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(format!("{} (run it in a terminal window, not through a pipe or redirect)", NEED));
    }
    let known = known_support();
    if let Known::No(why) = known {
        return Err(format!("{}: {}", NEED, why));
    }
    let answers = query().unwrap_or_default();
    match known {
        Known::Yes => Ok(answers),
        _ if answers.sixel => Ok(answers),
        _ => Err(format!(
            "{}, and this one doesn't report SIXEL support. Use iTerm2, WezTerm, foot, mlterm or another SIXEL terminal (or tmux 3.4+ in front of one).",
            NEED
        )),
    }
}

/// Ask the terminal about itself: `ESC [ 16 t` for the cell size, then
/// `ESC [ c` (primary device attributes), which every terminal answers, so
/// its reply marks the end. Gives up after a second.
fn query() -> io::Result<Answers> {
    crossterm::terminal::enable_raw_mode()?;
    let result = (|| {
        let mut out = io::stdout();
        out.write_all(b"\x1b[16t\x1b[c")?;
        out.flush()?;
        let stdin = io::stdin();
        let fd = stdin.as_raw_fd();
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut got = Vec::new();
        while !has_device_attributes(&got) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
            // SAFETY: one valid pollfd, and the timeout is bounded.
            let n = unsafe { libc::poll(&mut pfd, 1, left.as_millis().max(1) as libc::c_int) };
            if n <= 0 {
                break;
            }
            // Straight from the descriptor: Rust's buffered stdin could hold
            // back keys typed meanwhile, which the game's input reader
            // would then never see.
            let mut buf = [0u8; 256];
            // SAFETY: reads at most buf.len() bytes into buf.
            let k = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
            if k <= 0 {
                break;
            }
            got.extend_from_slice(&buf[..k as usize]);
        }
        Ok(parse_answers(&got))
    })();
    let _ = crossterm::terminal::disable_raw_mode();
    result
}

/// Whether the bytes hold a whole device-attributes reply (`ESC [ ? ... c`).
fn has_device_attributes(b: &[u8]) -> bool {
    find(b, b"\x1b[?").map_or(false, |i| b[i + 3..].iter().any(|&c| c == b'c'))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Pull the replies out of what the terminal sent back: `ESC [ ? 62 ; 4 ; ... c`
/// and `ESC [ 6 ; height ; width t`.
pub fn parse_answers(b: &[u8]) -> Answers {
    let s = String::from_utf8_lossy(b);
    let mut a = Answers::default();
    if let Some(i) = s.find("\x1b[?") {
        if let Some(j) = s[i..].find('c') {
            a.sixel = s[i + 3..i + j].split(';').any(|p| p == "4");
        }
    }
    if let Some(i) = s.find("\x1b[6;") {
        if let Some(j) = s[i..].find('t') {
            let nums: Vec<f64> = s[i + 4..i + j].split(';').filter_map(|n| n.parse().ok()).collect();
            if let [h, w] = nums[..] {
                if w > 0.0 && h > 0.0 {
                    a.cell_px = Some((w, h));
                }
            }
        }
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_terminal_replies() {
        let a = parse_answers(b"\x1b[6;17;8t\x1b[?62;4;22c");
        assert_eq!(a, Answers { sixel: true, cell_px: Some((8.0, 17.0)) });
        // No 4 in the attributes: no SIXEL (24 must not count).
        let a = parse_answers(b"\x1b[?62;22;24c");
        assert_eq!(a, Answers { sixel: false, cell_px: None });
        assert!(has_device_attributes(b"\x1b[6;17;8t\x1b[?1;2c"));
        assert!(!has_device_attributes(b"\x1b[6;17;8t\x1b[?1;2"));
        assert!(!parse_answers(b"").sixel);
    }
}
