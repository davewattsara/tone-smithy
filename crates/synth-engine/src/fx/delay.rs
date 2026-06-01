//! Stereo delay with ping-pong mode and a low-cut filter in the feedback path.

use super::biquad::{Biquad, one_pole_highpass_coeffs};

/// Maximum delay time supported, in seconds. Buffer allocated once at init.
const MAX_DELAY_SECS: f32 = 2.0;

/// Stereo delay effect.
///
/// In normal mode both channels delay independently with shared time and
/// feedback. In ping-pong mode the L delay output feeds back into the R
/// channel input and vice versa.
#[derive(Debug, Clone)]
pub struct StereoDelay {
    sample_rate_hz: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    buf_len: usize,
    write_pos: usize,

    /// Delay time in samples.
    delay_samples: usize,
    /// Feedback amount 0..=0.95.
    feedback: f32,
    /// Dry/wet mix 0..=1.
    mix: f32,
    /// Ping-pong routing.
    ping_pong: bool,

    /// Low-cut filter in the feedback path (one per channel).
    fb_filter_l: Biquad,
    fb_filter_r: Biquad,
    fb_cutoff_hz: f32,
}

impl StereoDelay {
    /// Construct with default parameters. Allocates delay buffers.
    pub fn new(sample_rate_hz: f32) -> Self {
        let buf_len = (MAX_DELAY_SECS * sample_rate_hz) as usize + 1;
        let mut delay = Self {
            sample_rate_hz,
            buf_l: vec![0.0; buf_len],
            buf_r: vec![0.0; buf_len],
            buf_len,
            write_pos: 0,
            delay_samples: (0.375 * sample_rate_hz) as usize, // 375 ms default
            feedback: 0.35,
            mix: 0.30,
            ping_pong: false,
            fb_filter_l: Biquad::default(),
            fb_filter_r: Biquad::default(),
            fb_cutoff_hz: 200.0,
        };
        delay.recompute_filter();
        delay
    }

    /// Update all delay parameters.
    ///
    /// `time_secs` 0.001–2.0; `feedback` 0–0.95; `mix` 0–1;
    /// `lowcut_hz` 20–2000; `ping_pong` toggles routing.
    pub fn set_params(&mut self, time_secs: f32, feedback: f32, mix: f32, lowcut_hz: f32, ping_pong: bool) {
        self.delay_samples =
            ((time_secs.clamp(0.001, MAX_DELAY_SECS) * self.sample_rate_hz) as usize).min(self.buf_len - 1);
        self.feedback = feedback.clamp(0.0, 0.95);
        self.mix = mix.clamp(0.0, 1.0);
        self.ping_pong = ping_pong;
        if (lowcut_hz - self.fb_cutoff_hz).abs() > 0.5 {
            self.fb_cutoff_hz = lowcut_hz.clamp(20.0, 2_000.0);
            self.recompute_filter();
        }
    }

    /// Process one stereo sample.
    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let read_pos = (self.write_pos + self.buf_len - self.delay_samples) % self.buf_len;

        let delayed_l = self.buf_l[read_pos];
        let delayed_r = self.buf_r[read_pos];

        // Feedback routing: straight or ping-pong
        let (fb_l, fb_r) = if self.ping_pong {
            (delayed_r, delayed_l) // swap channels
        } else {
            (delayed_l, delayed_r)
        };

        // Apply low-cut to feedback to prevent build-up of low-frequency mud
        let fb_l = self.fb_filter_l.process(fb_l) * self.feedback;
        let fb_r = self.fb_filter_r.process(fb_r) * self.feedback;

        self.buf_l[self.write_pos] = left + fb_l;
        self.buf_r[self.write_pos] = right + fb_r;
        self.write_pos = (self.write_pos + 1) % self.buf_len;

        let out_l = left + self.mix * (delayed_l - left);
        let out_r = right + self.mix * (delayed_r - right);
        (out_l, out_r)
    }

    fn recompute_filter(&mut self) {
        let (b0, b1, b2, a1, a2) = one_pole_highpass_coeffs(self.fb_cutoff_hz, self.sample_rate_hz);
        self.fb_filter_l.set_coeffs(b0, b1, b2, a1, a2);
        self.fb_filter_r.set_coeffs(b0, b1, b2, a1, a2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_mix_is_transparent() {
        let mut d = StereoDelay::new(48_000.0);
        d.set_params(0.375, 0.35, 0.0, 200.0, false);
        let (l, r) = d.process(0.5, -0.5);
        assert!((l - 0.5).abs() < 1e-5, "expected 0.5, got {l}");
        assert!((r + 0.5).abs() < 1e-5, "expected -0.5, got {r}");
    }

    #[test]
    fn delay_echo_appears_after_delay_time() {
        let sr = 48_000.0_f32;
        let mut d = StereoDelay::new(sr);
        d.set_params(0.100, 0.5, 1.0, 200.0, false); // 100 ms delay, full wet
        let delay_samples = (0.100 * sr) as usize;
        let mut buf = vec![0.0_f32; delay_samples + 100];
        // Single impulse at sample 0
        d.process(1.0, 0.0);
        for slot in buf.iter_mut().skip(1) {
            let (l, _) = d.process(0.0, 0.0);
            *slot = l;
        }
        // Peak should appear around delay_samples
        let peak_pos = buf
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        let expected = delay_samples;
        assert!(
            peak_pos.abs_diff(expected) < 5,
            "echo peak at {peak_pos}, expected near {expected}"
        );
    }

    #[test]
    fn no_feedback_runaway() {
        let mut d = StereoDelay::new(48_000.0);
        d.set_params(0.010, 0.95, 0.5, 200.0, false);
        for _ in 0..48_000 * 2 {
            let (l, r) = d.process(0.1, 0.1);
            assert!(l.abs() < 10.0, "feedback runaway L: {l}");
            assert!(r.abs() < 10.0, "feedback runaway R: {r}");
        }
    }
}
