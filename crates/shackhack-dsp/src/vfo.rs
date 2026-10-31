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
            audio: Vec::new(),
        }
    }

    pub fn configure(&mut self, sample_rate: f64, demod: DemodKind) {
        self.demod = demod;
        match demod {
            DemodKind::Wfm => {
                self.decim = (sample_rate / 240_000.0).round().max(1.0) as u32;
                let if_rate = sample_rate / self.decim as f64;
                self.wfm_keep = (if_rate / AUDIO_RATE).round().max(1.0) as u32;
            }
            DemodKind::Nfm | DemodKind::Am => {
                self.decim = (sample_rate / AUDIO_RATE).round().max(1.0) as u32;
                self.wfm_keep = 1;
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
                        num.atan2(den.max(1e-12))
                    }
                };
                if self.demod == DemodKind::Wfm {
                    let dt = decim as f64 / sample_rate;
                    let x = (dt / 75e-6) as f32;
                    self.deemph += x * (a - self.deemph);
                    a = self.deemph;
                }
                let level = a.abs().max(1e-6);
                self.agc = self.agc * 0.995 + level * 0.005;
                let gain = (0.2 / self.agc).clamp(0.2, 40.0);
                let sample = (a * gain * fade * self.mix).clamp(-1.0, 1.0);
                self.wfm_count += 1;
                if self.wfm_count >= self.wfm_keep {
                    self.wfm_count = 0;
                    self.audio.push(sample);
                }
            }
        }
        &self.audio
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
}
