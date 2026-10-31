use rs_rtl::{BoardVariant, DeviceDescriptors, DeviceId, RtlSdr, TunerType};
use shackhack_bands::{DEFAULT_GAIN_TENTH_DB, GENERIC_MAX_HZ, GENERIC_MIN_HZ, V4_MIN_HZ};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, SourceError>;

impl From<rs_rtl::Error> for SourceError {
    fn from(e: rs_rtl::Error) -> Self {
        SourceError::Message(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub label: String,
    pub tuner: String,
    pub min_hz: u64,
    pub max_hz: u64,
    pub hf_ok: bool,
    pub gains: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Rtl,
    Synth,
}

pub enum IqSource {
    Rtl(RtlSource),
    Synth(SynthSource),
}

impl IqSource {
    pub fn open_rtl() -> Result<Self> {
        Ok(Self::Rtl(RtlSource::open()?))
    }

    pub fn open_synth(sample_rate: u32) -> Self {
        Self::Synth(SynthSource::new(sample_rate))
    }

    pub fn info(&self) -> DeviceInfo {
        match self {
            IqSource::Rtl(s) => s.info.clone(),
            IqSource::Synth(s) => s.info.clone(),
        }
    }

    pub fn start(&mut self, center_hz: u32, sample_rate: u32, gain: i32) -> Result<()> {
        match self {
            IqSource::Rtl(s) => s.start(center_hz, sample_rate, gain),
            IqSource::Synth(s) => {
                s.lo_hz = center_hz as f64;
                s.sample_rate = sample_rate as f64;
                s.gain_tenth_db = gain;
                Ok(())
            }
        }
    }

    pub fn hop(&mut self, center_hz: u32) -> Result<()> {
        match self {
            IqSource::Rtl(s) => s.hop(center_hz),
            IqSource::Synth(s) => {
                s.lo_hz = center_hz as f64;
                Ok(())
            }
        }
    }

    pub fn set_gain(&mut self, gain: i32) -> Result<()> {
        match self {
            IqSource::Rtl(s) => s.set_gain(gain),
            IqSource::Synth(s) => {
                s.gain_tenth_db = gain;
                Ok(())
            }
        }
    }

    pub fn next_chunk(&mut self) -> Result<Option<Vec<u8>>> {
        match self {
            IqSource::Rtl(s) => s.next_chunk(),
            IqSource::Synth(s) => Ok(Some(s.next_chunk())),
        }
    }
}

pub struct RtlSource {
    sdr: RtlSdr,
    reader: Option<rs_rtl::AsyncReadHandle>,
    control: Option<rs_rtl::AsyncReadControlHandle>,
    drop_after_hop: u32,
    info: DeviceInfo,
}

impl RtlSource {
    pub fn open() -> Result<Self> {
        let descriptors = DeviceDescriptors::new()?;
        if descriptors.is_empty() {
            return Err(SourceError::Message(
                "No RTL-SDR found. On Windows, run: shackhack --setup-driver  (WinUSB on Bulk-In Interface 0; no kernel driver to compile)."
                    .into(),
            ));
        }
        let desc = descriptors.iter().next().cloned();
        let sdr = RtlSdr::open(DeviceId::Index(0))?;
        let tuner = match sdr.tuner_type() {
            TunerType::R820T => "R820T",
            TunerType::R828D => "R828D",
            TunerType::E4000 => "E4000",
        };
        let hf_ok = matches!(sdr.board_variant(), BoardVariant::RtlSdrBlogV4);
        let min_hz = if hf_ok {
            V4_MIN_HZ
        } else if sdr.tuner_type() == TunerType::E4000 {
            52_000_000
        } else {
            GENERIC_MIN_HZ
        };
        let product = desc
            .as_ref()
            .and_then(|d| d.product.clone())
            .unwrap_or_else(|| "RTL-SDR".into());
        let label = format!("{product} / {tuner}");
        let mut gains = sdr.gains().to_vec();
        gains.sort_unstable();
        Ok(Self {
            sdr,
            reader: None,
            control: None,
            drop_after_hop: 0,
            info: DeviceInfo {
                label,
                tuner: tuner.into(),
                min_hz,
                max_hz: GENERIC_MAX_HZ,
                hf_ok,
                gains,
            },
        })
    }

    fn start(&mut self, center_hz: u32, sample_rate: u32, gain: i32) -> Result<()> {
        self.sdr.set_sample_rate(sample_rate)?;
        self.sdr.set_center_freq(center_hz)?;
        self.sdr.set_gain_manual(gain)?;
        let reader = self.sdr.start_streaming()?;
        self.control = Some(reader.control_handle());
        self.reader = Some(reader);
        self.drop_after_hop = 2;
        Ok(())
    }

    fn hop(&mut self, center_hz: u32) -> Result<()> {
        if let Some(c) = &self.control {
            c.tune(center_hz)?;
            self.drop_after_hop = 2;
            Ok(())
        } else {
            self.sdr.set_center_freq(center_hz).map_err(Into::into)
        }
    }

    fn set_gain(&mut self, gain: i32) -> Result<()> {
        if let Some(c) = &self.control {
            c.set_gain(gain)?;
        } else {
            self.sdr.set_gain_manual(gain)?;
        }
        Ok(())
    }

    fn next_chunk(&mut self) -> Result<Option<Vec<u8>>> {
        let reader = self
            .reader
            .as_ref()
            .ok_or_else(|| SourceError::Message("not streaming".into()))?;
        loop {
            match reader.recv()? {
                None => return Ok(None),
                Some(data) => {
                    if self.drop_after_hop > 0 {
                        self.drop_after_hop -= 1;
                        continue;
                    }
                    return Ok(Some(data));
                }
            }
        }
    }
}

pub struct SynthSource {
    lo_hz: f64,
    sample_rate: f64,
    phase: f64,
    gain_tenth_db: i32,
    info: DeviceInfo,
}

impl SynthSource {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            lo_hz: 98_000_000.0,
            sample_rate: sample_rate as f64,
            phase: 0.0,
            gain_tenth_db: DEFAULT_GAIN_TENTH_DB,
            info: DeviceInfo {
                label: "Synth (no dongle)".into(),
                tuner: "none".into(),
                min_hz: GENERIC_MIN_HZ,
                max_hz: GENERIC_MAX_HZ,
                hf_ok: false,
                gains: (0..=49).map(|g| g * 10).collect(),
            },
        }
    }

    fn next_chunk(&mut self) -> Vec<u8> {
        // 8192 IQ samples, a few fake FM stations inside the window.
        let n = 8192usize;
        let mut bytes = vec![0u8; n * 2];
        let stations = [88.5e6, 91.1e6, 94.7e6, 98.5e6, 101.3e6, 105.1e6, 107.7e6];
        // Map tenths-dB to amplitude so the Gain slider is visible without a dongle.
        let amp = 10f64.powf((self.gain_tenth_db as f64 / 10.0 - 29.6) / 20.0) * 0.12;
        for i in 0..n {
            let t = i as f64 / self.sample_rate;
            let mut re = 0.0;
            let mut im = 0.0;
            for (k, f) in stations.iter().enumerate() {
                let offset = *f - self.lo_hz;
                if offset.abs() > self.sample_rate * 0.45 {
                    continue;
                }
                let audio = (2.0 * std::f64::consts::PI * (800.0 + k as f64 * 220.0) * t).sin();
                let inst = offset + 75_000.0 * audio * 0.15;
                self.phase += 2.0 * std::f64::consts::PI * inst / self.sample_rate;
                let (s, c) = self.phase.sin_cos();
                re += c * amp;
                im += s * amp;
            }
            self.phase %= 2.0 * std::f64::consts::PI;
            bytes[i * 2] = ((re.clamp(-1.0, 1.0) * 100.0) + 127.5) as u8;
            bytes[i * 2 + 1] = ((im.clamp(-1.0, 1.0) * 100.0) + 127.5) as u8;
        }
        bytes
    }
}

pub fn list_rtl_devices() -> std::result::Result<Vec<String>, String> {
    let d = DeviceDescriptors::new().map_err(|e| e.to_string())?;
    Ok(d.iter()
        .map(|dev| {
            format!(
                "#{} {} {}",
                dev.index,
                dev.product.as_deref().unwrap_or("RTL-SDR"),
                dev.serial.as_deref().unwrap_or("")
            )
        })
        .collect())
}
