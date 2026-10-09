//! Dense multiband compressor editor and compact global header.

// Rust guideline compliant 2026-02-21
mod header;
mod types;

pub(crate) use header::render_header;
pub use types::*;
