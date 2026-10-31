//! Hardware LO window math. Crop 10% of each edge (tuner rolloff).

pub const EDGE_CROP: f64 = 0.10;

pub fn usable_half_bw(sample_rate: f64) -> f64 {
    sample_rate * (0.5 - EDGE_CROP)
}

pub fn window_contains(lo_hz: f64, freq_hz: f64, sample_rate: f64) -> bool {
    (freq_hz - lo_hz).abs() <= usable_half_bw(sample_rate)
}

/// Place the LO so `freq_hz` sits near the entry edge of the window, giving
/// the sweeper a long software-VFO walk before the next PLL hop.
///
/// `direction` is +1 (up), -1 (down), or 0 (center on freq).
pub fn hop_center(freq_hz: f64, sample_rate: f64, direction: i8) -> f64 {
    let half = usable_half_bw(sample_rate);
    let margin = sample_rate * 0.05;
    let lo = match direction {
        1 => freq_hz + half - margin,
        -1 => freq_hz - half + margin,
        _ => freq_hz,
    };
    lo.max(0.0)
}

pub fn clamp_lo(lo_hz: f64, min_hz: f64, max_hz: f64, sample_rate: f64) -> f64 {
    let half = sample_rate * 0.5;
    lo_hz.clamp(min_hz + half, (max_hz - half).max(min_hz + half))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_hop_puts_freq_near_low_edge() {
        let sr = 2_400_000.0;
        let freq = 88_000_000.0;
        let lo = hop_center(freq, sr, 1);
        assert!(window_contains(lo, freq, sr));
        assert!(freq < lo);
        let next = freq + 150_000.0 * 8.0;
        assert!(window_contains(lo, next, sr));
    }

    #[test]
    fn outside_window_is_detected() {
        let sr = 2_400_000.0;
        let lo = 100_000_000.0;
        assert!(window_contains(lo, 100_000_000.0, sr));
        assert!(!window_contains(lo, 102_000_000.0, sr));
    }
}
