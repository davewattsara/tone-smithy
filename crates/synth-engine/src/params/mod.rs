//! The engine's parameter tree and outward-facing snapshot.
//!
//! See [`tree`] for the full design rationale.

mod ids;
mod snapshot;
pub(super) mod tree;

pub use ids::ParamId;
pub use snapshot::{ParamSnapshot, SampleParams};
pub use tree::ParameterTree;
