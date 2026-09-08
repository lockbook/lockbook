//! Headphones duplex: mic → 24 kHz mono i16, speaker ← same. No AEC.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use tokio::sync::mpsc;
use tracing::error;

pub const VOICE_HZ: u32 = 24_000;
const PLAY_AHEAD_SECS: u32 = 120;

#[derive(Default)]
struct PlayQ {
    samples: VecDeque<i16>,
}

impl PlayQ {
    fn len(&self) -> usize {
        self.samples.len()
    }

    fn clear(&mut self) {
        self.samples.clear();
    }

    fn push(&mut self, pcm: &[i16], cap: usize) -> usize {
        if pcm.is_empty() || self.samples.len() >= cap {
            return 0;
        }
        let n = pcm.len().min(cap - self.samples.len());
        self.samples.extend(&pcm[..n]);
        n
    }

    fn pop_sample(&mut self) -> Option<i16> {
        self.samples.pop_front()
    }
}

pub struct Duplex {
    pub mic: mpsc::UnboundedReceiver<Vec<i16>>,
    mic_tx: mpsc::UnboundedSender<Vec<i16>>,
    play: Arc<Mutex<PlayQ>>,
    _in: Stream,
    _out: Stream,
    pub out_hz: u32,
    pub in_name: String,
    pub out_name: String,
}

impl Duplex {
    pub fn open(input: &str, output: &str) -> Result<Self, String> {
        let in_dev = named_or_default(true, input)?;
        let out_dev = named_or_default(false, output)?;
        let (mic_tx, mic_rx) = mpsc::unbounded_channel();
        let play = Arc::new(Mutex::new(PlayQ::default()));
        let (inn, _in_hz, in_name) = start_input(&in_dev, mic_tx.clone())?;
        let (out, out_hz, out_name) = start_output(&out_dev, Arc::clone(&play))?;
        Ok(Self { mic: mic_rx, mic_tx, play, _in: inn, _out: out, out_hz, in_name, out_name })
    }

    pub fn input_names() -> Result<Vec<String>, String> {
        device_names(true)
    }

    pub fn output_names() -> Result<Vec<String>, String> {
        device_names(false)
    }

    pub fn set_input(&mut self, spec: &str) -> Result<String, String> {
        let names = Self::input_names()?;
        let want = super::tools::pick_named(&names, &self.in_name, spec)?;
        let device = named_device(true, &want)?;
        let (stream, _hz, name) = start_input(&device, self.mic_tx.clone())?;
        self._in = stream;
        self.in_name = name;
        Ok(format!("input is {}", self.in_name))
    }

    pub fn set_output(&mut self, spec: &str) -> Result<String, String> {
        let names = Self::output_names()?;
        let want = super::tools::pick_named(&names, &self.out_name, spec)?;
        let device = named_device(false, &want)?;
        let (stream, hz, name) = start_output(&device, Arc::clone(&self.play))?;
        self.play.lock().unwrap().clear();
        self._out = stream;
        self.out_hz = hz;
        self.out_name = name;
        Ok(format!("output is {}", self.out_name))
    }

    pub fn push_voice_pcm16(&self, bytes: &[u8]) -> usize {
        let samples = pcm16_le(bytes);
        let up = resample(&samples, VOICE_HZ, self.out_hz);
        let mut q = self.play.lock().unwrap();
        let cap = (self.out_hz as usize).saturating_mul(PLAY_AHEAD_SECS as usize);
        q.push(&up, cap)
    }

    pub fn barge_in(&self) {
        self.play.lock().unwrap().clear();
    }

    pub fn queue_len(&self) -> usize {
        self.play.lock().unwrap().len()
    }

    /// Pause both streams and drop the play queue. Hangup calls this before
    /// drop so the speaker doesn't keep draining leftover PCM.
    pub fn stop(&mut self) {
        self.play.lock().unwrap().clear();
        let _ = self._in.pause();
        let _ = self._out.pause();
    }
}

pub fn pcm16_le(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn device_names(input: bool) -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    let iter = if input {
        host.input_devices()
            .map_err(|e| format!("input devices: {e}"))?
    } else {
        host.output_devices()
            .map_err(|e| format!("output devices: {e}"))?
    };
    let mut names = Vec::new();
    for d in iter {
        if let Ok(n) = d.name() {
            if !n.is_empty() {
                names.push(n);
            }
        }
    }
    Ok(names)
}

fn named_device(input: bool, want: &str) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let iter = if input {
        host.input_devices()
            .map_err(|e| format!("input devices: {e}"))?
    } else {
        host.output_devices()
            .map_err(|e| format!("output devices: {e}"))?
    };
    for d in iter {
        if d.name().ok().as_deref() == Some(want) {
            return Ok(d);
        }
    }
    Err(format!("{want} is gone"))
}

fn named_or_default(input: bool, want: &str) -> Result<cpal::Device, String> {
    if !want.is_empty() {
        if let Ok(d) = named_device(input, want) {
            return Ok(d);
        }
    }
    let host = cpal::default_host();
    if input {
        host.default_input_device()
            .ok_or_else(|| "no default input device".into())
    } else {
        host.default_output_device()
            .ok_or_else(|| "no default output device".into())
    }
}

pub fn i16_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

pub fn resample(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if input.is_empty() || from == 0 || to == 0 {
        return Vec::new();
    }
    if from == to {
        return input.to_vec();
    }
    let n = (input.len() as u64 * u64::from(to) / u64::from(from)).max(1) as usize;
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let pos = i as f64 * f64::from(from) / f64::from(to);
        let j = (pos as usize).min(last);
        let frac = pos - j as f64;
        let a = f64::from(input[j]);
        let b = f64::from(input[(j + 1).min(last)]);
        out.push((a + (b - a) * frac).round() as i16);
    }
    out
}

fn start_input(
    device: &cpal::Device, tx: mpsc::UnboundedSender<Vec<i16>>,
) -> Result<(Stream, u32, String), String> {
    let name = device.name().unwrap_or_else(|_| "input".into());
    let cfg = device
        .default_input_config()
        .map_err(|e| format!("input config: {e}"))?;
    let hz = cfg.sample_rate().0;
    let ch = cfg.channels() as usize;
    let format = cfg.sample_format();
    let stream_cfg: StreamConfig = cfg.into();
    let ch = ch.max(1);
    let stream = match format {
        SampleFormat::F32 => device
            .build_input_stream(
                &stream_cfg,
                move |data: &[f32], _| {
                    let mono: Vec<i16> = data
                        .chunks(ch)
                        .map(|f| (f[0].clamp(-1.0, 1.0) * 32767.0) as i16)
                        .collect();
                    let _ = tx.send(resample(&mono, hz, VOICE_HZ));
                },
                |e| error!(error = %e, "chat mic"),
                None,
            )
            .map_err(|e| format!("mic stream: {e}"))?,
        SampleFormat::I16 => device
            .build_input_stream(
                &stream_cfg,
                move |data: &[i16], _| {
                    let mono: Vec<i16> = data.chunks(ch).map(|f| f[0]).collect();
                    let _ = tx.send(resample(&mono, hz, VOICE_HZ));
                },
                |e| error!(error = %e, "chat mic"),
                None,
            )
            .map_err(|e| format!("mic stream: {e}"))?,
        other => return Err(format!("unsupported input format {other}")),
    };
    stream.play().map_err(|e| format!("mic start: {e}"))?;
    Ok((stream, hz, name))
}

fn start_output(
    device: &cpal::Device, play: Arc<Mutex<PlayQ>>,
) -> Result<(Stream, u32, String), String> {
    let name = device.name().unwrap_or_else(|_| "output".into());
    let cfg = device
        .default_output_config()
        .map_err(|e| format!("output config: {e}"))?;
    let hz = cfg.sample_rate().0;
    let ch = cfg.channels() as usize;
    let format = cfg.sample_format();
    let stream_cfg: StreamConfig = cfg.into();
    let ch = ch.max(1);
    let stream = match format {
        SampleFormat::F32 => device
            .build_output_stream(
                &stream_cfg,
                move |data: &mut [f32], _| {
                    let mut q = play.lock().unwrap();
                    for frame in data.chunks_mut(ch) {
                        let s = q.pop_sample().unwrap_or(0);
                        let f = s as f32 / 32768.0;
                        for slot in frame {
                            *slot = f;
                        }
                    }
                },
                |e| error!(error = %e, "chat speaker"),
                None,
            )
            .map_err(|e| format!("speaker stream: {e}"))?,
        SampleFormat::I16 => device
            .build_output_stream(
                &stream_cfg,
                move |data: &mut [i16], _| {
                    let mut q = play.lock().unwrap();
                    for frame in data.chunks_mut(ch) {
                        let s = q.pop_sample().unwrap_or(0);
                        for slot in frame {
                            *slot = s;
                        }
                    }
                },
                |e| error!(error = %e, "chat speaker"),
                None,
            )
            .map_err(|e| format!("speaker stream: {e}"))?,
        other => return Err(format!("unsupported output format {other}")),
    };
    stream.play().map_err(|e| format!("speaker start: {e}"))?;
    Ok((stream, hz, name))
}

#[cfg(test)]
mod tests {
    use super::resample;

    #[test]
    fn resample_identity() {
        let v = vec![0i16, 100, -100];
        assert_eq!(resample(&v, 24000, 24000), v);
    }

    #[test]
    fn resample_double_rate_length() {
        let v = vec![0i16, 1000];
        assert_eq!(resample(&v, 24000, 48000).len(), 4);
    }
}
