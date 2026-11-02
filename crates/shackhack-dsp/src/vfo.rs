use crate::iq::ComplexSample;
use crate::AUDIO_RATE;

pub const VFO_COUNT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemodKind {
    Wfm,
    Nfm,
    Am,
}

impl DemodKind {
    pub fn passband_hz(self) -> f64 {
        match self {
            DemodKind::Wfm => 200_000.0,
            DemodKind::Nfm => 12_500.0,
            DemodKind::Am => 8_000.0,
        }
    }
}

/// Software VFO: NCO mix, one-pole IIR, boxcar decimate, analog demod, AGC.
pub struct Vfo {
    pub enabled: bool,
    pub freq_hz: f64,
    pub mix: f32,
    pub demod: DemodKind,
    /// Held on a click. The sweeper does not move a parked VFO.
    pub parked: bool,
    nco_phase: f64,
    iir_i: f32,
    iir_q: f32,
    prev_i: f32,
    prev_q: f32,
    dc: f32,
    agc: f32,
    deemph: f32,
    acc_i: f32,
    acc_q: f32,
    acc_n: u32,
    decim: u32,
    wfm_keep: u32,
    wfm_count: u32,
    agc_alpha: f32,
    lpf: Vec<f32>,
    lpf_hist: Vec<f32>,
    lpf_pos: usize,
    audio: Vec<f32>,
}

impl Vfo {
    pub fn new(enabled: bool, mix: f32) -> Self {
        Self {
            enabled,
            freq_hz: 98_500_000.0,
            mix,
            demod: DemodKind::Wfm,
            parked: false,
            nco_phase: 0.0,
            iir_i: 0.0,
            iir_q: 0.0,
            prev_i: 0.0,
            prev_q: 0.0,
            dc: 0.0,
            agc: 1.0,
            deemph: 0.0,
            acc_i: 0.0,
            acc_q: 0.0,
            acc_n: 0,
            decim: 50,
            wfm_keep: 5,
            wfm_count: 0,
            agc_alpha: agc_alpha(AUDIO_RATE),
            lpf: Vec::new(),
            lpf_hist: Vec::new(),
            lpf_pos: 0,
            audio: Vec::new(),
        }
    }

    pub fn configure(&mut self, sample_rate: f64, demod: DemodKind) {
        self.demod = demod;
        self.agc_alpha = agc_alpha(AUDIO_RATE);
        self.wfm_count = 0;
        self.lpf_pos = 0;
        match demod {
            DemodKind::Wfm => {
                self.decim = (sample_rate / 240_000.0).round().max(1.0) as u32;
                let if_rate = sample_rate / self.decim as f64;
                self.wfm_keep = (if_rate / AUDIO_RATE).round().max(1.0) as u32;
                // Broadcast FM is a stereo multiplex. The mono program sits
                // under 15 kHz. The 19 kHz pilot and the 38 kHz difference
                // signal have to be removed before the cut down to 48 kHz,
                // or they fold back into the voice.
                self.lpf = audio_lowpass(if_rate, 15_000.0, self.wfm_keep);
                self.lpf_hist = vec![0.0; self.lpf.len()];
            }
            DemodKind::Nfm | DemodKind::Am => {
                self.decim = (sample_rate / AUDIO_RATE).round().max(1.0) as u32;
                self.wfm_keep = 1;
                self.lpf.clear();
                self.lpf_hist.clear();
            }
        }
    }

    pub fn process(
        &mut self,
        samples: &[ComplexSample],
        lo_hz: f64,
        sample_rate: f64,
        fade: f32,
    ) -> &[f32] {
        self.audio.clear();
        if !self.enabled || sample_rate <= 0.0 {
            return &self.audio;
        }
        let offset = self.freq_hz - lo_hz;
        let nco_step = std::f64::consts::TAU * offset / sample_rate;
        let decim = self.decim.max(1);
        let fc = (self.demod.passband_hz() * 0.5).min(sample_rate * 0.45);
        let alpha = ((std::f64::consts::TAU * fc / sample_rate) as f32).clamp(0.0005, 0.5);

        for s in samples {
            self.nco_phase += nco_step;
            if self.nco_phase > std::f64::consts::TAU {
                self.nco_phase -= std::f64::consts::TAU;
            } else if self.nco_phase < -std::f64::consts::TAU {
                self.nco_phase += std::f64::consts::TAU;
            }
            let (sn, cs) = (self.nco_phase as f32).sin_cos();
            let mi = s.re * cs + s.im * sn;
            let mq = s.im * cs - s.re * sn;
            self.iir_i += alpha * (mi - self.iir_i);
            self.iir_q += alpha * (mq - self.iir_q);
            self.acc_i += self.iir_i;
            self.acc_q += self.iir_q;
            self.acc_n += 1;
            if self.acc_n >= decim {
                let i = self.acc_i / decim as f32;
                let q = self.acc_q / decim as f32;
                self.acc_i = 0.0;
                self.acc_q = 0.0;
                self.acc_n = 0;
                let mut a = match self.demod {
                    DemodKind::Am => {
                        let env = (i * i + q * q).sqrt();
                        self.dc = self.dc * 0.995 + env * 0.005;
                        env - self.dc
                    }
                    DemodKind::Wfm | DemodKind::Nfm => {
                        let num = q * self.prev_i - i * self.prev_q;
                        let den = i * self.prev_i + q * self.prev_q;
                        self.prev_i = i;
                        self.prev_q = q;
                        // den is the dot product. It is negative when the phase
                        // step passes 90 degrees. Clamping it folds broadcast FM,
                        // whose 75 kHz peaks are about 112 degrees at the 240 kHz IF.
                        fm_discriminator(num, den)
                    }
                };
                if self.demod == DemodKind::Wfm {
                    let dt = decim as f64 / sample_rate;
                    let x = (dt / 75e-6) as f32;
                    self.deemph += x * (a - self.deemph);
                    a = self.deemph;
                }
                let Some(a) = self.decimate_audio(a) else {
                    continue;
                };
                // The level tracker has to be slower than the audio itself.
                // A few milliseconds turns the voice into the control signal
                // and the station sounds crushed.
                let level = a.abs().max(1e-6);
                self.agc += self.agc_alpha * (level - self.agc);
                let gain = (0.2 / self.agc).clamp(0.2, 40.0);
                let sample = (a * gain * fade * self.mix).clamp(-1.0, 1.0);
                self.audio.push(sample);
            }
        }
        &self.audio
    }

    /// Low-pass, then keep one sample per `wfm_keep` so the output is 48 kHz.
    fn decimate_audio(&mut self, sample: f32) -> Option<f32> {
        let filtered = if self.lpf.is_empty() {
            sample
        } else {
            let n = self.lpf.len();
            self.lpf_hist[self.lpf_pos] = sample;
            self.lpf_pos += 1;
            if self.lpf_pos == n {
                self.lpf_pos = 0;
            }
            let mut acc = 0.0f32;
            let mut idx = (self.lpf_pos + n - 1) % n;
            for &tap in &self.lpf {
                acc += tap * self.lpf_hist[idx];
                idx = if idx == 0 { n - 1 } else { idx - 1 };
            }
            acc
        };
        self.wfm_count += 1;
        if self.wfm_count >= self.wfm_keep {
            self.wfm_count = 0;
            Some(filtered)
        } else {
            None
        }
    }
}

fn agc_alpha(audio_rate: f64) -> f32 {
    let tau = 0.20_f64;
    (1.0 - (-1.0 / (tau * audio_rate)).exp()) as f32
}

/// Odd-length Blackman sinc. Cuts the FM stereo multiplex off before decimation.
fn audio_lowpass(if_rate: f64, cutoff: f64, decim: u32) -> Vec<f32> {
    let nyquist = if_rate / decim.max(1) as f64 / 2.0;
    let width = (nyquist - cutoff).max(1_000.0);
    let mut n = ((3.3 * if_rate / width).round() as usize).clamp(15, 191);
    if n.is_multiple_of(2) {
        n += 1;
    }
    let mid = (n / 2) as f64;
    let mut taps = Vec::with_capacity(n);
    for i in 0..n {
        let x = i as f64 - mid;
        let sinc = if x.abs() < 1e-9 {
            2.0 * cutoff / if_rate
        } else {
            (std::f64::consts::TAU * cutoff / if_rate * x).sin() / (std::f64::consts::PI * x)
        };
        let p = i as f64 / (n as f64 - 1.0);
        let window = 0.42 - 0.5 * (std::f64::consts::TAU * p).cos()
            + 0.08 * (2.0 * std::f64::consts::TAU * p).cos();
        taps.push((sinc * window) as f32);
    }
    let sum: f32 = taps.iter().sum();
    if sum.abs() > 1e-12 {
        for tap in &mut taps {
            *tap /= sum;
        }
    }
    taps
}

/// `atan2(cross, dot)`. A zero vector has no phase step.
fn fm_discriminator(cross: f32, dot: f32) -> f32 {
    if cross.abs() < 1e-20 && dot.abs() < 1e-20 {
        0.0
    } else {
        cross.atan2(dot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex;

    #[test]
    fn am_carrier_makes_audio() {
        let mut v = Vfo::new(true, 1.0);
        v.freq_hz = 1_000_000.0;
        v.configure(2_400_000.0, DemodKind::Am);
        let mut iq = Vec::new();
        for n in 0..48_000 {
            let t = n as f64 / 2_400_000.0;
            let m = 0.5 + 0.5 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin();
            iq.push(Complex::new(m as f32, 0.0));
        }
        let audio = v.process(&iq, 1_000_000.0, 2_400_000.0, 1.0);
        assert!(audio.len() > 100);
        let energy: f32 = audio.iter().map(|x| x * x).sum::<f32>() / audio.len() as f32;
        assert!(energy > 1e-6);
    }

    #[test]
    fn fm_discriminator_keeps_steps_past_90_degrees() {
        let step = 120.0_f32.to_radians();
        let (q, i) = step.sin_cos();
        let phase = fm_discriminator(q, i);
        assert!(
            (phase - step).abs() < 0.01,
            "phase {phase} rad, expected {step}"
        );

        let (q, i) = (-step).sin_cos();
        let phase = fm_discriminator(q, i);
        assert!((phase - (-step)).abs() < 0.01, "phase {phase} rad");
    }

    #[test]
    fn wfm_demodulates_a_1_khz_tone_on_95_5() {
        let mut v = Vfo::new(true, 1.0);
        let sr = 2_400_000.0;
        let lo = 96_340_000.0;
        let freq = 95_500_000.0;
        v.freq_hz = freq;
        v.configure(sr, DemodKind::Wfm);
        let deviation = 40_000.0;
        let tone = 1_000.0;
        let n = 96_000;
        let mut iq = Vec::with_capacity(n);
        let mut phase = 0.0;
        for k in 0..n {
            let t = k as f64 / sr;
            let rf = freq + deviation * (std::f64::consts::TAU * tone * t).sin();
            phase += std::f64::consts::TAU * (rf - lo) / sr;
            iq.push(Complex::new(phase.cos() as f32, phase.sin() as f32));
        }
        let audio = v.process(&iq, lo, sr, 1.0);
        assert!(audio.len() > 800, "audio len {}", audio.len());
        let skip = 400;
        let rest = &audio[skip..];
        let mut sin_dot = 0.0f64;
        let mut cos_dot = 0.0f64;
        let mut energy = 0.0f64;
        for (i, &sample) in rest.iter().enumerate() {
            let t = (skip + i) as f64 / crate::AUDIO_RATE as f64;
            let w = std::f64::consts::TAU * tone * t;
            let a = sample as f64;
            sin_dot += a * w.sin();
            cos_dot += a * w.cos();
            energy += a * a;
        }
        let mean = energy / rest.len() as f64;
        assert!(mean > 1e-4, "audio energy {mean}");
        let corr = (sin_dot * sin_dot + cos_dot * cos_dot).sqrt()
            / (energy * rest.len() as f64 / 2.0).sqrt();
        assert!(corr > 0.9, "correlation {corr}");
    }

    #[test]
    fn wfm_rejects_the_stereo_subcarrier() {
        let mut v = Vfo::new(true, 1.0);
        let sr = 2_400_000.0;
        let lo = 96_340_000.0;
        let freq = 95_500_000.0;
        v.freq_hz = freq;
        v.configure(sr, DemodKind::Wfm);
        let n = 192_000;
        let mut iq = Vec::with_capacity(n);
        let mut phase = 0.0;
        for k in 0..n {
            let t = k as f64 / sr;
            let voice = 5_000.0 * (std::f64::consts::TAU * 1_000.0 * t).sin();
            let stereo = 50_000.0 * (std::f64::consts::TAU * 30_000.0 * t).sin();
            phase += std::f64::consts::TAU * (freq - lo + voice + stereo) / sr;
            iq.push(Complex::new(phase.cos() as f32, phase.sin() as f32));
        }
        let audio = v.process(&iq, lo, sr, 1.0);
        let skip = 800;
        assert!(audio.len() > skip + 400, "audio len {}", audio.len());
        let voice = correlate(&audio[skip..], 1_000.0);
        // 30 kHz folds to 18 kHz when samples are dropped to 48 kHz.
        let alias = correlate(&audio[skip..], 18_000.0);
        assert!(voice > 0.8, "voice correlation {voice}");
        assert!(alias < 0.15, "aliased stereo correlation {alias}");
    }

    #[test]
    fn nfm_demodulates_a_tone() {
        let mut v = Vfo::new(true, 1.0);
        let sr = 2_400_000.0;
        let freq = 162_550_000.0;
        v.freq_hz = freq;
        v.configure(sr, DemodKind::Nfm);
        let n = 96_000;
        let mut iq = Vec::with_capacity(n);
        let mut phase = 0.0;
        for k in 0..n {
            let t = k as f64 / sr;
            let dev = 2_500.0 * (std::f64::consts::TAU * 1_000.0 * t).sin();
            phase += std::f64::consts::TAU * dev / sr;
            iq.push(Complex::new(phase.cos() as f32, phase.sin() as f32));
        }
        let audio = v.process(&iq, freq, sr, 1.0);
        assert!(audio.len() > 400, "audio len {}", audio.len());
        let corr = correlate(&audio[200..], 1_000.0);
        assert!(corr > 0.85, "correlation {corr}");
    }

    fn correlate(audio: &[f32], hz: f64) -> f64 {
        let mut sin_dot = 0.0f64;
        let mut cos_dot = 0.0f64;
        let mut energy = 0.0f64;
        for (i, &sample) in audio.iter().enumerate() {
            let w = std::f64::consts::TAU * hz * i as f64 / crate::AUDIO_RATE;
            let a = sample as f64;
            sin_dot += a * w.sin();
            cos_dot += a * w.cos();
            energy += a * a;
        }
        if energy < 1e-12 {
            return 0.0;
        }
        (sin_dot * sin_dot + cos_dot * cos_dot).sqrt() / (energy * audio.len() as f64 / 2.0).sqrt()
    }
}
