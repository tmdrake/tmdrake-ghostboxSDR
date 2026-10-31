use std::collections::VecDeque;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;

pub struct AudioOut {
    _stream: cpal::Stream,
}

impl AudioOut {
    pub fn start(queue: Arc<Mutex<VecDeque<f32>>>) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("no audio output device"))?;
        let config = device.default_output_config()?;
        let channels = config.channels() as usize;
        let err_fn = |e| eprintln!("audio error: {e}");

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let cfg: cpal::StreamConfig = config.into();
                device.build_output_stream(
                    &cfg,
                    move |data: &mut [f32], _| fill(data, channels, &queue),
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let cfg: cpal::StreamConfig = config.into();
                device.build_output_stream(
                    &cfg,
                    move |data: &mut [i16], _| {
                        let mut tmp = vec![0.0_f32; data.len()];
                        fill(&mut tmp, channels, &queue);
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
        Ok(Self { _stream: stream })
    }
}

fn fill(out: &mut [f32], channels: usize, queue: &Arc<Mutex<VecDeque<f32>>>) {
    let mut q = queue.lock();
    let frames = out.len() / channels.max(1);
    for f in 0..frames {
        let s = q.pop_front().unwrap_or(0.0);
        for c in 0..channels {
            out[f * channels + c] = s;
        }
    }
}
