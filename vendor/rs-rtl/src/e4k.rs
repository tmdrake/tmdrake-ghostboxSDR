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

const GAINS_TENTH_DB: &[i32] = &[
    0, 20, 40, 60, 80, 100, 120, 140, 160, 180, 200, 220, 240, 260, 280, 300,
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
        let mut best = 0usize;
        let mut best_err = i32::MAX;
        for (i, g) in GAINS_TENTH_DB.iter().enumerate() {
            let err = (g - gain_tenth_db).abs();
            if err < best_err {
                best = i;
                best_err = err;
            }
        }
        self.gain_tenth_db = GAINS_TENTH_DB[best];
        self.write(dev, GAIN1, (best as u8) << 4)
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
