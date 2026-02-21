
//! UI widgets for searching messages in one or more rooms.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.SearchMessagesButton = #(SearchMessagesButton::register_widget(vm)) {
        width: Fit,
        height: 35,
        margin: 0
        enabled: false

        draw_bg +: {
            color: (COLOR_BG_DISABLED)
            // color: (COLOR_ROBRIX_PURPLE) // or `color: (COLOR_ACTIVE_PRIMARY)`
            // color_hover: (COLOR_PRIMARY_DARKER) // make it whiter (this value is mixed in with `color`)
        }
        draw_icon +: {
            svg_file: (ICON_SEARCH)
            color: (COLOR_FG_DISABLED)
            // color: (COLOR_PRIMARY),
            // color_hover: (COLOR_PRIMARY),
        }
        icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -1, right: -2} }

        // text: "Search Messages"
        text: "Search (TODO)"
        draw_text +: {
            color: (COLOR_FG_DISABLED)
            // color: (COLOR_PRIMARY),
            // color_hover: (COLOR_PRIMARY),
        }
    }

    
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchMessagesButton {
    #[deref] button: Button,
}

impl Widget for SearchMessagesButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.button.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            if self.button.clicked(actions) {

                // cx.action(AddRoomAction::SearchMessagesButtonClicked);
            }
        }
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.button.draw_walk(cx, scope, walk)
    }
}

#[derive(Debug)]
pub enum AddRoomAction {
    SearchMessagesButtonClicked,
}
