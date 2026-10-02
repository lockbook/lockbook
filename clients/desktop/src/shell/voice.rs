//! The desktop's audio for a chat's spoken conversation: the default
//! microphone and speaker through cpal, resampled to and from the wire's
//! 24 kHz mono. Nothing cancels the speaker out of the microphone here, so
//! without headphones the reply is heard as the user and cuts itself off.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use tracing::error;
use workspace_rs::voice;
use workspace_rs::workspace::Workspace;

const WIRE_HZ: u32 = 24_000;

#[derive(Default)]
pub struct Voice {
    duplex: Option<Duplex>,
}

impl Voice {
    /// Takes what the workspace has for the audio. Called after every frame.
    pub fn service(&mut self, workspace: &mut Workspace) {
        workspace.pump_chats();
        let out = voice::take();
        if out.start {
            match Duplex::open() {
                Ok(duplex) => self.duplex = Some(duplex),
                Err(e) => {
                    error!("voice: {e}");
                    voice::hang_up();
                }
            }
        }
        if let Some(duplex) = &self.duplex {
            if out.flush {
                duplex.flush();
            }
            if let Some((reply, pcm)) = out.play {
                duplex.play(reply, &pcm);
            }
        }
        if out.stop {
            self.duplex = None;
        }
    }
}

/// The replies' audio at the speaker's rate, in order, with how much of
/// each has played and which have played since last reported.
#[derive(Default)]
struct Queue {
    chunks: VecDeque<(u32, Vec<i16>, usize)>,
    played: Vec<(u32, u64)>,
    touched: Vec<u32>,
}

impl Queue {
    /// The next sample, counting it as played for its reply.
    fn pop(&mut self) -> Option<i16> {
        let (reply, chunk, at) = self.chunks.front_mut()?;
        let sample = chunk[*at];
        *at += 1;
        let reply = *reply;
        if *at == chunk.len() {
            self.chunks.pop_front();
        }
        match self.played.iter_mut().find(|(r, _)| *r == reply) {
            Some((_, frames)) => *frames += 1,
            None => self.played.push((reply, 1)),
        }
        if !self.touched.contains(&reply) {
            self.touched.push(reply);
        }
        Some(sample)
    }

    /// How far each reply played since last asked has got, in frames.
    fn report(&mut self) -> Vec<(u32, u64)> {
        let touched = std::mem::take(&mut self.touched);
        touched
            .into_iter()
            .filter_map(|reply| {
                let frames = self.played.iter().find(|(r, _)| *r == reply)?.1;
                Some((reply, frames))
            })
            .collect()
    }
}

struct Duplex {
    queue: Arc<Mutex<Queue>>,
    out_hz: u32,
    _input: Stream,
    _output: Stream,
}

impl Duplex {
    fn open() -> Result<Self, String> {
        let host = cpal::default_host();
        let input = host.default_input_device().ok_or("no microphone")?;
        let output = host.default_output_device().ok_or("no speaker")?;
        let queue = Arc::new(Mutex::new(Queue::default()));
        let _input = start_input(&input)?;
        let (_output, out_hz) = start_output(&output, Arc::clone(&queue))?;
        Ok(Self { queue, out_hz, _input, _output })
    }

    fn play(&self, reply: u32, pcm: &[u8]) {
        let samples = resample(&pcm16_le(pcm), WIRE_HZ, self.out_hz);
        if samples.is_empty() {
            return;
        }
        self.queue
            .lock()
            .unwrap()
            .chunks
            .push_back((reply, samples, 0));
    }

    fn flush(&self) {
        self.queue.lock().unwrap().chunks.clear();
    }
}

fn pcm16_le(bytes: &[u8]) -> Vec<i16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c))
        .collect()
}

/// Linear interpolation, chunk by chunk.
fn resample(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if input.is_empty() || from == to {
        return input.to_vec();
    }
    let n = (input.len() as u64 * u64::from(to) / u64::from(from)).max(1) as usize;
    let last = input.len() - 1;
    (0..n)
        .map(|i| {
            let pos = i as f64 * f64::from(from) / f64::from(to);
            let j = (pos as usize).min(last);
            let frac = pos - j as f64;
            let (a, b) = (f64::from(input[j]), f64::from(input[(j + 1).min(last)]));
            (a + (b - a) * frac).round() as i16
        })
        .collect()
}

/// The microphone, sent up as the wire takes it.
fn start_input(device: &cpal::Device) -> Result<Stream, String> {
    let cfg = device
        .default_input_config()
        .map_err(|e| format!("microphone: {e}"))?;
    let (hz, channels, format) =
        (cfg.sample_rate().0, cfg.channels().max(1) as usize, cfg.sample_format());
    let cfg: StreamConfig = cfg.into();
    let send = move |mono: Vec<i16>| {
        let samples = resample(&mono, hz, WIRE_HZ);
        voice::audio(samples.iter().flat_map(|s| s.to_le_bytes()).collect());
    };
    let err = |e| error!("voice microphone: {e}");
    let stream = match format {
        SampleFormat::F32 => device.build_input_stream(
            &cfg,
            move |data: &[f32], _| {
                send(
                    data.chunks(channels)
                        .map(|f| (f[0].clamp(-1.0, 1.0) * 32767.0) as i16)
                        .collect(),
                )
            },
            err,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &cfg,
            move |data: &[i16], _| send(data.chunks(channels).map(|f| f[0]).collect()),
            err,
            None,
        ),
        other => return Err(format!("microphone gives {other}, which is not taken")),
    }
    .map_err(|e| format!("microphone: {e}"))?;
    stream.play().map_err(|e| format!("microphone: {e}"))?;
    Ok(stream)
}

/// The speaker, playing the queue and reporting what it has played.
fn start_output(device: &cpal::Device, queue: Arc<Mutex<Queue>>) -> Result<(Stream, u32), String> {
    let cfg = device
        .default_output_config()
        .map_err(|e| format!("speaker: {e}"))?;
    let (hz, channels, format) =
        (cfg.sample_rate().0, cfg.channels().max(1) as usize, cfg.sample_format());
    let cfg: StreamConfig = cfg.into();
    let fill = move |frames: usize, put: &mut dyn FnMut(usize, i16)| {
        let mut queue = queue.lock().unwrap();
        for frame in 0..frames {
            put(frame, queue.pop().unwrap_or(0));
        }
        let played = queue.report();
        drop(queue);
        for (reply, frames) in played {
            voice::played(reply, frames * 1000 / u64::from(hz));
        }
    };
    let err = |e| error!("voice speaker: {e}");
    let stream = match format {
        SampleFormat::F32 => device.build_output_stream(
            &cfg,
            move |data: &mut [f32], _| {
                fill(data.len() / channels, &mut |frame, s| {
                    data[frame * channels..(frame + 1) * channels].fill(s as f32 / 32768.0)
                })
            },
            err,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            &cfg,
            move |data: &mut [i16], _| {
                fill(data.len() / channels, &mut |frame, s| {
                    data[frame * channels..(frame + 1) * channels].fill(s)
                })
            },
            err,
            None,
        ),
        other => return Err(format!("speaker takes {other}, which is not given")),
    }
    .map_err(|e| format!("speaker: {e}"))?;
    stream.play().map_err(|e| format!("speaker: {e}"))?;
    Ok((stream, hz))
}
