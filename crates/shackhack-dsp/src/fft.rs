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
        let mut peak = f32::NEG_INFINITY;
        let mut floor_acc = 0.0_f32;
        for (i, bin) in self.input.iter().enumerate() {
            let mag2 = bin.norm_sqr().max(1e-20);
            let v = 10.0 * mag2.log10();
            db[i] = v;
            if v > peak {
                peak = v;
            }
            floor_acc += v;
        }
        suppress_dc(&mut db);
        let floor_db = floor_acc / self.size as f32;
        Spectrum {
            db,
            peak_db: peak,
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
    if n < 8 {
        return;
    }
    let c = n / 2;
    let fill = (db[c - 3] + db[c + 3]) * 0.5;
    let start = c.saturating_sub(2);
    let end = (c + 2).min(n - 1);
    for bin in db.iter_mut().take(end + 1).skip(start) {
        *bin = fill;
    }
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
