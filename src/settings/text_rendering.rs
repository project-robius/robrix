//! The setting that draws text in a simpler, faster way for weak GPUs.
//!
//! It's saved per device rather than per account, so it applies to the login screen too.

use makepad_widgets::{text::fonts::{GlyphRendering, get_glyph_rendering, set_glyph_rendering}, *};

use crate::persistence;

pub fn is_simpler_text_rendering_enabled(cx: &Cx) -> bool {
    get_glyph_rendering(cx) == GlyphRendering::Hybrid
}

/// Simpler text draws glyphs from SDFs that a worker builds,
/// which is much cheaper for a weak GPU than drawing every glyph from its outline.
pub fn apply_simpler_text_rendering(cx: &mut Cx, enabled: bool) {
    set_glyph_rendering(cx, if enabled { GlyphRendering::Hybrid } else { GlyphRendering::PlatformDefault });
}

/// Applies the given setting right away and saves it for future launches.
pub fn set_simpler_text_rendering(cx: &mut Cx, enabled: bool) {
    apply_simpler_text_rendering(cx, enabled);
    if let Err(e) = persistence::save_simpler_text_rendering(enabled) {
        error!("Failed to save the text rendering setting. Error: {e}");
    }
}
