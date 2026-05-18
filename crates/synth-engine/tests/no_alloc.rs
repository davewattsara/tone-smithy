//! Real-time safety: the audio path must never allocate.
//!
//! `Engine::process_stereo` and `Engine::handle` run on the audio thread
//! per `docs/planning/03-architecture/design-patterns.md` §2.1; an
//! allocation inside either is a build failure. This test installs the
//! `assert_no_alloc` global allocator and renders 10 seconds of audio
//! through a representative event mix inside `assert_no_alloc`'s
//! forbidden-allocation scope.
//!
//! Why not also wrap snapshot publishing? `Engine::snapshot()` returns
//! a `ParamSnapshot` by value with no allocation; the per-block
//! `Arc::new` that the audio host wraps around it is intentional and
//! lives outside the engine (see design-patterns.md §1.4 and §2.5 — the
//! recycled-pool optimisation lands in a later milestone).

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use synth_engine::{Engine, EngineEvent, ParamId, Waveform};

#[global_allocator]
static A: AllocDisabler = AllocDisabler;

/// 48 kHz keeps the numbers easy to reason about and matches the
/// vast majority of consumer audio devices.
const SAMPLE_RATE_HZ: f32 = 48_000.0;

/// 256-frame block: a typical low-latency size. Anything reasonable
/// works; the test just needs the engine to run many blocks.
const BLOCK_FRAMES: usize = 256;

/// Render this many seconds of audio. Long enough that any per-block
/// alloc (snapshot publishing, intermediate buffers, etc.) shows up.
const DURATION_SECS: f32 = 10.0;

#[test]
fn process_stereo_and_handle_do_not_allocate() {
    let mut engine = Engine::new(SAMPLE_RATE_HZ);

    // Allocate the output buffer *outside* the no-alloc scope.
    let mut buffer = vec![0.0_f32; BLOCK_FRAMES * 2];

    let blocks_per_second = SAMPLE_RATE_HZ as usize / BLOCK_FRAMES;
    let total_blocks = (DURATION_SECS as usize) * blocks_per_second;

    assert_no_alloc(|| {
        // Mix of events the audio path actually sees: note on/off,
        // continuous parameter change, discrete waveform change.
        engine.handle(EngineEvent::SetOscillatorWaveform {
            waveform: Waveform::Saw,
        });
        engine.handle(EngineEvent::NoteOn {
            note_midi: 60,
            velocity: 100,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::PitchOffsetSemis,
            value: 0.0,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::AmpReleaseSecs,
            value: 0.5,
        });

        for block_index in 0..total_blocks {
            // Periodically poke an event so the handle() path is
            // exercised inside the no-alloc scope too.
            if block_index.is_multiple_of(64) {
                engine.handle(EngineEvent::ParameterChange {
                    id: ParamId::PitchOffsetSemis,
                    #[allow(clippy::cast_precision_loss)]
                    value: ((block_index % 24) as f32) - 12.0,
                });
            }
            // Re-trigger every second to drive the envelope state
            // machine through all phases.
            if block_index.is_multiple_of(blocks_per_second) && block_index > 0 {
                engine.handle(EngineEvent::NoteOff { note_midi: 60 });
                engine.handle(EngineEvent::NoteOn {
                    note_midi: 67,
                    velocity: 100,
                });
            }
            engine.process_stereo(&mut buffer, BLOCK_FRAMES);
        }

        engine.handle(EngineEvent::NoteOff { note_midi: 67 });
    });

    // Sanity: ensure the loop actually produced audio (otherwise we'd
    // pass the no-alloc check by doing nothing).
    let peak = buffer.iter().fold(0.0_f32, |acc, s| acc.max(s.abs()));
    assert!(peak > 0.0, "expected audible output, peak was {peak}");
}
