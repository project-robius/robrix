//! Modal used to forward a message to another room.

use makepad_widgets::*;
use matrix_sdk::ruma::{
    OwnedEventId, OwnedRoomId, RoomId,
    events::room::message::RoomMessageEventContent,
};

use crate::{
    i18n::{AppLanguage, tr_key},
    shared::popup_list::{PopupKind, enqueue_popup_notification},
    sliding_sync::{MatrixRequest, submit_async_request},
};

type ForwardMessageCloseHandler = Box<dyn FnOnce()>;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ForwardMessageModal = #(ForwardMessageModal::register_widget(vm)) {
        width: Fit
        height: Fit

        wrapper := RoundedView {
            width: 420
            height: Fit
            align: Align{x: 0.5}
            flow: Down
            padding: Inset{top: 26, right: 32, bottom: 20, left: 32}
            spacing: 12

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 4.0
            }

            title := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                text: "Forward Message"
                draw_text +: {
                    text_style: TITLE_TEXT {font_size: 13}
                    color: #000
                }
            }

            body := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                text: "Enter the destination Matrix room ID."
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 11.5}
                    color: #000
                }
            }

            destination_room_id_input := RobrixTextInput {
                width: Fill
                height: Fit
                empty_text: "!room:example.org"
                padding: 8
                flow: Flow.Right{wrap: false}
                draw_bg.border_size: 0.0
            }

            error_label := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                text: ""
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 10}
                    color: (COLOR_FG_DANGER_RED)
                }
            }

            buttons_view := View {
                width: Fill
                height: Fit
                flow: Right
                padding: Inset{top: 8, bottom: 4}
                align: Align{x: 1.0, y: 0.5}
                spacing: 14

                cancel_button := RobrixNeutralIconButton {
                    width: 120
                    align: Align{x: 0.5, y: 0.5}
                    padding: 15
                    draw_icon +: { svg: (ICON_FORBIDDEN) }
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Cancel"
                }

                forward_button := RobrixPositiveIconButton {
                    width: 120
                    align: Align{x: 0.5, y: 0.5}
                    padding: 15
                    draw_icon +: { svg: (ICON_SEND) }
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Forward"
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ForwardMessageContent {
    pub source_room_id: OwnedRoomId,
    pub source_event_id: OwnedEventId,
    pub message: RoomMessageEventContent,
}

#[derive(Clone, Debug, Default)]
pub enum ForwardMessageModalAction {
    Open(ForwardMessageContent),
    Close,
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForwardModalCloseEffect {
    None,
    ClearOnly,
    EmitClose,
}

pub fn forward_modal_close_effect(
    cancel_clicked: bool,
    escape_pressed: bool,
    passive_dismissed: bool,
) -> ForwardModalCloseEffect {
    if passive_dismissed {
        ForwardModalCloseEffect::ClearOnly
    } else if cancel_clicked || escape_pressed {
        ForwardModalCloseEffect::EmitClose
    } else {
        ForwardModalCloseEffect::None
    }
}

pub fn build_forward_message_request(
    content: ForwardMessageContent,
    destination_room_id: OwnedRoomId,
) -> MatrixRequest {
    MatrixRequest::ForwardMessage {
        source_room_id: content.source_room_id,
        source_event_id: content.source_event_id,
        destination_room_id,
        message: content.message,
    }
}

impl ActionDefaultRef for ForwardMessageModalAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: ForwardMessageModalAction = ForwardMessageModalAction::None;
        &DEFAULT
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct ForwardMessageModal {
    #[deref] view: View,
    #[rust] content: Option<ForwardMessageContent>,
    #[rust] app_language: AppLanguage,
    #[rust] close_actions_emitted: usize,
}

impl Widget for ForwardMessageModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for ForwardMessageModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let destination_input = self.view.text_input(cx, ids!(destination_room_id_input));
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        let forward_button = self.view.button(cx, ids!(forward_button));

        let passive_dismissed = actions.iter()
            .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)));
        match forward_modal_close_effect(
            cancel_button.clicked(actions),
            destination_input.escaped(actions),
            passive_dismissed,
        ) {
            ForwardModalCloseEffect::ClearOnly => {
                self.clear_pending_message(cx);
                return;
            }
            ForwardModalCloseEffect::EmitClose => {
                self.emit_close(cx, None);
                return;
            }
            ForwardModalCloseEffect::None => {}
        }

        if destination_input.changed(actions).is_some() {
            self.clear_error(cx);
        }

        if forward_button.clicked(actions) || destination_input.returned(actions).is_some() {
            let Some(content) = self.content.clone() else {
                self.emit_close(cx, None);
                return;
            };
            let destination_room_id_text = destination_input.text().trim().to_string();
            let destination_room_id = match parse_destination_room_id(&destination_room_id_text) {
                Ok(room_id) => room_id,
                Err(error) => {
                    self.show_error(cx, &error);
                    return;
                }
            };
            let submit_handler: ForwardMessageCloseHandler = Box::new(move || {
                submit_async_request(build_forward_message_request(content, destination_room_id));
            });
            enqueue_popup_notification(
                tr_key(self.app_language, "forward_modal.popup.submitting"),
                PopupKind::Info,
                Some(3.0),
            );
            self.emit_close(cx, Some(submit_handler));
        }
    }
}

impl ForwardMessageModal {
    pub fn show(&mut self, cx: &mut Cx, content: ForwardMessageContent, app_language: AppLanguage) {
        self.content = Some(content);
        self.app_language = app_language;
        self.close_actions_emitted = 0;
        self.apply_static_text(cx);
        self.clear_error(cx);
        let input = self.view.text_input(cx, ids!(destination_room_id_input));
        input.set_text(cx, "");
        input.set_key_focus(cx);
        self.view.button(cx, ids!(cancel_button)).reset_hover(cx);
        self.view.button(cx, ids!(forward_button)).reset_hover(cx);
        self.redraw(cx);
    }

    fn apply_static_text(&mut self, cx: &mut Cx) {
        self.view.label(cx, ids!(title))
            .set_text(cx, tr_key(self.app_language, "forward_modal.title"));
        self.view.label(cx, ids!(body))
            .set_text(cx, tr_key(self.app_language, "forward_modal.body"));
        self.view.text_input(cx, ids!(destination_room_id_input))
            .set_empty_text(cx, tr_key(self.app_language, "forward_modal.input.destination_room_id").to_string());
        self.view.button(cx, ids!(cancel_button))
            .set_text(cx, tr_key(self.app_language, "forward_modal.button.cancel"));
        self.view.button(cx, ids!(forward_button))
            .set_text(cx, tr_key(self.app_language, "forward_modal.button.forward"));
    }

    fn show_error(&mut self, cx: &mut Cx, error: &str) {
        self.view.label(cx, ids!(error_label)).set_text(cx, error);
        self.redraw(cx);
    }

    fn clear_error(&mut self, cx: &mut Cx) {
        self.view.label(cx, ids!(error_label)).set_text(cx, "");
        self.redraw(cx);
    }

    fn clear_pending_message(&mut self, cx: &mut Cx) {
        self.content = None;
        self.clear_error(cx);
    }

    fn emit_close(&mut self, cx: &mut Cx, close_handler: Option<ForwardMessageCloseHandler>) {
        if let Some(close_handler) = close_handler {
            close_handler();
        }
        self.clear_pending_message(cx);
        self.close_actions_emitted += 1;
        cx.action(ForwardMessageModalAction::Close);
    }
}

impl ForwardMessageModalRef {
    pub fn show(&self, cx: &mut Cx, content: ForwardMessageContent, app_language: AppLanguage) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, content, app_language);
    }
}

pub fn parse_destination_room_id(value: &str) -> Result<OwnedRoomId, String> {
    if value.trim().is_empty() {
        return Err("Please enter a destination room ID.".to_string());
    }
    RoomId::parse(value.trim())
        .map_err(|_| "Please enter a valid Matrix room ID, such as !room:example.org.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_invalid_room_id() {
        assert!(parse_destination_room_id("").is_err());
        assert!(parse_destination_room_id("not-a-room").is_err());
        assert!(parse_destination_room_id("@alice:example.org").is_err());
    }

    #[test]
    fn test_forward_submit_request_accepts_room_id() {
        let room_id = parse_destination_room_id("!dest:example.org").unwrap();
        assert_eq!(room_id.as_str(), "!dest:example.org");
    }

    #[test]
    fn test_forward_submit_request() {
        let source_room_id = OwnedRoomId::try_from("!source:example.org").unwrap();
        let source_event_id = OwnedEventId::try_from("$event:example.org").unwrap();
        let destination_room_id = OwnedRoomId::try_from("!dest:example.org").unwrap();
        let request = build_forward_message_request(
            ForwardMessageContent {
                source_room_id: source_room_id.clone(),
                source_event_id: source_event_id.clone(),
                message: RoomMessageEventContent::text_plain("hello"),
            },
            destination_room_id.clone(),
        );

        match request {
            MatrixRequest::ForwardMessage {
                source_room_id: actual_source_room_id,
                source_event_id: actual_source_event_id,
                destination_room_id: actual_destination_room_id,
                message,
            } => {
                assert_eq!(actual_source_room_id, source_room_id);
                assert_eq!(actual_source_event_id, source_event_id);
                assert_eq!(actual_destination_room_id, destination_room_id);
                assert!(matches!(message.msgtype, matrix_sdk::ruma::events::room::message::MessageType::Text(..)));
            }
            _ => panic!("expected MatrixRequest::ForwardMessage"),
        }
    }

    #[test]
    fn test_forward_modal_opens() {
        let action = ForwardMessageModalAction::Open(ForwardMessageContent {
            source_room_id: OwnedRoomId::try_from("!source:example.org").unwrap(),
            source_event_id: OwnedEventId::try_from("$event:example.org").unwrap(),
            message: RoomMessageEventContent::text_plain("hello"),
        });

        assert!(matches!(action, ForwardMessageModalAction::Open(_)));
    }

    #[test]
    fn test_forward_cancel_no_feedback_loop() {
        assert_eq!(
            forward_modal_close_effect(true, false, false),
            ForwardModalCloseEffect::EmitClose,
        );
    }

    #[test]
    fn test_forward_escape_no_feedback_loop() {
        assert_eq!(
            forward_modal_close_effect(false, true, false),
            ForwardModalCloseEffect::EmitClose,
        );
    }

    #[test]
    fn test_forward_dismiss_no_feedback_loop() {
        assert_eq!(
            forward_modal_close_effect(false, false, true),
            ForwardModalCloseEffect::ClearOnly,
        );
    }
}
