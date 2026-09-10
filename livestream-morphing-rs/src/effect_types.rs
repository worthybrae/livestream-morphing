use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parameter definition — describes a single tunable knob on an effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub id: &'static str,
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub step: f32,
}

impl ParamDef {
    pub const fn new(
        id: &'static str,
        name: &'static str,
        min: f32,
        max: f32,
        default: f32,
        step: f32,
    ) -> Self {
        Self { id, name, min, max, default, step }
    }
}

/// Runtime parameter values — maps param id → current value.
pub type ParamValues = HashMap<String, f32>;

/// Context passed to every effect on each frame.
pub struct FrameCtx {
    pub delta_seconds: f32,
    pub frame_number: u32,
    pub width: u32,
    pub height: u32,
}

/// The core effect trait. Each visual effect implements this.
pub trait Effect: Send {
    /// Unique identifier (e.g. "quantize").
    fn id(&self) -> &'static str;
    /// Human-readable name (e.g. "Color Quantize").
    fn name(&self) -> &'static str;
    /// Parameter definitions with ranges and defaults.
    fn params(&self) -> Vec<ParamDef>;
    /// Allocate scratch buffers for the given frame dimensions.
    fn init(&mut self, width: u32, height: u32);
    /// Discard temporal history after frames were processed by an independent instance.
    fn clear_history(&mut self) {}
    /// Apply the effect to a frame in-place.
    fn apply(&mut self, frame: &mut crate::effects::RawFrame, params: &ParamValues, ctx: &FrameCtx);
}

