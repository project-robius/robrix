//! Robrix2 design tokens — the semantic color / size / radius / type layer.
//!
//! This is the single source of truth for the "AI workspace" visual language
//! described in `docs/ui-visual-spec-zh.md`. It is **light-first**: the primary
//! content surfaces are light, with a small set of explicit *dark surface* tokens
//! for the desktop navigation rail and the mobile login screen. (A full dark theme
//! is intentionally out of scope for this round — see the spec, §11.)
//!
//! ## Naming
//! All tokens use the `RBX_` prefix and a `GROUP_ROLE[_STATE]` shape so they never
//! collide with the legacy `COLOR_*` / `SPACE_*` tokens in `styles.rs`:
//!
//! - `RBX_BG_*`      surfaces / backgrounds (incl. hover / selected / pressed / disabled)
//! - `RBX_FG_*`      text / foreground
//! - `RBX_ACCENT*`   the teal brand-accent (primary CTA, selection, focus, links)
//! - `RBX_STROKE_*`  borders / dividers
//! - `RBX_<STATE>_FG` / `RBX_<STATE>_BG`  semantic state pairs (success/warning/danger/info/neutral)
//! - `RBX_NAV_*`     dark navigation-rail surfaces
//! - `RBX_LOGIN_*`   dark login surfaces
//! - `RBX_CODE_*`    dark code/SQL output panel (timeline)
//! - `RBX_RADIUS_*`  corner radii
//! - `RBX_CONTROL_H_* / RBX_ROW_H_* / RBX_ICON_* / RBX_AVATAR_*`  sizing
//! - `RBX_SHADOW* / RBX_SCRIM`  elevation / modal scrim
//! - `RBX_FOCUS_*`   keyboard-nav focus ring
//! - `RBX_TEXT_*`    type-scale `TextStyle` presets
//!
//! ## Primary color migration — done
//! The teal `RBX_ACCENT` (#0D7988) is the single primary/CTA/focus color, and
//! `COLOR_ACTIVE_PRIMARY` in `styles.rs` now resolves to it, so the ~40 remaining
//! call sites moved together rather than a screen at a time — every one of them
//! means "primary", "active" or "focus", and staging them would only have grown
//! the stretch where blue and teal sat side by side. New UI should still name
//! `RBX_ACCENT` directly; `COLOR_ACTIVE_PRIMARY` survives as a compatibility
//! alias for the call sites that have not been renamed yet.
//! `RBX_LEGACY_BLUE` records the retired #0F88FE and now has no users.
//!
//! Tokens are registered into the global `mod.widgets.*` namespace (so any other
//! `script_mod!` block can read them via `(RBX_TOKEN)` after `use mod.widgets.*`).
//! A curated subset is also exported as Rust `Vec4` consts for use in
//! `script_apply_eval!` / programmatic styling, mirroring the convention in
//! `styles.rs`.
//!
//! NOTE: inside the `script_mod!` block, only `//` comments are allowed — `///`
//! doc comments are parsed by Rust as `#[doc]` attributes and will not compile.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // =========================================================================
    // 1. SURFACES — light-first backgrounds + interaction states
    // =========================================================================
    // Page background. The calm cool-white the whole app sits on.
    mod.widgets.RBX_BG_CANVAS         = #xF7F9FC
    // Card / sheet / elevated surface.
    mod.widgets.RBX_BG_SURFACE        = #xFFFFFF
    // Subtle inset surface (grouped rows, secondary panels, table zebra).
    mod.widgets.RBX_BG_SURFACE_SUBTLE = #xF4F7FB
    // Sunken surface for light code/preview insets.
    mod.widgets.RBX_BG_SUNKEN         = #xEEF2F8
    // Hover wash over a surface (rows, list items).
    mod.widgets.RBX_BG_HOVER          = #xEFF4FB
    // Selected row / item background (teal-tinted, == RBX_ACCENT_SOFT).
    mod.widgets.RBX_BG_SELECTED       = #xE4F5F7
    // Pressed surface (rows, list items, ghost buttons).
    mod.widgets.RBX_BG_PRESSED        = #xE7ECF3
    // Disabled control surface.
    mod.widgets.RBX_BG_DISABLED       = #xF0F2F6
    // Fully transparent + press/hover overlay washes (used by Agent Registry tiles).
    mod.widgets.RBX_TRANSPARENT       = #x00000000
    mod.widgets.RBX_HIT_HOVER         = #x00000008
    mod.widgets.RBX_HIT_DOWN          = #x00000012

    // =========================================================================
    // 2. FOREGROUND — text & icons on light surfaces
    // =========================================================================
    // Primary text (titles, body). Deep blue-grey, never pure black.
    mod.widgets.RBX_FG_PRIMARY    = #x16233B
    // Secondary text (subtitles, meta, helper).
    mod.widgets.RBX_FG_SECONDARY  = #x5A6B86
    // Tertiary text (timestamps, faint captions).
    mod.widgets.RBX_FG_TERTIARY   = #x687283
    // Text/icon on top of an accent or dark fill.
    mod.widgets.RBX_FG_ON_ACCENT  = #xFFFFFF
    // Disabled text.
    mod.widgets.RBX_FG_DISABLED   = #xAEB7C6

    // =========================================================================
    // 3. ACCENT — the teal brand color (primary CTA, selection, focus)
    // =========================================================================
    // Primary accent (calm teal — the "Sign in securely" / primary button color).
    mod.widgets.RBX_ACCENT         = #x0D7988
    // Accent hover.
    mod.widgets.RBX_ACCENT_HOVER   = #x0A6675
    // Accent pressed.
    mod.widgets.RBX_ACCENT_PRESSED = #x085460
    // Soft accent tint (selected chip bg, highlighted row, focus ring fill).
    mod.widgets.RBX_ACCENT_SOFT    = #xE4F5F7
    // Hyperlink / inline-link color.
    mod.widgets.RBX_LINK           = #x167CB9
    // Hovered link. Same hue, darkened like ACCENT → ACCENT_HOVER, so a link
    // deepens on hover instead of jumping to another hue.
    mod.widgets.RBX_LINK_HOVER     = #x136DA3

    // =========================================================================
    // 4. BRAND — logo colors. Use sparingly (brand entry points, app icon, the
    //    room-identity avatar). Do NOT flood functional UI with purple.
    // =========================================================================
    mod.widgets.RBX_BRAND_PURPLE  = #x572DCC
    mod.widgets.RBX_BRAND_CYAN    = #x05CDC7
    mod.widgets.RBX_BRAND_BLUE    = #x2D7CFF
    // Default room / space identity avatar fill (the teal "#" square).
    mod.widgets.RBX_IDENTITY_TEAL = #x14B8A6
    // DEPRECATED legacy bright-blue primary (styles.rs COLOR_ACTIVE_PRIMARY).
    // Named only for migration; new UI uses RBX_ACCENT.
    mod.widgets.RBX_LEGACY_BLUE   = #x0F88FE

    // =========================================================================
    // 5. STROKES & DIVIDERS
    // =========================================================================
    // Default card / control border (low contrast).
    mod.widgets.RBX_STROKE_SOFT   = #xE6EBF2
    // Stronger border (focused / emphasized control).
    mod.widgets.RBX_STROKE_STRONG = #xD5DEEA
    // Floating-overlay border (toast / notification card). Distinctly darker than
    // STRONG so a white card reads clearly against the light canvas WITHOUT a
    // shadow. Tune this one value to make overlay edges softer / harder.
    mod.widgets.RBX_STROKE_OVERLAY = #xC8D2DE
    // Hairline divider between rows (alpha black).
    mod.widgets.RBX_DIVIDER       = #x00000010

    // =========================================================================
    // 6. SEMANTIC STATES — fg/bg pairs. One meaning = one color, always.
    //    success = Connected / Healthy / Enabled / Active / Synced
    //    warning = Approval required / Pending / Waiting
    //    danger  = Failed / Rejected / Error / risk action
    //    info    = capability / linked object / neutral metadata highlight
    //    neutral = Idle / secondary / disabled-ish
    //    (accent badges use RBX_ACCENT_SOFT bg + RBX_ACCENT fg — not a state pair.)
    // =========================================================================
    mod.widgets.RBX_SUCCESS_FG = #x197F45
    mod.widgets.RBX_SUCCESS_BG = #xE8F6EE
    mod.widgets.RBX_WARNING_FG = #x9C6009
    mod.widgets.RBX_WARNING_BG = #xFBF1DD
    mod.widgets.RBX_DANGER_FG  = #xB93429
    mod.widgets.RBX_DANGER_BG  = #xFBE9E7
    mod.widgets.RBX_INFO_FG    = #x1C67B0
    mod.widgets.RBX_INFO_BG    = #xE7F0FB
    mod.widgets.RBX_NEUTRAL_FG = #x5A6B86
    mod.widgets.RBX_NEUTRAL_BG = #xEEF1F6

    // Agent framework badge colors (fg = label text, bg = pill fill). One pair
    // per supported framework, used by the Agent Registry tag pills.
    mod.widgets.RBX_FW_OCTOS_FG    = #x1488B5
    mod.widgets.RBX_FW_OCTOS_BG    = #xE6F2F9
    mod.widgets.RBX_FW_HERMES_FG   = #xC47D1E
    mod.widgets.RBX_FW_HERMES_BG   = #xFBF1E3
    mod.widgets.RBX_FW_OPENCLAW_FG = #x6A52C4
    mod.widgets.RBX_FW_OPENCLAW_BG = #xEEE9FB

    // =========================================================================
    // 7. DARK SURFACES — desktop nav rail + mobile login (the only dark zones
    //    in this round). Kept explicit rather than as a full theme.
    // =========================================================================
    // Desktop left navigation rail background.
    mod.widgets.RBX_NAV_BG             = #x1A2336
    // Nav item idle foreground.
    mod.widgets.RBX_NAV_FG             = #xAEBAD0
    // Nav item active foreground.
    mod.widgets.RBX_NAV_FG_ACTIVE      = #xFFFFFF
    // Nav item hover background (between rail bg and active).
    mod.widgets.RBX_NAV_ITEM_HOVER_BG  = #x222D43
    // Nav item active / selected pill background.
    mod.widgets.RBX_NAV_ITEM_ACTIVE_BG = #x2A3650
    // Nav rail hairline / section divider.
    mod.widgets.RBX_NAV_DIVIDER        = #x2C384F
    // Mobile login page background (deep navy).
    mod.widgets.RBX_LOGIN_BG           = #x0E1626
    // Mobile login field / card surface on the dark page.
    mod.widgets.RBX_LOGIN_SURFACE      = #x16213A

    // =========================================================================
    // 8. CODE PANEL (dark) — timeline code / SQL output (CodeOutputCard, §4.7).
    // =========================================================================
    mod.widgets.RBX_CODE_BG      = #x1B2433   // dark navy panel
    mod.widgets.RBX_CODE_FG      = #xD7DEE8   // default code text
    mod.widgets.RBX_CODE_KEYWORD = #x7CC4FF   // keyword / function
    mod.widgets.RBX_CODE_STRING  = #x8FD19A   // string / value
    mod.widgets.RBX_CODE_COMMENT = #x7F8B9B   // comment / muted

    // =========================================================================
    // 8b. SYNTAX PALETTE (light) — the twin of the dark RBX_CODE_* panel above,
    //     for code rendered on a *light* surface: the markdown code blocks in
    //     the timeline and the raw-event viewer. One palette, so a keyword is
    //     the same color everywhere code appears.
    // =========================================================================
    mod.widgets.RBX_SYNTAX_TEXT     = #x24292e   // identifiers, punctuation, default ink
    mod.widgets.RBX_SYNTAX_MUTED    = #x6a737d   // comments, whitespace marks
    mod.widgets.RBX_SYNTAX_LITERAL  = #x005cc5   // numbers, constants, highlighted delimiters
    mod.widgets.RBX_SYNTAX_KEYWORD  = #xd73a49   // control-flow and declaration keywords
    mod.widgets.RBX_SYNTAX_STRING   = #x22863a   // string literals
    mod.widgets.RBX_SYNTAX_FUNCTION = #x6f42c1   // function names
    mod.widgets.RBX_SYNTAX_TYPE     = #xe36209   // type names
    mod.widgets.RBX_SYNTAX_ERROR    = #xcb2431   // error decoration
    mod.widgets.RBX_SYNTAX_WARNING  = #xb08800   // warning decoration

    // =========================================================================
    // 8c. COMPOSER — the room input bar's own neutral ramp (spec §4.8). Kept
    //     separate from the page surfaces because the composer sits *on* the
    //     canvas and needs its own quiet contrast steps.
    // =========================================================================
    mod.widgets.RBX_COMPOSER_BG          = #xF0F4FA   // composer / attach-row fill
    mod.widgets.RBX_COMPOSER_BG_SUNKEN   = #xE8EEF8   // inset row inside the composer
    mod.widgets.RBX_COMPOSER_BG_ACTIVE   = #xE0E8F0   // active/selected inset row
    mod.widgets.RBX_COMPOSER_INPUT_BG    = #xF7F9FD   // text field fill
    mod.widgets.RBX_COMPOSER_INPUT_HOVER = #xEEF3F9
    mod.widgets.RBX_COMPOSER_INPUT_DOWN  = #xE5ECF5
    mod.widgets.RBX_COMPOSER_STROKE      = #xD7DFEA   // text field border
    mod.widgets.RBX_COMPOSER_INK         = #x333333   // primary glyph/icon ink
    mod.widgets.RBX_COMPOSER_INK_MUTED   = #x555555   // secondary ink
    mod.widgets.RBX_COMPOSER_INK_FAINT   = #x999999   // placeholder / disabled ink
    mod.widgets.RBX_COMPOSER_GREY_HOVER  = #xE0E0E0   // neutral button hover
    mod.widgets.RBX_COMPOSER_GREY_DOWN   = #xD0D0D0   // neutral button pressed

    // Accent washes: the accent at low alpha, for hover/active tints over dark
    // or image backgrounds (spaces rail). Derived from RBX_ACCENT — keep in
    // step with it.
    mod.widgets.RBX_ACCENT_WASH_HOVER  = #x0D798824
    mod.widgets.RBX_ACCENT_WASH_ACTIVE = #x0D79883D

    // Pressed overlay wash, one step darker than RBX_HIT_DOWN.
    mod.widgets.RBX_HIT_PRESSED = #x0000001E
    // Drop shadow under the mobile navigation bar (heavier than RBX_SHADOW,
    // because it separates two light surfaces rather than floating over one).
    mod.widgets.RBX_SHADOW_NAV  = #x00000055

    // Inline toggle text ("show 3 more") inside small-state groups: near-ink,
    // deepening on interaction.
    mod.widgets.RBX_INK_TOGGLE       = #x232A31
    mod.widgets.RBX_INK_TOGGLE_HOVER = #x1A1F25
    mod.widgets.RBX_INK_TOGGLE_DOWN  = #x0E1217

    // Controls that sit on top of media (video player chrome), where the page
    // palette would disappear against the frame behind them.
    mod.widgets.RBX_MEDIA_CONTROL_BG       = #x111827
    mod.widgets.RBX_MEDIA_CONTROL_BG_HOVER = #x374151

    // Neutral icon grey for indicator glyphs drawn in a shader.
    mod.widgets.RBX_ICON_NEUTRAL = #x888888
    // Mention/keyboard-focus highlight in the composer's autocomplete.
    mod.widgets.RBX_MENTION_FOCUS = #x1C274C
    // Near-opaque light green wash marking a freshly-arrived timeline item.
    mod.widgets.RBX_HIGHLIGHT_NEW = #xDAF5E5F0

    // =========================================================================
    // 9. ELEVATION — modal scrim + drop-shadow colors. Cards lean on radius +
    //    border; reserve shadow for floating layers (sheets / modals / dropdowns
    //    / composer). Blur & offset live in the recipe; the token is the color.
    // =========================================================================
    // Page scrim behind a modal / bottom sheet (navy @ ~50%).
    mod.widgets.RBX_SCRIM         = #x16233B80
    // Card / dropdown / popup drop-shadow color (~15%).
    mod.widgets.RBX_SHADOW        = #x16233B26
    // Sheet / modal drop-shadow color (~25%).
    mod.widgets.RBX_SHADOW_STRONG = #x16233B40

    // =========================================================================
    // 10. FOCUS — keyboard-navigation focus indication (== accent), spec §7.1.
    //     RING/WIDTH are for components that draw their own border and can turn
    //     it accent on focus. TINT is the fallback for flat controls: the button
    //     shader insets its box by `border_size`, so switching a border on just
    //     for focus would resize the control — a tinted fill does not.
    // =========================================================================
    mod.widgets.RBX_FOCUS_RING  = #x0D7988
    mod.widgets.RBX_FOCUS_WIDTH = 2.0
    mod.widgets.RBX_FOCUS_TINT  = #x0A6675

    // =========================================================================
    // 11. RADIUS scale — bigger & softer than the legacy RADIUS_* (4/6/8).
    //     Cards lean on radius + border, not heavy shadow.
    //     NOTE: the card default (MD) is intentionally tight (8) so cards line up
    //     visually with the room composer / input bar (which uses XS = 6). Keep
    //     cards calm and crisp rather than pill-soft.
    // =========================================================================
    // Tightened scale (squarer look, per design direction): every surface gets
    // smaller corners than the original 6/8/8/16/20.
    // Extra-extra-small: tightest radius, used by Agent Registry cards/sheets.
    mod.widgets.RBX_RADIUS_XXS  = 4.0
    mod.widgets.RBX_RADIUS_XS   = 4.0
    mod.widgets.RBX_RADIUS_SM   = 6.0
    // Card / sheet default. Shares SM's value on purpose: small surfaces and
    // cards use one calm, tight radius.
    mod.widgets.RBX_RADIUS_MD   = 6.0
    mod.widgets.RBX_RADIUS_LG   = 12.0
    mod.widgets.RBX_RADIUS_XL   = 16.0
    // Fully-rounded (pill) — use on badges / chips.
    mod.widgets.RBX_RADIUS_PILL = 100.0

    // =========================================================================
    // 12. SPACING — the 4px grid SPACE_XS..SPACE_XXL (4..24) from styles.rs still
    //     applies. These add the two larger section-level steps the new layouts use.
    // =========================================================================
    mod.widgets.RBX_SPACE_2XL = 32
    mod.widgets.RBX_SPACE_3XL = 40

    // =========================================================================
    // 13. SIZING — control heights, list-row heights, icon & avatar sizes, the
    //     mobile bottom-tab height, and the min touch target. Anchored to the 4px
    //     rhythm. Use these instead of hardcoding 32/36/40/48/52 per screen.
    // =========================================================================
    // Control heights (buttons, inputs, segmented tabs).
    mod.widgets.RBX_CONTROL_H_SM = 32.0   // compact button / chip
    mod.widgets.RBX_CONTROL_H_MD = 36.0   // standard button / segmented tab (== SETTINGS_BUTTON_HEIGHT)
    mod.widgets.RBX_CONTROL_H_LG = 44.0   // large button / text input
    // List / setting row min heights (touch-first on mobile).
    mod.widgets.RBX_ROW_H_DESKTOP = 48.0
    mod.widgets.RBX_ROW_H_MOBILE  = 52.0
    // Minimum touch target.
    mod.widgets.RBX_TAP_MIN       = 44.0
    // Mobile bottom tab bar height.
    mod.widgets.RBX_BOTTOM_TAB_H  = 56.0
    // Icon sizes.
    mod.widgets.RBX_ICON_XS = 12.0
    mod.widgets.RBX_ICON_SM = 16.0
    mod.widgets.RBX_ICON_MD = 20.0
    mod.widgets.RBX_ICON_LG = 24.0
    // Avatar sizes.
    mod.widgets.RBX_AVATAR_SM = 28.0   // inline / row
    mod.widgets.RBX_AVATAR_MD = 40.0   // message / list
    mod.widgets.RBX_AVATAR_LG = 48.0   // hero / room identity

    // =========================================================================
    // 14. TYPE SCALE — semantic TextStyle presets. Built on the app's real font
    //     providers (APP_FONT_REGULAR / APP_FONT_BOLD). Sizes match Makepad's
    //     dense scale used elsewhere in robrix2 (9–17). line_spacing matches the
    //     1.3 used by styles.rs message text, so multi-line wraps stay readable.
    // =========================================================================
    // Mobile/desktop page title (e.g. "Settings", room hero name).
    // Font providers for the RBX_TEXT_* scale. Upstream Robrix ships no
    // system-font links, so these sit on the theme fonts; the
    // `fonts/macos-system-fonts` branch swaps them for the OS's San Francisco
    // and PingFang families.
    mod.widgets.RBX_FONT_REGULAR = theme.font_regular {}
    mod.widgets.RBX_FONT_BOLD = theme.font_bold {}

    mod.widgets.RBX_TEXT_PAGE_TITLE    = mod.widgets.RBX_FONT_BOLD    { font_size: 17.0, line_spacing: 1.25 }
    // Section / group title inside a page.
    mod.widgets.RBX_TEXT_SECTION_TITLE = mod.widgets.RBX_FONT_BOLD    { font_size: 13.0, line_spacing: 1.25 }
    // Card title.
    mod.widgets.RBX_TEXT_CARD_TITLE    = mod.widgets.RBX_FONT_BOLD    { font_size: 12.0, line_spacing: 1.3 }
    // Default body / list-row title.
    mod.widgets.RBX_TEXT_BODY          = mod.widgets.RBX_FONT_REGULAR { font_size: 11.0, line_spacing: 1.35 }
    // Emphasized body (selected value, key figure).
    mod.widgets.RBX_TEXT_BODY_STRONG   = mod.widgets.RBX_FONT_BOLD    { font_size: 11.0, line_spacing: 1.35 }
    // Meta / caption / helper text.
    mod.widgets.RBX_TEXT_META          = mod.widgets.RBX_FONT_REGULAR { font_size: 9.5,  line_spacing: 1.3 }
    // Badge / chip label (single line).
    mod.widgets.RBX_TEXT_BADGE         = mod.widgets.RBX_FONT_BOLD    { font_size: 9.0 }
}

// =============================================================================
// Rust-side Vec4 mirror — for programmatic styling (script_apply_eval!, shaders,
// dynamic widgets). All semantic *colors* are mirrored; add non-color tokens
// (TextStyle / spacing / radius / sizing) here only if a Rust call site needs them.
// Values are straight sRGB/255 (matching styles.rs); 8-digit hex carries alpha.
// =============================================================================

// --- Surfaces ---
/// #F7F9FC — page canvas.
pub const RBX_BG_CANVAS:         Vec4 = vec4(0.969, 0.976, 0.988, 1.0);
/// #FFFFFF — surface.
pub const RBX_BG_SURFACE:        Vec4 = vec4(1.0, 1.0, 1.0, 1.0);
/// #F4F7FB — subtle surface.
pub const RBX_BG_SURFACE_SUBTLE: Vec4 = vec4(0.957, 0.969, 0.984, 1.0);
/// #EEF2F8 — sunken surface.
pub const RBX_BG_SUNKEN:         Vec4 = vec4(0.933, 0.949, 0.973, 1.0);
/// #EFF4FB — hover wash.
pub const RBX_BG_HOVER:          Vec4 = vec4(0.937, 0.957, 0.984, 1.0);
/// #E4F5F7 — selected row.
pub const RBX_BG_SELECTED:       Vec4 = vec4(0.894, 0.961, 0.969, 1.0);
/// #E7ECF3 — pressed surface.
pub const RBX_BG_PRESSED:        Vec4 = vec4(0.906, 0.925, 0.953, 1.0);
/// #F0F2F6 — disabled surface.
pub const RBX_BG_DISABLED:       Vec4 = vec4(0.941, 0.949, 0.965, 1.0);

// --- Foreground ---
/// #16233B — primary text.
pub const RBX_FG_PRIMARY:        Vec4 = vec4(0.086, 0.137, 0.231, 1.0);
/// #5A6B86 — secondary text.
pub const RBX_FG_SECONDARY:      Vec4 = vec4(0.353, 0.420, 0.525, 1.0);
/// #687283 — tertiary text (darkened for WCAG AA on light surfaces).
pub const RBX_FG_TERTIARY:       Vec4 = vec4(0.408, 0.447, 0.514, 1.0);
/// #FFFFFF — on-accent foreground.
pub const RBX_FG_ON_ACCENT:      Vec4 = vec4(1.0, 1.0, 1.0, 1.0);
/// #AEB7C6 — disabled text.
pub const RBX_FG_DISABLED:       Vec4 = vec4(0.682, 0.718, 0.776, 1.0);

// --- Accent ---
/// #0D7988 — primary teal accent (deepened so white 11px labels clear AA).
pub const RBX_ACCENT:            Vec4 = vec4(0.051, 0.475, 0.533, 1.0);
/// #0A6675 — accent hover.
pub const RBX_ACCENT_HOVER:      Vec4 = vec4(0.039, 0.400, 0.459, 1.0);
/// #085460 — accent pressed.
pub const RBX_ACCENT_PRESSED:    Vec4 = vec4(0.031, 0.329, 0.376, 1.0);
/// #E4F5F7 — soft accent tint.
pub const RBX_ACCENT_SOFT:       Vec4 = vec4(0.894, 0.961, 0.969, 1.0);
/// #167CB9 — link.
pub const RBX_LINK:              Vec4 = vec4(0.086, 0.486, 0.725, 1.0);
/// #136DA3 — hovered link (same hue as `RBX_LINK`, darkened).
pub const RBX_LINK_HOVER:        Vec4 = vec4(0.075, 0.427, 0.639, 1.0);

// --- Brand ---
/// #572DCC — brand purple.
pub const RBX_BRAND_PURPLE:      Vec4 = vec4(0.341, 0.176, 0.800, 1.0);
/// #05CDC7 — brand cyan.
pub const RBX_BRAND_CYAN:        Vec4 = vec4(0.020, 0.804, 0.780, 1.0);
/// #2D7CFF — brand blue.
pub const RBX_BRAND_BLUE:        Vec4 = vec4(0.176, 0.486, 1.0, 1.0);
/// #14B8A6 — room/space identity teal.
pub const RBX_IDENTITY_TEAL:     Vec4 = vec4(0.078, 0.722, 0.651, 1.0);
/// #0F88FE — DEPRECATED legacy blue. Use RBX_ACCENT for new work.
pub const RBX_LEGACY_BLUE:       Vec4 = vec4(0.059, 0.533, 0.996, 1.0);

// --- Strokes ---
/// #E6EBF2 — soft stroke.
pub const RBX_STROKE_SOFT:       Vec4 = vec4(0.902, 0.922, 0.949, 1.0);
/// #D5DEEA — strong stroke.
pub const RBX_STROKE_STRONG:     Vec4 = vec4(0.835, 0.871, 0.918, 1.0);

// --- Semantic states ---
/// #197F45 — success fg.
pub const RBX_SUCCESS_FG:        Vec4 = vec4(0.098, 0.498, 0.271, 1.0);
/// #E8F6EE — success bg.
pub const RBX_SUCCESS_BG:        Vec4 = vec4(0.910, 0.965, 0.933, 1.0);
/// #9C6009 — warning fg.
pub const RBX_WARNING_FG:        Vec4 = vec4(0.612, 0.376, 0.035, 1.0);
/// #FBF1DD — warning bg.
pub const RBX_WARNING_BG:        Vec4 = vec4(0.984, 0.945, 0.867, 1.0);
/// #B93429 — danger fg.
pub const RBX_DANGER_FG:         Vec4 = vec4(0.725, 0.204, 0.161, 1.0);
/// #FBE9E7 — danger bg.
pub const RBX_DANGER_BG:         Vec4 = vec4(0.984, 0.914, 0.906, 1.0);
/// #1C67B0 — info fg.
pub const RBX_INFO_FG:           Vec4 = vec4(0.110, 0.404, 0.690, 1.0);
/// #E7F0FB — info bg.
pub const RBX_INFO_BG:           Vec4 = vec4(0.906, 0.941, 0.984, 1.0);
/// #5A6B86 — neutral fg (== secondary).
pub const RBX_NEUTRAL_FG:        Vec4 = vec4(0.353, 0.420, 0.525, 1.0);
/// #EEF1F6 — neutral bg.
pub const RBX_NEUTRAL_BG:        Vec4 = vec4(0.933, 0.945, 0.965, 1.0);

// --- Dark surfaces ---
/// #1A2336 — dark nav rail background.
pub const RBX_NAV_BG:            Vec4 = vec4(0.102, 0.137, 0.212, 1.0);
/// #AEBAD0 — nav item idle fg.
pub const RBX_NAV_FG:            Vec4 = vec4(0.682, 0.729, 0.816, 1.0);
/// #FFFFFF — nav item active fg.
pub const RBX_NAV_FG_ACTIVE:     Vec4 = vec4(1.0, 1.0, 1.0, 1.0);
/// #2A3650 — nav item active bg.
pub const RBX_NAV_ITEM_ACTIVE_BG: Vec4 = vec4(0.165, 0.212, 0.314, 1.0);
/// #0E1626 — mobile login background.
pub const RBX_LOGIN_BG:          Vec4 = vec4(0.055, 0.086, 0.149, 1.0);
/// #16213A — mobile login surface.
pub const RBX_LOGIN_SURFACE:     Vec4 = vec4(0.086, 0.129, 0.227, 1.0);

// --- Code panel ---
/// #1B2433 — dark code panel background.
pub const RBX_CODE_BG:           Vec4 = vec4(0.106, 0.141, 0.200, 1.0);
/// #D7DEE8 — code text.
pub const RBX_CODE_FG:           Vec4 = vec4(0.843, 0.871, 0.910, 1.0);

// --- Elevation / focus ---
/// #16233B @ 50% — modal/sheet scrim.
pub const RBX_SCRIM:             Vec4 = vec4(0.086, 0.137, 0.231, 0.5);
/// #16233B @ 15% — drop-shadow color.
pub const RBX_SHADOW:            Vec4 = vec4(0.086, 0.137, 0.231, 0.15);
/// #0D7988 — focus ring (== accent).
pub const RBX_FOCUS_RING:        Vec4 = vec4(0.051, 0.475, 0.533, 1.0);

// =============================================================================
// Contrast guard
// =============================================================================

#[cfg(test)]
mod tests {
    /// Every foreground/background pair the design system promises will be used
    /// together, and the text that sits on it. WCAG AA wants 4.5:1 for body
    /// text, which at this type scale (9–17px) is everything here.
    const SEMANTIC_PAIRS: &[(&str, &str, &str)] = &[
        ("RBX_FG_PRIMARY", "RBX_BG_CANVAS", "body text on the page canvas"),
        ("RBX_FG_PRIMARY", "RBX_BG_SURFACE", "body text on a card"),
        ("RBX_FG_SECONDARY", "RBX_BG_SURFACE", "subtitles / meta on a card"),
        ("RBX_FG_SECONDARY", "RBX_BG_CANVAS", "subtitles / meta on the canvas"),
        ("RBX_FG_TERTIARY", "RBX_BG_SURFACE", "timestamps on a card"),
        ("RBX_FG_TERTIARY", "RBX_BG_CANVAS", "timestamps on the canvas"),
        ("RBX_FG_ON_ACCENT", "RBX_ACCENT", "label on the primary CTA"),
        ("RBX_FG_ON_ACCENT", "RBX_ACCENT_HOVER", "label on a hovered CTA"),
        ("RBX_LINK", "RBX_BG_SURFACE", "inline link on a card"),
        ("RBX_ACCENT", "RBX_ACCENT_SOFT", "accent badge / selected chip"),
        ("RBX_SUCCESS_FG", "RBX_SUCCESS_BG", "success badge"),
        ("RBX_WARNING_FG", "RBX_WARNING_BG", "warning / pending badge"),
        ("RBX_DANGER_FG", "RBX_DANGER_BG", "danger badge"),
        ("RBX_INFO_FG", "RBX_INFO_BG", "info / capability chip"),
        ("RBX_NEUTRAL_FG", "RBX_NEUTRAL_BG", "neutral / idle badge"),
        ("RBX_NAV_FG", "RBX_NAV_BG", "idle item in the desktop nav rail"),
        ("RBX_NAV_FG_ACTIVE", "RBX_NAV_BG", "active item in the desktop nav rail"),
        ("RBX_CODE_FG", "RBX_CODE_BG", "code panel body"),
        ("RBX_CODE_KEYWORD", "RBX_CODE_BG", "code panel keyword"),
        ("RBX_CODE_STRING", "RBX_CODE_BG", "code panel string"),
        ("RBX_CODE_COMMENT", "RBX_CODE_BG", "code panel comment"),
    ];

    /// Reads the token values out of this file's own `script_mod!` block, so
    /// the test guards what actually renders rather than the Rust mirror.
    fn token_hex(name: &str) -> (u8, u8, u8) {
        let source = include_str!("design_tokens.rs");
        let needle = format!("mod.widgets.{name} ");
        for line in source.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with(&needle) {
                continue;
            }
            let Some((_, rhs)) = trimmed.split_once('=') else { continue };
            let value = rhs.split("//").next().unwrap_or(rhs).trim();
            let hex = value.trim_start_matches("#x");
            if hex.len() >= 6 {
                return (
                    u8::from_str_radix(&hex[0..2], 16).unwrap(),
                    u8::from_str_radix(&hex[2..4], 16).unwrap(),
                    u8::from_str_radix(&hex[4..6], 16).unwrap(),
                );
            }
        }
        panic!("design token {name} not found in design_tokens.rs");
    }

    fn relative_luminance((r, g, b): (u8, u8, u8)) -> f64 {
        fn channel(c: u8) -> f64 {
            let c = c as f64 / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        }
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }

    fn contrast(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> f64 {
        let (a, b) = (relative_luminance(fg), relative_luminance(bg));
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// The palette was retuned to clear WCAG AA; this keeps it there. If a new
    /// color fails, darken (or lighten) it along the same hue rather than
    /// deleting the pair — `tools/ux-harness contrast --fg .. --bg ..` prints
    /// the nearest compliant value.
    #[test]
    fn semantic_token_pairs_meet_wcag_aa() {
        let mut failures = Vec::new();
        for (fg, bg, usage) in SEMANTIC_PAIRS {
            let ratio = contrast(token_hex(fg), token_hex(bg));
            if ratio < 4.5 {
                failures.push(format!("{fg} on {bg} = {ratio:.2}:1 ({usage})"));
            }
        }
        assert!(
            failures.is_empty(),
            "design tokens below WCAG AA 4.5:1:\n  {}",
            failures.join("\n  "),
        );
    }
}
