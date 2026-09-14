//! The timeline card that renders an agent-chat owner approval request.
//!
//! One card sits (hidden by default) inside every `Message` widget's content
//! column. `populate_message_view` calls [`AgentApprovalCardRef::set_state`]
//! on it for every message: with `None` for ordinary messages, which hides it,
//! or with the parsed request plus its current decision state.
//!
//! Button clicks surface through [`AgentApprovalCardRef::clicked_action`],
//! which the enclosing `Message` turns into a `MessageAction` for the
//! `RoomScreen` to act on. The card itself holds no protocol state beyond the
//! actions it is currently showing.

use makepad_widgets::*;

use super::approval::{ApprovalAction, ApprovalDecisionState, ApprovalRequest};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // Decision buttons follow robrix2's AgentApproval{Primary,Secondary,Danger}Button
    // recipes: a tinted surface with a 1px semantic stroke, RBX_CONTROL_H_MD tall.
    mod.widgets.AgentApprovalPrimaryButton = Button {
        visible: false
        width: Fit
        height: (mod.widgets.RBX_CONTROL_H_MD)
        spacing: 0
        padding: Inset{left: 12.0, right: 12.0, top: 7.0, bottom: 7.0}
        icon_walk: Walk{width: 0, height: 0}
        draw_bg +: {
            color: (mod.widgets.RBX_SUCCESS_BG)
            color_hover: (mod.widgets.RBX_SUCCESS_BG)
            color_down: (mod.widgets.RBX_BG_PRESSED)
            color_disabled: (mod.widgets.RBX_BG_DISABLED)
            border_radius: (mod.widgets.RBX_RADIUS_MD)
            border_size: 1.0
            border_color: (mod.widgets.RBX_SUCCESS_FG)
            border_color_hover: (mod.widgets.RBX_SUCCESS_FG)
            border_color_down: (mod.widgets.RBX_SUCCESS_FG)
        }
        draw_text +: {
            text_style: (mod.widgets.RBX_TEXT_BODY_STRONG)
            color: (mod.widgets.RBX_SUCCESS_FG)
            color_hover: (mod.widgets.RBX_SUCCESS_FG)
            color_down: (mod.widgets.RBX_SUCCESS_FG)
            color_disabled: (mod.widgets.RBX_FG_DISABLED)
        }
        text: ""
    }

    mod.widgets.AgentApprovalSecondaryButton = Button {
        visible: false
        width: Fit
        height: (mod.widgets.RBX_CONTROL_H_MD)
        spacing: 0
        padding: Inset{left: 12.0, right: 12.0, top: 7.0, bottom: 7.0}
        icon_walk: Walk{width: 0, height: 0}
        draw_bg +: {
            color: (mod.widgets.RBX_BG_SURFACE)
            color_hover: (mod.widgets.RBX_BG_HOVER)
            color_down: (mod.widgets.RBX_BG_PRESSED)
            color_disabled: (mod.widgets.RBX_BG_DISABLED)
            border_radius: (mod.widgets.RBX_RADIUS_MD)
            border_size: 1.0
            border_color: (mod.widgets.RBX_STROKE_STRONG)
            border_color_hover: (mod.widgets.RBX_STROKE_STRONG)
            border_color_down: (mod.widgets.RBX_STROKE_STRONG)
        }
        draw_text +: {
            text_style: (mod.widgets.RBX_TEXT_BODY_STRONG)
            color: (mod.widgets.RBX_FG_SECONDARY)
            color_hover: (mod.widgets.RBX_FG_PRIMARY)
            color_down: (mod.widgets.RBX_FG_PRIMARY)
            color_disabled: (mod.widgets.RBX_FG_DISABLED)
        }
        text: ""
    }

    mod.widgets.AgentApprovalDangerButton = Button {
        visible: false
        width: Fit
        height: (mod.widgets.RBX_CONTROL_H_MD)
        spacing: 0
        padding: Inset{left: 12.0, right: 12.0, top: 7.0, bottom: 7.0}
        icon_walk: Walk{width: 0, height: 0}
        draw_bg +: {
            color: (mod.widgets.RBX_DANGER_BG)
            color_hover: (mod.widgets.RBX_DANGER_BG)
            color_down: (mod.widgets.RBX_BG_PRESSED)
            color_disabled: (mod.widgets.RBX_BG_DISABLED)
            border_radius: (mod.widgets.RBX_RADIUS_MD)
            border_size: 1.0
            border_color: (mod.widgets.RBX_DANGER_FG)
            border_color_hover: (mod.widgets.RBX_DANGER_FG)
            border_color_down: (mod.widgets.RBX_DANGER_FG)
        }
        draw_text +: {
            text_style: (mod.widgets.RBX_TEXT_BODY_STRONG)
            color: (mod.widgets.RBX_DANGER_FG)
            color_hover: (mod.widgets.RBX_DANGER_FG)
            color_down: (mod.widgets.RBX_DANGER_FG)
            color_disabled: (mod.widgets.RBX_FG_DISABLED)
        }
        text: ""
    }

    // The badge after an agent's name, sized exactly like robrix2's bot badge:
    // 16px tall, 6px side padding, 3px radius. Accent-tinted for workflow roles;
    // the neutral pair is applied at populate time for agents with no role.
    mod.widgets.AgentBadge = RoundedView {
        visible: false
        width: Fit
        height: 16.0
        align: Align{x: 0.5, y: 0.5}
        margin: Inset{top: 20.0, right: 10.0}
        padding: Inset{left: 6.0, right: 6.0}
        show_bg: true
        draw_bg +: {
            color: (mod.widgets.RBX_ACCENT_SOFT)
            border_radius: 3.0
        }
        agent_badge_label := Label {
            width: Fit, height: Fit
            padding: 0
            draw_text +: {
                text_style: theme.font_regular { font_size: 8.5, top_drop: -0.08 }
                color: (mod.widgets.RBX_ACCENT)
            }
            text: ""
        }
    }

    mod.widgets.AgentApprovalCard = #(AgentApprovalCard::register_widget(vm)) {
        visible: false
        width: Fill, height: Fit
        flow: Down
        spacing: 8.0
        margin: Inset{top: 8.0, right: 10.0}
        padding: Inset{left: 16.0, right: 16.0, top: 12.0, bottom: 16.0}

        show_bg: true
        draw_bg +: {
            color: (mod.widgets.RBX_WARNING_BG)
            border_radius: (mod.widgets.RBX_RADIUS_SM)
            border_size: 1.0
            border_color: (mod.widgets.RBX_WARNING_FG)
        }

        header := View {
            width: Fill, height: Fit
            flow: Right
            spacing: 8.0
            align: Align{y: 0.5}

            title_label := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                padding: 0
                draw_text +: {
                    text_style: (mod.widgets.RBX_TEXT_CARD_TITLE)
                    color: (mod.widgets.RBX_WARNING_FG)
                }
                text: ""
            }

            status_badge := RoundedView {
                width: Fit, height: Fit
                padding: Inset{left: 8.0, right: 8.0, top: 4.0, bottom: 4.0}
                show_bg: true
                draw_bg +: {
                    color: (mod.widgets.RBX_BG_SURFACE)
                    border_radius: (mod.widgets.RBX_RADIUS_PILL)
                }
                status_label := Label {
                    width: Fit, height: Fit
                    padding: 0
                    draw_text +: {
                        text_style: (mod.widgets.RBX_TEXT_BADGE)
                        color: (mod.widgets.RBX_WARNING_FG)
                    }
                    text: ""
                }
            }
        }

        summary_label := Label {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            padding: 0
            draw_text +: {
                text_style: (mod.widgets.RBX_TEXT_BODY)
                color: (mod.widgets.RBX_FG_PRIMARY)
            }
            text: ""
        }

        button_row := View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            spacing: 8.0

            approve_once_button := mod.widgets.AgentApprovalPrimaryButton {}
            approve_task_button := mod.widgets.AgentApprovalSecondaryButton {}
            approve_always_button := mod.widgets.AgentApprovalSecondaryButton {}
            deny_button := mod.widgets.AgentApprovalDangerButton {}
        }

        hint_label := Label {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            padding: 0
            draw_text +: {
                text_style: (mod.widgets.RBX_TEXT_META)
                color: (mod.widgets.RBX_FG_SECONDARY)
            }
            text: "Text replies are not approval. Only these buttons send a structured verdict, and the agent-chat server makes the final decision."
        }
    }
}

/// Everything the card needs to draw one approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalCardState {
    pub title: String,
    pub summary: String,
    pub decision: ApprovalDecisionState,
    pub actions: Vec<ApprovalAction>,
}

impl ApprovalCardState {
    /// Builds the card state for `request` in the given decision state.
    pub fn new(request: &ApprovalRequest, decision: ApprovalDecisionState) -> Self {
        Self {
            title: request.title(),
            summary: request.summary(),
            decision,
            actions: request.actions.clone(),
        }
    }
}

/// The four button slots, in the order hagency lists them.
const BUTTON_SLOTS: [(&str, &[LiveId]); 4] = [
    ("approve_once", ids!(button_row.approve_once_button)),
    ("approve_task", ids!(button_row.approve_task_button)),
    ("approve_always", ids!(button_row.approve_always_button)),
    ("deny", ids!(button_row.deny_button)),
];

/// A card showing an agent-chat approval request with its decision buttons.
#[derive(Script, ScriptHook, Widget)]
pub struct AgentApprovalCard {
    #[deref] view: View,
    /// The actions whose buttons are currently visible, so a click can be
    /// mapped back to the action the bridge asked for.
    #[rust] actions: Vec<ApprovalAction>,
}

impl Widget for AgentApprovalCard {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AgentApprovalCard {
    /// Shows the card for `state`, or hides it entirely when `state` is `None`.
    pub fn set_state(&mut self, cx: &mut Cx, state: Option<&ApprovalCardState>) {
        let Some(state) = state else {
            self.actions.clear();
            self.view.set_visible(cx, false);
            return;
        };
        self.view.set_visible(cx, true);
        self.view.label(cx, ids!(header.title_label)).set_text(cx, &state.title);
        self.view.label(cx, ids!(summary_label)).set_text(cx, &state.summary);

        let (status_text, live_buttons, chosen): (&str, bool, Option<&ApprovalAction>) = match &state.decision {
            ApprovalDecisionState::Pending => ("Pending", true, None),
            ApprovalDecisionState::Expired => ("Expired", false, None),
            ApprovalDecisionState::Sending(action) => ("Sending…", false, Some(action)),
            ApprovalDecisionState::Sent(action) => ("Decided", false, Some(action)),
        };
        self.view.label(cx, ids!(header.status_badge.status_label)).set_text(cx, status_text);

        self.actions = if live_buttons { state.actions.clone() } else { Vec::new() };
        let mut any_button_visible = false;
        for (id, path) in BUTTON_SLOTS {
            let button = self.view.button(cx, path);
            let offered = state.actions.iter().find(|action| action.id == id);
            let (visible, text, enabled) = match (offered, chosen) {
                // After a decision, keep only the chosen button as a disabled receipt.
                (Some(action), Some(chosen_action)) if chosen_action.id == id => {
                    (true, format!("✓ {}", action.label), false)
                }
                (Some(_), Some(_)) => (false, String::new(), false),
                (Some(action), None) => (live_buttons, action.label.clone(), live_buttons),
                (None, _) => (false, String::new(), false),
            };
            button.set_visible(cx, visible);
            if visible {
                any_button_visible = true;
                button.set_text(cx, &text);
                button.set_enabled(cx, enabled);
                if enabled {
                    // A recycled portal-list item may carry stale hover state.
                    button.reset_hover(cx);
                }
            }
        }
        self.view.view(cx, ids!(button_row)).set_visible(cx, any_button_visible);
        self.view.label(cx, ids!(hint_label)).set_visible(cx, live_buttons);
        self.view.redraw(cx);
    }

    /// Returns the action whose button was clicked in `actions`, if any.
    fn clicked_action(&self, cx: &mut Cx, actions: &Actions) -> Option<ApprovalAction> {
        if self.actions.is_empty() {
            return None;
        }
        for (id, path) in BUTTON_SLOTS {
            if self.view.button(cx, path).clicked(actions) {
                return self.actions.iter().find(|action| action.id == id).cloned();
            }
        }
        None
    }
}

impl AgentApprovalCardRef {
    /// See [`AgentApprovalCard::set_state`].
    pub fn set_state(&self, cx: &mut Cx, state: Option<&ApprovalCardState>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_state(cx, state);
        }
    }

    /// Returns the action whose button was clicked in `actions`, if any.
    pub fn clicked_action(&self, cx: &mut Cx, actions: &Actions) -> Option<ApprovalAction> {
        self.borrow().and_then(|inner| inner.clicked_action(cx, actions))
    }
}
