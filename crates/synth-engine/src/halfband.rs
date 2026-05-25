//! 31-tap Hann-windowed half-band FIR decimation filter.
//!
//! Decimates a 2× oversampled stream to the base sample rate, suppressing
//! aliasing components above the base Nyquist. Used by [`crate::fm::FmBank`]
//! so that FM operators run at 2× rate while the slot output is delivered at
//! the normal audio rate.
//!
//! The half-band property means every other coefficient is exactly zero
//! (except the centre tap). The Hann window tapers the outermost pair of
//! taps to zero as well, leaving 15 non-zero taps: 7 symmetric pairs plus
//! the centre. Evaluation uses the symmetry to reach 8 multiplies + 13 adds.
//! Stopband attenuation is approximately −44 dB, sufficient to keep FM
//! aliasing artefacts inaudible in a mix.
//!
//! All state is stack-allocated; no heap, no locks, no syscalls.

// 31-tap half-band FIR coefficients, indexed h[0..=30].
// Non-zero at even indices {2,4,...,14,16,...,28} and centre h[15].
// h[0] = h[30] = 0 (Hann window tapers to 0 at the endpoints).
// Derived from the windowed-sinc formula: h_ideal[k] = sin(πk/2)/(πk)
// multiplied by the Hann window w[n] = 0.5·(1 − cos(2πn/30)) for n=0..30.
const H: [f32; 31] = [
    0.0,       // h[0]  — windowed to 0
    0.0,       // h[1]  — half-band zero
    0.001058,  // h[2]
    0.0,       // h[3]  — half-band zero
    -0.004788, // h[4]
    0.0,       // h[5]  — half-band zero
    0.012219,  // h[6]
    0.0,       // h[7]  — half-band zero
    -0.025129, // h[8]
    0.0,       // h[9]  — half-band zero
    0.047748,  // h[10]
    0.0,       // h[11] — half-band zero
    -0.095969, // h[12]
    0.0,       // h[13] — half-band zero
    0.314839,  // h[14]
    0.500000,  // h[15] — centre tap
    0.314839,  // h[16]
    0.0,       // h[17] — half-band zero
    -0.095969, // h[18]
    0.0,       // h[19] — half-band zero
    0.047748,  // h[20]
    0.0,       // h[21] — half-band zero
    -0.025129, // h[22]
    0.0,       // h[23] — half-band zero
    0.012219,  // h[24]
    0.0,       // h[25] — half-band zero
    -0.004788, // h[26]
    0.0,       // h[27] — half-band zero
    0.001058,  // h[28]
    0.0,       // h[29] — half-band zero
    0.0,       // h[30] — windowed to 0
];

/// Ring-buffer size — next power of two above 31 taps.
const BUF: usize = 32;
const MASK: usize = BUF - 1;

/// Symmetric 31-tap half-band FIR decimation filter.
///
/// Push two oversampled samples with [`push`][HalfBand::push], then call
/// [`compute`][HalfBand::compute] to get the decimated output.
pub struct HalfBand {
    buf: [f32; BUF],
    /// Index where the *next* push will write.
    head: usize,
}

impl HalfBand {
    /// Creates a zeroed filter state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buf: [0.0; BUF],
            head: 0,
        }
    }

    /// Inserts one sample into the circular delay line.
    pub fn push(&mut self, x: f32) {
        self.buf[self.head] = x;
        self.head = (self.head + 1) & MASK;
    }

    /// Computes the FIR output from the current delay-line state.
    ///
    /// Call once after two [`push`][HalfBand::push] calls (one oversampled
    /// pair) to obtain the decimated base-rate sample. The initial 15 output
    /// samples contain the filter's settling transient (zeros fed from the
    /// pre-filled-with-zero delay line) and are inaudible in practice.
    #[must_use]
    pub fn compute(&self) -> f32 {
        // `b(k)` fetches the sample `k` places before the current head —
        // i.e. the k-th most recent value pushed. The most recent push
        // is at b(1); b(16) is the centre-tap sample.
        let b = |k: usize| self.buf[self.head.wrapping_sub(k) & MASK];

        // Exploit filter symmetry H[k] = H[30-k] to pair symmetric taps,
        // reducing 15 multiplies to 8. Non-zero positions: even k except
        // k=0,30 (windowed to 0) plus centre k=15.
        H[15] * b(16)
            + H[14] * (b(15) + b(17))
            + H[12] * (b(13) + b(19))
            + H[10] * (b(11) + b(21))
            + H[8] * (b(9) + b(23))
            + H[6] * (b(7) + b(25))
            + H[4] * (b(5) + b(27))
            + H[2] * (b(3) + b(29))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_converges_to_unity() {
        let mut f = HalfBand::new();
        // Feed 1.0 pairs until the delay line is fully filled.
        for _ in 0..32 {
            f.push(1.0);
            f.push(1.0);
            let _ = f.compute();
        }
        // After settling, DC input should give DC output ≈ 1.0.
        f.push(1.0);
        f.push(1.0);
        let out = f.compute();
        assert!((out - 1.0).abs() < 0.001, "DC output should be ~1.0, got {out}");
    }

    #[test]
    fn nyquist_alternating_is_rejected() {
        let mut f = HalfBand::new();
        // The oversampled Nyquist (alternating ±1 at 2× rate) falls in the
        // stopband and should be strongly attenuated. After settling, the
        // magnitude should be well below −20 dB (< 0.1).
        let mut sign = 1.0_f32;
        for _ in 0..64 {
            f.push(sign);
            sign = -sign;
            f.push(sign);
            sign = -sign;
            let _ = f.compute();
        }
        f.push(sign);
        sign = -sign;
        f.push(sign);
        let out = f.compute().abs();
        assert!(out < 0.1, "Nyquist should be attenuated, got |out|={out}");
    }

    #[test]
    fn impulse_response_matches_coefficients() {
        let mut f = HalfBand::new();
        // Push a single impulse as the first sample of pair 0, then zeros.
        // After the push(1.0)+push(0.0), head=2 and the impulse is at b(2).
        // Each additional pair advances head by 2, so after 7 more pairs
        // head=16 and b(16)=buf[0]=1.0, which is the centre-tap position.
        f.push(1.0);
        f.push(0.0);
        for _ in 0..7 {
            f.push(0.0);
            f.push(0.0);
        }
        // The output should equal the centre tap H[15] = 0.5.
        let out = f.compute();
        assert!(
            (out - H[15]).abs() < 1e-5,
            "impulse at centre tap: expected {}, got {out}",
            H[15]
        );
    }
}
