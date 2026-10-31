//! US analog voice-band presets for a generic RTL-SDR (~24 MHz–1766 MHz).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demod {
    Wfm,
    Nfm,
    Am,
}

impl Demod {
    pub fn as_str(self) -> &'static str {
        match self {
            Demod::Wfm => "WFM",
            Demod::Nfm => "NFM",
            Demod::Am => "AM",
        }
    }

    pub fn passband_hz(self) -> f64 {
        match self {
            Demod::Wfm => 200_000.0,
            Demod::Nfm => 12_500.0,
            Demod::Am => 8_000.0,
        }
    }

    pub fn all() -> &'static [Demod] {
        &[Demod::Wfm, Demod::Nfm, Demod::Am]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepMode {
    Bounce,
    Forward,
    Backward,
    Random,
}

impl SweepMode {
    pub fn as_str(self) -> &'static str {
        match self {
            SweepMode::Bounce => "bounce",
            SweepMode::Forward => "forward",
            SweepMode::Backward => "backward",
            SweepMode::Random => "random",
        }
    }

    pub fn all() -> &'static [SweepMode] {
        &[
            SweepMode::Bounce,
            SweepMode::Forward,
            SweepMode::Backward,
            SweepMode::Random,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub start_hz: u64,
    pub end_hz: u64,
    pub demod: Demod,
    pub step_hz: u64,
    pub dwell_ms: u32,
    pub notes: &'static str,
    /// True when this band needs HF (direct sampling or a V4 upconverter).
    pub needs_hf: bool,
}

pub const GENERIC_MIN_HZ: u64 = 24_000_000;
pub const GENERIC_MAX_HZ: u64 = 1_766_000_000;
pub const V4_MIN_HZ: u64 = 500_000;
pub const DEFAULT_SAMPLE_RATE: u32 = 2_400_000;
pub const DEFAULT_GAIN_TENTH_DB: i32 = 296; // 29.6 dB — mid, avoids FM overload

pub const PRESETS: &[BandPreset] = &[
    BandPreset {
        id: "fm",
        name: "FM Broadcast",
        start_hz: 88_000_000,
        end_hz: 108_000_000,
        demod: Demod::Wfm,
        step_hz: 150_000,
        dwell_ms: 30,
        notes: "Classic shack-hack / spirit-box sweep",
        needs_hf: false,
    },
    BandPreset {
        id: "vhf-low",
        name: "VHF Low",
        start_hz: 30_000_000,
        end_hz: 50_000_000,
        demod: Demod::Nfm,
        step_hz: 20_000,
        dwell_ms: 40,
        notes: "Lowest analog voice without an upconverter",
        needs_hf: false,
    },
    BandPreset {
        id: "6m",
        name: "6 m Ham",
        start_hz: 50_000_000,
        end_hz: 54_000_000,
        demod: Demod::Nfm,
        step_hz: 20_000,
        dwell_ms: 40,
        notes: "FM simplex around 52.525",
        needs_hf: false,
    },
    BandPreset {
        id: "air",
        name: "Airband",
        start_hz: 118_000_000,
        end_hz: 137_000_000,
        demod: Demod::Am,
        step_hz: 25_000,
        dwell_ms: 40,
        notes: "AM voice, towers and approach",
        needs_hf: false,
    },
    BandPreset {
        id: "2m",
        name: "2 m Ham",
        start_hz: 144_000_000,
        end_hz: 148_000_000,
        demod: Demod::Nfm,
        step_hz: 15_000,
        dwell_ms: 40,
        notes: "Repeaters plus 146.520 simplex",
        needs_hf: false,
    },
    BandPreset {
        id: "vhf-high",
        name: "VHF High",
        start_hz: 150_000_000,
        end_hz: 174_000_000,
        demod: Demod::Nfm,
        step_hz: 12_500,
        dwell_ms: 35,
        notes: "Analog fire/EMS/sheriff, marine, NOAA",
        needs_hf: false,
    },
    BandPreset {
        id: "marine",
        name: "Marine VHF",
        start_hz: 156_000_000,
        end_hz: 162_025_000,
        demod: Demod::Nfm,
        step_hz: 25_000,
        dwell_ms: 40,
        notes: "Channel 16 is 156.800 MHz",
        needs_hf: false,
    },
    BandPreset {
        id: "noaa",
        name: "NOAA Weather",
        start_hz: 162_400_000,
        end_hz: 162_550_000,
        demod: Demod::Nfm,
        step_hz: 25_000,
        dwell_ms: 80,
        notes: "Seven continuous analog weather channels",
        needs_hf: false,
    },
    BandPreset {
        id: "70cm",
        name: "70 cm Ham",
        start_hz: 420_000_000,
        end_hz: 450_000_000,
        demod: Demod::Nfm,
        step_hz: 25_000,
        dwell_ms: 35,
        notes: "Repeaters plus 446.000 simplex",
        needs_hf: false,
    },
    BandPreset {
        id: "gmrs",
        name: "GMRS / FRS",
        start_hz: 462_000_000,
        end_hz: 467_712_500,
        demod: Demod::Nfm,
        step_hz: 12_500,
        dwell_ms: 35,
        notes: "Handheld analog voice",
        needs_hf: false,
    },
    BandPreset {
        id: "am-bcst",
        name: "AM Broadcast",
        start_hz: 530_000,
        end_hz: 1_700_000,
        demod: Demod::Am,
        step_hz: 10_000,
        dwell_ms: 50,
        notes: "Needs HF: Blog V4 or direct sampling. Disabled on generic sticks.",
        needs_hf: true,
    },
];

pub fn default_preset() -> &'static BandPreset {
    &PRESETS[0]
}

pub fn preset_by_id(id: &str) -> Option<&'static BandPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

pub fn available_presets(hf_ok: bool) -> impl Iterator<Item = &'static BandPreset> {
    PRESETS.iter().filter(move |p| hf_ok || !p.needs_hf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fm_is_inside_generic_range() {
        let fm = default_preset();
        assert!(fm.start_hz >= GENERIC_MIN_HZ);
        assert!(fm.end_hz <= GENERIC_MAX_HZ);
        assert_eq!(fm.demod, Demod::Wfm);
    }

    #[test]
    fn am_broadcast_needs_hf() {
        let am = preset_by_id("am-bcst").unwrap();
        assert!(am.needs_hf);
        assert!(am.end_hz < GENERIC_MIN_HZ);
    }
}
