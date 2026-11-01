//! Controls saved between launches.
//!
//! Windows: `%LOCALAPPDATA%\ShackHack\settings.txt`
//! Linux: `$HOME/.local/share/ShackHack/settings.txt`

use std::fs;
use std::path::PathBuf;

use shackhack_bands::{Demod, SweepMode};
use shackhack_dsp::vfo::VFO_COUNT;
use shackhack_sdr::{Command, RadioHandle};

#[derive(Clone, Debug)]
pub struct Settings {
    pub preset_id: String,
    pub min_mhz: f64,
    pub max_mhz: f64,
    pub gain_db: f32,
    pub volume: f32,
    pub mute: bool,
    pub scanning: bool,
    pub dwell_ms: u32,
    pub step_hz: u64,
    pub mode: SweepMode,
    pub demod: Demod,
    pub vfo_on: [bool; VFO_COUNT],
    pub vfo_mix: [f32; VFO_COUNT],
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preset_id: "fm".into(),
            min_mhz: 88.0,
            max_mhz: 108.0,
            gain_db: 29.6,
            volume: 0.8,
            mute: false,
            scanning: true,
            dwell_ms: 30,
            step_hz: 150_000,
            mode: SweepMode::Bounce,
            demod: Demod::Wfm,
            vfo_on: [true; VFO_COUNT],
            vfo_mix: {
                let mut mix = [0.12; VFO_COUNT];
                mix[0] = 1.0;
                mix
            },
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Self::default();
        let Ok(text) = fs::read_to_string(path()) else {
            return s;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "preset" => s.preset_id = v.to_string(),
                "min_mhz" => s.min_mhz = v.parse().unwrap_or(s.min_mhz),
                "max_mhz" => s.max_mhz = v.parse().unwrap_or(s.max_mhz),
                "gain_db" => s.gain_db = v.parse().unwrap_or(s.gain_db),
                "volume" => s.volume = v.parse().unwrap_or(s.volume),
                "mute" => s.mute = v == "1" || v.eq_ignore_ascii_case("true"),
                "scanning" => s.scanning = v != "0" && !v.eq_ignore_ascii_case("false"),
                "dwell_ms" => s.dwell_ms = v.parse().unwrap_or(s.dwell_ms),
                "step_hz" => s.step_hz = v.parse().unwrap_or(s.step_hz),
                "mode" => s.mode = parse_mode(v),
                "demod" => s.demod = parse_demod(v),
                "vfo_on" => s.vfo_on = parse_bools(v, s.vfo_on),
                "vfo_mix" => s.vfo_mix = parse_floats(v, s.vfo_mix),
                _ => {}
            }
        }
        if s.min_mhz > s.max_mhz {
            std::mem::swap(&mut s.min_mhz, &mut s.max_mhz);
        }
        s
    }

    pub fn save(&self) {
        let path = path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let on = self
            .vfo_on
            .iter()
            .map(|b| if *b { "1" } else { "0" })
            .collect::<Vec<_>>()
            .join(",");
        let mix = self
            .vfo_mix
            .iter()
            .map(|m| format!("{m:.3}"))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!(
            "preset={}\nmin_mhz={}\nmax_mhz={}\ngain_db={}\nvolume={}\nmute={}\nscanning={}\ndwell_ms={}\nstep_hz={}\nmode={}\ndemod={}\nvfo_on={on}\nvfo_mix={mix}\n",
            self.preset_id,
            self.min_mhz,
            self.max_mhz,
            self.gain_db,
            self.volume,
            if self.mute { "1" } else { "0" },
            if self.scanning { "1" } else { "0" },
            self.dwell_ms,
            self.step_hz,
            mode_str(self.mode),
            demod_str(self.demod),
        );
        let _ = fs::write(path, text);
    }

    pub fn push(&self, radio: &RadioHandle) {
        if self.preset_id == "custom" || self.preset_id == "full" {
            radio.send(Command::SetRange {
                min_hz: (self.min_mhz * 1e6) as u64,
                max_hz: (self.max_mhz * 1e6) as u64,
            });
        } else if let Some(p) = shackhack_bands::preset_by_id(&self.preset_id) {
            radio.send(Command::ApplyPreset {
                start_hz: p.start_hz,
                end_hz: p.end_hz,
                step_hz: self.step_hz,
                dwell_ms: self.dwell_ms,
                demod: self.demod,
                mode: self.mode,
            });
        }
        radio.send(Command::SetMode(self.mode));
        radio.send(Command::SetDwell(self.dwell_ms));
        radio.send(Command::SetStep(self.step_hz));
        radio.send(Command::SetDemod(self.demod));
        radio.send(Command::SetGain((self.gain_db * 10.0).round() as i32));
        radio.send(Command::SetVolume(self.volume));
        radio.send(Command::SetMute(self.mute));
        radio.send(Command::SetSweeping(self.scanning));
        for i in 0..VFO_COUNT {
            radio.send(Command::EnableVfo {
                index: i,
                on: self.vfo_on[i],
            });
            radio.send(Command::SetMix {
                index: i,
                mix: self.vfo_mix[i],
            });
        }
    }
}

fn path() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        if !local.is_empty() {
            return PathBuf::from(local).join("ShackHack").join("settings.txt");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("ShackHack")
        .join("settings.txt")
}

fn parse_mode(s: &str) -> SweepMode {
    match s.to_ascii_lowercase().as_str() {
        "forward" => SweepMode::Forward,
        "backward" => SweepMode::Backward,
        "random" => SweepMode::Random,
        _ => SweepMode::Bounce,
    }
}

fn mode_str(m: SweepMode) -> &'static str {
    match m {
        SweepMode::Bounce => "bounce",
        SweepMode::Forward => "forward",
        SweepMode::Backward => "backward",
        SweepMode::Random => "random",
    }
}

fn parse_demod(s: &str) -> Demod {
    match s.to_ascii_lowercase().as_str() {
        "nfm" => Demod::Nfm,
        "am" => Demod::Am,
        _ => Demod::Wfm,
    }
}

fn demod_str(d: Demod) -> &'static str {
    match d {
        Demod::Wfm => "wfm",
        Demod::Nfm => "nfm",
        Demod::Am => "am",
    }
}

fn parse_bools(s: &str, mut fallback: [bool; VFO_COUNT]) -> [bool; VFO_COUNT] {
    for (i, part) in s.split(',').take(VFO_COUNT).enumerate() {
        fallback[i] = part.trim() != "0";
    }
    fallback
}

fn parse_floats(s: &str, mut fallback: [f32; VFO_COUNT]) -> [f32; VFO_COUNT] {
    for (i, part) in s.split(',').take(VFO_COUNT).enumerate() {
        if let Ok(v) = part.trim().parse::<f32>() {
            fallback[i] = v.clamp(0.0, 1.5);
        }
    }
    fallback
}
