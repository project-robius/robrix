//! The `LocationPreview` is a small view that shows the current location
//! and allows the user to send their location to a room.
//!
//! This view is not visible by default, only when the user requests it
//! by clicking on the location button in the message input bar.
//! The `RoomScreen` widget then shows this view above the message input bar.

use std::time::SystemTime;

use makepad_widgets::*;
use robius_location::Coordinates;

use crate::location::{get_latest_location, request_location_update, LocationAction, LocationRequest, LocationUpdate};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.LocationPreview = #(LocationPreview::register_widget(vm)) {
        visible: false
        width: Fill
        height: Fit
        flow: Down
        // to align this view just below the RoomInputBar's curved border
        margin: Inset{top: 1}
        padding: Inset{left: 12, top: 10, bottom: 10, right: 10}
        spacing: 8

        show_bg: true,
        draw_bg +: {
            color: (COLOR_BG_PREVIEW),
            border_radius: 5.0,
            border_size: 2.0
        }

        Label {
            width: Fill,
            height: Fit,
            draw_text +: {
                flow: Flow.Right{wrap: true},
                color: (MESSAGE_TEXT_COLOR),
                text_style: MESSAGE_TEXT_STYLE { font_size: 10.0 },
            }
            text: "Send your location to this room?"
        }

        location_label := Label {
            width: Fill,
            height: Fit,
            align: Align{x: 0.0, y: 0.5},
            padding: Inset{left: 10, bottom: 7}
            draw_text +: {
                flow: Flow.Right{wrap: true},
                color: (MESSAGE_TEXT_COLOR),
                text_style: MESSAGE_TEXT_STYLE {},
            }
            text: "➡ Fetching current location..."
        }

        View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true},
            align: Align{x: 0.0, y: 0.5}

            cancel_location_button := RobrixIconButton {
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                margin: Inset{right: 15}
                draw_icon +: {
                    svg_file: (ICON_FORBIDDEN)
                    color: (COLOR_FG_DANGER_RED),
                }
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1, top: -1} }

                draw_bg +: {
                    border_color: (COLOR_FG_DANGER_RED),
                    color: (COLOR_BG_DANGER_RED)
                }
                text: "Cancel"
                draw_text +: {
                    color: (COLOR_FG_DANGER_RED),
                }
            }

            send_location_button := RobrixIconButton {
                // disabled by default; will be enabled upon receiving valid location update.
                enabled: false,
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon +: {
                    svg_file: (ICON_SEND)
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }

                draw_bg +: {
                    border_color: (COLOR_FG_ACCEPT_GREEN),
                    color: (COLOR_BG_ACCEPT_GREEN)
                }
                text: "Yes"
                draw_text +: {
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
            }
        }
    }
}


#[derive(Script, ScriptHook, Widget)]
struct LocationPreview {
    #[deref] view: View,
    #[rust] coords: Option<Result<Coordinates, robius_location::Error>>,
    #[rust] timestamp: Option<SystemTime>,
}

impl Widget for LocationPreview {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let mut needs_redraw = false;
        if let Event::Actions(actions) = event {
            for action in actions {
                match action.downcast_ref() {
                    Some(LocationAction::Update(LocationUpdate { coordinates, time })) => {
                        self.coords = Some(Ok(*coordinates));
                        self.timestamp = *time;
                        self.button(cx, ids!(send_location_button)).set_enabled(cx, true);
                        needs_redraw = true;
                    }
                    Some(LocationAction::Error(e)) => {
                        self.coords = Some(Err(*e));
                        self.timestamp = None;
                        self.button(cx, ids!(send_location_button)).set_enabled(cx, false);
                        needs_redraw = true;
                    }
                    _ => { }
                }
            }

            // NOTE: the send location button click event is handled
            //       in the RoomScreen handle_event function.

            // Handle the cancel location button being clicked.
            if self.button(cx, ids!(cancel_location_button)).clicked(actions) {
                self.clear();
                needs_redraw = true;
            }
        }

        if needs_redraw {
            self.redraw(cx);
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let text = match self.coords {
            Some(Ok(c)) => {
                format!("➡ Current location: {:.6}, {:.6}", c.latitude, c.longitude)
            }
            Some(Err(e)) => format!("➡ Error getting location: {e:?}"),
            None => String::from("➡ Current location is not yet available."),
        };
        self.label(cx, ids!(location_label)).set_text(cx, &text);
        self.view.draw_walk(cx, scope, walk)
    }
}


impl LocationPreview {
    fn show(&mut self) {
        request_location_update(LocationRequest::UpdateOnce);
        if let Some(loc) = get_latest_location() {
            self.coords = Some(Ok(loc.coordinates));
            self.timestamp = loc.time;
        }
        self.visible = true;
    }

    fn clear(&mut self) {
        self.coords = None;
        self.timestamp = None;
        self.visible = false;
    }

    pub fn get_current_data(&self) -> Option<(Coordinates, Option<SystemTime>)> {
        self.coords
            .as_ref()
            .and_then(|res| res.ok())
            .map(|c| (c, self.timestamp))
    }
}

impl LocationPreviewRef {
    pub fn show(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show();
        }
    }

    pub fn clear(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear();
        }
    }

    pub fn get_current_data(&self) -> Option<(Coordinates, Option<SystemTime>)> {
        self.borrow().and_then(|inner| inner.get_current_data())
    }
}
