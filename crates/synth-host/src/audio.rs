//! Audio output via `cpal`.
//!
//! M0 scope: open the default output device and write silence. Real DSP
//! integration lands in later milestones (engine wiring at M1, MIDI events
//! flowing into the audio thread at M3).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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
/// # Errors
///
/// Returns [`AudioError`] if no default output device exists, the device
/// cannot report a default config, the stream cannot be built, the stream
/// cannot be started, or the device uses a sample format this build does
/// not yet handle.
pub fn start_silent() -> Result<AudioStream, AudioError> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(AudioError::NoDefaultDevice)?;
    let supported = device.default_output_config()?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels();
    let sample_format = supported.sample_format();
    let buffer_latency_hint = describe_buffer_latency(supported.buffer_size(), sample_rate);
    let config: cpal::StreamConfig = supported.into();

    tracing::info!(
        "opening default output device: {} channel(s), {} Hz, {:?}, {}",
        channels,
        sample_rate,
        sample_format,
        buffer_latency_hint,
    );

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

    Ok(AudioStream {
        _stream: stream,
        sample_rate,
        channels,
        buffer_latency_hint,
    })
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
