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

    /// Park at the start of a pass. Forward and bounce start at the bottom
    /// going up. Backward starts at the top going down.
    pub fn restart(&mut self) {
        match self.cfg.mode {
            Mode::Backward => {
                self.freq_hz = self.cfg.max_hz.max(self.cfg.min_hz);
                self.direction = -1;
            }
            Mode::Random => {
                self.freq_hz = self.cfg.min_hz;
                self.direction = 0;
            }
            Mode::Forward | Mode::Bounce => {
                self.freq_hz = self.cfg.min_hz;
                self.direction = 1;
            }
        }
        self.prev_freq_hz = self.freq_hz;
        self.samples_in_dwell = 0;
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.cfg.mode = mode;
        match mode {
            Mode::Forward => self.direction = 1,
            Mode::Backward => self.direction = -1,
            Mode::Random => self.direction = 0,
            Mode::Bounce => {
                if self.direction == 0 {
                    self.direction = 1;
                }
            }
        }
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
                self.direction = 1;
                let cur = self.freq_hz;
                let next = cur.saturating_add(step);
                if cur >= max {
                    min
                } else if next >= max {
                    max
                } else {
                    next
                }
            }
            Mode::Backward => {
                self.direction = -1;
                let cur = self.freq_hz;
                let next = cur.saturating_sub(step);
                if cur <= min {
                    max
                } else if next <= min {
                    min
                } else {
                    next
                }
            }
            Mode::Bounce => bounce_step(self.freq_hz, step, min, max, &mut self.direction),
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

/// One bounce step. Lands on `min` and `max`, then leaves on the next step.
/// Stays in `i64` so an overshoot cannot wrap through `u64`.
fn bounce_step(freq_hz: u64, step: u64, min: u64, max: u64, direction: &mut i8) -> u64 {
    let step_i = step as i64;
    let min_i = min as i64;
    let max_i = max as i64;
    let cur = freq_hz as i64;
    let next = cur + *direction as i64 * step_i;
    let f = if *direction >= 0 {
        if next < max_i {
            next
        } else if cur < max_i {
            max_i
        } else {
            *direction = -1;
            (max_i - step_i).max(min_i)
        }
    } else if next > min_i {
        next
    } else if cur > min_i {
        min_i
    } else {
        *direction = 1;
        (min_i + step_i).min(max_i)
    };
    f as u64
}

/// Step for a full-range pass. One direction takes about 90 seconds.
/// A step that already finishes in under three minutes is left alone.
/// Hops stay at most 2 MHz, one RTL window, so the slices still overlap.
pub fn full_range_step(span_hz: u64, dwell_ms: u32, current: u64) -> u64 {
    let dwell = u64::from(dwell_ms.clamp(10, 500));
    let pass_ms = span_hz.saturating_div(current.max(1)).saturating_mul(dwell);
    if current >= 100_000 && pass_ms <= 180_000 {
        return current;
    }
    let steps = (90_000u64 / dwell).max(1);
    let raw = (span_hz / steps).clamp(250_000, 2_000_000);
    (raw / 50_000).max(1) * 50_000
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
        assert!(s.freq_hz < 130);
    }

    #[test]
    fn bounce_reaches_both_ends_of_a_wide_span() {
        let cfg = SweepConfig {
            min_hz: 52_000_000,
            max_hz: 1_700_000_000,
            step_hz: 250_000,
            dwell_ms: 1,
            mode: Mode::Bounce,
        };
        let mut s = Sweeper::new(cfg, 2_400_000.0);
        s.restart();
        let mut saw_max = false;
        let mut saw_min = false;
        for _ in 0..20_000 {
            s.samples_in_dwell = s.dwell_samples;
            s.consume_samples(0);
            if s.freq_hz == 1_700_000_000 {
                saw_max = true;
                assert_eq!(s.direction(), 1);
            }
            if saw_max && s.freq_hz == 52_000_000 {
                saw_min = true;
                assert_eq!(s.direction(), -1);
                break;
            }
        }
        assert!(saw_max, "never landed on the top");
        assert!(saw_min, "never bounced back to the bottom");
        s.samples_in_dwell = s.dwell_samples;
        s.consume_samples(0);
        assert_eq!(s.direction(), 1);
        assert!(s.freq_hz > 52_000_000);
    }

    #[test]
    fn forward_lands_on_the_top_then_wraps() {
        let cfg = SweepConfig {
            min_hz: 52_000_000,
            max_hz: 1_700_000_000,
            step_hz: 150_000,
            dwell_ms: 1,
            mode: Mode::Forward,
        };
        let mut s = Sweeper::new(cfg, 2_400_000.0);
        s.restart();
        let mut saw_max = false;
        for _ in 0..20_000 {
            s.samples_in_dwell = s.dwell_samples;
            s.consume_samples(0);
            if s.freq_hz == 1_700_000_000 {
                saw_max = true;
                assert_eq!(s.direction(), 1);
                break;
            }
        }
        assert!(saw_max, "forward never landed on the top");
        s.samples_in_dwell = s.dwell_samples;
        s.consume_samples(0);
        assert_eq!(s.freq_hz, 52_000_000);
    }

    #[test]
    fn backward_lands_on_the_bottom_then_wraps() {
        let cfg = SweepConfig {
            min_hz: 52_000_000,
            max_hz: 1_700_000_000,
            step_hz: 250_000,
            dwell_ms: 1,
            mode: Mode::Backward,
        };
        let mut s = Sweeper::new(cfg, 2_400_000.0);
        s.restart();
        assert_eq!(s.freq_hz, 1_700_000_000);
        assert_eq!(s.direction(), -1);
        let mut saw_min = false;
        for _ in 0..20_000 {
            s.samples_in_dwell = s.dwell_samples;
            s.consume_samples(0);
            if s.freq_hz == 52_000_000 {
                saw_min = true;
                assert_eq!(s.direction(), -1);
                break;
            }
        }
        assert!(saw_min, "backward never landed on the bottom");
        s.samples_in_dwell = s.dwell_samples;
        s.consume_samples(0);
        assert_eq!(s.freq_hz, 1_700_000_000);
        assert_eq!(s.direction(), -1);
    }

    #[test]
    fn forward_and_backward_hit_an_unaligned_end() {
        let mut up = Sweeper::new(
            SweepConfig {
                min_hz: 100,
                max_hz: 125,
                step_hz: 10,
                dwell_ms: 1,
                mode: Mode::Forward,
            },
            1000.0,
        );
        let mut saw_top = false;
        for _ in 0..20 {
            up.samples_in_dwell = up.dwell_samples;
            up.consume_samples(0);
            if up.freq_hz == 125 {
                saw_top = true;
                break;
            }
        }
        assert!(saw_top);
        up.samples_in_dwell = up.dwell_samples;
        up.consume_samples(0);
        assert_eq!(up.freq_hz, 100);

        let mut down = Sweeper::new(
            SweepConfig {
                min_hz: 100,
                max_hz: 130,
                step_hz: 10,
                dwell_ms: 1,
                mode: Mode::Backward,
            },
            1000.0,
        );
        down.freq_hz = 130;
        let mut saw_bottom = false;
        for _ in 0..20 {
            down.samples_in_dwell = down.dwell_samples;
            down.consume_samples(0);
            if down.freq_hz == 100 {
                saw_bottom = true;
                break;
            }
        }
        assert!(saw_bottom);
        down.samples_in_dwell = down.dwell_samples;
        down.consume_samples(0);
        assert_eq!(down.freq_hz, 130);
    }

    #[test]
    fn full_range_step_finishes_a_pass_near_ninety_seconds() {
        let span = 1_700_000_000 - 52_000_000;
        let step = full_range_step(span, 90, 150_000);
        let pass_ms = span / step * 90;
        assert!((250_000..=2_000_000).contains(&step), "{step}");
        assert!((60_000..=120_000).contains(&pass_ms), "{pass_ms} ms");
        assert_eq!(full_range_step(span, 90, 2_000_000), 2_000_000);
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
