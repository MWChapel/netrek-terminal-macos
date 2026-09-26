//! Sound effects. Each effect is synthesized at startup (retro square waves,
//! sweeps and filtered noise), written to a temporary WAV file, and played
//! with the system's command-line player (`afplay` on macOS; `paplay` or
//! `aplay` on Linux) so no audio libraries are needed.

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const RATE: u32 = 22050;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sfx {
    Torp,
    Phaser,
    Plasma,
    Explosion,
    Death,
    TorpBurst,
    Hit,
    ShieldUp,
    ShieldDown,
    Cloak,
    Alert,
    Message,
    Warning,
    Orbit,
    Capture,
}

impl Sfx {
    const ALL: [Sfx; 15] = [
        Sfx::Torp,
        Sfx::Phaser,
        Sfx::Plasma,
        Sfx::Explosion,
        Sfx::Death,
        Sfx::TorpBurst,
        Sfx::Hit,
        Sfx::ShieldUp,
        Sfx::ShieldDown,
        Sfx::Cloak,
        Sfx::Alert,
        Sfx::Message,
        Sfx::Warning,
        Sfx::Orbit,
        Sfx::Capture,
    ];

    /// Minimum time between two plays of the same effect.
    fn cooldown(self) -> Duration {
        Duration::from_millis(match self {
            Sfx::Torp => 60,
            Sfx::TorpBurst => 90,
            Sfx::Hit => 150,
            Sfx::Explosion => 120,
            Sfx::Alert => 8000,
            Sfx::Warning => 400,
            Sfx::Message => 250,
            _ => 100,
        })
    }
}

pub struct Sound {
    pub enabled: bool,
    player: Option<&'static str>,
    dir: PathBuf,
    files: HashMap<Sfx, PathBuf>,
    last: HashMap<Sfx, Instant>,
    children: Vec<Child>,
}

impl Sound {
    pub fn new(enabled: bool) -> Sound {
        let player = ["afplay", "paplay", "aplay"].into_iter().find(|p| {
            Command::new("which").arg(p).stdout(Stdio::null()).stderr(Stdio::null()).status().map_or(false, |s| s.success())
        });
        let dir = std::env::temp_dir().join(format!("netrek-sfx-{}", std::process::id()));
        let mut s = Sound { enabled: enabled && player.is_some(), player, dir, files: HashMap::new(), last: HashMap::new(), children: Vec::new() };
        if s.player.is_some() && std::fs::create_dir_all(&s.dir).is_ok() {
            for fx in Sfx::ALL {
                let path = s.dir.join(format!("{:?}.wav", fx).to_lowercase());
                if std::fs::write(&path, wav(&synth(fx))).is_ok() {
                    s.files.insert(fx, path);
                }
            }
        }
        s
    }

    pub fn available(&self) -> bool {
        self.player.is_some()
    }

    /// Play an effect at `volume` (0..1), unless muted or played too recently.
    pub fn play(&mut self, fx: Sfx, volume: f32) {
        if !self.enabled || volume <= 0.02 {
            return;
        }
        let now = Instant::now();
        if self.last.get(&fx).map_or(false, |t| now.duration_since(*t) < fx.cooldown()) {
            return;
        }
        self.children.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
        if self.children.len() > 12 {
            return; // don't pile up players
        }
        let (Some(player), Some(path)) = (self.player, self.files.get(&fx)) else { return };
        let mut cmd = Command::new(player);
        if player == "afplay" {
            cmd.arg("-v").arg(format!("{:.2}", volume.clamp(0.0, 1.0)));
        }
        cmd.arg(path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Ok(child) = cmd.spawn() {
            self.children.push(child);
            self.last.insert(fx, now);
        }
    }
}

impl Drop for Sound {
    fn drop(&mut self) {
        for c in self.children.iter_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// ----------------------------------------------------------------------
// synthesis

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn square(phase: f32) -> f32 {
    if phase.fract() < 0.5 {
        1.0
    } else {
        -1.0
    }
}

fn saw(phase: f32) -> f32 {
    phase.fract() * 2.0 - 1.0
}

/// A tone whose frequency follows `freq(t)` (t in 0..1), shaped by an
/// attack/decay envelope.
fn sweep(dur: f32, wave: fn(f32) -> f32, freq: impl Fn(f32) -> f32, amp: f32) -> Vec<f32> {
    let n = (dur * RATE as f32) as usize;
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            phase += freq(t) / RATE as f32;
            let env = (t * 60.0).min(1.0) * (1.0 - t).powf(1.5);
            wave(phase) * env * amp
        })
        .collect()
}

/// Low-passed noise with a decay; `bright` 0..1 controls the filter.
fn boom(dur: f32, bright: f32, amp: f32, seed: u32) -> Vec<f32> {
    let n = (dur * RATE as f32) as usize;
    let mut rng = Noise(seed);
    let mut lp = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let k = bright * (1.0 - t * 0.8);
            lp += (rng.next() - lp) * k;
            let env = (t * 200.0).min(1.0) * (1.0 - t).powi(3);
            lp * env * amp * 2.5
        })
        .collect()
}

fn mix(a: &mut Vec<f32>, b: &[f32], offset: usize) {
    if a.len() < offset + b.len() {
        a.resize(offset + b.len(), 0.0);
    }
    for (i, v) in b.iter().enumerate() {
        a[offset + i] += v;
    }
}

fn at(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

/// Phaser: a Trek-style energy whine. Two slightly detuned sawtooths near
/// 1.6 kHz beat against each other, with a sub-octave and a shimmering
/// overtone, a 46 Hz power buzz, a bright crackle as the beam lances out,
/// and a drooping pitch as it dies, all softened by a low-pass filter.
fn phaser() -> Vec<f32> {
    let dur = 0.55;
    let n = (dur * RATE as f32) as usize;
    let rate = RATE as f32;
    let mut noise = Noise(29);
    let (mut p1, mut p2, mut p3, mut p4) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let (mut lp, mut crackle_lp) = (0.0f32, 0.0f32);
    (0..n)
        .map(|i| {
            let secs = i as f32 / rate;
            let t = i as f32 / n as f32;
            let f = 1650.0 + 70.0 * (secs * TAU * 24.0).sin() - 380.0 * t * t;
            p1 += f / rate;
            p2 += f * 1.0072 / rate;
            p3 += f * 0.5 / rate;
            p4 += f * 2.003 / rate;
            let tone = 0.45 * saw(p1) + 0.45 * saw(p2) + 0.22 * square(p3) + 0.18 * (p4 * TAU).sin();
            let buzz = 1.0 - 0.35 * (0.5 + 0.5 * (secs * TAU * 46.0).sin());
            lp += (tone * buzz - lp) * 0.3;
            // High-passed noise burst for the initial crackle.
            let nz = noise.next();
            crackle_lp += (nz - crackle_lp) * 0.2;
            let crackle = (nz - crackle_lp) * (1.0 - secs / 0.06).max(0.0) * 0.9;
            let release = if t < 0.6 { 1.0 } else { (1.0 - (t - 0.6) / 0.4).powf(1.5) };
            let env = (secs * 300.0).min(1.0) * release;
            (lp * env + crackle) * 0.32
        })
        .collect()
}

fn synth(fx: Sfx) -> Vec<f32> {
    match fx {
        // Classic "pew": square wave diving from high to low.
        Sfx::Torp => sweep(0.14, square, |t| 1300.0 * (1.0 - t) + 250.0, 0.25),
        Sfx::Phaser => phaser(),
        // Plasma: slow, heavy warble.
        Sfx::Plasma => sweep(0.5, square, |t| 180.0 + 90.0 * (t * TAU * 9.0).sin() + 120.0 * t, 0.25),
        Sfx::Explosion => {
            let mut s = boom(0.9, 0.12, 0.9, 7);
            mix(&mut s, &sweep(0.5, square, |t| 120.0 * (1.0 - t) + 40.0, 0.15), 0);
            s
        }
        Sfx::Death => {
            let mut s = boom(1.6, 0.18, 1.0, 11);
            mix(&mut s, &boom(1.2, 0.05, 0.8, 23), at(0.15));
            mix(&mut s, &sweep(1.2, saw, |t| 400.0 * (1.0 - t) + 30.0, 0.2), 0);
            s
        }
        Sfx::TorpBurst => boom(0.25, 0.35, 0.6, 3),
        Sfx::Hit => {
            let mut s = boom(0.22, 0.5, 0.7, 5);
            mix(&mut s, &sweep(0.18, square, |_| 90.0, 0.25), 0);
            s
        }
        Sfx::ShieldUp => sweep(0.28, |p| (p * TAU).sin(), |t| 300.0 + 700.0 * t, 0.35),
        Sfx::ShieldDown => sweep(0.28, |p| (p * TAU).sin(), |t| 1000.0 - 700.0 * t, 0.35),
        Sfx::Cloak => {
            let mut s = boom(0.6, 0.03, 0.8, 17);
            mix(&mut s, &sweep(0.6, |p| (p * TAU).sin(), |t| 600.0 - 400.0 * t, 0.15), 0);
            s
        }
        // Two-tone red alert klaxon, three times.
        Sfx::Alert => {
            let mut s = Vec::new();
            for k in 0..3 {
                mix(&mut s, &sweep(0.22, square, |_| 740.0, 0.18), at(k as f32 * 0.5));
                mix(&mut s, &sweep(0.22, square, |_| 560.0, 0.18), at(k as f32 * 0.5 + 0.25));
            }
            s
        }
        Sfx::Message => {
            let mut s = sweep(0.07, |p| (p * TAU).sin(), |_| 1320.0, 0.3);
            mix(&mut s, &sweep(0.09, |p| (p * TAU).sin(), |_| 1760.0, 0.3), at(0.09));
            s
        }
        Sfx::Warning => sweep(0.12, square, |_| 440.0, 0.15),
        Sfx::Orbit => {
            let mut s = Vec::new();
            for (k, f) in [523.0, 659.0, 784.0].into_iter().enumerate() {
                mix(&mut s, &sweep(0.35, |p| (p * TAU).sin(), move |_| f, 0.22), at(k as f32 * 0.08));
            }
            s
        }
        // Planet captured: a short fanfare.
        Sfx::Capture => {
            let mut s = Vec::new();
            for (k, f) in [392.0, 523.0, 659.0, 784.0, 1046.0].into_iter().enumerate() {
                let len = if k == 4 { 0.5 } else { 0.13 };
                mix(&mut s, &sweep(len, square, move |_| f, 0.14), at(k as f32 * 0.11));
            }
            s
        }
    }
}

/// 16-bit mono PCM WAV.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32000.0) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every effect synthesizes to a non-silent, correctly sized WAV.
    #[test]
    fn effects_render() {
        let dir = std::env::temp_dir().join("netrek-sfx-test");
        std::fs::create_dir_all(&dir).unwrap();
        for fx in Sfx::ALL {
            let s = synth(fx);
            assert!(!s.is_empty(), "{:?} is empty", fx);
            assert!(s.iter().any(|v| v.abs() > 0.05), "{:?} is silent", fx);
            let w = wav(&s);
            assert_eq!(w.len(), 44 + s.len() * 2);
            std::fs::write(dir.join(format!("{:?}.wav", fx).to_lowercase()), w).unwrap();
        }
    }
}
