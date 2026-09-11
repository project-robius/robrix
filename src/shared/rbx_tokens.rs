//! A local slice of robrix2's `RBX_*` design-token layer.
//!
//! robrix2 styles every screen through a semantic token layer
//! (`src/shared/design_tokens.rs` there, governed by `docs/ui-visual-spec-zh.md`).
//! Upstream Robrix has no such layer, so the agent-chat card and badges would
//! otherwise fall back to upstream's older look and hardcoded colours. This
//! module defines just the tokens the surfaces ported from robrix2 use (the
//! agent-chat card and badges, and the account menu), with the values copied
//! verbatim from robrix2, under the same `RBX_*` names — so if the full design
//! system is ever ported, these definitions are simply deleted and every
//! reference resolves to the real thing unchanged.
//!
//! Typography keeps robrix2's sizes, weights and line spacing but sits on the
//! theme fonts, because robrix2's `RBX_FONT_*` styles load custom font files
//! (`system_latin.ttf`, `system_cjk.ttc`) that upstream does not ship.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // ── Surfaces ─────────────────────────────────────────────────────────
    mod.widgets.RBX_BG_SURFACE   = #xFFFFFF
    mod.widgets.RBX_BG_HOVER     = #xEFF4FB
    mod.widgets.RBX_BG_PRESSED   = #xE7ECF3
    mod.widgets.RBX_BG_DISABLED  = #xF0F2F6

    // ── Foreground ───────────────────────────────────────────────────────
    mod.widgets.RBX_FG_PRIMARY   = #x16233B
    mod.widgets.RBX_FG_SECONDARY = #x5A6B86
    mod.widgets.RBX_FG_DISABLED  = #xAEB7C6

    // ── Accent (teal) ────────────────────────────────────────────────────
    mod.widgets.RBX_ACCENT         = #x0D7988
    mod.widgets.RBX_ACCENT_HOVER   = #x0A6675
    mod.widgets.RBX_ACCENT_PRESSED = #x085460
    mod.widgets.RBX_ACCENT_SOFT    = #xE4F5F7

    // ── Strokes & scrims ─────────────────────────────────────────────────
    mod.widgets.RBX_STROKE_STRONG = #xD5DEEA
    mod.widgets.RBX_DIVIDER       = #x00000010
    mod.widgets.RBX_SCRIM         = #x16233B80

    // ── Semantic status pairs (fg on bg) ─────────────────────────────────
    mod.widgets.RBX_SUCCESS_FG = #x197F45
    mod.widgets.RBX_SUCCESS_BG = #xE8F6EE
    mod.widgets.RBX_WARNING_FG = #x9C6009
    mod.widgets.RBX_WARNING_BG = #xFBF1DD
    mod.widgets.RBX_DANGER_FG  = #xB93429
    mod.widgets.RBX_DANGER_BG  = #xFBE9E7
    mod.widgets.RBX_NEUTRAL_FG = #x5A6B86
    mod.widgets.RBX_NEUTRAL_BG = #xEEF1F6

    // ── Shape & size ─────────────────────────────────────────────────────
    mod.widgets.RBX_RADIUS_SM    = 6.0
    mod.widgets.RBX_RADIUS_MD    = 6.0
    mod.widgets.RBX_RADIUS_PILL  = 100.0
    mod.widgets.RBX_CONTROL_H_MD = 36.0
    mod.widgets.RBX_ICON_LG      = 24.0

    // ── Typography ───────────────────────────────────────────────────────
    mod.widgets.RBX_TEXT_CARD_TITLE  = theme.font_bold    { font_size: 12.0, line_spacing: 1.3 }
    mod.widgets.RBX_TEXT_BODY        = theme.font_regular { font_size: 11.0, line_spacing: 1.35 }
    mod.widgets.RBX_TEXT_BODY_STRONG = theme.font_bold    { font_size: 11.0, line_spacing: 1.35 }
    mod.widgets.RBX_TEXT_META        = theme.font_regular { font_size: 9.5,  line_spacing: 1.3 }
    mod.widgets.RBX_TEXT_BADGE       = theme.font_bold    { font_size: 9.0 }
}
