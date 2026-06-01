//! 3-tap chorus with two-LFO modulation.
//!
//! Three delay-line taps per channel. LFO1 drives taps 0 and 2 in phase;
//! LFO2 (90° ahead) drives tap 1. Left and right run LFO1 in opposite
//! phase to create stereo width without comb artefacts on mono signals.

use std::f32::consts::TAU;

/// Maximum chorus delay per tap, in samples at 48 kHz.
/// 40 ms headroom: 40 ms × 48 = 1920, round up.
const MAX_DELAY_SAMPLES: usize = 2048;

/// Number of chorus taps per channel.
const TAPS: usize = 3;

/// Centre delay for each tap (in ms), spread so taps don't alias.
const TAP_CENTRES_MS: [f32; TAPS] = [12.0, 16.0, 21.0];

/// 3-tap stereo chorus effect.
#[derive(Debug, Clone)]
pub struct Chorus {
    sample_rate_hz: f32,

    /// Circular delay buffer, left channel.
    buf_l: Box<[f32; MAX_DELAY_SAMPLES]>,
    /// Circular delay buffer, right channel.
    buf_r: Box<[f32; MAX_DELAY_SAMPLES]>,
    write_pos: usize,

    lfo1_phase: f32,
    lfo2_phase: f32,

    /// LFO rate in Hz.
    rate_hz: f32,
    /// Modulation depth in samples (half-range).
    depth_samples: f32,
    /// Dry/wet mix (0 = dry, 1 = wet).
    mix: f32,
    /// Stereo spread (0 = mono, 1 = full spread).
    spread: f32,
}

impl Chorus {
    /// Construct with default parameters.
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            buf_l: Box::new([0.0; MAX_DELAY_SAMPLES]),
            buf_r: Box::new([0.0; MAX_DELAY_SAMPLES]),
            write_pos: 0,
            lfo1_phase: 0.0,
            lfo2_phase: 0.25, // 90° ahead of LFO1
            rate_hz: 0.5,
            depth_samples: 0.0, // updated via set_params
            mix: 0.5,
            spread: 0.5,
        }
    }

    /// Update chorus parameters.
    ///
    /// `rate_hz` 0.1–8; `depth_ms` 0–15; `mix` 0–1; `spread` 0–1.
    pub fn set_params(&mut self, rate_hz: f32, depth_ms: f32, mix: f32, spread: f32) {
        self.rate_hz = rate_hz.clamp(0.1, 8.0);
        self.depth_samples = (depth_ms.clamp(0.0, 15.0) * self.sample_rate_hz / 1_000.0) * 0.5;
        self.mix = mix.clamp(0.0, 1.0);
        self.spread = spread.clamp(0.0, 1.0);
    }

    /// Process one stereo sample.
    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let lfo_inc = self.rate_hz / self.sample_rate_hz;

        // Write current input into delay buffers
        self.buf_l[self.write_pos] = left;
        self.buf_r[self.write_pos] = right;

        // Accumulate tap outputs
        let mut wet_l = 0.0_f32;
        let mut wet_r = 0.0_f32;

        for (i, centre_ms) in TAP_CENTRES_MS.iter().enumerate() {
            let centre_samples = centre_ms * self.sample_rate_hz / 1_000.0;

            // Tap 1 uses LFO2 (90° ahead); taps 0 and 2 use LFO1
            let lfo_sin = if i == 1 {
                (TAU * self.lfo2_phase).sin()
            } else {
                (TAU * self.lfo1_phase).sin()
            };

            // Left and right use opposite LFO phase for stereo spread.
            // Spread controls the phase inversion amount.
            let lfo_l = lfo_sin;
            let lfo_r = lfo_sin * (1.0 - 2.0 * self.spread); // invert when spread=1

            let delay_l = (centre_samples + self.depth_samples * lfo_l).max(0.0);
            let delay_r = (centre_samples + self.depth_samples * lfo_r).max(0.0);

            wet_l += Self::read_interp(&self.buf_l, self.write_pos, delay_l);
            wet_r += Self::read_interp(&self.buf_r, self.write_pos, delay_r);
        }

        // Normalise taps
        wet_l /= TAPS as f32;
        wet_r /= TAPS as f32;

        // Advance LFOs
        self.lfo1_phase = (self.lfo1_phase + lfo_inc).fract();
        self.lfo2_phase = (self.lfo2_phase + lfo_inc).fract();

        // Advance write position
        self.write_pos = (self.write_pos + 1) % MAX_DELAY_SAMPLES;

        let out_l = left + self.mix * (wet_l - left);
        let out_r = right + self.mix * (wet_r - right);
        (out_l, out_r)
    }

    /// Linear interpolation read from circular buffer.
    #[inline]
    fn read_interp(buf: &[f32; MAX_DELAY_SAMPLES], write_pos: usize, delay: f32) -> f32 {
        let delay_int = delay as usize;
        let frac = delay - delay_int as f32;

        let pos0 = (write_pos + MAX_DELAY_SAMPLES - delay_int) % MAX_DELAY_SAMPLES;
        let pos1 = (write_pos + MAX_DELAY_SAMPLES - delay_int - 1) % MAX_DELAY_SAMPLES;

        buf[pos0] * (1.0 - frac) + buf[pos1] * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chorus_zero_mix_is_transparent() {
        let mut c = Chorus::new(48_000.0);
        c.set_params(0.5, 3.0, 0.0, 0.5); // mix = 0 → dry pass-through
        let (l, r) = c.process(0.5, -0.5);
        assert!((l - 0.5).abs() < 1e-5, "expected 0.5, got {l}");
        assert!((r + 0.5).abs() < 1e-5, "expected -0.5, got {r}");
    }

    #[test]
    fn chorus_does_not_blow_up() {
        let mut c = Chorus::new(48_000.0);
        c.set_params(2.0, 10.0, 0.5, 0.8);
        for _ in 0..48_000 {
            let (l, r) = c.process(0.5, 0.5);
            assert!(l.is_finite(), "chorus produced non-finite L");
            assert!(r.is_finite(), "chorus produced non-finite R");
        }
    }
}
