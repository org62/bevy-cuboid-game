//! Shared UI, camera and diagnostics used by every level, split by concern:
//!
//! - [`camera`]: the rigid follow camera, orbit input and mouse settings;
//! - [`ui`]: hint boxes, tutorial/controls/settings dialogs, victory/defeat
//!   overlays, the objective banner, input-device detection and cursor grab;
//! - [`diag`]: the F3 frame-pacing overlay.
//!
//! Everything is re-exported flat, so call sites keep using
//! `shared_ui::foo` regardless of which file a symbol lives in.

mod camera;
mod diag;
mod ui;

pub use camera::*;
pub use diag::*;
pub use ui::*;
