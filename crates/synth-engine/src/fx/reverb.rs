//! FDN-8 reverb — 8-channel feedback delay network.
//!
//! Based on the Jot (1992) / Smith FDN architecture:
//! - 8 delay lines with prime-number lengths (scaled by `size`)
//! - Hadamard feedback matrix (unitary, lossless)
//! - Per-line one-pole absorption filters (controlled by `damping`)
//! - Pre-delay line
//!
//! Denormal guard: a tiny DC offset (1e-20) is injected into each
//! delay-line input each sample to keep values from falling into the
//! sub-normal range during long tails.

/// Prime-number delay line lengths in samples at 48 kHz, chosen to give
/// a dense, inharmonic echo density. Scaled by `size` parameter.
const BASE_DELAYS: [usize; 8] = [1009, 1201, 1307, 1511, 1601, 1801, 2003, 2207];

/// Maximum size factor — delay lines scale from BASE up to 1.0×.
const MAX_SIZE: f32 = 1.0;
/// Minimum size factor.
const MIN_SIZE: f32 = 0.1;

/// Maximum pre-delay, in seconds.
const MAX_PREDELAY_SECS: f32 = 0.050;

/// Hadamard matrix H8 × (1/sqrt(8)), computed once at compile time.
/// Entries are ±1/sqrt(8). We store ±1 and scale the sum by INV_SQRT8.
const H8: [[i8; 8]; 8] = [
    [1, 1, 1, 1, 1, 1, 1, 1],
    [1, -1, 1, -1, 1, -1, 1, -1],
    [1, 1, -1, -1, 1, 1, -1, -1],
    [1, -1, -1, 1, 1, -1, -1, 1],
    [1, 1, 1, 1, -1, -1, -1, -1],
    [1, -1, 1, -1, -1, 1, -1, 1],
    [1, 1, -1, -1, -1, -1, 1, 1],
    [1, -1, -1, 1, -1, 1, 1, -1],
];
const INV_SQRT8: f32 = 0.353_553_4; // 1/sqrt(8)

const DENORMAL_GUARD: f32 = 1.0e-20;

/// FDN-8 reverb.
pub struct Fdn8Reverb {
    sample_rate_hz: f32,

    /// Circular delay line storage for all 8 channels.
    buffers: [Vec<f32>; 8],
    /// Length of each delay line in samples (set by size parameter).
    lengths: [usize; 8],
    /// Write position (shared — all lines advance together).
    write_pos: usize,

    /// Pre-delay circular buffer (stereo).
    predelay_buf: Vec<f32>,
    predelay_len: usize,
    predelay_write: usize,

    /// Per-line one-pole absorption filter state.
    abs_state: [f32; 8],
    /// One-pole absorption coefficient (computed from damping + decay).
    abs_coeff: f32,
    /// Feedback gain per line (computed from decay time).
    fb_gain: f32,

    decay_secs: f32,
    size: f32,
    mix: f32,
}

impl Fdn8Reverb {
    /// Construct with default parameters. Allocates delay buffers once.
    pub fn new(sample_rate_hz: f32) -> Self {
        let max_len = (BASE_DELAYS[7] as f32 * 1.05) as usize + 1; // headroom
        let predelay_len = (MAX_PREDELAY_SECS * sample_rate_hz) as usize + 1;

        let mut reverb = Self {
            sample_rate_hz,
            buffers: std::array::from_fn(|_| vec![0.0; max_len]),
            lengths: [0; 8],
            write_pos: 0,
            predelay_buf: vec![0.0; predelay_len * 2], // stereo interleaved
            predelay_len,
            predelay_write: 0,
            abs_state: [0.0; 8],
            abs_coeff: 0.0,
            fb_gain: 1.0,
            decay_secs: 2.0,
            size: 0.7,
            mix: 0.25,
        };
        reverb.recompute(0.5, 2.0, 0.7);
        reverb
    }

    /// Update reverb parameters.
    ///
    /// `predelay_ms` 0–50; `decay_secs` 0.1–30; `size` 0.1–1.0;
    /// `damping` 0–1; `mix` 0–1.
    pub fn set_params(&mut self, predelay_ms: f32, decay_secs: f32, size: f32, damping: f32, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
        let predelay_samples = (predelay_ms.clamp(0.0, 50.0) * self.sample_rate_hz / 1_000.0) as usize;
        self.predelay_len = predelay_samples.max(1).min(self.predelay_buf.len() / 2);
        self.recompute(damping, decay_secs, size);
    }

    /// Process one stereo sample through the reverb.
    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Write to pre-delay
        let pd_idx = self.predelay_write * 2;
        self.predelay_buf[pd_idx] = left;
        self.predelay_buf[pd_idx + 1] = right;
        let pd_read =
            (self.predelay_write + self.predelay_buf.len() / 2 - self.predelay_len) % (self.predelay_buf.len() / 2);
        let in_l = self.predelay_buf[pd_read * 2];
        let in_r = self.predelay_buf[pd_read * 2 + 1];
        self.predelay_write = (self.predelay_write + 1) % (self.predelay_buf.len() / 2);

        // Read from all 8 delay lines
        let mut channel_out = [0.0_f32; 8];
        #[allow(clippy::needless_range_loop)]
        for i in 0..8 {
            let read_pos = (self.write_pos + self.buffers[i].len() - self.lengths[i]) % self.buffers[i].len();
            channel_out[i] = self.buffers[i][read_pos];
        }

        // Hadamard mix
        let mut mixed = [0.0_f32; 8];
        for i in 0..8 {
            let mut sum = 0.0_f32;
            for j in 0..8 {
                sum += f32::from(H8[i][j]) * channel_out[j];
            }
            mixed[i] = sum * INV_SQRT8;
        }

        // Write back through absorption filters + feedback gain
        // Channels 0-3 get L input, 4-7 get R input
        let inputs = [in_l, in_l, in_l, in_l, in_r, in_r, in_r, in_r];
        for i in 0..8 {
            let fed = mixed[i] * self.fb_gain;
            // One-pole LP absorption
            self.abs_state[i] = self.abs_state[i] * self.abs_coeff + fed * (1.0 - self.abs_coeff);
            let write_val = inputs[i] + self.abs_state[i] + DENORMAL_GUARD;
            let buf_len = self.buffers[i].len();
            self.buffers[i][self.write_pos % buf_len] = write_val;
        }

        self.write_pos = self.write_pos.wrapping_add(1);

        // Output: sum left bank (0-3) and right bank (4-7)
        let wet_l = (channel_out[0] + channel_out[1] + channel_out[2] + channel_out[3]) * 0.25;
        let wet_r = (channel_out[4] + channel_out[5] + channel_out[6] + channel_out[7]) * 0.25;

        let out_l = left + self.mix * (wet_l - left);
        let out_r = right + self.mix * (wet_r - right);
        (out_l, out_r)
    }

    fn recompute(&mut self, damping: f32, decay_secs: f32, size: f32) {
        self.decay_secs = decay_secs.clamp(0.1, 30.0);
        self.size = size.clamp(MIN_SIZE, MAX_SIZE);

        // Scale delay lengths by size
        for (i, &base) in BASE_DELAYS.iter().enumerate() {
            let len = ((base as f32 * self.size) as usize)
                .max(2)
                .min(self.buffers[i].len() - 1);
            self.lengths[i] = len;
        }

        // Feedback gain from Schroeder's decay formula:
        // T60 = -60 dB in decay_secs. For each delay line of length L:
        // g = 10^(-3 * L / (sr * decay_secs))
        // We use the average length for a single global gain.
        let avg_len = BASE_DELAYS.iter().sum::<usize>() as f32 / 8.0 * self.size;
        self.fb_gain = 10.0_f32.powf(-3.0 * avg_len / (self.sample_rate_hz * self.decay_secs));

        // Absorption coefficient: higher damping → lower coefficient → more HF loss
        let damping = damping.clamp(0.0, 1.0);
        // Map: damping=0 → abs_coeff=0 (no absorption), damping=1 → abs_coeff=0.98 (heavy)
        self.abs_coeff = damping * 0.98;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_mix_is_transparent() {
        let mut r = Fdn8Reverb::new(48_000.0);
        r.set_params(0.0, 2.0, 0.7, 0.5, 0.0);
        // Warm up
        for _ in 0..100 {
            r.process(1.0, 1.0);
        }
        let (l, right) = r.process(0.5, -0.5);
        assert!((l - 0.5).abs() < 0.01, "expected ~0.5, got {l}");
        assert!((right + 0.5).abs() < 0.01, "expected ~-0.5, got {right}");
    }

    #[test]
    fn reverb_tail_is_finite() {
        let mut r = Fdn8Reverb::new(48_000.0);
        r.set_params(10.0, 5.0, 0.8, 0.5, 0.5);
        // One second of input then 29 seconds of tail
        for _ in 0..48_000 {
            r.process(0.5, 0.5);
        }
        for i in 0..48_000 * 29 {
            let (l, right) = r.process(0.0, 0.0);
            assert!(l.is_finite(), "reverb L non-finite at sample {i}");
            assert!(right.is_finite(), "reverb R non-finite at sample {i}");
        }
    }

    #[test]
    fn reverb_tail_decays_to_near_zero() {
        let mut r = Fdn8Reverb::new(48_000.0);
        r.set_params(0.0, 2.0, 0.7, 0.5, 1.0); // mix=1 so we can measure the tail
        // Feed a pulse
        for _ in 0..100 {
            r.process(0.5, 0.5);
        }
        // Run for 10× the decay time (20 s), checking that the tail fades
        let mut final_level = f32::MAX;
        for _ in 0..48_000 * 20 {
            let (l, _) = r.process(0.0, 0.0);
            final_level = l.abs();
        }
        assert!(final_level < 0.01, "reverb tail still loud after 20 s: {final_level}");
    }
}
