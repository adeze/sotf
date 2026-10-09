//! Plugin chain controller.
//!
//! Encapsulates plugin chain management, parameter editing, EQ operations,
//! and preset management. Every mutation returns a `PluginUpdateEffect` so the
//! UI knows whether to do a structural rebuild or a zero-dropout parameter update.

pub use super::plugin_param_map::param_index_to_engine_param;

mod adjust;
mod crossover;
#[cfg(feature = "dev-api")]
pub mod dev_api;
mod linear_phase;
mod misc;
mod parameter_metadata;
mod plugin_controller;
mod set;
#[cfg(test)]
mod tests;
mod types;

pub use misc::*;
#[doc(inline)]
pub use parameter_metadata::{
    desktop_eq_param_value, desktop_param_enabled, desktop_param_indices, desktop_param_spec,
    desktop_param_value,
};
pub use plugin_controller::*;
pub use set::set_plugin_param_value;
pub use types::*;
