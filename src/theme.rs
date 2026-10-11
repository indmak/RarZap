//! Design tokens (UI 设计规划 §3): spacing / radius / typography scale.
//! Colors come from `gpui-component` Theme at render time — no hardcoded rgb() in app code.

use gpui_kit::App;

/// 4px baseline spacing scale (logical px).
pub mod space {
    pub const S1: f32 = 4.0;
    pub const S2: f32 = 8.0;
    pub const S3: f32 = 12.0;
    pub const S4: f32 = 16.0;
    pub const S5: f32 = 20.0;
    pub const S6: f32 = 24.0;
    pub const S8: f32 = 32.0;
    pub const S12: f32 = 48.0;
}

/// Corner radii (logical px).
pub mod radius {
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 10.0;
    pub const LG: f32 = 14.0;
}

/// Window specs (§4.1).
pub mod window {
    pub const DEFAULT_W: f32 = 720.0;
    pub const DEFAULT_H: f32 = 520.0;
    pub const MIN_W: f32 = 560.0;
    pub const MIN_H: f32 = 420.0;
}

/// Hook for future custom theme overrides. Currently gpui-component Theme is authoritative.
pub fn init_theme(_cx: &mut App) {}
