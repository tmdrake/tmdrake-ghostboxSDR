use num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

use crate::iq::ComplexSample;

pub struct FftProcessor {
    size: usize,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    scratch: Vec<Complex<f32>>,
    input: Vec<Complex<f32>>,
}

#[derive(Debug, Clone)]
pub struct Spectrum {
    pub db: Vec<f32>,
    pub peak_db: f32,
    pub floor_db: f32,
}

impl FftProcessor {
    pub fn new(size: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(size);
        let window = hann(size);
        Self {
            size,
            fft,
            window,
            scratch: vec![Complex::new(0.0, 0.0); size],
            input: vec![Complex::new(0.0, 0.0); size],
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// `samples` should contain at least `size` IQ samples. Extra samples are ignored.
    pub fn process(&mut self, samples: &[ComplexSample]) -> Spectrum {
        let n = self.size.min(samples.len());
        self.input.fill(Complex::new(0.0, 0.0));
        for (i, sample) in samples.iter().enumerate().take(n) {
            self.input[i] = *sample * self.window[i];
        }
        self.fft
            .process_with_scratch(&mut self.input, &mut self.scratch);
        fftshift_inplace(&mut self.input);

        let mut db = vec![0.0_f32; self.size];
        for (i, bin) in self.input.iter().enumerate() {
            let mag2 = bin.norm_sqr().max(1e-20);
            db[i] = 10.0 * mag2.log10();
        }
        suppress_dc(&mut db);
        let (floor_db, peak_db) = percentile_span(&db);
        Spectrum {
            db,
            peak_db,
            floor_db,
        }
    }
}

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let x = std::f32::consts::TAU * i as f32 / n as f32;
            0.5 - 0.5 * x.cos()
        })
        .collect()
}

fn fftshift_inplace(buf: &mut [Complex<f32>]) {
    let n = buf.len();
    let half = n / 2;
    for i in 0..half {
        buf.swap(i, i + half);
    }
}

fn suppress_dc(db: &mut [f32]) {
    let n = db.len();
    if n < 32 {
        return;
    }
    let c = n / 2;
    let wing = (n / 128).max(4);
    let left = c.saturating_sub(wing + 2);
    let right = (c + wing + 2).min(n - 1);
    let fill = (db[left] + db[right]) * 0.5;
    let start = c.saturating_sub(wing);
    let end = (c + wing).min(n - 1);
    for bin in db.iter_mut().take(end + 1).skip(start) {
        *bin = fill;
    }
}

/// Noise floor at the 15th percentile, plus the true peak for overload checks.
fn percentile_span(db: &[f32]) -> (f32, f32) {
    if db.is_empty() {
        return (-100.0, -40.0);
    }
    let mut sorted = db.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let floor = sorted[sorted.len() * 15 / 100];
    let peak = sorted[sorted.len() - 1];
    (floor, peak.max(floor + 12.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_peaks_off_dc() {
        let n = 256;
        let mut proc = FftProcessor::new(n);
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let ph = std::f32::consts::TAU * 16.0 * i as f32 / n as f32;
            samples.push(Complex::new(ph.cos(), ph.sin()));
        }
        let spec = proc.process(&samples);
        let peak_bin = spec
            .db
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_ne!(peak_bin, n / 2);
        assert!(spec.peak_db > spec.floor_db + 10.0);
    }
}
