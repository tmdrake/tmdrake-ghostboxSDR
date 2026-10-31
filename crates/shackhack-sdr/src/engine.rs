use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, Sender, TryRecvError};
use parking_lot::Mutex;
use shackhack_bands::{BandPreset, Demod, SweepMode, DEFAULT_GAIN_TENTH_DB, DEFAULT_SAMPLE_RATE};
use shackhack_dsp::fft::FftProcessor;
use shackhack_dsp::iq::{peak_magnitude, u8_iq_to_complex, ComplexSample};
use shackhack_dsp::sweep::{Mode, SweepConfig, Sweeper};
use shackhack_dsp::vfo::{DemodKind, Vfo, VFO_COUNT};
use shackhack_dsp::window::window_contains;

use crate::source::{IqSource, SourceKind};

#[derive(Debug, Clone)]
pub enum Command {
    SetSweeping(bool),
    ApplyPreset {
        start_hz: u64,
        end_hz: u64,
        step_hz: u64,
        dwell_ms: u32,
        demod: Demod,
        mode: SweepMode,
    },
    SetMode(SweepMode),
    SetDwell(u32),
    SetStep(u64),
    SetRange {
        min_hz: u64,
        max_hz: u64,
    },
    SetDemod(Demod),
    SetGain(i32),
    TuneA(u64),
    ParkVfo {
        index: usize,
        hz: u64,
    },
    EnableVfo {
        index: usize,
        on: bool,
    },
    SetMix {
        index: usize,
        mix: f32,
    },
    SetVolume(f32),
    SetMute(bool),
    Stop,
}

#[derive(Debug, Clone)]
pub struct VfoSnap {
    pub enabled: bool,
    pub freq_hz: f64,
    pub display_hz: f64,
    pub mix: f32,
    pub demod: DemodKind,
    pub in_window: bool,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub connected: bool,
    pub device_label: String,
    pub tuner_min_hz: u64,
    pub tuner_max_hz: u64,
    pub hf_ok: bool,
    pub gains: Vec<i32>,
    pub gain_tenth_db: i32,
    pub lo_hz: f64,
    pub sample_rate: f64,
    pub spectrum: Vec<f32>,
    pub floor_db: f32,
    pub peak_db: f32,
    pub clip: bool,
    pub vfos: [VfoSnap; VFO_COUNT],
    pub sweeping: bool,
    pub min_hz: u64,
    pub max_hz: u64,
    pub step_hz: u64,
    pub dwell_ms: u32,
    pub mode: Mode,
    pub demod: DemodKind,
    pub error: Option<String>,
    pub audio_rms: f32,
    pub hop: bool,
    pub volume: f32,
    pub mute: bool,
}

impl Snapshot {
    fn disconnected(err: Option<String>) -> Self {
        Self {
            connected: false,
            device_label: "No device".into(),
            tuner_min_hz: shackhack_bands::GENERIC_MIN_HZ,
            tuner_max_hz: shackhack_bands::GENERIC_MAX_HZ,
            hf_ok: false,
            gains: vec![DEFAULT_GAIN_TENTH_DB],
            gain_tenth_db: DEFAULT_GAIN_TENTH_DB,
            lo_hz: 98_000_000.0,
            sample_rate: DEFAULT_SAMPLE_RATE as f64,
            spectrum: vec![-80.0; 4096],
            floor_db: -80.0,
            peak_db: -40.0,
            clip: false,
            vfos: std::array::from_fn(|i| VfoSnap {
                enabled: i == 0,
                freq_hz: 98_500_000.0,
                display_hz: 98_500_000.0,
                mix: if i == 0 { 1.0 } else { 0.35 },
                demod: DemodKind::Wfm,
                in_window: true,
            }),
            sweeping: false,
            min_hz: 88_000_000,
            max_hz: 108_000_000,
            step_hz: 150_000,
            dwell_ms: 30,
            mode: Mode::Bounce,
            demod: DemodKind::Wfm,
            error: err,
            audio_rms: 0.0,
            hop: false,
            volume: 0.8,
            mute: false,
        }
    }
}

pub struct RadioHandle {
    pub snapshot: Arc<Mutex<Snapshot>>,
    pub audio: Arc<Mutex<VecDeque<f32>>>,
    cmd: Sender<Command>,
}

impl RadioHandle {
    pub fn send(&self, cmd: Command) {
        let _ = self.cmd.send(cmd);
    }
}

pub fn start(kind: SourceKind) -> RadioHandle {
    let snapshot = Arc::new(Mutex::new(Snapshot::disconnected(None)));
    let audio = Arc::new(Mutex::new(VecDeque::with_capacity(48_000)));
    let (cmd_tx, cmd_rx) = bounded::<Command>(64);
    let snap = snapshot.clone();
    let audio_q = audio.clone();

    thread::Builder::new()
        .name("shackhack-radio".into())
        .spawn(move || {
            let mut source = match kind {
                SourceKind::Rtl => match IqSource::open_rtl() {
                    Ok(s) => s,
                    Err(e) => {
                        snap.lock().error = Some(e.to_string());
                        return;
                    }
                },
                SourceKind::Synth => IqSource::open_synth(DEFAULT_SAMPLE_RATE),
            };

            let info = source.info();
            {
                let mut s = snap.lock();
                s.connected = true;
                s.device_label = info.label.clone();
                s.tuner_min_hz = info.min_hz;
                s.tuner_max_hz = info.max_hz;
                s.hf_ok = info.hf_ok;
                s.gains = info.gains.clone();
                s.error = None;
            }

            let preset = shackhack_bands::default_preset();
            let sample_rate = DEFAULT_SAMPLE_RATE;
            let mut gain = nearest_gain(&info.gains, DEFAULT_GAIN_TENTH_DB);
            let mut lo = 98_000_000.0_f64;
            if let Err(e) = source.start(lo as u32, sample_rate, gain) {
                snap.lock().error = Some(e.to_string());
                return;
            }

            let sr = sample_rate as f64;
            let mut sweeper = Sweeper::new(config_from_preset(preset, SweepMode::Bounce), sr);
            sweeper.freq_hz = preset.start_hz + (preset.end_hz - preset.start_hz) / 2;
            let mut vfos: [Vfo; VFO_COUNT] = std::array::from_fn(|i| {
                let mut v = Vfo::new(i == 0, if i == 0 { 1.0 } else { 0.35 });
                v.configure(sr, DemodKind::Wfm);
                v.freq_hz = sweeper.freq_hz as f64;
                v
            });
            let mut fft = FftProcessor::new(4096);
            let mut iq: Vec<ComplexSample> = Vec::new();
            let mut fft_acc: Vec<ComplexSample> = Vec::new();
            let mut sweeping = true;
            let mut fade = 1.0_f32;
            let mut hop_fade = 0u32;
            let mut volume = 0.8_f32;
            let mut mute = false;
            let mut demod = DemodKind::Wfm;
            let mut running = true;
            let tuner_min = info.min_hz as f64;
            let tuner_max = info.max_hz as f64;

            lo = sweeper.needed_lo(sr, tuner_min, tuner_max);
            let _ = source.hop(lo as u32);

            while running {
                loop {
                    match cmd_rx.try_recv() {
                        Ok(cmd) => match cmd {
                            Command::Stop => running = false,
                            Command::SetSweeping(on) => sweeping = on,
                            Command::ApplyPreset {
                                start_hz,
                                end_hz,
                                step_hz,
                                dwell_ms,
                                demod: d,
                                mode,
                            } => {
                                demod = to_kind(d);
                                for v in &mut vfos {
                                    v.configure(sr, demod);
                                }
                                sweeper.apply_config(
                                    SweepConfig {
                                        min_hz: start_hz,
                                        max_hz: end_hz,
                                        step_hz,
                                        dwell_ms,
                                        mode: to_mode(mode),
                                    },
                                    sr,
                                );
                                sweeper.freq_hz = start_hz;
                                vfos[0].freq_hz = start_hz as f64;
                                lo = sweeper.needed_lo(sr, tuner_min, tuner_max);
                                hop_now(&mut source, lo, &mut hop_fade, &mut fade);
                            }
                            Command::SetMode(m) => {
                                sweeper.cfg.mode = to_mode(m);
                            }
                            Command::SetDwell(ms) => {
                                sweeper.cfg.dwell_ms = ms.max(5);
                                sweeper.set_sample_rate(sr);
                            }
                            Command::SetStep(hz) => sweeper.cfg.step_hz = hz.max(1),
                            Command::SetRange { min_hz, max_hz } => {
                                sweeper.cfg.min_hz = min_hz;
                                sweeper.cfg.max_hz = max_hz.max(min_hz + sweeper.cfg.step_hz);
                            }
                            Command::SetDemod(d) => {
                                demod = to_kind(d);
                                for v in &mut vfos {
                                    v.configure(sr, demod);
                                }
                            }
                            Command::SetGain(g) => {
                                gain = nearest_gain(&info.gains, g);
                                let _ = source.set_gain(gain);
                            }
                            Command::TuneA(hz) => {
                                sweeper.freq_hz = hz;
                                sweeper.prev_freq_hz = hz;
                                vfos[0].freq_hz = hz as f64;
                                if sweeper.needs_hop(lo, sr) {
                                    lo = sweeper.needed_lo(sr, tuner_min, tuner_max);
                                    hop_now(&mut source, lo, &mut hop_fade, &mut fade);
                                }
                            }
                            Command::ParkVfo { index, hz } => {
                                if let Some(v) = vfos.get_mut(index) {
                                    v.freq_hz = hz as f64;
                                    v.enabled = true;
                                }
                            }
                            Command::EnableVfo { index, on } => {
                                if let Some(v) = vfos.get_mut(index) {
                                    v.enabled = on;
                                }
                            }
                            Command::SetMix { index, mix } => {
                                if let Some(v) = vfos.get_mut(index) {
                                    v.mix = mix.clamp(0.0, 1.5);
                                }
                            }
                            Command::SetVolume(v) => volume = v.clamp(0.0, 1.5),
                            Command::SetMute(m) => mute = m,
                        },
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            running = false;
                            break;
                        }
                    }
                }
                if !running {
                    break;
                }

                let chunk = match source.next_chunk() {
                    Ok(Some(c)) => c,
                    Ok(None) => break,
                    Err(e) => {
                        snap.lock().error = Some(e.to_string());
                        thread::sleep(Duration::from_millis(200));
                        continue;
                    }
                };

                u8_iq_to_complex(&chunk, &mut iq);
                let n = iq.len() as u64;
                let clip = peak_magnitude(&iq) > 0.92;

                if hop_fade > 0 {
                    hop_fade -= 1;
                    fade = 0.15;
                } else {
                    fade = (fade + 0.15).min(1.0);
                }

                if sweeping {
                    vfos[0].freq_hz = sweeper.freq_hz as f64;
                    let stepped = sweeper.consume_samples(n);
                    if stepped {
                        vfos[0].freq_hz = sweeper.freq_hz as f64;
                        if sweeper.needs_hop(lo, sr) {
                            lo = sweeper.needed_lo(sr, tuner_min, tuner_max);
                            hop_now(&mut source, lo, &mut hop_fade, &mut fade);
                        }
                    }
                }

                let vol = if mute { 0.0 } else { volume };
                let mut mix_buf: Vec<f32> = Vec::new();
                for v in &mut vfos {
                    let audio = v.process(&iq, lo, sr, fade * vol);
                    if mix_buf.len() < audio.len() {
                        mix_buf.resize(audio.len(), 0.0);
                    }
                    for (o, a) in mix_buf.iter_mut().zip(audio.iter()) {
                        *o += *a;
                    }
                }
                let mut rms = 0.0_f32;
                if !mix_buf.is_empty() {
                    rms =
                        (mix_buf.iter().map(|x| x * x).sum::<f32>() / mix_buf.len() as f32).sqrt();
                    let mut q = audio_q.lock();
                    q.extend(mix_buf.iter().copied());
                    while q.len() > 24_000 {
                        q.pop_front();
                    }
                }

                fft_acc.extend_from_slice(&iq);
                if fft_acc.len() >= fft.size() {
                    let spec = fft.process(&fft_acc);
                    fft_acc.clear();
                    let hop_flag = hop_fade > 0;
                    let mut s = snap.lock();
                    s.connected = true;
                    s.lo_hz = lo;
                    s.sample_rate = sr;
                    s.spectrum = spec.db;
                    s.floor_db = spec.floor_db;
                    s.peak_db = spec.peak_db;
                    s.clip = clip;
                    s.gain_tenth_db = gain;
                    s.sweeping = sweeping;
                    s.min_hz = sweeper.cfg.min_hz;
                    s.max_hz = sweeper.cfg.max_hz;
                    s.step_hz = sweeper.cfg.step_hz;
                    s.dwell_ms = sweeper.cfg.dwell_ms;
                    s.mode = sweeper.cfg.mode;
                    s.demod = demod;
                    s.audio_rms = rms;
                    s.hop = hop_flag;
                    s.volume = volume;
                    s.mute = mute;
                    for (i, v) in vfos.iter().enumerate() {
                        let display = if i == 0 && sweeping {
                            sweeper.display_hz()
                        } else {
                            v.freq_hz
                        };
                        s.vfos[i] = VfoSnap {
                            enabled: v.enabled,
                            freq_hz: v.freq_hz,
                            display_hz: display,
                            mix: v.mix,
                            demod,
                            in_window: window_contains(lo, v.freq_hz, sr),
                        };
                    }
                }
            }
        })
        .expect("spawn radio thread");

    RadioHandle {
        snapshot,
        audio,
        cmd: cmd_tx,
    }
}

fn hop_now(source: &mut IqSource, lo: f64, hop_fade: &mut u32, fade: &mut f32) {
    let _ = source.hop(lo.max(0.0) as u32);
    *hop_fade = 3;
    *fade = 0.0;
}

fn nearest_gain(gains: &[i32], want: i32) -> i32 {
    if gains.is_empty() {
        return want;
    }
    *gains
        .iter()
        .min_by_key(|g| (**g - want).abs())
        .unwrap_or(&want)
}

fn to_kind(d: Demod) -> DemodKind {
    match d {
        Demod::Wfm => DemodKind::Wfm,
        Demod::Nfm => DemodKind::Nfm,
        Demod::Am => DemodKind::Am,
    }
}

fn to_mode(m: SweepMode) -> Mode {
    match m {
        SweepMode::Bounce => Mode::Bounce,
        SweepMode::Forward => Mode::Forward,
        SweepMode::Backward => Mode::Backward,
        SweepMode::Random => Mode::Random,
    }
}

fn config_from_preset(p: &BandPreset, mode: SweepMode) -> SweepConfig {
    SweepConfig {
        min_hz: p.start_hz,
        max_hz: p.end_hz,
        step_hz: p.step_hz,
        dwell_ms: p.dwell_ms,
        mode: to_mode(mode),
    }
}

#[cfg(test)]
mod tests {
    use super::nearest_gain;

    #[test]
    fn snaps_to_closest_tenth_db() {
        let g = [0, 90, 140, 270, 296, 328, 496];
        assert_eq!(nearest_gain(&g, 300), 296);
        assert_eq!(nearest_gain(&g, 0), 0);
        assert_eq!(nearest_gain(&[], 296), 296);
    }
}
