//! A simple modal that displays basic confirmation (yes/no) dialog.

use std::borrow::Cow;

use makepad_widgets::*;


live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::RobrixIconButton;

    // A confirmation modal with no icons in the buttons.
    // The accept button is blue and the cancel button is gray.
    pub ConfirmationModal = {{ConfirmationModal}} {
        width: Fit
        height: Fit

        wrapper = <RoundedView> {
            width: 400
            height: Fit
            align: {x: 0.5}
            flow: Down
            padding: {top: 30, right: 40, bottom: 20, left: 40}

            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY)
                border_radius: 4
            }

            title_view = <View> {
                width: Fill,
                height: Fit,
                padding: {top: 0, bottom: 25}
                align: {x: 0.5, y: 0.0}

                title = <Label> {
                    flow: RightWrap,
                    draw_text: {
                        text_style: <TITLE_TEXT>{font_size: 13},
                        color: #000
                        wrap: Word
                    }
                }
            }

            body_view = <View> {
                width: Fill, height: Fit
                align: {x: 0.5, y: 0.0}

                body = <Label> {
                    width: Fill, height: Fit
                    flow: RightWrap,
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 11.5},
                        color: #000
                        wrap: Word
                    }
                }
            }

            buttons_view = <View> {
                width: Fill, height: Fit
                flow: Right,
                padding: {top: 20, bottom: 20}
                margin: {right: -15}
                align: {x: 1.0, y: 0.5}
                spacing: 20

                cancel_button = <RobrixIconButton> {
                    width: 120,
                    align: {x: 0.5, y: 0.5}
                    padding: 15,
                    icon_walk: {width: 0, height: 0, margin: 0}
    
                    draw_bg: {
                        border_size: 1.0
                        border_color: (COLOR_BG_DISABLED),
                        color: (COLOR_SECONDARY)
                    }
                    draw_text: {
                        color: (COLOR_TEXT),
                    }
                    text: "Cancel"
                }

                accept_button = <RobrixIconButton> {
                    width: 120
                    align: {x: 0.5, y: 0.5}
                    padding: 15,
                    icon_walk: {width: 0, height: 0, margin: 0}

                    draw_bg: {
                        border_size: 1.0
                        border_color: (COLOR_ACTIVE_PRIMARY_DARKER),
                        color: (COLOR_ACTIVE_PRIMARY)
                    }
                    draw_text: {
                        color: (COLOR_PRIMARY),
                    }
                    text: "Confirm"
                }
            }
        }
    }

    // A confirmation modal for a positive action.
    // The accept button is green with a checkmark icon.
    pub PositiveConfirmationModal = <ConfirmationModal> {
        wrapper = { 
            buttons_view = {
                cancel_button = {
                    draw_icon: {
                        svg_file: (ICON_FORBIDDEN)
                        color: (COLOR_TEXT),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }
                }
                accept_button = {
                    draw_icon: {
                        svg_file: (ICON_CHECKMARK)
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_FG_ACCEPT_GREEN),
                        color: (COLOR_BG_ACCEPT_GREEN)
                    }
                    draw_text: {
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                }
            }
        }
    }

    // A confirmation modal for a negative action.
    // The accept button is red with a forbidden icon.
    pub NegativeConfirmationModal = <ConfirmationModal> {
        wrapper = {
            buttons_view = {
                cancel_button = {
                    draw_icon: {
                        svg_file: (ICON_FORBIDDEN)
                        color: (COLOR_TEXT),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }
                }
                accept_button = {
                    draw_icon: {
                        svg_file: (ICON_CLOSE)
                        color: (COLOR_FG_DANGER_RED),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_FG_DANGER_RED),
                        color: (COLOR_BG_DANGER_RED)
                    }
                    draw_text: {
                        color: (COLOR_FG_DANGER_RED),
                    }
                }
            }
        }
    }
}

/// Widget actions emitted by the ConfirmationModal.
#[derive(Clone, Copy, Debug, DefaultNone)]
pub enum ConfirmationModalAction {
    /// Emitted by this modal when it should be closed after the user clicked a button.
    ///
    /// The contained boolean indicates whether the user clicked the
    /// accept button (true) or cancel button (false).
    Close(bool),
    None
}

/// Defines the content and behavior of a confirmation modal.
///
/// Only the title and body text are required.
/// Everything else can be left as default values like so:
/// ```rust,no_run
/// let content = ConfirmationModalContent {
///     title_text: "Confirm deletion".into()
///     body_text: "Are you sure you want to delete this file?".into()
///     ..Default::default()
/// };
/// ```
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct ConfirmationModalContent {
    /// The title text of the modal, shown at the top.
    pub title_text: Cow<'static, str>,
    /// The body text of the modal, shown below the title and above the buttons.
    pub body_text: Cow<'static, str>,
    /// The text for the accept button.
    /// If `None`, the button's default text of "Confirm" will be shown.
    pub accept_button_text: Option<Cow<'static, str>>,
    /// The text for the cancel button.
    /// If `None`, the button's default text of "Cancel" will be shown.
    pub cancel_button_text: Option<Cow<'static, str>>,
    /// A callback to be called when the accept button is clicked.
    pub on_accept_clicked: Option<Box<dyn FnOnce(&mut Cx)>>,
    /// A callback to be called when the cancel button is clicked.
    pub on_cancel_clicked: Option<Box<dyn FnOnce(&mut Cx)>>,
}
impl std::fmt::Debug for ConfirmationModalContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmationModalContent")
            .field("title", &self.title_text)
            .field("body", &self.body_text)
            .field("accept_button", &self.accept_button_text)
            .field("cancel_button", &self.cancel_button_text)
            .field("on_accept_clicked", &self.on_accept_clicked.is_some())
            .field("on_cancel_clicked", &self.on_cancel_clicked.is_some())
            .finish()
    }
}


#[derive(Live, LiveHook, Widget)]
pub struct ConfirmationModal {
    #[deref] view: View,
    #[rust] content: ConfirmationModalContent,
}

impl Widget for ConfirmationModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for ConfirmationModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let accept_button = self.view.button(ids!(accept_button));
        let cancel_button = self.view.button(ids!(cancel_button));

        // Handle canceling/closing the modal.
        let cancel_clicked = cancel_button.clicked(actions);
        if cancel_clicked ||
            actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // a `ConfirmationModalAction::Close` action, as that would cause
            // an infinite action feedback loop.
            if cancel_clicked {
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    ConfirmationModalAction::Close(false),
                );
            }
            if let Some(on_cancel_clicked) = self.content.on_cancel_clicked.take() {
                on_cancel_clicked(cx);
            }
            return;
        }

        // If the accept button was clicked, emit the action and call the on_accept callback.
        if accept_button.clicked(actions) {
            if let Some(on_accept_clicked) = self.content.on_accept_clicked.take() {
                on_accept_clicked(cx);
            }
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                ConfirmationModalAction::Close(true),
            );
        }
    }
}

impl ConfirmationModal {
    pub fn show(&mut self, cx: &mut Cx, content: ConfirmationModalContent) {
        self.content = content;
        self.apply_content(cx);
    }

    fn apply_content(&mut self, cx: &mut Cx) {
        self.view.label(ids!(title)).set_text(cx, &self.content.title_text);
        self.view.label(ids!(body)).set_text(cx, &self.content.body_text);
        self.view.button(ids!(accept_button)).set_text(
            cx,
            self.content.accept_button_text.as_deref().unwrap_or("Confirm"),
        );
        self.view.button(ids!(cancel_button)).set_text(
            cx,
            self.content.cancel_button_text.as_deref().unwrap_or("Cancel"),
        );

        self.view.button(ids!(cancel_button)).reset_hover(cx);
        self.view.button(ids!(accept_button)).reset_hover(cx);
        self.view.button(ids!(accept_button)).set_enabled(cx, true);
        self.view.button(ids!(cancel_button)).set_enabled(cx, true);
        self.view.redraw(cx);
    }
}

impl ConfirmationModalRef {
    /// Shows the confirmation modal with the given content.
    pub fn show(&self, cx: &mut Cx, content: ConfirmationModalContent) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, content);
    }

    /// Returns `Some(bool)` if this modal was closed by one of the given `actions`.
    ///
    /// If `true`, the user clicked the accept button; if `false`, the user clicked the cancel button.
    /// See [`ConfirmationModalAction::Close`] for more.
    pub fn closed(&self, actions: &Actions) -> Option<bool> {
        if let ConfirmationModalAction::Close(accepted) = actions.find_widget_action(self.widget_uid()).cast_ref() {
            Some(*accepted)
        } else {
            None
        }
    }
}
