//! A modal dialog for binding or unbinding bots to a room.

use makepad_widgets::*;
use ruma::{OwnedUserId, UserId};

use crate::{
    app::{AppState, BotSettingsState, RoomBotBindingState},
    i18n::{AppLanguage, tr_fmt, tr_key},
    persistence,
    shared::popup_list::{PopupKind, enqueue_popup_notification},
    sliding_sync::{MatrixRequest, current_user_id, submit_async_request},
    utils::RoomNameId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.BotBindingModalLabel = Label {
        width: Fill
        height: Fit
        draw_text +: {
            text_style: REGULAR_TEXT { font_size: 10.5 }
            color: #666
        }
        text: ""
    }

    mod.widgets.BotBindingModal = #(BotBindingModal::register_widget(vm)) {
        width: Fill { max: 1000 }
        // TODO: i'd like for this height to be Fit with a max of Rel { base: Full, factor: 0.90 },
        //       but Makepad doesn't allow Fit views with a max to be scrolled.
        height: Fill // { max: 1400 }
        margin: 40,
        align: Align{x: 0.5, y: 0}
        flow: Down
        padding: Inset{top: 20, right: 25, bottom: 20, left: 25}

        RoundedView {
            width: Fill
            height: Fit
            align: Align{x: 0.5}
            flow: Down
            padding: Inset{top: 28, right: 24, bottom: 20, left: 24}
            spacing: 18

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 6.0
            }

            title := Label {
                width: Fill
                height: Fit
                draw_text +: {
                    text_style: TITLE_TEXT { font_size: 14 }
                    color: #000
                }
                text: "Manage Room Bots"
            }

            body := mod.widgets.BotBindingModalLabel {
                text: ""
            }

            form := RoundedView {
                width: Fill
                height: Fit
                flow: Down
                spacing: 12
                padding: 16

                show_bg: true
                draw_bg +: {
                    color: #F5F5F7
                    border_radius: 6.0
                }

                current_room_bots_label := mod.widgets.BotBindingModalLabel {
                    text: "Current Room Bots"
                }

                current_room_bots_dropdown := DropDownFlat {
                    width: Fill
                    height: 40
                    align: Align{y: 0.5}
                    padding: Inset{left: 12, top: 11, bottom: 11, right: 30}
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 11.5 }
                        color: #333
                        color_hover: uniform(#222)
                        color_focus: uniform(#222)
                        color_down: uniform(#222)
                    }
                    draw_bg +: {
                        color: uniform(#fff)
                        color_hover: uniform(#F0F0F2)
                        color_focus: uniform(#F0F0F2)
                        color_down: uniform(#E8E8EA)
                        border_color: uniform(#CCC)
                        border_color_hover: uniform(#AAA)
                        border_color_focus: uniform((COLOR_ACTIVE_PRIMARY))
                        arrow_color: uniform(#888)
                        arrow_color_hover: uniform(#555)
                    }
                    labels: ["No bots currently added"]
                }

                known_bots_label := mod.widgets.BotBindingModalLabel {
                    text: "Known Bots"
                }

                known_bots_dropdown := DropDownFlat {
                    width: Fill
                    height: 40
                    align: Align{y: 0.5}
                    padding: Inset{left: 12, top: 11, bottom: 11, right: 30}
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 11.5 }
                        color: #333
                        color_hover: uniform(#222)
                        color_focus: uniform(#222)
                        color_down: uniform(#222)
                    }
                    draw_bg +: {
                        color: uniform(#fff)
                        color_hover: uniform(#F0F0F2)
                        color_focus: uniform(#F0F0F2)
                        color_down: uniform(#E8E8EA)
                        border_color: uniform(#CCC)
                        border_color_hover: uniform(#AAA)
                        border_color_focus: uniform((COLOR_ACTIVE_PRIMARY))
                        arrow_color: uniform(#888)
                        arrow_color_hover: uniform(#555)
                    }
                    labels: ["Custom bot user ID"]
                }

                user_id_label := mod.widgets.BotBindingModalLabel {
                    text: "Bot Matrix User ID"
                }

                user_id_input := RobrixTextInput {
                    width: Fill
                    height: Fit
                    padding: 12
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 11.5 }
                        color: #000
                    }
                    empty_text: "@bot_weather:server or bot_weather"
                }

                remark_label := mod.widgets.BotBindingModalLabel {
                    text: "Bot Remark"
                }

                remark_input := RobrixTextInput {
                    width: Fill
                    height: Fit
                    padding: 12
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 11.5 }
                        color: #000
                    }
                    empty_text: "What is this bot used for?"
                }

                remark_controls := View {
                    width: Fill
                    height: Fit
                    flow: Right
                    align: Align{x: 1.0, y: 0.5}

                    save_remark_button := RobrixNeutralIconButton {
                        width: 150
                        align: Align{x: 0.5, y: 0.5}
                        padding: 10
                        draw_icon.svg: (ICON_CHECKMARK)
                        icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                        text: "Save Remark"
                    }
                }
            }

            status_label := Label {
                width: Fill
                height: Fit
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10.5 }
                    color: #000
                }
                text: ""
            }

            buttons := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{x: 1.0, y: 0.5}
                spacing: 14

                cancel_button := RobrixNeutralIconButton {
                    width: 100
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_FORBIDDEN)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Cancel"
                }

                unbind_button := RobrixNegativeIconButton {
                    width: 120
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Unbind"
                }

                bind_button := RobrixPositiveIconButton {
                    width: 120
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_ADD_USER)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Bind"
                }
            }
        }
    }
}

/// Actions emitted by other widgets to show or hide the `BotBindingModal`.
#[derive(Clone, Debug)]
pub enum BotBindingModalAction {
    /// Open the modal to bind or unbind bots in the given room.
    Open(RoomNameId),
    /// Close the modal.
    Close,
}

#[derive(Script, ScriptHook, Widget)]
pub struct BotBindingModal {
    #[deref]
    view: View,
    #[rust]
    room_name_id: Option<RoomNameId>,
    #[rust]
    known_bot_user_ids: Vec<OwnedUserId>,
    #[rust]
    room_bound_bots: Vec<RoomBotBindingState>,
    #[rust]
    app_language: AppLanguage,
}

impl Widget for BotBindingModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(app_state) = scope.data.get::<AppState>()
            && self.app_language != app_state.app_language
        {
            self.app_language = app_state.app_language;
            self.update_static_texts(cx);
            if let Some(room_name_id) = self.room_name_id.clone() {
                self.set_title_and_body(cx, &room_name_id);
                self.update_room_bound_bots_value(cx);
            }
        }
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for BotBindingModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let cancel_button = self.view.button(cx, ids!(buttons.cancel_button));
        let bind_button = self.view.button(cx, ids!(buttons.bind_button));
        let unbind_button = self.view.button(cx, ids!(buttons.unbind_button));
        let current_room_bots_dropdown = self.view.drop_down(cx, ids!(form.current_room_bots_dropdown));
        let known_bots_dropdown = self.view.drop_down(cx, ids!(form.known_bots_dropdown));
        let user_id_input = self.view.text_input(cx, ids!(form.user_id_input));
        let remark_input = self.view.text_input(cx, ids!(form.remark_input));
        let save_remark_button = self.view.button(cx, ids!(form.remark_controls.save_remark_button));
        let mut status_label = self.view.label(cx, ids!(status_label));

        let cancel_clicked = cancel_button.clicked(actions);
        if cancel_clicked
            || actions
                .iter()
                .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            if cancel_clicked {
                cx.action(BotBindingModalAction::Close);
            }
            return;
        }

        if known_bots_dropdown.changed(actions).is_some() {
            let selected_item = known_bots_dropdown.selected_item();
            if selected_item == 0 {
                user_id_input.set_text(cx, "");
                remark_input.set_text(cx, "");
                user_id_input.set_key_focus(cx);
            } else if let Some(bot_user_id) = self.known_bot_user_ids.get(selected_item - 1) {
                user_id_input.set_text(cx, bot_user_id.as_str());
                remark_input.set_text(
                    cx,
                    self.room_bot_remark(bot_user_id.as_ref()).unwrap_or(""),
                );
            }
            status_label.set_text(cx, "");
            self.view.redraw(cx);
        }

        if current_room_bots_dropdown.changed(actions).is_some() {
            let selected_item = current_room_bots_dropdown.selected_item();
            if let Some(room_bot_binding) = selected_item
                .checked_sub(1)
                .and_then(|index| self.room_bound_bots.get(index))
            {
                user_id_input.set_text(cx, room_bot_binding.bot_user_id.as_str());
                remark_input.set_text(cx, &room_bot_binding.remark);
                known_bots_dropdown.set_selected_item(
                    cx,
                    self.known_bot_user_ids
                        .iter()
                        .position(|bot_user_id| bot_user_id.as_str() == room_bot_binding.bot_user_id.as_str())
                        .map_or(0, |index| index + 1),
                );
            }
            status_label.set_text(cx, "");
            self.view.redraw(cx);
        }

        if save_remark_button.clicked(actions) || remark_input.returned(actions).is_some() {
            let Some(room_name_id) = self.room_name_id.as_ref() else { return };
            let raw_user_id = user_id_input.text();
            let raw_user_id = raw_user_id.trim();
            if raw_user_id.is_empty() {
                script_apply_eval!(cx, status_label, {
                    text: #(tr_key(self.app_language, "bot_binding_modal.status.enter_user_id")),
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_DANGER_RED
                    }
                });
                user_id_input.set_key_focus(cx);
                self.view.redraw(cx);
                return;
            }

            let full_user_id = if raw_user_id.starts_with('@') || raw_user_id.contains(':') {
                if raw_user_id.starts_with('@') {
                    raw_user_id.to_owned()
                } else {
                    format!("@{raw_user_id}")
                }
            } else {
                let Some(current_user_id) = current_user_id() else {
                    script_apply_eval!(cx, status_label, {
                        text: #(tr_key(self.app_language, "bot_binding_modal.status.current_user_unavailable")),
                        draw_text +: {
                            color: mod.widgets.COLOR_FG_DANGER_RED
                        }
                    });
                    self.view.redraw(cx);
                    return;
                };
                format!("@{raw_user_id}:{}", current_user_id.server_name())
            };
            let Ok(bot_user_id) = UserId::parse(&full_user_id).map(|user_id| user_id.to_owned()) else {
                let status = tr_fmt(
                    self.app_language,
                    "bot_binding_modal.status.invalid_user_id",
                    [("full_user_id", full_user_id.as_str())].as_ref(),
                );
                script_apply_eval!(cx, status_label, {
                    text: #(status),
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_DANGER_RED
                    }
                });
                user_id_input.set_key_focus(cx);
                self.view.redraw(cx);
                return;
            };
            let remark = remark_input.text().trim().to_string();
            let Some(app_state) = scope.data.get_mut::<AppState>() else {
                script_apply_eval!(cx, status_label, {
                    text: #(tr_key(self.app_language, "bot_binding_modal.status.state_unavailable")),
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_DANGER_RED
                    }
                });
                self.view.redraw(cx);
                return;
            };
            if app_state
                .bot_settings
                .set_room_bot_remark(room_name_id.room_id(), bot_user_id.as_ref(), remark)
            {
                self.room_bound_bots = app_state.bot_settings.room_bindings_for(room_name_id.room_id());
                self.update_room_bound_bots_value(cx);
                let current_room_bots_dropdown = self.view.drop_down(cx, ids!(form.current_room_bots_dropdown));
                current_room_bots_dropdown.set_selected_item(
                    cx,
                    self.room_bound_bots
                        .iter()
                        .position(|binding| binding.bot_user_id.as_str() == bot_user_id.as_str())
                        .map_or(0, |index| index + 1),
                );
                persist_bot_settings(app_state);
                script_apply_eval!(cx, status_label, {
                    text: #(tr_key(self.app_language, "bot_binding_modal.status.remark_saved")),
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_ACCEPT_GREEN
                    }
                });
            } else {
                script_apply_eval!(cx, status_label, {
                    text: #(tr_key(self.app_language, "bot_binding_modal.status.remark_requires_added_bot")),
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_DANGER_RED
                    }
                });
            }
            self.view.redraw(cx);
            return;
        }

        let mut handle_submit = |bound: bool| {
            let Some(room_name_id) = self.room_name_id.as_ref() else { return };

            let raw_user_id = user_id_input.text();
            let raw_user_id = raw_user_id.trim();
            if raw_user_id.is_empty() {
                script_apply_eval!(cx, status_label, {
                    text: #(tr_key(self.app_language, "bot_binding_modal.status.enter_user_id")),
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_DANGER_RED
                    }
                });
                user_id_input.set_key_focus(cx);
                self.view.redraw(cx);
                return;
            }

            let full_user_id = if raw_user_id.starts_with('@') || raw_user_id.contains(':') {
                if raw_user_id.starts_with('@') {
                    raw_user_id.to_owned()
                } else {
                    format!("@{raw_user_id}")
                }
            } else {
                let Some(current_user_id) = current_user_id() else {
                    script_apply_eval!(cx, status_label, {
                        text: #(tr_key(self.app_language, "bot_binding_modal.status.current_user_unavailable")),
                        draw_text +: {
                            color: mod.widgets.COLOR_FG_DANGER_RED
                        }
                    });
                    self.view.redraw(cx);
                    return;
                };
                format!("@{raw_user_id}:{}", current_user_id.server_name())
            };

            let Ok(bot_user_id) = UserId::parse(&full_user_id).map(|user_id| user_id.to_owned()) else {
                let status = tr_fmt(
                    self.app_language,
                    "bot_binding_modal.status.invalid_user_id",
                    [("full_user_id", full_user_id.as_str())].as_ref(),
                );
                script_apply_eval!(cx, status_label, {
                    text: #(status),
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_DANGER_RED
                    }
                });
                user_id_input.set_key_focus(cx);
                self.view.redraw(cx);
                return;
            };

            submit_async_request(MatrixRequest::SetRoomBotBinding {
                room_id: room_name_id.room_id().clone(),
                bound,
                bot_user_id: bot_user_id.clone(),
            });
            enqueue_popup_notification(
                if bound {
                    tr_fmt(
                        self.app_language,
                        "bot_binding_modal.popup.inviting",
                        [("bot_user_id", bot_user_id.as_str())].as_ref(),
                    )
                } else {
                    tr_fmt(
                        self.app_language,
                        "bot_binding_modal.popup.removing",
                        [("bot_user_id", bot_user_id.as_str())].as_ref(),
                    )
                },
                PopupKind::Info,
                Some(4.0),
            );
            cx.action(BotBindingModalAction::Close);
        };

        if bind_button.clicked(actions) || user_id_input.returned(actions).is_some() {
            handle_submit(true);
            return;
        }
        if unbind_button.clicked(actions) {
            handle_submit(false);
        }
    }
}

impl BotBindingModal {
    fn set_title_and_body(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) {
        self.view
            .label(cx, ids!(title))
            .set_text(cx, tr_key(self.app_language, "bot_binding_modal.title"));
        self.view
            .label(cx, ids!(body))
            .set_text(
                cx,
                &tr_fmt(
                    self.app_language,
                    "bot_binding_modal.body",
                    [("room_name", room_name_id.to_string().as_str())].as_ref(),
                ),
            );
    }

    fn known_bot_labels(&self) -> Vec<String> {
        let mut labels = Vec::with_capacity(self.known_bot_user_ids.len() + 1);
        labels.push(tr_key(self.app_language, "bot_binding_modal.dropdown.custom").to_string());
        labels.extend(self.known_bot_user_ids.iter().map(ToString::to_string));
        labels
    }

    fn room_bot_remark(&self, bot_user_id: &UserId) -> Option<&str> {
        self.room_bound_bots
            .iter()
            .find(|binding| binding.bot_user_id.as_str() == bot_user_id.as_str())
            .map(|binding| binding.remark.as_str())
    }

    fn room_bound_bot_labels(&self) -> Vec<String> {
        if self.room_bound_bots.is_empty() {
            return vec![
                tr_key(self.app_language, "bot_binding_modal.hint.current_bound_none").to_string()
            ];
        }
        self.room_bound_bots
            .iter()
            .map(|binding| {
                let remark = binding.remark.trim();
                if remark.is_empty() {
                    binding.bot_user_id.as_str().to_string()
                } else {
                    format!("{} ({})", binding.bot_user_id.as_str(), remark)
                }
            })
            .collect()
    }

    fn update_room_bound_bots_value(&mut self, cx: &mut Cx) {
        self.view
            .drop_down(cx, ids!(form.current_room_bots_dropdown))
            .set_labels(cx, self.room_bound_bot_labels());
    }

    fn update_static_texts(&mut self, cx: &mut Cx) {
        self.view
            .label(cx, ids!(form.current_room_bots_label))
            .set_text(cx, tr_key(self.app_language, "bot_binding_modal.label.current_room_bots"));
        self.view
            .label(cx, ids!(form.known_bots_label))
            .set_text(cx, tr_key(self.app_language, "bot_binding_modal.label.known_bots"));
        self.view
            .label(cx, ids!(form.user_id_label))
            .set_text(cx, tr_key(self.app_language, "bot_binding_modal.label.user_id"));
        self.view
            .label(cx, ids!(form.remark_label))
            .set_text(cx, tr_key(self.app_language, "bot_binding_modal.label.remark"));
        self.view
            .text_input(cx, ids!(form.user_id_input))
            .set_empty_text(
                cx,
                tr_key(self.app_language, "bot_binding_modal.input.placeholder").to_string(),
            );
        self.view
            .text_input(cx, ids!(form.remark_input))
            .set_empty_text(
                cx,
                tr_key(self.app_language, "bot_binding_modal.input.remark_placeholder").to_string(),
            );
        self.view
            .button(cx, ids!(form.remark_controls.save_remark_button))
            .set_text(cx, tr_key(self.app_language, "bot_binding_modal.button.save_remark"));
        self.view
            .button(cx, ids!(buttons.cancel_button))
            .set_text(cx, tr_key(self.app_language, "bot_binding_modal.button.cancel"));
        self.view
            .button(cx, ids!(buttons.bind_button))
            .set_text(cx, tr_key(self.app_language, "bot_binding_modal.button.bind"));
        self.view
            .button(cx, ids!(buttons.unbind_button))
            .set_text(cx, tr_key(self.app_language, "bot_binding_modal.button.unbind"));
        self.view
            .drop_down(cx, ids!(form.known_bots_dropdown))
            .set_labels(cx, self.known_bot_labels());
    }

    pub fn show(
        &mut self,
        cx: &mut Cx,
        room_name_id: RoomNameId,
        bot_settings: &BotSettingsState,
        app_language: AppLanguage,
    ) {
        self.app_language = app_language;
        self.room_bound_bots = bot_settings.room_bindings_for(room_name_id.room_id());
        self.known_bot_user_ids = bot_settings.known_bot_user_ids();
        for bound_bot_user_id in bot_settings.all_bound_bot_user_ids() {
            if !self
                .known_bot_user_ids
                .iter()
                .any(|known_bot_user_id| known_bot_user_id.as_str() == bound_bot_user_id.as_str())
            {
                self.known_bot_user_ids.push(bound_bot_user_id);
            }
        }
        self.known_bot_user_ids
            .sort_by(|lhs, rhs| lhs.as_str().cmp(rhs.as_str()));
        self.known_bot_user_ids
            .dedup_by(|lhs, rhs| lhs.as_str() == rhs.as_str());
        self.room_name_id = Some(room_name_id.clone());

        self.set_title_and_body(cx, &room_name_id);
        self.update_static_texts(cx);
        self.update_room_bound_bots_value(cx);

        let current_room_bots_dropdown = self.view.drop_down(cx, ids!(form.current_room_bots_dropdown));
        let known_bots_dropdown = self.view.drop_down(cx, ids!(form.known_bots_dropdown));
        let user_id_input = self.view.text_input(cx, ids!(form.user_id_input));
        let remark_input = self.view.text_input(cx, ids!(form.remark_input));
        let selected_item = self
            .room_bound_bots
            .first()
            .and_then(|binding|
                self.known_bot_user_ids
                    .iter()
                    .position(|known_bot_user_id| known_bot_user_id.as_str() == binding.bot_user_id.as_str())
            )
            .map_or(0, |index| index + 1);
        current_room_bots_dropdown.set_selected_item(
            cx,
            if self.room_bound_bots.is_empty() { 0 } else { 1 },
        );
        known_bots_dropdown.set_selected_item(cx, selected_item);
        if let Some(bound_bot) = self.room_bound_bots.first() {
            user_id_input.set_text(cx, bound_bot.bot_user_id.as_str());
            remark_input.set_text(cx, &bound_bot.remark);
        } else {
            user_id_input.set_text(cx, "");
            remark_input.set_text(cx, "");
        }
        user_id_input.set_is_read_only(cx, false);
        user_id_input.set_key_focus(cx);
        self.view.label(cx, ids!(status_label)).set_text(cx, "");
        self.view.button(cx, ids!(buttons.bind_button)).set_enabled(cx, true);
        self.view.button(cx, ids!(buttons.unbind_button)).set_enabled(cx, true);
        self.view.button(cx, ids!(buttons.cancel_button)).set_enabled(cx, true);
        self.view.button(cx, ids!(form.remark_controls.save_remark_button)).set_enabled(cx, true);
        self.view.button(cx, ids!(buttons.bind_button)).reset_hover(cx);
        self.view.button(cx, ids!(buttons.unbind_button)).reset_hover(cx);
        self.view.button(cx, ids!(buttons.cancel_button)).reset_hover(cx);
        self.view.button(cx, ids!(form.remark_controls.save_remark_button)).reset_hover(cx);
        self.view.redraw(cx);
    }
}

impl BotBindingModalRef {
    pub fn show(
        &self,
        cx: &mut Cx,
        room_name_id: RoomNameId,
        bot_settings: &BotSettingsState,
        app_language: AppLanguage,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, room_name_id, bot_settings, app_language);
    }
}

fn persist_bot_settings(app_state: &AppState) {
    if let Some(user_id) = current_user_id() {
        if let Err(e) = persistence::save_app_state(app_state.clone(), user_id) {
            error!("Failed to persist bot settings. Error: {e}");
        }
    }
}
