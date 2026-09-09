//! Privacy-related settings within the SettingsScreen.
//!
//! Currently this just includes the list of blocked/ignored users.

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedUserId;

use crate::{
    block_user_modal::{BlockUserModalAction, BlockUserRequest},
    sliding_sync::{BlockedUsersUpdated, get_blocked_users},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A single blocked user in the blocked users list.
    mod.widgets.BlockedUserEntry = #(BlockedUserEntry::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down

        View {
            width: Fill, height: Fit
            flow: Right,
            padding: 10,
            spacing: 10,
            align: Align{y: 0.5}

            unblock_button := RobrixPositiveIconButton {
                height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
                padding: Inset{left: 12, right: 15}
                icon_walk: Walk{width: 0, height: 0}
                spacing: 0
                text: "Unblock"
            }

            user_id := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true},
                max_lines: 2,
                text_overflow: Ellipsis,
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: theme.font_bold { font_size: 12 },
                }
            }
        }

        LineH { padding: 10, margin: Inset{left: 5, right: 5} }
    }

    mod.widgets.PrivacySettings = #(PrivacySettings::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down

        LineH { width: 425, padding: 10, margin: Inset{top: 20, bottom: 5} }

        TitleLabel {
            text: "Privacy Settings"
        }

        SubsectionLabel {
            text: "Blocked Users"
        }

        mod.widgets.SettingsSectionDescription {
            body: "<ul><li>You won't see any messages or invites from a blocked user, in any room.</li></ul>"
        }

        no_blocked_users_label := View {
            width: Fill, height: Fit
            Label {
                width: Fill, height: Fit
                margin: Inset{top: 10, bottom: 8, left: 13, right: 10},
                flow: Flow.Right{wrap: true},
                draw_text +: {
                    color: (COLOR_TEXT_WARNING_NOT_FOUND),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                }
                text: "You haven't blocked anyone."
            }
        }

        RoundedView {
            width: Fill, height: Fit
            margin: 5,

            show_bg: true,
            draw_bg +: {
                color: #F6F8F9
                border_radius: 4.0
            }

            blocked_users_list := FlatList {
                width: Fill,
                height: Fit,
                spacing: 0.0
                flow: Down,

                grab_key_focus: true,
                drag_scrolling: true,
                scroll_bars: ScrollBars { show_scroll_x: false, show_scroll_y: false },

                blocked_user_entry := mod.widgets.BlockedUserEntry { }
            }
        }
    }
}


/// A single blocked user shown in the list of blocked users.
///
/// Note that this intentionally excludes the blocked users' display names
/// and avatars, and they may be offensive and our current user won't want to see them.
#[derive(Script, ScriptHook, Widget)]
pub struct BlockedUserEntry {
    #[deref] view: View,
    #[rust] user_id: Option<OwnedUserId>,
}

impl Widget for BlockedUserEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let Some(user_id) = self.user_id.as_ref() else { return };
        let Event::Actions(actions) = event else { return };
        if self.view.button(cx, ids!(unblock_button)).clicked(actions) {
            cx.action(BlockUserModalAction::Open(BlockUserRequest {
                user_id: user_id.clone(),
                display_name: None,
                block: false,
                reject_invite_to: None,
            }));
        }
        if actions.iter().any(|a| matches!(a.downcast_ref(), Some(BlockUserModalAction::Close))) {
            self.view.button(cx, ids!(unblock_button)).reset_hover(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let user_id = scope.props.get::<OwnedUserId>().unwrap();
        self.view.label(cx, ids!(user_id)).set_text(cx, user_id.as_str());
        if self.user_id.as_ref() != Some(user_id) {
            self.user_id = Some(user_id.clone());
        }

        self.view.draw_walk(cx, scope, walk)
    }
}


/// The privacy settings section, which lists the users blocked by this account.
#[derive(Script, ScriptHook, Widget)]
pub struct PrivacySettings {
    #[deref] view: View,

    /// The blocked users known to this widget, or `None` if they haven't been loaded yet.
    #[rust] blocked_users: Option<Vec<OwnedUserId>>,
}

impl Widget for PrivacySettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(BlockedUsersUpdated(blocked_users)) = action.downcast_ref() {
                    self.blocked_users = Some(blocked_users.clone());
                    self.view.redraw(cx);
                }
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let blocked_users = &*self.blocked_users.get_or_insert_with(get_blocked_users);
        self.view.view(cx, ids!(no_blocked_users_label)).set_visible(cx, blocked_users.is_empty());

        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            // Here, we only need to handle drawing the blocked users list.
            let flat_list_ref = subview.as_flat_list();
            let Some(mut list) = flat_list_ref.borrow_mut() else {
                error!("!!! PrivacySettings::draw_walk(): BUG: expected a FlatList widget, but got something else");
                continue;
            };
            for user_id in blocked_users {
                if let Some(item) = list.item(cx, LiveId::from_str(user_id.as_str()), id!(blocked_user_entry)) {
                    item.draw_all(cx, &mut Scope::with_props(user_id));
                }
            }
            // Drop the entries for any users who were unblocked and removed from this list.
            list.items.retain_visible();
        }
        DrawStep::done()
    }
}

impl PrivacySettingsRef {
    /// Reloads the list of blocked users shown by this section.
    pub fn populate(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.blocked_users = Some(get_blocked_users());
        inner.redraw(cx);
    }
}
