//! DSP for ShackHack: IQ convert, FFT, software VFOs, analog demod, sweeper.

pub mod colormap;
pub mod fft;
pub mod iq;
pub mod sweep;
pub mod vfo;
pub mod window;

pub use colormap::turbo;
pub use fft::{FftProcessor, Spectrum};
pub use iq::{u8_iq_to_complex, ComplexSample};
pub use sweep::{SweepConfig, Sweeper};
pub use vfo::{DemodKind, Vfo, VFO_COUNT};
pub use window::{hop_center, usable_half_bw, window_contains};

pub const AUDIO_RATE: f64 = 48_000.0;
