//! Filter section for the subtractive voice.
//!
//! M2 ships the single TPT state-variable filter (12 dB/oct, four
//! modes — LP / HP / BP / Notch). M17 (v1.1) adds a second filter and
//! the serial/parallel [`FilterRouting`] between the two.

pub use svf::{FilterMode, StateVariableFilter};

mod svf;

/// How the two per-voice filters are connected in the signal path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterRouting {
    /// Filter 1 feeds filter 2 (series).
    Serial,
    /// Filter 1 and filter 2 process the slot mix independently and
    /// their outputs are averaged (equal-power-ish at 0.5 each).
    Parallel,
    /// Filter 2 is bypassed entirely — the voice output is filter 1 only,
    /// and filter 2 is not evaluated (no extra SVFs per voice). The
    /// **default**, so presets that omit the routing key — every v1.0
    /// preset, which predates filter 2 — sound exactly as they did before
    /// filter 2 existed. Appended at index 2 so saved 0/1 values keep
    /// their meaning.
    #[default]
    Off,
}

impl FilterRouting {
    /// Maps to the stable index used on the parameter bus and in presets.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Serial => 0,
            Self::Parallel => 1,
            Self::Off => 2,
        }
    }

    /// Inverse of [`index`](Self::index). Any out-of-range value maps to
    /// the default (`Off`).
    #[must_use]
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Serial,
            1 => Self::Parallel,
            _ => Self::Off,
        }
    }
}

/// Roll-off slope of a [`StateVariableFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterSlope {
    /// 2-pole, 12 dB/oct — the original single-stage SVF response.
    #[default]
    TwelveDbOct,
    /// 4-pole, 24 dB/oct, produced by cascading a second SVF stage.
    TwentyFourDbOct,
}

impl FilterSlope {
    /// Maps to the stable index used on the parameter bus and in presets.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::TwelveDbOct => 0,
            Self::TwentyFourDbOct => 1,
        }
    }

    /// Inverse of [`index`](Self::index). Any out-of-range value maps to
    /// the default (`TwelveDbOct`).
    #[must_use]
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => Self::TwentyFourDbOct,
            _ => Self::TwelveDbOct,
        }
    }
}
