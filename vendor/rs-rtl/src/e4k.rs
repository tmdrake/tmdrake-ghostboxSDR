//! Elonics E4000 tuner. Used by the Terratec T Stick PLUS and other early DVB sticks.
//!
//! The chip is zero-IF. The PLL, band select, RF tracking filter, and the three
//! IF filters follow osmocom `librtlsdr` (`tuner_e4k.c`). The tuner delivers
//! only noise if MASTER1 is left in standby or the VHF RF filter is never set.

use tracing::warn;

use crate::device::Device;
use crate::error::{Error, Result};

const ADDR: u8 = 0xc8;

const MASTER1: u8 = 0x00;
const CLK_INP: u8 = 0x05;
const REF_CLK: u8 = 0x06;
const SYNTH1: u8 = 0x07;
const SYNTH3: u8 = 0x09;
const SYNTH4: u8 = 0x0a;
const SYNTH5: u8 = 0x0b;
const SYNTH7: u8 = 0x0d;
const FILT1: u8 = 0x10;
const FILT2: u8 = 0x11;
const FILT3: u8 = 0x12;
const FILT3_DISABLE: u8 = 1 << 5;
const GAIN1: u8 = 0x14;
const GAIN2: u8 = 0x15;
const GAIN3: u8 = 0x16;
const GAIN4: u8 = 0x17;
const AGC1: u8 = 0x1a;
const AGC4: u8 = 0x1d;
const AGC5: u8 = 0x1e;
const AGC6: u8 = 0x1f;
const AGC7: u8 = 0x20;
const AGC11: u8 = 0x24;
const DC5: u8 = 0x2d;
const DCTIME1: u8 = 0x70;
const DCTIME2: u8 = 0x71;
const BIAS: u8 = 0x78;
const CLKOUT_PWDN: u8 = 0x7a;

/// Upper edge of each PLL row, the SYNTH7 byte, and the output divider.
const PLL: &[(u32, u8, u32)] = &[
    (72_400_000, (1 << 3) | 7, 48),
    (81_200_000, (1 << 3) | 6, 40),
    (108_300_000, (1 << 3) | 5, 32),
    (162_500_000, (1 << 3) | 4, 24),
    (216_600_000, (1 << 3) | 3, 16),
    (325_000_000, (1 << 3) | 2, 12),
    (350_000_000, (1 << 3) | 1, 8),
    (432_000_000, 3, 8),
    (667_000_000, 2, 6),
    (1_200_000_000, 1, 4),
];

/// UHF RF-filter centers, Hz. The low nibble of FILT1 selects the closest one.
const RF_UHF: &[u32] = &[
    360_000_000,
    380_000_000,
    405_000_000,
    425_000_000,
    450_000_000,
    475_000_000,
    505_000_000,
    540_000_000,
    575_000_000,
    615_000_000,
    670_000_000,
    720_000_000,
    760_000_000,
    840_000_000,
    890_000_000,
    970_000_000,
];

/// L-band RF-filter centers, Hz.
const RF_L: &[u32] = &[
    1_300_000_000,
    1_320_000_000,
    1_360_000_000,
    1_410_000_000,
    1_445_000_000,
    1_460_000_000,
    1_490_000_000,
    1_530_000_000,
    1_560_000_000,
    1_590_000_000,
    1_640_000_000,
    1_660_000_000,
    1_680_000_000,
    1_700_000_000,
    1_720_000_000,
    1_750_000_000,
];

/// Mixer-filter bandwidths, Hz. FILT2 bits 7–4. Eight entries are 27 MHz.
const MIX_BW: &[u32] = &[
    27_000_000, 27_000_000, 27_000_000, 27_000_000, 27_000_000, 27_000_000, 27_000_000, 27_000_000,
    4_600_000, 4_200_000, 3_800_000, 3_400_000, 3_300_000, 2_700_000, 2_300_000, 1_900_000,
];

/// Channel-filter bandwidths, Hz. FILT3 bits 4–0.
const CHAN_BW: &[u32] = &[
    5_500_000, 5_300_000, 5_000_000, 4_800_000, 4_600_000, 4_400_000, 4_300_000, 4_100_000,
    3_900_000, 3_800_000, 3_700_000, 3_600_000, 3_400_000, 3_300_000, 3_200_000, 3_100_000,
    3_000_000, 2_950_000, 2_900_000, 2_800_000, 2_750_000, 2_700_000, 2_600_000, 2_550_000,
    2_500_000, 2_450_000, 2_400_000, 2_300_000, 2_280_000, 2_240_000, 2_200_000, 2_150_000,
];

/// RC-filter bandwidths, Hz. FILT2 bits 3–0.
const RC_BW: &[u32] = &[
    21_400_000, 21_000_000, 17_600_000, 14_700_000, 12_400_000, 10_600_000, 9_000_000, 7_700_000,
    6_400_000, 5_300_000, 4_400_000, 3_400_000, 2_600_000, 1_800_000, 1_200_000, 1_000_000,
];

/// LNA gain in tenths of a dB, then the GAIN1 code. The codes are not linear.
const LNA_GAIN: &[(i32, u8)] = &[
    (-50, 0),
    (-25, 1),
    (0, 4),
    (25, 5),
    (50, 6),
    (75, 7),
    (100, 8),
    (125, 9),
    (150, 10),
    (175, 11),
    (200, 12),
    (250, 13),
    (300, 14),
];

/// Slider steps, tenths of a dB. These are the LNA + mixer steps librtlsdr publishes.
/// The six IF stages stay at the fixed moderate setting from `e4k_init`.
const GAINS_TENTH_DB: &[i32] = &[
    -10, 15, 40, 65, 90, 115, 140, 165, 190, 215, 240, 290, 340, 420,
];

#[derive(Clone, Copy)]
enum IfFilter {
    Mix,
    Chan,
    Rc,
}

#[derive(Clone)]
pub struct E4k {
    xtal: u32,
    gain_tenth_db: i32,
}

impl E4k {
    pub fn new(xtal: u32) -> Self {
        Self {
            xtal,
            gain_tenth_db: 200,
        }
    }

    pub fn if_freq(&self) -> u32 {
        0
    }

    pub fn gains(&self) -> &'static [i32] {
        GAINS_TENTH_DB
    }

    pub fn init(&mut self, dev: &Device) -> Result<()> {
        // The first transaction is not ACKed. It wakes the I2C state machine.
        let _ = dev.i2c_read(ADDR, 1);
        // RESET | normal-mode | POR clear. Bit 1 must stay set. Writing 0x00
        // here looks like "end the reset" but it is standby, and the ADC
        // then hears only the tuner's own noise.
        self.write(dev, MASTER1, 0x07)?;
        self.write(dev, CLK_INP, 0x00)?;
        self.write(dev, REF_CLK, 0x00)?;
        self.write(dev, CLKOUT_PWDN, 0x96)?;
        self.magic_init(dev)?;

        self.write(dev, AGC4, 0x10)?;
        self.write(dev, AGC5, 0x04)?;
        self.write(dev, AGC6, 0x1a)?;
        // Manual LNA and mixer. The slider is the only gain control.
        self.set_mask(dev, AGC1, 0x0f, 0x00)?;
        self.set_mask(dev, AGC7, 0x01, 0x00)?;

        // Moderate IF gain from librtlsdr. Stages 5 and 6 cannot go below 3 dB.
        self.if_gain(dev, 1, 6)?;
        self.if_gain(dev, 2, 0)?;
        self.if_gain(dev, 3, 0)?;
        self.if_gain(dev, 4, 0)?;
        self.if_gain(dev, 5, 9)?;
        self.if_gain(dev, 6, 9)?;

        // Narrowest filters until set_sample_rate opens them to the sample rate.
        self.set_if_filter(dev, IfFilter::Mix, 1_900_000)?;
        self.set_if_filter(dev, IfFilter::Rc, 1_000_000)?;
        self.set_if_filter(dev, IfFilter::Chan, 2_150_000)?;
        self.set_mask(dev, FILT3, FILT3_DISABLE, 0)?;

        // Time-variant DC correction notches out a station that sits near the LO.
        self.set_mask(dev, DC5, 0x03, 0)?;
        self.set_mask(dev, DCTIME1, 0x03, 0)?;
        self.set_mask(dev, DCTIME2, 0x03, 0)?;

        self.write(dev, BIAS, 0x03)?;
        self.write_gain(dev, self.gain_tenth_db)?;
        Ok(())
    }

    pub fn standby(&mut self, dev: &Device) -> Result<()> {
        // Bit 1 clear is standby. Leave reset and POR alone.
        self.set_mask(dev, MASTER1, 0x02, 0)
    }

    pub fn set_freq(&mut self, dev: &Device, freq: u32) -> Result<()> {
        let freq = freq.clamp(52_000_000, 1_700_000_000);
        let (synth7, divider) = pll_row(freq);
        let fosc = self.xtal as u64;
        let fvco = freq as u64 * divider as u64;
        let z = (fvco / fosc) as u8;
        let remainder = fvco - fosc * u64::from(z);
        let x = ((remainder * 65_536) / fosc) as u16;

        self.write(dev, SYNTH7, synth7)?;
        self.write(dev, SYNTH3, z)?;
        self.write(dev, SYNTH4, (x & 0xff) as u8)?;
        self.write(dev, SYNTH5, (x >> 8) as u8)?;

        let band = band_for(freq);
        // Clear the band bits before writing them. One combined write leaves
        // a gap between 325 and 350 MHz.
        self.set_mask(dev, SYNTH1, 0x06, 0)?;
        self.set_mask(dev, SYNTH1, 0x06, (band & 0x03) << 1)?;
        self.write(dev, BIAS, if band == 3 { 0 } else { 3 })?;
        self.set_mask(dev, FILT1, 0x0f, rf_filter_index(freq))?;

        let synth1 = self.read(dev, SYNTH1)?;
        if synth1 & 0x01 == 0 {
            warn!("E4000 PLL not locked at {freq} Hz");
        }
        Ok(())
    }

    pub fn set_gain_auto(&mut self, dev: &Device) -> Result<()> {
        // IF serial, LNA autonomous. Mixer gain follows the AGC.
        self.set_mask(dev, AGC1, 0x0f, 0x09)?;
        self.set_mask(dev, AGC7, 0x01, 0x01)?;
        self.set_mask(dev, AGC11, 0x07, 0)
    }

    pub fn set_gain_manual(&mut self, dev: &Device, gain_tenth_db: i32) -> Result<()> {
        self.set_mask(dev, AGC1, 0x0f, 0x00)?;
        self.set_mask(dev, AGC7, 0x01, 0x00)?;
        self.set_mask(dev, AGC11, 0x07, 0)?;
        self.gain_tenth_db = gain_tenth_db;
        self.write_gain(dev, gain_tenth_db)
    }

    pub fn read_gain(&self, _dev: &Device) -> Result<i32> {
        Ok(self.gain_tenth_db)
    }

    /// Program the three IF filters. The returned value is the tuner IF, which
    /// is 0. Callers write that into the RTL2832 digital mixer.
    pub fn set_bandwidth(&mut self, dev: &Device, bw: u32, _rate: u32) -> Result<u32> {
        self.set_if_filter(dev, IfFilter::Mix, bw)?;
        self.set_if_filter(dev, IfFilter::Rc, bw)?;
        self.set_if_filter(dev, IfFilter::Chan, bw)?;
        self.set_mask(dev, FILT3, FILT3_DISABLE, 0)?;
        Ok(0)
    }

    fn magic_init(&self, dev: &Device) -> Result<()> {
        // Elonics bring-up block. 0x86 = 0x50 sets mixer polarity A.
        self.write(dev, 0x7e, 0x01)?;
        self.write(dev, 0x7f, 0xfe)?;
        self.write(dev, 0x82, 0x00)?;
        self.write(dev, 0x86, 0x50)?;
        self.write(dev, 0x87, 0x20)?;
        self.write(dev, 0x88, 0x01)?;
        self.write(dev, 0x9f, 0x7f)?;
        self.write(dev, 0xa0, 0x07)
    }

    fn write_gain(&mut self, dev: &Device, gain_tenth_db: i32) -> Result<()> {
        let (lna_code, mix_bit, snapped) = split_gain(gain_tenth_db);
        self.gain_tenth_db = snapped;
        self.set_mask(dev, GAIN1, 0x0f, lna_code)?;
        self.set_mask(dev, GAIN2, 0x01, mix_bit)
    }

    fn if_gain(&self, dev: &Device, stage: u8, db: i8) -> Result<()> {
        let (reg, mask, val) = if_stage_field(stage, db).ok_or_else(|| {
            Error::InvalidParam(format!("E4000 IF stage {stage} has no {db} dB step"))
        })?;
        self.set_mask(dev, reg, mask, val)
    }

    fn set_if_filter(&self, dev: &Device, filter: IfFilter, bw: u32) -> Result<()> {
        let (reg, mask, val) = if_filter_field(filter, bw);
        self.set_mask(dev, reg, mask, val)
    }

    fn set_mask(&self, dev: &Device, reg: u8, mask: u8, val: u8) -> Result<()> {
        let cur = self.read(dev, reg)?;
        let next = (cur & !mask) | (val & mask);
        if next != cur {
            self.write(dev, reg, next)?;
        }
        Ok(())
    }

    fn write(&self, dev: &Device, reg: u8, val: u8) -> Result<()> {
        dev.i2c_write(ADDR, &[reg, val])
    }

    fn read(&self, dev: &Device, reg: u8) -> Result<u8> {
        dev.i2c_read_reg(ADDR, reg)
    }
}

fn pll_row(freq: u32) -> (u8, u32) {
    for &(limit, synth7, divider) in PLL {
        if freq < limit {
            return (synth7, divider);
        }
    }
    (1, 4)
}

fn band_for(freq: u32) -> u8 {
    if freq < 140_000_000 {
        0
    } else if freq < 350_000_000 {
        1
    } else if freq < 1_135_000_000 {
        2
    } else {
        3
    }
}

/// FILT1 low nibble. VHF2 and VHF3 are index 0. UHF and L pick the closest center.
fn rf_filter_index(freq: u32) -> u8 {
    match band_for(freq) {
        0 | 1 => 0,
        2 => closest(RF_UHF, freq) as u8,
        _ => closest(RF_L, freq) as u8,
    }
}

fn closest(table: &[u32], freq: u32) -> usize {
    let mut best_i = 0;
    let mut best = u32::MAX;
    for (i, &center) in table.iter().enumerate() {
        let delta = freq.abs_diff(center);
        if delta < best {
            best = delta;
            best_i = i;
        }
    }
    best_i
}

fn if_filter_index(filter: IfFilter, bw: u32) -> usize {
    match filter {
        IfFilter::Mix => closest(MIX_BW, bw),
        IfFilter::Chan => closest(CHAN_BW, bw),
        IfFilter::Rc => closest(RC_BW, bw),
    }
}

fn if_filter_field(filter: IfFilter, bw: u32) -> (u8, u8, u8) {
    let idx = if_filter_index(filter, bw) as u8;
    match filter {
        IfFilter::Mix => (FILT2, 0xf0, idx << 4),
        IfFilter::Chan => (FILT3, 0x1f, idx),
        IfFilter::Rc => (FILT2, 0x0f, idx),
    }
}

/// (register, mask, value already shifted). `None` when the stage or dB step does not exist.
fn if_stage_field(stage: u8, db: i8) -> Option<(u8, u8, u8)> {
    let (reg, shift, width, table): (u8, u8, u8, &[i8]) = match stage {
        1 => (GAIN3, 0, 1, &[-3, 6]),
        2 => (GAIN3, 1, 2, &[0, 3, 6, 9]),
        3 => (GAIN3, 3, 2, &[0, 3, 6, 9]),
        4 => (GAIN3, 5, 2, &[0, 1, 2, 2]),
        5 => (GAIN4, 0, 3, &[3, 6, 9, 12, 15, 15, 15, 15]),
        6 => (GAIN4, 3, 3, &[3, 6, 9, 12, 15, 15, 15, 15]),
        _ => return None,
    };
    let idx = table.iter().position(|step| *step == db)? as u8;
    let mask = ((1u8 << width) - 1) << shift;
    Some((reg, mask, idx << shift))
}

/// LNA code, mixer bit, and the snapped gain in tenths of a dB.
/// Mixer is 4 dB (bit 0 clear) up to 34 dB, then 12 dB. The rest goes to the LNA.
fn split_gain(gain_tenth_db: i32) -> (u8, u8, i32) {
    let snapped = nearest(GAINS_TENTH_DB, gain_tenth_db);
    let mix_db = if snapped > 340 { 12 } else { 4 };
    let lna_tenths = (snapped - mix_db * 10).min(300);
    let mut best = LNA_GAIN[0];
    let mut best_err = i32::MAX;
    for pair in LNA_GAIN {
        let err = (pair.0 - lna_tenths).abs();
        if err < best_err {
            best = *pair;
            best_err = err;
        }
    }
    let mix_bit = if mix_db == 12 { 1 } else { 0 };
    (best.1, mix_bit, snapped)
}

fn nearest(table: &[i32], want: i32) -> i32 {
    table
        .iter()
        .copied()
        .min_by_key(|g| (g - want).abs())
        .unwrap_or(want)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fm_broadcast_uses_the_vhf_filter_and_divider_32() {
        assert_eq!(rf_filter_index(95_500_000), 0);
        assert_eq!(rf_filter_index(96_340_000), 0);
        assert_eq!(pll_row(96_340_000), ((1 << 3) | 5, 32));
        assert_eq!(band_for(95_500_000), 0);
    }

    #[test]
    fn uhf_filter_picks_the_closest_center() {
        assert_eq!(rf_filter_index(774_000_000), 12);
        assert_eq!(band_for(774_000_000), 2);
    }

    #[test]
    fn sample_rate_2_4_mhz_selects_the_librtlsdr_if_filters() {
        assert_eq!(if_filter_index(IfFilter::Mix, 2_400_000), 14);
        assert_eq!(if_filter_index(IfFilter::Chan, 2_400_000), 26);
        assert_eq!(if_filter_index(IfFilter::Rc, 2_400_000), 12);
    }

    #[test]
    fn gain_near_30_db_is_lna_25_db_and_mixer_4_db() {
        let (code, mix_bit, snapped) = split_gain(296);
        assert_eq!(snapped, 290);
        assert_eq!(mix_bit, 0);
        assert_eq!(code, 13);
    }

    #[test]
    fn top_gain_step_uses_the_12_db_mixer() {
        let (code, mix_bit, snapped) = split_gain(420);
        assert_eq!(snapped, 420);
        assert_eq!(mix_bit, 1);
        assert_eq!(code, 14);
    }

    #[test]
    fn if_stage_fields_match_the_moderate_init() {
        assert_eq!(if_stage_field(1, 6), Some((GAIN3, 0x01, 0x01)));
        assert_eq!(if_stage_field(5, 9), Some((GAIN4, 0x07, 0x02)));
        assert_eq!(if_stage_field(6, 9), Some((GAIN4, 0x38, 0x10)));
        assert_eq!(if_stage_field(4, 9), None);
    }
}
