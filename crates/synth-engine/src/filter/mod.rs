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
    /// Filter 1 feeds filter 2 (series). The default, matching the
    /// single-filter behaviour when filter 2 is left wide open.
    #[default]
    Serial,
    /// Filter 1 and filter 2 process the slot mix independently and
    /// their outputs are averaged (equal-power-ish at 0.5 each).
    Parallel,
}

impl FilterRouting {
    /// Maps to the stable index used on the parameter bus and in presets.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Serial => 0,
            Self::Parallel => 1,
        }
    }

    /// Inverse of [`index`](Self::index). Any out-of-range value maps to
    /// the default (`Serial`).
    #[must_use]
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => Self::Parallel,
            _ => Self::Serial,
        }
    }
}
