//! Elonics E4000 tuner. Used by the Terratec T Stick PLUS and other early DVB sticks.
//!
//! The chip is zero-IF. The PLL divider and band edges are the ones printed in
//! the Elonics programming notes: pick the first row whose limit is above the
//! wanted frequency, then program Z and the 16-bit fraction.

use crate::device::Device;
use crate::error::Result;

const ADDR: u8 = 0xc8;

const MASTER1: u8 = 0x00;
const CLK_INP: u8 = 0x05;
const REF_CLK: u8 = 0x06;
const SYNTH1: u8 = 0x07;
const SYNTH3: u8 = 0x09;
const SYNTH4: u8 = 0x0a;
const SYNTH5: u8 = 0x0b;
const SYNTH7: u8 = 0x0d;
const GAIN1: u8 = 0x14;
const GAIN2: u8 = 0x15;
const GAIN3: u8 = 0x16;
const GAIN4: u8 = 0x17;
const AGC1: u8 = 0x1a;
const AGC7: u8 = 0x20;
const BIAS: u8 = 0x78;

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

/// 0–70 dB. The slider can now reach the weak-signal end of the E4000.
const GAINS_TENTH_DB: &[i32] = &[
    0, 50, 100, 150, 200, 250, 300, 350, 400, 450, 500, 550, 600, 650, 700,
];

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
        let _ = dev.i2c_read(ADDR, 1);
        self.write(dev, MASTER1, 0x07)?;
        self.write(dev, CLK_INP, 0x00)?;
        self.write(dev, REF_CLK, 0x00)?;
        self.write(dev, MASTER1, 0x00)?;
        self.write(dev, BIAS, 0x03)?;
        // Manual gain. Leave the AGC loops off so the slider is the only control.
        self.write(dev, AGC1, 0x00)?;
        let agc7 = self.read(dev, AGC7).unwrap_or(0);
        self.write(dev, AGC7, agc7 & !0x01)?;
        self.write_gain(dev, self.gain_tenth_db)?;
        Ok(())
    }

    pub fn standby(&mut self, dev: &Device) -> Result<()> {
        self.write(dev, MASTER1, 0x02)
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

        let band: u8 = if freq < 140_000_000 {
            0
        } else if freq < 350_000_000 {
            1
        } else if freq < 1_135_000_000 {
            2
        } else {
            3
        };
        let synth1 = self.read(dev, SYNTH1)?;
        self.write(dev, SYNTH1, (synth1 & !0x06) | ((band & 0x03) << 1))?;
        self.write(dev, BIAS, if band == 3 { 0 } else { 3 })?;
        Ok(())
    }

    pub fn set_gain_auto(&mut self, dev: &Device) -> Result<()> {
        self.write_gain(dev, 200)
    }

    pub fn set_gain_manual(&mut self, dev: &Device, gain_tenth_db: i32) -> Result<()> {
        self.gain_tenth_db = gain_tenth_db;
        self.write_gain(dev, gain_tenth_db)
    }

    pub fn read_gain(&self, _dev: &Device) -> Result<i32> {
        Ok(self.gain_tenth_db)
    }

    pub fn set_bandwidth(&mut self, _dev: &Device, bw: u32, _rate: u32) -> Result<u32> {
        Ok(bw)
    }

    fn write_gain(&mut self, dev: &Device, gain_tenth_db: i32) -> Result<()> {
        let mut best = GAINS_TENTH_DB[0];
        let mut best_err = i32::MAX;
        for g in GAINS_TENTH_DB {
            let err = (g - gain_tenth_db).abs();
            if err < best_err {
                best = *g;
                best_err = err;
            }
        }
        self.gain_tenth_db = best;
        let db = (best / 10).clamp(0, 70);
        // LNA takes the first 25 dB in 5 dB steps. Mixer adds 6. Six IF
        // stages take the rest in 3 dB steps, up to 9 dB each.
        let lna = (db.min(25) / 5) as u8;
        let mut rest = db - i32::from(lna) * 5;
        let mixer: u8 = if rest >= 6 { 1 } else { 0 };
        if mixer == 1 {
            rest -= 6;
        }
        let mut stage = [0u8; 6];
        for step in &mut stage {
            let give = if rest >= 9 {
                9
            } else if rest >= 6 {
                6
            } else if rest >= 3 {
                3
            } else {
                0
            };
            rest -= give;
            *step = (give / 3) as u8;
        }
        self.write(dev, GAIN1, lna & 0x0f)?;
        self.write(dev, GAIN2, mixer)?;
        let gain3 = stage[0] | (stage[1] << 2) | (stage[2] << 4) | (stage[3] << 6);
        let gain4 = stage[4] | (stage[5] << 2);
        self.write(dev, GAIN3, gain3)?;
        self.write(dev, GAIN4, gain4)
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
