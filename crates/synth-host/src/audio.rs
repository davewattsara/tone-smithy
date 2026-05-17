//! Audio output via `cpal`.
//!
//! M1 scope: open the default output device, build a stream that drives a
//! `synth_engine::Engine` from the audio callback, and report the chosen
//! sample rate / buffer size for the UI status line. The silent variant
//! ([`start_silent`]) is kept so we can still bring the device up without
//! the engine while diagnosing audio-host issues.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use synth_engine::Engine;
use synth_engine::param_bus::{EngineEventReceiver, SnapshotSlot, store_snapshot};
use thiserror::Error;

/// Errors that can occur while opening or starting the audio output stream.
#[derive(Debug, Error)]
pub enum AudioError {
    /// No default output device is available on the current host.
    #[error("no default output device available")]
    NoDefaultDevice,

    /// The host returned no default stream configuration for the device.
    #[error("could not query default output config: {0}")]
    DefaultConfig(#[from] cpal::DefaultStreamConfigError),

    /// The host refused to build an output stream with the requested config.
    #[error("could not build output stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),

    /// The host refused to start the stream.
    #[error("could not start output stream: {0}")]
    StartStream(#[from] cpal::PlayStreamError),

    /// The default output device uses a sample format we do not yet handle.
    /// All common formats (f32 / i16 / u16) are supported; this only fires
    /// for unusual host configurations.
    #[error("unsupported sample format: {0:?}")]
    UnsupportedSampleFormat(cpal::SampleFormat),

    /// The default output device opened with a channel count the engine
    /// does not yet support. M1 expects mono or stereo (1 or 2 channels);
    /// surround layouts will be addressed when the project supports them.
    #[error("unsupported channel count {0}; engine currently expects 1 or 2 channels")]
    UnsupportedChannelCount(u16),
}

/// Describes the format the default output device opens with, so the
/// caller can build an [`Engine`] at the correct sample rate before the
/// stream is started.
pub struct DefaultOutputFormat {
    /// Sample rate in Hz.
    pub sample_rate: u32,

    /// Channel count.
    pub channels: u16,

    /// Human-readable buffer-size range / latency summary.
    pub buffer_latency_hint: String,
}

/// Queries the default output device for the format it would open at,
/// without starting a stream. Lets `synth-app` size the engine before
/// calling [`start_with_engine`].
///
/// # Errors
///
/// Returns [`AudioError::NoDefaultDevice`] if no default output device
/// exists, or [`AudioError::DefaultConfig`] if its default config cannot
/// be queried.
pub fn default_output_format() -> Result<DefaultOutputFormat, AudioError> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(AudioError::NoDefaultDevice)?;
    let supported = device.default_output_config()?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels();
    let buffer_latency_hint = describe_buffer_latency(supported.buffer_size(), sample_rate);
    Ok(DefaultOutputFormat { sample_rate, channels, buffer_latency_hint })
}

/// A running audio output stream.
///
/// The stream stays alive for as long as this value lives. Drop it to stop
/// audio. `cpal::Stream` is intentionally `!Send` because some platforms
/// require it to be dropped on the same thread that created it; this value
/// inherits that constraint.
pub struct AudioStream {
    _stream: cpal::Stream,

    /// Sample rate the device opened at, in Hz.
    pub sample_rate: u32,

    /// Channel count the device opened with (typically 2 for stereo).
    pub channels: u16,

    /// Human-readable summary of the device's supported buffer-size range and
    /// the corresponding output latency at the open sample rate. The exact
    /// runtime buffer size is whatever cpal picks from this range (the M0
    /// build uses `BufferSize::Default`); later milestones will let the user
    /// pin a specific size.
    pub buffer_latency_hint: String,
}

/// Opens the default output device and writes silence to it.
///
/// Returns an [`AudioStream`] that **must be kept alive** for as long as
/// audio should play. Dropping the returned value stops the stream.
///
/// Used in M0-style diagnostics and tests; for actual playback prefer
/// [`start_with_engine`].
///
/// # Errors
///
/// Returns [`AudioError`] if no default output device exists, the device
/// cannot report a default config, the stream cannot be built, the stream
/// cannot be started, or the device uses a sample format this build does
/// not yet handle.
pub fn start_silent() -> Result<AudioStream, AudioError> {
    let DeviceOpen { device, config, sample_rate, channels, sample_format, buffer_latency_hint } =
        open_default_output()?;
    log_open(channels, sample_rate, sample_format, &buffer_latency_hint);

    let err_fn = |err: cpal::StreamError| tracing::error!("audio stream error: {err}");

    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config,
            move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            move |data: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    *sample = 0;
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config,
            move |data: &mut [u16], _info: &cpal::OutputCallbackInfo| {
                // Silence in unsigned PCM is the midpoint, not zero.
                for sample in data.iter_mut() {
                    *sample = u16::MAX / 2;
                }
            },
            err_fn,
            None,
        )?,
        other => return Err(AudioError::UnsupportedSampleFormat(other)),
    };

    stream.play()?;

    Ok(AudioStream { _stream: stream, sample_rate, channels, buffer_latency_hint })
}

/// Opens the default output device and drives it from `engine`.
///
/// The engine is moved into the audio callback and lives on the audio
/// thread for the lifetime of the stream. Each block the callback:
///
/// 1. drains every event waiting on `events` and applies it,
/// 2. renders samples via [`Engine::process_stereo`], and
/// 3. publishes a fresh [`synth_engine::ParamSnapshot`] into `snapshot_slot`.
///
/// Only `f32` output is supported on the playback path right now (it's the
/// universal modern format; the silent path keeps i16/u16 for diagnostics).
///
/// # Errors
///
/// Returns [`AudioError`] for the same reasons as [`start_silent`], plus
/// [`AudioError::UnsupportedSampleFormat`] if the device opens at a
/// non-`f32` format, and [`AudioError::UnsupportedChannelCount`] if the
/// device opens with more than 2 channels.
pub fn start_with_engine(
    mut engine: Engine,
    events: EngineEventReceiver,
    snapshot_slot: SnapshotSlot,
) -> Result<AudioStream, AudioError> {
    let DeviceOpen { device, config, sample_rate, channels, sample_format, buffer_latency_hint } =
        open_default_output()?;
    log_open(channels, sample_rate, sample_format, &buffer_latency_hint);

    if sample_format != cpal::SampleFormat::F32 {
        return Err(AudioError::UnsupportedSampleFormat(sample_format));
    }
    if channels != 1 && channels != 2 {
        return Err(AudioError::UnsupportedChannelCount(channels));
    }

    // Pre-allocate the scratch buffer the callback uses for stereo
    // engine output. Sizing to MAX_BLOCK_SIZE keeps the audio callback
    // allocation-free no matter what buffer size cpal hands us — see
    // design-patterns.md §2.5 and §2.1.
    let mut stereo_scratch: Vec<f32> = vec![0.0; synth_engine::MAX_BLOCK_SIZE * 2];

    let channels_usize = usize::from(channels);
    let err_fn = |err: cpal::StreamError| tracing::error!("audio stream error: {err}");

    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
            // 1. Drain any queued events. Bounded loop: the queue
            //    capacity caps how many events arrive between blocks.
            while let Some(event) = events.try_recv() {
                engine.handle(event);
            }

            // 2. Render samples.
            let frames = data.len() / channels_usize;
            // If cpal ever asks for more frames than our scratch allows,
            // truncate. This shouldn't happen — MAX_BLOCK_SIZE is well
            // above any common driver buffer — but we'd rather render
            // silence into the tail than allocate on the audio thread.
            let frames = frames.min(synth_engine::MAX_BLOCK_SIZE);
            let scratch = &mut stereo_scratch[..frames * 2];
            engine.process_stereo(scratch, frames);

            // Interleave/route to the output buffer. Mono outputs take
            // the left channel only; stereo outputs map L/R directly.
            match channels_usize {
                1 => {
                    for frame_index in 0..frames {
                        data[frame_index] = scratch[frame_index * 2];
                    }
                }
                2 => {
                    for frame_index in 0..frames {
                        data[frame_index * 2] = scratch[frame_index * 2];
                        data[frame_index * 2 + 1] = scratch[frame_index * 2 + 1];
                    }
                }
                _ => unreachable!("channel count validated above"),
            }
            // Zero anything beyond what we rendered (defensive; the
            // truncation path above is the only way this matters).
            for sample in data.iter_mut().skip(frames * channels_usize) {
                *sample = 0.0;
            }

            // 3. Publish a snapshot. One `Arc::new` allocation per
            //    block — outside the DSP hot path; the recycled-pool
            //    optimisation from design-patterns.md §2.5 is a later
            //    milestone. The C6 `assert_no_alloc` test scopes to
            //    `Engine::process_stereo` only, not the publishing.
            store_snapshot(&snapshot_slot, engine.snapshot());
        },
        err_fn,
        None,
    )?;

    stream.play()?;

    Ok(AudioStream { _stream: stream, sample_rate, channels, buffer_latency_hint })
}

/// Bundle of values returned by `open_default_output` so the two stream
/// builders ([`start_silent`] / [`start_with_engine`]) share device-open
/// logic without dragging in a builder pattern.
struct DeviceOpen {
    device: cpal::Device,
    config: cpal::StreamConfig,
    sample_rate: u32,
    channels: u16,
    sample_format: cpal::SampleFormat,
    buffer_latency_hint: String,
}

fn open_default_output() -> Result<DeviceOpen, AudioError> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(AudioError::NoDefaultDevice)?;
    let supported = device.default_output_config()?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels();
    let sample_format = supported.sample_format();
    let buffer_latency_hint = describe_buffer_latency(supported.buffer_size(), sample_rate);
    let config: cpal::StreamConfig = supported.into();
    Ok(DeviceOpen { device, config, sample_rate, channels, sample_format, buffer_latency_hint })
}

fn log_open(channels: u16, sample_rate: u32, sample_format: cpal::SampleFormat, buffer_latency_hint: &str) {
    tracing::info!(
        "opening default output device: {} channel(s), {} Hz, {:?}, {}",
        channels,
        sample_rate,
        sample_format,
        buffer_latency_hint,
    );
}

/// Formats a device's supported buffer-size range together with the latency
/// each end of the range implies at the open sample rate. Used in the M0
/// status string so the plan's "latency reported" criterion is met without
/// committing to a specific runtime buffer size yet.
fn describe_buffer_latency(supported: &cpal::SupportedBufferSize, sample_rate: u32) -> String {
    let frames_to_ms = |frames: u32| (f64::from(frames) * 1000.0) / f64::from(sample_rate);
    match supported {
        cpal::SupportedBufferSize::Range { min, max } if min == max => {
            format!("buffer {min} frames (~{:.1} ms)", frames_to_ms(*min))
        }
        cpal::SupportedBufferSize::Range { min, max } => {
            format!(
                "buffer {min}–{max} frames (~{:.1}–{:.1} ms)",
                frames_to_ms(*min),
                frames_to_ms(*max),
            )
        }
        cpal::SupportedBufferSize::Unknown => "buffer size unreported by driver".to_string(),
    }
}
