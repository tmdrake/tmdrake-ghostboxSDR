/// Compact turbo-like colormap. `t` is 0..1 (floor → peak).
pub fn turbo(t: f32) -> [u8; 4] {
    let x = t.clamp(0.0, 1.0);
    let r = (34.0 + x * (1171.0 + x * (-1484.0 + x * 722.0))).clamp(0.0, 255.0) as u8;
    let g = (8.0 + x * (850.0 + x * (-380.0 + x * -80.0))).clamp(0.0, 255.0) as u8;
    let b = (38.0 + x * (4.0 + x * (900.0 + x * (-920.0)))).clamp(0.0, 255.0) as u8;
    [r, g, b, 255]
}

pub fn db_to_color(db: f32, floor: f32, ceil: f32) -> [u8; 4] {
    let span = (ceil - floor).max(8.0);
    turbo((db - floor) / span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_opaque() {
        assert_eq!(turbo(0.0)[3], 255);
        assert_eq!(turbo(1.0)[3], 255);
    }
}
