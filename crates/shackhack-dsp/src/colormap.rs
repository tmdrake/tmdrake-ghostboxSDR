/// Compact turbo-like colormap. `t` is 0..1 (floor → peak).
pub fn turbo(t: f32) -> [u8; 4] {
    let x = t.clamp(0.0, 1.0);
    let r = (34.0 + x * (1171.0 + x * (-1484.0 + x * 722.0))).clamp(0.0, 255.0) as u8;
    let g = (8.0 + x * (850.0 + x * (-380.0 + x * -80.0))).clamp(0.0, 255.0) as u8;
    let b = (38.0 + x * (4.0 + x * (900.0 + x * (-920.0)))).clamp(0.0, 255.0) as u8;
    [r, g, b, 255]
}

/// Night-vision phosphor: black → dark green → lime → hot white.
pub fn nv(t: f32) -> [u8; 4] {
    let x = t.clamp(0.0, 1.0);
    let r = (x * x * 210.0).clamp(0.0, 255.0) as u8;
    let g = (8.0 + x * 247.0).clamp(0.0, 255.0) as u8;
    let b = (x * x * 90.0).clamp(0.0, 255.0) as u8;
    [r, g, b, 255]
}

/// SDR# classic analyzer gradient, evenly spaced.
/// Black → blue → cyan → green → yellow → red → white.
pub fn sharp(t: f32) -> [u8; 4] {
    const STOPS: [[u8; 3]; 7] = [
        [0, 0, 0],
        [0, 0, 255],
        [0, 255, 255],
        [0, 255, 0],
        [255, 255, 0],
        [255, 0, 0],
        [255, 255, 255],
    ];
    let x = t.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let i = (x.floor() as usize).min(STOPS.len() - 2);
    let f = x - i as f32;
    let a = STOPS[i];
    let b = STOPS[i + 1];
    let mix = |lo: u8, hi: u8| (lo as f32 + (hi as f32 - lo as f32) * f).round() as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2]), 255]
}

pub fn db_to_color(db: f32, floor: f32, ceil: f32) -> [u8; 4] {
    let span = (ceil - floor).max(8.0);
    sharp((db - floor) / span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_opaque() {
        assert_eq!(turbo(0.0)[3], 255);
        assert_eq!(turbo(1.0)[3], 255);
        assert_eq!(nv(0.0)[3], 255);
        assert_eq!(nv(1.0)[3], 255);
        assert!(nv(1.0)[1] > nv(0.0)[1]);
    }

    #[test]
    fn sharp_matches_sdr_stops() {
        assert_eq!(sharp(0.0), [0, 0, 0, 255]);
        assert_eq!(sharp(1.0), [255, 255, 255, 255]);
        let blue = sharp(1.0 / 6.0);
        assert!(blue[2] > 250 && blue[0] < 5 && blue[1] < 5);
        let green = sharp(0.5);
        assert!(green[1] > 250 && green[0] < 5 && green[2] < 5);
        let yellow = sharp(4.0 / 6.0);
        assert!(yellow[0] > 250 && yellow[1] > 250 && yellow[2] < 5);
        // Just above the floor is blue, not phosphor green.
        let low = sharp(0.12);
        assert!(low[2] > low[1] && low[2] > low[0]);
    }

    #[test]
    fn db_to_color_uses_the_analyzer_gradient() {
        let floor = db_to_color(-80.0, -80.0, -35.0);
        let hot = db_to_color(-35.0, -80.0, -35.0);
        assert!(floor[0] <= 5 && floor[1] <= 5 && floor[2] <= 5);
        assert_eq!(hot, [255, 255, 255, 255]);
    }
}
