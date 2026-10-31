use num_complex::Complex;

pub type ComplexSample = Complex<f32>;

/// Convert interleaved unsigned 8-bit IQ (`I0 Q0 I1 Q1 …`) to complex f32 in ±1.
pub fn u8_iq_to_complex(bytes: &[u8], out: &mut Vec<ComplexSample>) {
    out.clear();
    let n = bytes.len() / 2;
    out.reserve(n);
    for &[i_u, q_u] in bytes.as_chunks::<2>().0 {
        let i = (i_u as f32 - 127.5) / 127.5;
        let q = (q_u as f32 - 127.5) / 127.5;
        out.push(Complex::new(i, q));
    }
}

pub fn peak_magnitude(samples: &[ComplexSample]) -> f32 {
    samples
        .iter()
        .map(|s| s.re.abs().max(s.im.abs()))
        .fold(0.0_f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midscale_is_near_zero() {
        let mut out = Vec::new();
        u8_iq_to_complex(&[128, 128, 128, 128], &mut out);
        assert_eq!(out.len(), 2);
        assert!(out[0].re.abs() < 0.01);
        assert!(out[0].im.abs() < 0.01);
    }
}
