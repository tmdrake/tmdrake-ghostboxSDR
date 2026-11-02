use std::collections::VecDeque;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;

use shackhack_dsp::AUDIO_RATE;

pub struct AudioOut {
    _stream: cpal::Stream,
    pub device_name: String,
}

struct Pace {
    /// Source samples to consume for each device frame.
    step: f64,
    frac: f64,
    held: f32,
}

impl AudioOut {
    /// Playback devices, including virtual cables. Names match the Windows sound list.
    pub fn output_devices() -> Vec<String> {
        let host = cpal::default_host();
        let Ok(devs) = host.output_devices() else {
            return Vec::new();
        };
        let mut names = Vec::new();
        for dev in devs {
            if let Ok(name) = dev.name() {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names
    }

    /// `preferred` is a device name such as `CABLE Input (VB-Audio Virtual Cable)`.
    /// An empty or missing name uses the Windows default playback device.
    pub fn start(
        queue: Arc<Mutex<VecDeque<f32>>>,
        preferred: Option<&str>,
    ) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let (device, device_name) = pick_device(&host, preferred)?;
        let config = device.default_output_config()?;
        let channels = config.channels() as usize;
        let device_rate = config.sample_rate().0 as f64;
        let format = config.sample_format();
        eprintln!("audio output: {device_name} @ {device_rate} Hz, {channels} ch, {format:?}");
        let pace = Arc::new(Mutex::new(Pace {
            step: AUDIO_RATE / device_rate,
            frac: 0.0,
            held: 0.0,
        }));
        let err_fn = |e| eprintln!("audio error: {e}");

        let stream = match format {
            cpal::SampleFormat::F32 => {
                let cfg: cpal::StreamConfig = config.into();
                let queue = queue.clone();
                let pace = pace.clone();
                device.build_output_stream(
                    &cfg,
                    move |data: &mut [f32], _| fill(data, channels, &queue, &pace),
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let cfg: cpal::StreamConfig = config.into();
                let queue = queue.clone();
                let pace = pace.clone();
                device.build_output_stream(
                    &cfg,
                    move |data: &mut [i16], _| {
                        let mut tmp = vec![0.0_f32; data.len()];
                        fill(&mut tmp, channels, &queue, &pace);
                        for (o, s) in data.iter_mut().zip(tmp) {
                            *o = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            other => anyhow::bail!("unsupported sample format {other}"),
        };
        stream.play()?;
        Ok(Self {
            _stream: stream,
            device_name,
        })
    }
}

fn pick_device(
    host: &cpal::Host,
    preferred: Option<&str>,
) -> anyhow::Result<(cpal::Device, String)> {
    if let Some(want) = preferred.map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(devs) = host.output_devices() {
            for dev in devs {
                if let Ok(name) = dev.name() {
                    if name == want {
                        return Ok((dev, name));
                    }
                }
            }
        }
    }
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("no audio output device"))?;
    let name = device.name().unwrap_or_else(|_| "Default".into());
    Ok((device, name))
}

fn fill(
    out: &mut [f32],
    channels: usize,
    queue: &Arc<Mutex<VecDeque<f32>>>,
    pace: &Arc<Mutex<Pace>>,
) {
    let mut q = queue.lock();
    let mut pace = pace.lock();
    let frames = out.len() / channels.max(1);
    for f in 0..frames {
        let s = pace.next_sample(&mut q);
        for c in 0..channels {
            out[f * channels + c] = s;
        }
    }
}

impl Pace {
    fn next_sample(&mut self, queue: &mut VecDeque<f32>) -> f32 {
        self.frac += self.step;
        while self.frac >= 1.0 {
            if let Some(sample) = queue.pop_front() {
                self.held = sample;
            }
            self.frac -= 1.0;
        }
        let next = queue.front().copied().unwrap_or(self.held);
        self.held + (next - self.held) * self.frac as f32
    }
}
