//! Tone Smithy egui front end.
//!
//! In later milestones this crate hosts the custom widget library (knob,
//! slider, mod matrix row, etc.) and the panel layout for each synth
//! section. M0 just exposes a single [`app::ToneSmithyApp`] that opens an
//! empty centred-heading window.
//!
//! See `docs/planning/03-architecture/ui-layer.md` for the architecture and
//! `docs/planning/05-design/ui-design.md` for the visual direction.

pub mod app;
