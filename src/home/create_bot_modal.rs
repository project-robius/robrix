//! A modal dialog for creating a Matrix child bot through BotFather slash commands.

use makepad_widgets::*;

use crate::utils::RoomNameId;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.CreateBotModalLabel = Label {
        width: Fill
        height: Fit
        draw_text +: {
            text_style: REGULAR_TEXT { font_size: 10.5 }
            color: #333
            wrap: Word
        }
        text: ""
    }

    mod.widgets.CreateBotModal = #(CreateBotModal::register_widget(vm)) {
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
                text: "Create Bot"
            }

            body := mod.widgets.CreateBotModalLabel {
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

                username_label := mod.widgets.CreateBotModalLabel {
                    text: "Username"
                }

                username_input := RobrixTextInput {
                    width: Fill
                    height: Fit
                    padding: 10
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 11.5 }
                        color: #000
                    }
                    empty_text: "weather"
                }

                username_hint := mod.widgets.CreateBotModalLabel {
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 9.5 }
                        color: #666
                    }
                    text: "Lowercase letters, digits, and underscores only. BotFather will create @bot_<username>:server."
                }

                display_name_label := mod.widgets.CreateBotModalLabel {
                    text: "Display Name"
                }

                display_name_input := RobrixTextInput {
                    width: Fill
                    height: Fit
                    padding: 10
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 11.5 }
                        color: #000
                    }
                    empty_text: "Weather Bot"
                }

                prompt_label := mod.widgets.CreateBotModalLabel {
                    text: "System Prompt (Optional)"
                }

                prompt_input := RobrixTextInput {
                    width: Fill
                    height: Fit
                    padding: 10
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 11.5 }
                        color: #000
                    }
                    empty_text: "You are a weather assistant."
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

                create_button := RobrixPositiveIconButton {
                    width: 130
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_CHECKMARK)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Create Bot"
                }
            }
        }
    }
}

fn is_valid_bot_username(username: &str) -> bool {
    !username.is_empty()
        && username
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateBotRequest {
    pub username: String,
    pub display_name: String,
    pub system_prompt: Option<String>,
}

#[derive(Clone, Debug)]
pub enum CreateBotModalAction {
    Close,
    Submit(CreateBotRequest),
}

#[derive(Script, ScriptHook, Widget)]
pub struct CreateBotModal {
    #[deref]
    view: View,
    #[rust]
    room_name_id: Option<RoomNameId>,
    #[rust]
    is_showing_error: bool,
}

impl Widget for CreateBotModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for CreateBotModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let cancel_button = self.view.button(cx, ids!(buttons.cancel_button));
        let create_button = self.view.button(cx, ids!(buttons.create_button));
        let username_input = self.view.text_input(cx, ids!(form.username_input));
        let display_name_input = self.view.text_input(cx, ids!(form.display_name_input));
        let prompt_input = self.view.text_input(cx, ids!(form.prompt_input));
        let mut status_label = self.view.label(cx, ids!(status_label));

        if cancel_button.clicked(actions)
            || actions
                .iter()
                .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            cx.action(CreateBotModalAction::Close);
            return;
        }

        if self.is_showing_error
            && (username_input.changed(actions).is_some()
                || display_name_input.changed(actions).is_some()
                || prompt_input.changed(actions).is_some())
        {
            self.is_showing_error = false;
            status_label.set_text(cx, "");
            self.view.redraw(cx);
        }

        if create_button.clicked(actions) || prompt_input.returned(actions).is_some() {
            let username = username_input.text().trim().to_string();
            if !is_valid_bot_username(&username) {
                self.is_showing_error = true;
                script_apply_eval!(cx, status_label, {
                    text: "Username must use lowercase letters, digits, or underscores."
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_DANGER_RED
                    }
                });
                self.view.redraw(cx);
                return;
            }

            let display_name = display_name_input.text().trim().to_string();
            let system_prompt = prompt_input.text().trim().to_string();

            cx.action(CreateBotModalAction::Submit(CreateBotRequest {
                username: username.clone(),
                display_name: if display_name.is_empty() {
                    username
                } else {
                    display_name
                },
                system_prompt: (!system_prompt.is_empty()).then_some(system_prompt),
            }));
        }
    }
}

impl CreateBotModal {
    pub fn show(&mut self, cx: &mut Cx, room_name_id: RoomNameId) {
        self.room_name_id = Some(room_name_id.clone());
        self.is_showing_error = false;

        self.view
            .label(cx, ids!(title))
            .set_text(cx, "Create Room Bot");
        self.view.label(cx, ids!(body)).set_text(
            cx,
            &format!(
                "Robrix will send `/createbot` to BotFather in {}. The bot becomes available immediately after octos creates it.",
                room_name_id
            ),
        );
        self.view
            .text_input(cx, ids!(form.username_input))
            .set_text(cx, "");
        self.view
            .text_input(cx, ids!(form.display_name_input))
            .set_text(cx, "");
        self.view
            .text_input(cx, ids!(form.prompt_input))
            .set_text(cx, "");
        self.view.label(cx, ids!(status_label)).set_text(cx, "");
        self.view
            .button(cx, ids!(buttons.create_button))
            .set_enabled(cx, true);
        self.view
            .button(cx, ids!(buttons.cancel_button))
            .set_enabled(cx, true);
        self.view
            .button(cx, ids!(buttons.create_button))
            .reset_hover(cx);
        self.view
            .button(cx, ids!(buttons.cancel_button))
            .reset_hover(cx);
        self.view.redraw(cx);
    }
}

impl CreateBotModalRef {
    pub fn show(&self, cx: &mut Cx, room_name_id: RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show(cx, room_name_id);
    }
}
