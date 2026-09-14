//! A small popup menu, anchored at the account-switcher button in the bottom-left of the
//! desktop navigation rail — the "Feishu-style" account menu ported from robrix2.
//!
//! Structure of the anchored card (top → bottom; it opens *upward* from the bottom of the
//! rail):
//!   * Active account header  → avatar + display name + user ID + an "Active" marker
//!   * A divider
//!   * "Account Settings"       → opens Settings
//!   * "Log Out"                → opens the LogoutConfirmModal
//!
//! robrix2's version also lists every *other* logged-in account as a switch row, plus a
//! "Log Into More Accounts" item. Both depend on robrix2's multi-account layer
//! (`account_manager` and `request_switch_account`), which upstream Robrix does not
//! have — it holds exactly one session. Those rows are therefore not rendered here; the
//! card keeps the same shape so they can slot back in if multi-account is ever ported.
//!
//! It reuses the same anchored-overlay pattern as the room context menu: a full-screen
//! scrim whose inner `main_content` card is positioned by the App (which clamps it to
//! the overlay container and sets its `margin` via `script_apply_eval!`). `show()`
//! returns the card's computed size so the App can anchor it upward — the card is
//! `height: Fit`, so there is no measurable height until it has been drawn.
//!
//! Desktop-only: opened from the rail's account-switcher button, and auto-closes if the
//! layout crosses the desktop/mobile breakpoint while open. The top profile avatar keeps
//! opening Settings directly, exactly as before.

use makepad_widgets::*;

use crate::{
    home::navigation_tab_bar::{get_own_profile, NavigationBarAction},
    logout::logout_confirm_modal::LogoutConfirmModalAction,
    settings::app_preferences::{AppPreferencesGlobal, ViewModeOverride},
    shared::avatar::AvatarWidgetExt,
    sliding_sync::current_user_id,
    utils,
};

/// The fixed width of the account menu card, in DIPs.
pub const ACCOUNT_MENU_WIDTH: f64 = 272.0;

// --- Height constants, kept in sync with the DSL below so `show()` can compute the
// card's total height for upward anchoring + clamping. ---
const CARD_VPAD: f64 = 6.0; // main_content padding (top == bottom == this)
const ROW_SPACING: f64 = 2.0; // main_content `spacing` between children
const HEADER_H: f64 = 58.0; // active-account header (avatar 40 + 9+9 padding)
const DIVIDER_H: f64 = 10.0; // LineH (shared height 2) + its 4 + 4 vertical margins
const ACTION_H: f64 = 40.0; // one action item (settings / logout)
const ACTION_COUNT: f64 = 2.0;
/// Children of `main_content` that contribute a `spacing` gap: header, divider, actions.
const ROW_COUNT: f64 = 1.0 + 1.0 + ACTION_COUNT;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A single action row: left-aligned icon + label, with an RBX surface/hover/pressed
    // background and a soft rounded highlight.
    mod.widgets.AccountMenuItem = RobrixIconButton {
        height: 40,
        width: Fill,
        margin: 0,
        padding: Inset{left: 12, right: 12, top: 8, bottom: 8}
        spacing: 12,
        align: Align{x: 0.0, y: 0.5}
        icon_walk: Walk{width: 18, height: 18, margin: Inset{right: 2}}

        draw_bg +: {
            color: (mod.widgets.RBX_BG_SURFACE)
            color_hover: (mod.widgets.RBX_BG_HOVER)
            color_down: (mod.widgets.RBX_BG_PRESSED)
            border_size: 0.0
            border_radius: (mod.widgets.RBX_RADIUS_SM)
        }
        draw_icon.color: (mod.widgets.RBX_ACCENT)
        draw_text +: {
            color: (mod.widgets.RBX_FG_PRIMARY)
            color_hover: (mod.widgets.RBX_FG_PRIMARY)
            color_down: (mod.widgets.RBX_FG_PRIMARY)
            text_style: (mod.widgets.RBX_TEXT_BODY)
        }
    }

    // Danger-styled action row (log out): red icon/text, red-tinted hover.
    mod.widgets.AccountMenuDangerItem = mod.widgets.AccountMenuItem {
        draw_bg +: {
            color: (mod.widgets.RBX_BG_SURFACE)
            color_hover: (mod.widgets.RBX_DANGER_BG)
            color_down: (mod.widgets.RBX_DANGER_BG)
        }
        draw_icon.color: (mod.widgets.RBX_DANGER_FG)
        draw_text +: {
            color: (mod.widgets.RBX_DANGER_FG)
            color_hover: (mod.widgets.RBX_DANGER_FG)
            color_down: (mod.widgets.RBX_DANGER_FG)
        }
    }

    mod.widgets.AccountMenu = set_type_default() do #(AccountMenu::register_widget(vm)) {
        ..mod.widgets.SolidView

        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        cursor: MouseCursor.Default,
        align: Align{x: 0, y: 0}

        show_bg: true
        draw_bg +: {
            color: (mod.widgets.RBX_SCRIM)
        }

        main_content := RoundedView {
            flow: Down
            width: 272,
            height: Fit,
            padding: 6,
            spacing: 2,

            show_bg: true
            // Flat card: tight corners, a defined border, no drop shadow (the scrim
            // already separates it from the content behind).
            draw_bg +: {
                color: (mod.widgets.RBX_BG_SURFACE)
                border_radius: (mod.widgets.RBX_RADIUS_SM)
                border_size: 1.0
                border_color: (mod.widgets.RBX_STROKE_STRONG)
            }

            // --- Active account header ---
            // height: Fit so the two-line name + user id is never clipped by the card.
            active_account_header := RoundedView {
                width: Fill,
                height: Fit,
                flow: Right,
                align: Align{y: 0.5}
                padding: Inset{left: 8, right: 8, top: 9, bottom: 9}
                spacing: 10,
                show_bg: true
                draw_bg +: {
                    color: (mod.widgets.RBX_ACCENT_SOFT)
                    border_radius: (mod.widgets.RBX_RADIUS_SM)
                }

                active_avatar := Avatar {
                    width: 40, height: 40
                }

                View {
                    width: Fill, height: Fit,
                    flow: Down,
                    spacing: 0,

                    active_name := Label {
                        width: Fill, height: Fit,
                        margin: 0, padding: 0,
                        text_overflow: Ellipsis
                        draw_text +: {
                            color: (mod.widgets.RBX_FG_PRIMARY)
                            text_style: (mod.widgets.RBX_TEXT_BODY_STRONG)
                        }
                        text: "Display Name"
                    }
                    active_user_id := Label {
                        width: Fill, height: Fit,
                        margin: 0, padding: 0,
                        text_overflow: Ellipsis
                        draw_text +: {
                            color: (mod.widgets.RBX_FG_SECONDARY)
                            text_style: (mod.widgets.RBX_TEXT_META)
                        }
                        text: "@user:server"
                    }
                }

                // "Current account" marker on the right: plain bold accent text, which
                // always renders and has ample contrast on the pale header.
                active_badge_label := Label {
                    width: Fit, height: Fit,
                    margin: 0, padding: 0,
                    draw_text +: {
                        color: (mod.widgets.RBX_ACCENT)
                        text_style: theme.font_bold { font_size: 10.5 }
                    }
                    text: "Active"
                }
            }

            divider := LineH {
                margin: Inset{top: 4, bottom: 4, left: 8, right: 8}
                draw_bg.color: (mod.widgets.RBX_DIVIDER)
            }

            // --- Actions ---
            settings_item := mod.widgets.AccountMenuItem {
                draw_icon +: { svg: (ICON_SETTINGS) }
                text: "Account Settings"
            }
            logout_item := mod.widgets.AccountMenuDangerItem {
                draw_icon +: { svg: (ICON_LOGOUT) }
                text: "Log Out"
            }
        }
    }
}

/// Action to request showing the account menu, anchored so the card's *bottom-left*
/// corner sits at `pos` (already computed by the emitter — the bottom-right of the
/// account-switcher button, so the menu opens upward). The App clamps this into the
/// overlay container using the size returned by [`AccountMenuRef::show`].
#[derive(Clone, Debug, Default)]
pub enum AccountMenuAction {
    Open { pos: DVec2 },
    #[default]
    None,
}

/// Whether the app is currently laid out as desktop, honouring the user's
/// view-mode override the same way the `HomeScreen`'s `AdaptiveView` does.
pub fn is_desktop_layout(cx: &mut Cx) -> bool {
    match cx.global::<AppPreferencesGlobal>().0.view_mode {
        ViewModeOverride::ForceWide => true,
        ViewModeOverride::ForceNarrow => false,
        ViewModeOverride::Automatic => cx.display_context.is_desktop(),
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct AccountMenu {
    #[deref] view: View,
    #[source] source: ScriptObjectRef,
    /// The effective layout (desktop vs mobile) at the moment the menu was opened. If
    /// it changes while the menu is open, the menu auto-closes — its anchored position
    /// is only valid for the layout it was opened in.
    #[rust(true)] opened_is_desktop: bool,
}

impl Widget for AccountMenu {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let step = self.view.draw_walk(cx, scope, walk);
        if self.visible {
            let main_content_area = self.view(cx, ids!(main_content)).area();
            cx.block_scrolling_except_within(main_content_area);
        }
        step
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.visible {
            return;
        }
        // Close if the layout switched between desktop and mobile while open, so the
        // menu can't linger at a now-wrong anchor in the other layout.
        if is_desktop_layout(cx) != self.opened_is_desktop {
            self.close(cx);
            return;
        }
        self.view.handle_event(cx, event, scope);

        // Close on backdrop click, Escape, or a system back gesture. Opened from a
        // button *click* (FingerUp) which is fully consumed by the time we become
        // visible, so no stray FingerUp lands on the scrim.
        let area = self.view.area();
        let close_menu = event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerUp(fue) if fue.is_over => {
                    !self.view(cx, ids!(main_content)).area().rect(cx).contains(fue.abs)
                }
                _ => false,
            };
        if close_menu {
            self.close(cx);
            return;
        }

        self.widget_match_event(cx, event, scope);
    }
}

impl WidgetMatchEvent for AccountMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        if self.button(cx, ids!(settings_item)).clicked(actions) {
            cx.action(NavigationBarAction::OpenSettings);
            self.close(cx);
        } else if self.button(cx, ids!(logout_item)).clicked(actions) {
            cx.action(LogoutConfirmModalAction::Open);
            self.close(cx);
        }
    }
}

impl AccountMenu {
    /// Populates the header from the current account and shows the menu, returning its
    /// expected `(width, height)` so the App can anchor + clamp it.
    fn show(&mut self, cx: &mut Cx) -> DVec2 {
        self.opened_is_desktop = is_desktop_layout(cx);

        // Populate the active-account header from our own profile (real avatar + display
        // name when available), mirroring how ProfileIcon draws its avatar.
        let own_profile = get_own_profile(cx);
        let active_avatar = self.view.avatar(cx, ids!(active_avatar));
        let (name_text, user_id_text) = if let Some(profile) = own_profile.as_ref() {
            let mut drew_image = false;
            if let Some(avatar_image) = profile.avatar_state.image() {
                drew_image = active_avatar
                    .show_image(cx, None, |cx, img| utils::load_avatar_image(&img, cx, avatar_image))
                    .is_ok();
            }
            if !drew_image {
                active_avatar.show_text(cx, None, None, profile.displayable_name());
            }
            (profile.displayable_name().to_string(), profile.user_id.to_string())
        } else if let Some(active) = current_user_id() {
            active_avatar.show_text(cx, None, None, active.as_str());
            (active.to_string(), active.to_string())
        } else {
            active_avatar.show_text(cx, None, None, "");
            ("Not logged in".to_string(), String::new())
        };
        self.label(cx, ids!(active_name)).set_text(cx, &name_text);
        self.label(cx, ids!(active_user_id)).set_text(cx, &user_id_text);
        // Hide the user-id line when it would just duplicate the name (no display name).
        self.label(cx, ids!(active_user_id))
            .set_visible(cx, !user_id_text.is_empty() && user_id_text != name_text);

        for id in [ids!(settings_item), ids!(logout_item)] {
            self.button(cx, id).reset_hover(cx);
        }

        self.visible = true;
        cx.set_key_focus(self.view.area());
        self.redraw(cx);

        let height = 2.0 * CARD_VPAD
            + HEADER_H
            + DIVIDER_H
            + ACTION_COUNT * ACTION_H
            + ROW_COUNT * ROW_SPACING;
        dvec2(ACCOUNT_MENU_WIDTH, height)
    }

    fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        cx.revert_key_focus();
        cx.unblock_scrolling();
        self.redraw(cx);
    }
}

impl AccountMenuRef {
    /// See [`AccountMenu::show`].
    pub fn show(&self, cx: &mut Cx) -> DVec2 {
        let Some(mut inner) = self.borrow_mut() else { return DVec2::default() };
        inner.show(cx)
    }

    pub fn is_currently_shown(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.visible)
    }
}
