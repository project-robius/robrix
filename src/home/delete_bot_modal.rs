//! A modal dialog for deleting a Matrix bot through BotFather slash commands.

use makepad_widgets::*;

use crate::utils::RoomNameId;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.DeleteBotModalLabel = Label {
        width: Fill
        height: Fit
        draw_text +: {
            text_style: REGULAR_TEXT { font_size: 10.5 }
            color: #333
            wrap: Word
        }
        text: ""
    }

    mod.widgets.DeleteBotModal = #(DeleteBotModal::register_widget(vm)) {
        width: Fit
        height: Fit

        RoundedView {
            width: 448
            height: Fit
            align: Align{x: 0.5}
            flow: Down
            padding: Inset{top: 28, right: 24, bottom: 20, left: 24}
            spacing: 16

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 6.0
            }

            title := Label {
                width: Fill
                height: Fit
                draw_text +: {
                    text_style: TITLE_TEXT { font_size: 13 }
                    color: #000
                    wrap: Word
                }
                text: "Delete Bot"
            }

            body := mod.widgets.DeleteBotModalLabel {
                text: ""
            }

            form := RoundedView {
                width: Fill
                height: Fit
                flow: Down
                spacing: 12
                padding: 14

                show_bg: true
                draw_bg +: {
                    color: (COLOR_SECONDARY)
                    border_radius: 4.0
                }

                user_id_label := mod.widgets.DeleteBotModalLabel {
                    text: "Bot Matrix User ID"
                }

                user_id_input := RobrixTextInput {
                    width: Fill
                    height: Fit
                    padding: 10
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 11.5 }
                        color: #000
                    }
                    empty_text: "@bot_weather:server or bot_weather"
                }

                user_id_hint := mod.widgets.DeleteBotModalLabel {
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 9.5 }
                        color: #666
                    }
                    text: "Use the full Matrix user ID when possible. A plain localpart like `bot_weather` will be resolved on your current homeserver."
                }
            }

            status_label := Label {
                width: Fill
                height: Fit
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10.5 }
                    color: #000
                    wrap: Word
                }
                text: ""
            }

            buttons := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{x: 1.0, y: 0.5}
                spacing: 16

                cancel_button := RobrixNeutralIconButton {
                    width: 110
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_FORBIDDEN)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Cancel"
                }

                delete_button := RobrixNegativeIconButton {
                    width: 130
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Delete Bot"
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteBotRequest {
    pub user_id_or_localpart: String,
}

#[derive(Clone, Debug)]
pub enum DeleteBotModalAction {
    Close,
    Submit(DeleteBotRequest),
}

#[derive(Script, ScriptHook, Widget)]
pub struct DeleteBotModal {
    #[deref]
    view: View,
    #[rust]
    is_showing_error: bool,
}

impl Widget for DeleteBotModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for DeleteBotModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let cancel_button = self.view.button(cx, ids!(buttons.cancel_button));
        let delete_button = self.view.button(cx, ids!(buttons.delete_button));
        let user_id_input = self.view.text_input(cx, ids!(form.user_id_input));
        let mut status_label = self.view.label(cx, ids!(status_label));

        if cancel_button.clicked(actions)
            || actions
                .iter()
                .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            cx.action(DeleteBotModalAction::Close);
            return;
        }

        if self.is_showing_error && user_id_input.changed(actions).is_some() {
            self.is_showing_error = false;
            status_label.set_text(cx, "");
            self.view.redraw(cx);
        }

        if delete_button.clicked(actions) || user_id_input.returned(actions).is_some() {
            let user_id_or_localpart = user_id_input.text().trim().to_string();
            if user_id_or_localpart.is_empty() {
                self.is_showing_error = true;
                script_apply_eval!(cx, status_label, {
                    text: "Enter the bot Matrix user ID or localpart to delete."
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_DANGER_RED
                    }
                });
                self.view.redraw(cx);
                return;
            }

            cx.action(DeleteBotModalAction::Submit(DeleteBotRequest {
                user_id_or_localpart,
            }));
        }
    }
}

impl DeleteBotModal {
    pub fn show(&mut self, cx: &mut Cx, room_name_id: RoomNameId) {
        self.is_showing_error = false;

        self.view
            .label(cx, ids!(title))
            .set_text(cx, "Delete Room Bot");
        self.view.label(cx, ids!(body)).set_text(
            cx,
            &format!(
                "Robrix will send `/deletebot` to BotFather in {}. This only removes bots already managed by octos.",
                room_name_id
            ),
        );
        self.view
            .text_input(cx, ids!(form.user_id_input))
            .set_text(cx, "");
        self.view.label(cx, ids!(status_label)).set_text(cx, "");
        self.view
            .button(cx, ids!(buttons.delete_button))
            .set_enabled(cx, true);
        self.view
            .button(cx, ids!(buttons.cancel_button))
            .set_enabled(cx, true);
        self.view
            .button(cx, ids!(buttons.delete_button))
            .reset_hover(cx);
        self.view
            .button(cx, ids!(buttons.cancel_button))
            .reset_hover(cx);
        self.view.redraw(cx);
    }
}

impl DeleteBotModalRef {
    pub fn show(&self, cx: &mut Cx, room_name_id: RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show(cx, room_name_id);
    }
}
