//! A popup menu that contains buttons for sending attachments or location to a room.
//! 
//! This is shown when clicking the add/plus-sign button in the RoomInputBar.

use makepad_widgets::*;
use makepad_widgets::makepad_platform::event::finger::TouchState;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RoomInputPopupMenuButton = RobrixIconButton {
        height: 38
        width: Fill
        margin: 0
        padding: Inset{left: 10, right: 12, top: 9, bottom: 9}
        spacing: 10
        align: Align{x: 0, y: 0.5}

        draw_bg +: {
            color: (COLOR_PRIMARY)
            color_hover: #xE0E8F0
            color_down: #xD0D8E8
            border_radius: 4.0
        }
        draw_text +: {
            color: (COLOR_TEXT)
            color_hover: (COLOR_TEXT)
            color_down: (COLOR_TEXT)
            text_style: REGULAR_TEXT {font_size: 11}
        }
        draw_icon.color: (COLOR_ACTIVE_PRIMARY_DARKER)
        icon_walk: Walk{width: 18, height: 18}
    }

    mod.widgets.RoomInputPopupMenu = set_type_default() do #(RoomInputPopupMenu::register_widget(vm)) {
        ..mod.widgets.SolidView

        visible: false
        width: Fill
        height: Fill
        flow: Overlay
        align: Align{x: 0, y: 1}
        cursor: MouseCursor.Default

        show_bg: false
        draw_bg +: {
            color: #00000000
        } 

        // This works kinda like the other context menus; we position the main content
        // within the entire room screen using margins.
        main_content := RoundedShadowView {
            width: 235
            height: Fit
            flow: Down
            padding: 6
            spacing: 2
            align: Align{x: 0, y: 0}

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 5.0
                border_size: 0.0
                shadow_color: #0005
                shadow_radius: 14.0
                shadow_offset: vec2(0.0, 4.0)
            }

            upload_photo_video_button := mod.widgets.RoomInputPopupMenuButton {
                draw_icon.svg: (ICON_ADD_PHOTO)
                text: "Upload photo or video"
            }

            upload_file_button := mod.widgets.RoomInputPopupMenuButton {
                draw_icon.svg: (ICON_ADD_ATTACHMENT)
                text: "Upload file"
            }

            send_location_button := mod.widgets.RoomInputPopupMenuButton {
                draw_icon.svg: (ICON_LOCATION_PIN)
                text: "Send current location"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RoomInputPopupMenuAction {
    Show { button_rect: Rect },
    UploadPhotoOrVideo,
    UploadFile,
    SendCurrentLocation,
    #[default]
    None,
}

impl ActionDefaultRef for RoomInputPopupMenuAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: RoomInputPopupMenuAction = RoomInputPopupMenuAction::None;
        &DEFAULT
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RoomInputPopupMenu {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
}

impl Widget for RoomInputPopupMenu {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.visible { return; }

        if matches!(event, Event::KeyUp(KeyEvent {key_code: KeyCode::Escape, .. }))
            || event.back_pressed()
        {
            self.close(cx);
            return;
        }

        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RoomInputPopupMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let action = if self.button(cx, ids!(upload_photo_video_button)).clicked(actions) {
            RoomInputPopupMenuAction::UploadPhotoOrVideo
        } else if self.button(cx, ids!(upload_file_button)).clicked(actions) {
            RoomInputPopupMenuAction::UploadFile
        } else if self.button(cx, ids!(send_location_button)).clicked(actions) {
            RoomInputPopupMenuAction::SendCurrentLocation
        } else {
            RoomInputPopupMenuAction::None
        };

        if action != RoomInputPopupMenuAction::None {
            self.close(cx);
            cx.widget_action(self.widget_uid(), action);
        }
    }
}

impl RoomInputPopupMenu {
    pub fn is_open(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.reset_button_hover(cx);
        self.visible = true;
        cx.set_key_focus(self.view.area());
        self.redraw(cx);
    }

    pub fn close(&mut self, cx: &mut Cx) {
        if !self.visible { return; }
        self.visible = false;
        cx.revert_key_focus();
        self.redraw(cx);
    }

    fn reset_button_hover(&mut self, cx: &mut Cx) {
        self.button(cx, ids!(upload_photo_video_button)).reset_hover(cx);
        self.button(cx, ids!(upload_file_button)).reset_hover(cx);
        self.button(cx, ids!(send_location_button)).reset_hover(cx);
    }

    pub fn is_event_within_popup_menu(&self, cx: &mut Cx, event: &Event) -> bool {
        let main_rect = self.view(cx, ids!(main_content)).area().rect(cx);
        match event {
            Event::MouseDown(e) => main_rect.contains(e.abs),
            Event::MouseUp(e) => main_rect.contains(e.abs),
            Event::MouseMove(e) => main_rect.contains(e.abs),
            Event::Scroll(e) => main_rect.contains(e.abs),
            Event::LongPress(e) => main_rect.contains(e.abs),
            Event::TouchUpdate(e) => e.touches.iter().any(|touch| main_rect.contains(touch.abs)),
            _ => false,
        }
    }

    pub fn should_dismiss_for_outside_event(&self, cx: &mut Cx, event: &Event) -> bool {
        let main_rect = self.view(cx, ids!(main_content)).area().rect(cx);
        match event {
            Event::MouseDown(e) => !main_rect.contains(e.abs),
            Event::LongPress(e) => !main_rect.contains(e.abs),
            Event::TouchUpdate(e) => e.touches.iter().any(|touch| {
                touch.state == TouchState::Start && !main_rect.contains(touch.abs)
            }),
            _ => false,
        }
    }
}

impl RoomInputPopupMenuRef {
    pub fn is_open(&self) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_open()
    }

    pub fn close(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.close(cx);
    }

    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }

    pub fn is_event_within_popup_menu(&self, cx: &mut Cx, event: &Event) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_event_within_popup_menu(cx, event)
    }

    pub fn should_dismiss_for_outside_event(&self, cx: &mut Cx, event: &Event) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.should_dismiss_for_outside_event(cx, event)
    }

    pub fn selected(&self, actions: &Actions) -> Option<RoomInputPopupMenuAction> {
        match actions.find_widget_action(self.widget_uid()).cast_ref() {
            RoomInputPopupMenuAction::None => None,
            action => Some(*action),
        }
    }

}
