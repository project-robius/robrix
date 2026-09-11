//! The agent-chat section of the App Settings screen.
//!
//! Only the DSL lives here; the toggle is wired up in
//! `crate::settings::app_settings` under `#[cfg(feature = "agent_chat")]`.
//! Builds without the feature get an empty placeholder from
//! `crate::agent_chat_dummy` instead, so the settings screen is unchanged.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.AgentChatPreferences = View {
        width: Fill, height: Fit
        flow: Down

        SubsectionLabel {
            text: "Agent-chat (experimental)"
        }

        View {
            width: Fill, height: Fit
            flow: Down,
            margin: Inset{left: 6},

            agent_chat_toggle := ToggleFlat {
                margin: Inset{left: 0.5, top: 5, bottom: 10}
                padding: Inset { left: 15}
                active: false,
                draw_bg +: { size: 21 }
                text: "Enable agent-chat workflow commands"
                draw_text +: {
                    // `SETTINGS_BOLD_TEXT_STYLE` is registered after this module; inline its definition.
                    text_style: theme.font_bold { font_size: (mod.widgets.SETTINGS_REGULAR_FONT_SIZE) },
                }
            }
            Html {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                margin: Inset{left: 14, top: 0, bottom: 0, right: 5}
                padding: 0,
                font_size: 11,
                font_color: #666,
                text_style_normal: MESSAGE_TEXT_STYLE { font_size: 11 },
                body: "<ul><li>Offers the <b>/create-issue</b>, <b>/go</b>, <b>/review</b> and <b>/status</b> commands in rooms that contain a <b>*_coordinator</b> agent. They are sent as plain text for the coordinator agent to act on.</li><li>Approval cards for <b>com.agentchat.approval</b> requests are always shown; this toggle does not affect them.</li></ul>"
            }
        }
    }
}
