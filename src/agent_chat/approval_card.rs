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

    mod.widgets.COLOR_AGENT_APPROVAL_BG = #FFF4E5
    mod.widgets.COLOR_AGENT_APPROVAL_BORDER = #E8C99A
    mod.widgets.COLOR_AGENT_APPROVAL_TITLE = #A35A00

    // A text-only variant of the Robrix icon buttons: no icon slot, tighter padding.
    mod.widgets.AgentApprovalPositiveButton = mod.widgets.RobrixPositiveIconButton {
        visible: false
        spacing: 0
        padding: Inset{left: 12, right: 12, top: 7, bottom: 7}
        icon_walk: Walk{width: 0, height: 0}
        text: ""
    }
    mod.widgets.AgentApprovalNeutralButton = mod.widgets.RobrixNeutralIconButton {
        visible: false
        spacing: 0
        padding: Inset{left: 12, right: 12, top: 7, bottom: 7}
        icon_walk: Walk{width: 0, height: 0}
        text: ""
    }
    mod.widgets.AgentApprovalNegativeButton = mod.widgets.RobrixNegativeIconButton {
        visible: false
        spacing: 0
        padding: Inset{left: 12, right: 12, top: 7, bottom: 7}
        icon_walk: Walk{width: 0, height: 0}
        text: ""
    }

    // The small badge shown after an agent's name: its workflow role and,
    // when the bridge stamped one, the message kind (`coordinator · request`).
    mod.widgets.AgentBadge = RoundedView {
        visible: false
        width: Fit, height: Fit
        margin: Inset{top: 19.0, right: 10.0}
        padding: Inset{left: 6, right: 6, top: 2, bottom: 2}
        show_bg: true
        draw_bg +: {
            color: (mod.widgets.COLOR_AGENT_APPROVAL_BG)
            border_radius: 3.0
            border_size: 1.0
            border_color: (mod.widgets.COLOR_AGENT_APPROVAL_BORDER)
        }
        agent_badge_label := Label {
            width: Fit, height: Fit
            padding: 0
            draw_text +: {
                text_style: mod.widgets.REGULAR_TEXT { font_size: 9 },
                color: (mod.widgets.COLOR_AGENT_APPROVAL_TITLE)
            }
            text: ""
        }
    }

    mod.widgets.AgentApprovalCard = #(AgentApprovalCard::register_widget(vm)) {
        visible: false
        width: Fill, height: Fit
        flow: Down
        spacing: 8
        margin: Inset{top: 8, right: 10}
        padding: Inset{left: 14, right: 14, top: 10, bottom: 12}

        show_bg: true
        draw_bg +: {
            color: (mod.widgets.COLOR_AGENT_APPROVAL_BG)
            border_radius: 6.0
            border_size: 1.0
            border_color: (mod.widgets.COLOR_AGENT_APPROVAL_BORDER)
        }

        header := View {
            width: Fill, height: Fit
            flow: Right
            spacing: 8
            align: Align{y: 0.5}

            title_label := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                padding: 0
                draw_text +: {
                    text_style: USERNAME_TEXT_STYLE {},
                    color: (mod.widgets.COLOR_AGENT_APPROVAL_TITLE)
                }
                text: ""
            }

            status_badge := RoundedView {
                width: Fit, height: Fit
                padding: Inset{left: 8, right: 8, top: 3, bottom: 3}
                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    border_radius: 8.0
                }
                status_label := Label {
                    width: Fit, height: Fit
                    padding: 0
                    draw_text +: {
                        text_style: mod.widgets.REGULAR_TEXT { font_size: 9 },
                        color: (mod.widgets.COLOR_AGENT_APPROVAL_TITLE)
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
                text_style: mod.widgets.MESSAGE_TEXT_STYLE {},
                color: (COLOR_TEXT)
            }
            text: ""
        }

        button_row := View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            spacing: 8

            approve_once_button := mod.widgets.AgentApprovalPositiveButton {}
            approve_task_button := mod.widgets.AgentApprovalNeutralButton {}
            approve_always_button := mod.widgets.AgentApprovalNeutralButton {}
            deny_button := mod.widgets.AgentApprovalNegativeButton {}
        }

        hint_label := Label {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            padding: 0
            draw_text +: {
                text_style: mod.widgets.REGULAR_TEXT { font_size: 9 },
                color: (COLOR_MESSAGE_NOTICE_TEXT)
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
