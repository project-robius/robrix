//! The `LocationPreview` is a small view that shows the current location
//! and allows the user to send their location to a room.
//!
//! This view is not visible by default, only when the user requests it
//! by clicking on the location button in the message input bar.
//! The `RoomScreen` widget then shows this view above the message input bar.

use std::time::SystemTime;

use makepad_widgets::*;
use matrix_sdk::ruma::MilliSecondsSinceUnixEpoch;
use robius_location::Coordinates;

use crate::{
    location::{request_location_update, LocationAction, LocationRequest, LocationUpdate},
    utils::time_ago,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.LocationPreview = set_type_default() do #(LocationPreview::register_widget(vm)) {
        ..mod.widgets.RoundedView

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
            color: (COLOR_BG_PREVIEW)
            border_radius: 5.0
            border_size: 2.0
        }

        Label {
            width: Fill,
            height: Fit,
            flow: Flow.Right{wrap: true},
            draw_text +: {
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
            flow: Flow.Right{wrap: true},
            draw_text +: {
                color: (MESSAGE_TEXT_COLOR),
                text_style: MESSAGE_TEXT_STYLE {},
            }
            text: "➡ Fetching current location..."
        }

        View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true},
            spacing: 15
            align: Align{x: 0.0, y: 0.5}

            cancel_location_button := RobrixNegativeIconButton {
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                margin: 0
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1, top: -1} }
                text: "Cancel"
            }

            send_location_button := RobrixPositiveIconButton {
                // disabled by default; will be enabled upon receiving valid location update.
                enabled: false,
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                margin: 0
                draw_icon.svg: (ICON_SEND)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Yes"
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

fn location_error_message(error: robius_location::Error) -> &'static str {
    use robius_location::Error;
    match error {
        Error::AuthorizationDenied =>
            "Permission denied. Give Robrix location access in your device's settings, then try again.",
        Error::TemporarilyUnavailable =>
            "Your location isn't available right now. Please try again in a moment.",
        Error::Network =>
            "A network problem prevented getting your location. Check your connection and try again.",
        Error::PermanentlyUnavailable =>
            "Location services aren't available on this device.",
        Error::AndroidEnvironment | Error::NotMainThread | Error::Unknown =>
            "Couldn't get your location. Please try again.",
    }
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
                match self.timestamp
                    .and_then(MilliSecondsSinceUnixEpoch::from_system_time)
                    .and_then(time_ago)
                {
                    Some(age) => format!("📍 Your location (from {}): {:.6}, {:.6}", age.to_ascii_lowercase(), c.latitude, c.longitude),
                    None => format!("📍 Your location: {:.6}, {:.6}", c.latitude, c.longitude),
                }
            }
            Some(Err(e)) => format!("⚠️ {}", location_error_message(e)),
            None => String::from("📍 Getting your location…"),
        };
        self.label(cx, ids!(location_label)).set_text(cx, &text);
        self.view.draw_walk(cx, scope, walk)
    }
}


impl LocationPreview {
    fn show(&mut self, cx: &mut Cx) {
        request_location_update(cx, LocationRequest::UpdateOnce);
        self.coords = None;
        self.timestamp = None;
        self.button(cx, ids!(send_location_button)).set_enabled(cx, false);
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
    pub fn show(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx);
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
