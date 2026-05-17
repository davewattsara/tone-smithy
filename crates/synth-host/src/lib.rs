//! Audio and MIDI I/O for Tone Smithy.
//!
//! Wraps `cpal` (audio output) and `midir` (MIDI input). In M0 only the audio
//! side is wired, writing silence to the default output device so the wiring
//! is verifiable end-to-end before any DSP exists.
//!
//! See `docs/planning/03-architecture/overview.md` for the threading model
//! and `docs/planning/03-architecture/design-patterns.md` for the real-time
//! safety rules that govern audio-callback code.

pub mod audio;
