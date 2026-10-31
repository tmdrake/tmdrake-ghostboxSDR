use crate::window::{clamp_lo, hop_center, window_contains};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Bounce,
    Forward,
    Backward,
    Random,
}

#[derive(Debug, Clone)]
pub struct SweepConfig {
    pub min_hz: u64,
    pub max_hz: u64,
    pub step_hz: u64,
    pub dwell_ms: u32,
    pub mode: Mode,
}

pub struct Sweeper {
    pub cfg: SweepConfig,
    pub freq_hz: u64,
    pub prev_freq_hz: u64,
    direction: i8,
    samples_in_dwell: u64,
    dwell_samples: u64,
    rng: u64,
}

impl Sweeper {
    pub fn new(cfg: SweepConfig, sample_rate: f64) -> Self {
        let freq = cfg.min_hz;
        let mut s = Self {
            cfg,
            freq_hz: freq,
            prev_freq_hz: freq,
            direction: 1,
            samples_in_dwell: 0,
            dwell_samples: 1,
            rng: 0xC0FFEE_u64.wrapping_mul(0x9E37_79B9_7F4A_7C15),
        };
        s.set_sample_rate(sample_rate);
        s
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        let dwell = (sample_rate * self.cfg.dwell_ms as f64 / 1000.0).max(1.0) as u64;
        self.dwell_samples = dwell;
    }

    pub fn apply_config(&mut self, cfg: SweepConfig, sample_rate: f64) {
        self.cfg = cfg;
        self.freq_hz = self.freq_hz.clamp(self.cfg.min_hz, self.cfg.max_hz);
        self.set_sample_rate(sample_rate);
    }

    pub fn dwell_frac(&self) -> f32 {
        (self.samples_in_dwell as f32 / self.dwell_samples.max(1) as f32).clamp(0.0, 1.0)
    }

    /// Display frequency: glides from previous step to current across the dwell.
    pub fn display_hz(&self) -> f64 {
        let a = self.prev_freq_hz as f64;
        let b = self.freq_hz as f64;
        a + (b - a) * self.dwell_frac() as f64
    }

    pub fn direction(&self) -> i8 {
        self.direction
    }

    /// Advance by `n` IQ samples. Returns true when the channel stepped.
    pub fn consume_samples(&mut self, n: u64) -> bool {
        self.samples_in_dwell += n;
        if self.samples_in_dwell < self.dwell_samples {
            return false;
        }
        self.samples_in_dwell = 0;
        self.prev_freq_hz = self.freq_hz;
        self.freq_hz = self.next_freq();
        true
    }

    fn next_freq(&mut self) -> u64 {
        let step = self.cfg.step_hz.max(1);
        let min = self.cfg.min_hz;
        let max = self.cfg.max_hz.max(min + step);
        match self.cfg.mode {
            Mode::Forward => {
                let n = self.freq_hz.saturating_add(step);
                if n > max {
                    min
                } else {
                    n
                }
            }
            Mode::Backward => {
                if self.freq_hz <= min + step {
                    max
                } else {
                    self.freq_hz - step
                }
            }
            Mode::Bounce => {
                let mut f = self.freq_hz as i64 + self.direction as i64 * step as i64;
                if f as u64 >= max {
                    f = max as i64;
                    self.direction = -1;
                } else if (f as u64) <= min {
                    f = min as i64;
                    self.direction = 1;
                }
                f as u64
            }
            Mode::Random => {
                self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let span = max.saturating_sub(min).max(1);
                let bins = (span / step).max(1);
                min + (self.rng % bins) * step
            }
        }
    }

    pub fn needed_lo(&self, sample_rate: f64, tuner_min: f64, tuner_max: f64) -> f64 {
        let dir = match self.cfg.mode {
            Mode::Backward => -1,
            Mode::Random => 0,
            _ => self.direction,
        };
        let lo = hop_center(self.freq_hz as f64, sample_rate, dir);
        clamp_lo(lo, tuner_min, tuner_max, sample_rate)
    }

    pub fn needs_hop(&self, lo_hz: f64, sample_rate: f64) -> bool {
        !window_contains(lo_hz, self.freq_hz as f64, sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_turns_around() {
        let cfg = SweepConfig {
            min_hz: 100,
            max_hz: 130,
            step_hz: 10,
            dwell_ms: 1,
            mode: Mode::Bounce,
        };
        let mut s = Sweeper::new(cfg, 1000.0);
        s.freq_hz = 130;
        s.direction = 1;
        s.samples_in_dwell = s.dwell_samples;
        s.consume_samples(0);
        assert_eq!(s.direction, -1);
        assert!(s.freq_hz <= 130);
    }

    #[test]
    fn hop_after_leaving_window() {
        let cfg = SweepConfig {
            min_hz: 88_000_000,
            max_hz: 108_000_000,
            step_hz: 150_000,
            dwell_ms: 30,
            mode: Mode::Forward,
        };
        let mut s = Sweeper::new(cfg, 2_400_000.0);
        let lo = s.needed_lo(2_400_000.0, 24e6, 1.7e9);
        assert!(!s.needs_hop(lo, 2_400_000.0));
        s.freq_hz = 100_000_000;
        assert!(s.needs_hop(lo, 2_400_000.0));
    }
}
