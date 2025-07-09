//! The RoomsListHeader contains the title label and loading spinner for rooms list.
//!
//! This widget is designed to be reused across both Desktop and Mobile variants 
//! of the RoomsSideBar to avoid code duplication.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;

    pub RoomsListHeader = {{RoomsListHeader}}<View> {
        width: Fill,
        height: Fit,
        flow: Right,
        visible: true,
        align: {
            x: 0.5,
            y: 0.5
        }
        sidebar_title = <Label> {
            flow: Right, // do not wrap
            text: "All Rooms"
            draw_text: {
                color: #x0
                text_style: <TITLE_TEXT>{}
            }
        },
        <View> {
            width: Fill,
            height: Fit,
        }
        loading_spinner = <LoadingSpinner> {
            width: 20,
            height: 20,
            draw_bg: {
                radius: 8.0,
                stroke_width: 2.0,
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RoomsListHeader {
    #[deref] view: View,
}

impl Widget for RoomsListHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(RoomsListHeaderAction::SetSyncStatus(is_syncing)) = action.downcast_ref() {
                    self.view(id!(loading_spinner)).set_visible(cx, *is_syncing);
                    self.redraw(cx);
                }
            }
        }
        
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

#[derive(Debug)]
pub enum RoomsListHeaderAction {
    SetSyncStatus(bool),
}
