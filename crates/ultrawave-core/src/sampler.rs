//! Exclusive-owner sampler. Every frame reads all voices before any RAM writes.
//! ROM and progressive RAM differ only at the common sampler's read boundary.
use crate::dsp::sample_engine::{quantize_12bit, SampleEngine, SampleRead};
use std::{fmt, ops::Range, sync::Arc};

pub enum Sample {
    Rom(Arc<[i16]>),
    Ram { capacity: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplerError {
    InvalidInstrument,
    InvalidVoice,
    ReadOnly,
    InvalidLength,
    InvalidPlayback,
    InvalidRegion,
    InstrumentCapacity,
}

impl fmt::Display for SamplerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInstrument => "invalid instrument",
            Self::InvalidVoice => "invalid voice",
            Self::ReadOnly => "cannot record into ROM",
            Self::InvalidLength => "capture length must be nonzero and fit RAM capacity",
            Self::InvalidPlayback => "playback requires finite nonzero ratio and finite gain",
            Self::InvalidRegion => "region must be ordered and within the sample extent",
            Self::InstrumentCapacity => {
                "reserve instrument capacity off-thread before appending ROM"
            }
        })
    }
}
impl std::error::Error for SamplerError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureStatus {
    pub extent: usize,
    pub written: usize,
    pub recording: bool,
}

enum Storage {
    Rom(Arc<[i16]>),
    Ram {
        samples: Box<[i16]>,
        valid_prefix: usize,
        capture: CaptureStatus,
    },
}

impl Storage {
    fn sample(&self, index: usize) -> Option<i16> {
        if index >= self.extent() {
            return None;
        }
        Some(match self {
            Self::Rom(samples) => samples[index],
            Self::Ram {
                samples,
                valid_prefix,
                ..
            } => {
                if index < *valid_prefix {
                    samples[index]
                } else {
                    0
                }
            }
        })
    }
    fn extent(&self) -> usize {
        match self {
            Self::Rom(samples) => samples.len(),
            Self::Ram { capture, .. } => capture.extent,
        }
    }
}

struct VoiceView<'a> {
    storage: &'a Storage,
    start: usize,
    end: usize,
}

impl SampleRead for VoiceView<'_> {
    fn extent(&self) -> usize {
        self.end.min(self.storage.extent())
    }

    fn pair(&self, index: usize) -> Option<(i16, i16)> {
        if index >= self.extent() {
            return None;
        }
        // A fractional reverse cursor can read just below its starting sample.
        // Clamp both interpolation endpoints, not only the integer cursor.
        if index < self.start {
            let sample = self.storage.sample(self.start)?;
            return Some((sample, sample));
        }
        Some((
            self.storage.sample(index)?,
            self.storage.sample((index + 1).min(self.extent() - 1))?,
        ))
    }
}

struct Voice {
    engine: SampleEngine,
    instrument: usize,
    start: usize,
    end: usize,
    ratio: f64,
    gain: f32,
    active: bool,
}

impl Voice {
    fn sounding(&self, extent: usize) -> bool {
        let position = self.engine.position();
        let end = self.end.min(extent) as f64;
        self.active
            && if self.ratio < 0.0 {
                position > self.start as f64 && position <= end
            } else {
                position >= self.start as f64 && position < end
            }
    }

    fn region(&mut self, region: Range<usize>) {
        self.start = region.start;
        self.end = region.end;
        self.active = !region.is_empty();
        self.engine.seek_at(if self.ratio < 0.0 {
            self.end
        } else {
            self.start
        } as f64);
    }
}

/// Construction, reservation and destruction are non-real-time. Other operations
/// are bounded by prepared counts. Samples cannot be mutated outside a frame.
pub struct Sampler {
    samples: Vec<Storage>,
    voices: Vec<Voice>,
    outputs: Vec<f32>,
    inputs: Vec<f32>,
}

impl Sampler {
    pub fn new(samples: Vec<Sample>, voices: usize) -> Self {
        let inputs = vec![0.0; samples.len()];
        Self {
            samples: samples
                .into_iter()
                .map(|sample| match sample {
                    Sample::Rom(samples) => Storage::Rom(samples),
                    Sample::Ram { capacity } => Storage::Ram {
                        samples: vec![0; capacity].into_boxed_slice(),
                        valid_prefix: 0,
                        capture: CaptureStatus {
                            extent: 0,
                            written: 0,
                            recording: false,
                        },
                    },
                })
                .collect(),
            voices: (0..voices)
                .map(|_| Voice {
                    engine: SampleEngine::new(),
                    instrument: 0,
                    start: 0,
                    end: 0,
                    ratio: 1.0,
                    gain: 1.0,
                    active: false,
                })
                .collect(),
            outputs: vec![0.0; voices],
            inputs,
        }
    }

    /// Non-real-time: reserve metadata for at least this many instruments.
    pub fn reserve_instruments(&mut self, capacity: usize) {
        self.samples
            .reserve(capacity.saturating_sub(self.samples.len()));
        self.inputs
            .reserve(capacity.saturating_sub(self.inputs.len()));
    }

    /// Install prepared immutable signed-12-bit audio without allocating, freeing
    /// old assets or disturbing voices. On refusal the caller retains ownership.
    pub fn append_rom(&mut self, samples: &Arc<[i16]>) -> Result<usize, SamplerError> {
        if self.samples.len() == self.samples.capacity()
            || self.inputs.len() == self.inputs.capacity()
        {
            return Err(SamplerError::InstrumentCapacity);
        }
        let instrument = self.samples.len();
        self.samples.push(Storage::Rom(samples.clone()));
        self.inputs.push(0.0);
        Ok(instrument)
    }

    /// Establish the full intended extent in O(1), retaining old audio inside it.
    /// Shrink discards the suffix logically; later extension cannot resurrect it.
    pub fn record(&mut self, instrument: usize, frames: usize) -> Result<(), SamplerError> {
        let storage = self
            .samples
            .get_mut(instrument)
            .ok_or(SamplerError::InvalidInstrument)?;
        let Storage::Ram {
            samples,
            valid_prefix,
            capture,
        } = storage
        else {
            return Err(SamplerError::ReadOnly);
        };
        if frames == 0 || frames > samples.len() {
            return Err(SamplerError::InvalidLength);
        }
        *valid_prefix = (*valid_prefix).min(frames);
        *capture = CaptureStatus {
            extent: frames,
            written: 0,
            recording: true,
        };
        for voice in &mut self.voices {
            if voice.instrument == instrument
                && (voice.engine.position() > frames as f64
                    || (voice.ratio > 0.0 && voice.engine.position() == frames as f64))
            {
                voice.active = false;
            }
        }
        Ok(())
    }

    pub fn play(
        &mut self,
        voice: usize,
        instrument: usize,
        ratio: f64,
        gain: f32,
    ) -> Result<(), SamplerError> {
        let end = self
            .extent(instrument)
            .ok_or(SamplerError::InvalidInstrument)?;
        self.play_region(voice, instrument, 0..end, ratio, gain)
    }

    /// Latch absolute source-frame bounds. Empty regions are valid silence.
    pub fn play_region(
        &mut self,
        voice: usize,
        instrument: usize,
        region: Range<usize>,
        ratio: f64,
        gain: f32,
    ) -> Result<(), SamplerError> {
        let extent = self
            .extent(instrument)
            .ok_or(SamplerError::InvalidInstrument)?;
        let voice = self
            .voices
            .get_mut(voice)
            .ok_or(SamplerError::InvalidVoice)?;
        if !ratio.is_finite() || ratio == 0.0 || !gain.is_finite() {
            return Err(SamplerError::InvalidPlayback);
        }
        if region.start > region.end || region.end > extent {
            return Err(SamplerError::InvalidRegion);
        }
        voice.instrument = instrument;
        voice.ratio = ratio;
        voice.gain = gain;
        voice.engine.trigger_at(0.0);
        voice.region(region);
        Ok(())
    }

    /// Seek an existing voice without resetting its pitch, gain or DSP history.
    /// Ended voices are never resurrected by a region change.
    pub fn set_region(&mut self, voice: usize, region: Range<usize>) -> Result<(), SamplerError> {
        let voice = self
            .voices
            .get_mut(voice)
            .ok_or(SamplerError::InvalidVoice)?;
        let Some(storage) = self.samples.get(voice.instrument) else {
            return Ok(());
        };
        if !voice.sounding(storage.extent()) {
            return Ok(());
        }
        if region.start > region.end || region.end > storage.extent() {
            return Err(SamplerError::InvalidRegion);
        }
        voice.region(region);
        Ok(())
    }

    pub fn voice_instrument(&self, voice: usize) -> Option<usize> {
        let voice = self.voices.get(voice)?;
        voice
            .sounding(self.samples.get(voice.instrument)?.extent())
            .then_some(voice.instrument)
    }

    pub fn extent(&self, instrument: usize) -> Option<usize> {
        self.samples.get(instrument).map(Storage::extent)
    }

    pub fn cut(&mut self, voice: usize) -> Result<(), SamplerError> {
        self.voices
            .get_mut(voice)
            .ok_or(SamplerError::InvalidVoice)?
            .active = false;
        Ok(())
    }

    /// Stop retains the full intended extent, not just the newly written prefix.
    pub fn stop(&mut self) {
        for voice in &mut self.voices {
            voice.active = false;
        }
        for storage in &mut self.samples {
            if let Storage::Ram { capture, .. } = storage {
                capture.recording = false;
            }
        }
    }

    pub fn capture_status(&self, instrument: usize) -> Option<CaptureStatus> {
        match self.samples.get(instrument)? {
            Storage::Ram { capture, .. } => Some(*capture),
            Storage::Rom(_) => None,
        }
    }

    /// Inspect one logical stored code without allocating or exposing mutable RAM.
    pub fn sample(&self, instrument: usize, index: usize) -> Option<i16> {
        self.samples.get(instrument)?.sample(index)
    }

    /// Route already-rendered voice outputs into one mono input per instrument.
    /// Inputs start at zero; only recording RAM destinations consume them. The
    /// closure cannot access this sampler, and must not allocate/block in real time.
    pub fn render_frame<T>(&mut self, route: impl FnOnce(&[f32], &mut [f32]) -> T) -> T {
        for (voice, output) in self.voices.iter_mut().zip(&mut self.outputs) {
            *output = 0.0;
            if !voice.active {
                continue;
            }
            let storage = &self.samples[voice.instrument];
            if !voice.sounding(storage.extent()) {
                voice.active = false;
                continue;
            }
            let view = VoiceView {
                storage,
                start: voice.start,
                end: voice.end,
            };
            *output = voice.engine.read_from(&view, voice.ratio) * voice.gain;
        }
        self.inputs.fill(0.0);
        let result = route(&self.outputs, &mut self.inputs);
        for (storage, input) in self.samples.iter_mut().zip(&self.inputs) {
            if let Storage::Ram {
                samples,
                valid_prefix,
                capture,
            } = storage
            {
                if capture.recording {
                    samples[capture.written] = quantize_12bit(*input);
                    capture.written += 1;
                    *valid_prefix = (*valid_prefix).max(capture.written);
                    capture.recording = capture.written < capture.extent;
                }
            }
        }
        result
    }
}
