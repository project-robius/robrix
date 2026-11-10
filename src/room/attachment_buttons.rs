//! `AttachmentButtons`: the room input bar's buttons for uploading a photo, video or file, or sending a location.
//!
//! While the message input is empty and the user hasn't focused it (the app focusing it doesn't count),
//! all three buttons show in a row, as long as the input's empty text still fits on one line beside them.
//! Otherwise they slide away behind one "+" button, which shows them in a popup menu instead.

use makepad_widgets::{animator::Ease, text_input::TextInputAction, *};
use crate::shared::{
    emoji_picker_popup::get_emoji_picker_popup,
    room_input_popup_menu::RoomInputPopupMenuAction,
    speech_text_input::SpeechTextInputRef,
};

/// How long the row takes to slide between the "+" button and the three attachment buttons.
const ANIMATION_DURATION_SECS: f64 = 0.25;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.AttachmentButton = RobrixIconButton {
        width: 39, height: 39
        margin: 0
        padding: 9
        spacing: 0,
        icon_walk: Walk{width: 21, height: 21}
        draw_icon +: {
            color: (COLOR_ACTIVE_PRIMARY_DARKER)
        },
        draw_bg +: {
            color: (COLOR_BG_PREVIEW)
            color_hover: (COLOR_INPUT_BAR_ADD_BUTTON_BG_HOVER)
            color_down: (COLOR_INPUT_BAR_ADD_BUTTON_BG_DOWN)
        }
    }

    mod.widgets.AttachmentButtons = #(AttachmentButtons::register_widget(vm)) {
        width: Fit, height: Fit
        // While the row slides, only the part of it that fits in our animated width shows.
        clip_x: true

        row := View {
            width: Fit, height: Fit
            spacing: 4
            open_popup_menu_button := mod.widgets.AttachmentButton {
                // The message input keeps its focus until the popup menu takes it.
                grab_key_focus: false
                draw_icon.svg: (ICON_ADD)
            }
            upload_photo_video_button := mod.widgets.AttachmentButton { draw_icon.svg: (ICON_ADD_PHOTO) }
            upload_file_button := mod.widgets.AttachmentButton { draw_icon.svg: (ICON_ADD_ATTACHMENT) }
            send_location_button := mod.widgets.AttachmentButton { draw_icon.svg: (ICON_LOCATION_PIN) }
        }
    }
}

#[derive(Script, Widget)]
pub struct AttachmentButtons {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// How far the buttons are expanded into a row, from 0 (just the "+" button) to 1.
    #[rust(1.0)] expanded_fraction: f64,
    #[rust] animation: Option<ExpandAnimation>,
    /// Whether our popup menu is open, since the message input loses its focus to it.
    #[rust] is_popup_menu_open: bool,
    /// The message input whose text, focus and width decide whether we expand.
    #[rust] input: SpeechTextInputRef,
    /// The text input inside `input`, looked up once since we check its actions on every event.
    #[rust] text_input: TextInputRef,
    /// The input bar's width at our last draw, so we notice when a resize changes what fits.
    #[rust] last_bar_width: f64,
    /// Fires once a resize has been drawn, to check again whether the expanded row fits.
    #[rust] fit_check_frame: Option<NextFrame>,
    /// The child widgets we use on every event, looked up once.
    #[rust] child_refs: Option<ChildRefs>,
}

#[derive(Clone)]
struct ChildRefs {
    row: ViewRef,
    open_popup_menu_button: ButtonRef,
    upload_photo_video_button: ButtonRef,
    upload_file_button: ButtonRef,
    send_location_button: ButtonRef,
}

struct ExpandAnimation {
    /// When the animation started, from [`Cx::monotonic_now()`].
    started_at: f64,
    from_fraction: f64,
    to_fraction: f64,
    next_frame: NextFrame,
}

impl ScriptHook for AttachmentButtons {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| self.update_button_visibility(cx));
    }

    fn on_after_reload(&mut self, _vm: &mut ScriptVm) {
        self.child_refs = None;
    }
}

impl Widget for AttachmentButtons {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.fit_check_frame.take_if(|frame| frame.is_event(event).is_some()).is_some() {
            self.update_for_input(cx, false);
        }
        if let Some(animation) = &mut self.animation
            && animation.next_frame.is_event(event).is_some()
        {
            let progress = Ease::OutCubic.map(((Cx::monotonic_now() - animation.started_at) / ANIMATION_DURATION_SECS).min(1.0));
            self.expanded_fraction = animation.from_fraction + (animation.to_fraction - animation.from_fraction) * progress;
            if progress < 1.0 {
                animation.next_frame = cx.new_next_frame();
            } else {
                self.animation = None;
                self.update_button_visibility(cx);
            }
            self.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let bar_width = cx.turtle().rect().size.x;
        if bar_width != self.last_bar_width {
            self.last_bar_width = bar_width;
            self.fit_check_frame = Some(cx.new_next_frame());
        }
        // At rest, only the "+" button or only the other three are shown, so the row draws just like its DSL says.
        if self.animation.is_none() {
            return self.view.draw_walk(cx, scope, walk);
        }
        let row = self.child_refs(cx).row.clone();
        let Some((spacing, margin_left)) = row.borrow().map(|row| (row.layout.spacing, row.walk.margin.left)) else {
            return self.view.draw_walk(cx, scope, walk);
        };
        // Expanding slides the row left, so the "+" button leaves on the left as the others come in from the right.
        let step = self.get_button_width(cx) + spacing;
        if let Some(mut row) = row.borrow_mut() {
            row.walk.margin.left = margin_left - step * self.expanded_fraction;
        }
        let walk = Walk { width: Size::Fixed(step - spacing + 2.0 * step * self.expanded_fraction), ..walk };
        let draw_step = self.view.draw_walk(cx, scope, walk);
        if let Some(mut row) = row.borrow_mut() {
            row.walk.margin.left = margin_left;
        }
        draw_step
    }
}

impl AttachmentButtons {
    fn child_refs(&mut self, cx: &mut Cx) -> &ChildRefs {
        self.child_refs.get_or_insert_with(|| ChildRefs {
            row: self.view.view(cx, ids!(row)),
            open_popup_menu_button: self.view.button(cx, ids!(open_popup_menu_button)),
            upload_photo_video_button: self.view.button(cx, ids!(upload_photo_video_button)),
            upload_file_button: self.view.button(cx, ids!(upload_file_button)),
            send_location_button: self.view.button(cx, ids!(send_location_button)),
        })
    }

    /// Returns how wide each button is, from the DSL, since mid-draw their drawn rects aren't valid.
    fn get_button_width(&mut self, cx: &mut Cx) -> f64 {
        let open_popup_menu_button = self.child_refs(cx).open_popup_menu_button.clone();
        match open_popup_menu_button.walk(cx).width {
            Size::Fixed(width) => width,
            _ => 0.0,
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) -> RoomInputPopupMenuAction {
        let ChildRefs {
            open_popup_menu_button,
            upload_photo_video_button,
            upload_file_button,
            send_location_button,
            ..
        } = self.child_refs(cx).clone();

        let action = if open_popup_menu_button.clicked(actions) {
            self.is_popup_menu_open = true;
            RoomInputPopupMenuAction::Show { button_rect: open_popup_menu_button.area().rect(cx) }
        } else if upload_photo_video_button.clicked(actions) {
            RoomInputPopupMenuAction::UploadPhotoOrVideo
        } else if upload_file_button.clicked(actions) {
            RoomInputPopupMenuAction::UploadFile
        } else if send_location_button.clicked(actions) {
            RoomInputPopupMenuAction::SendCurrentLocation
        } else {
            RoomInputPopupMenuAction::None
        };

        let was_popup_menu_closed = actions.iter().any(|action|
            matches!(action.downcast_ref(), Some(RoomInputPopupMenuAction::Closed))
        );
        if was_popup_menu_closed {
            self.is_popup_menu_open = false;
        }
        let has_input_changed = actions.filter_widget_actions(self.text_input.widget_uid()).any(|action| matches!(
            action.cast(),
            TextInputAction::KeyFocus | TextInputAction::KeyFocusLost | TextInputAction::Changed(_)
        ));
        if was_popup_menu_closed || has_input_changed {
            self.update_for_input(cx, true);
        }
        action
    }

    /// Expands the buttons if the message input is empty, nobody's writing in it, and there's room; or collapses them.
    fn update_for_input(&mut self, cx: &mut Cx, should_animate: bool) {
        // Our popup menu and the emoji picker take the input's focus while the user is still writing.
        let is_expanded = !self.is_popup_menu_open
            && !cx.has_key_focus_from_user(self.text_input.area())
            && !get_emoji_picker_popup(cx).is_currently_shown()
            && self.text_input.text().is_empty()
            && self.does_expanded_row_fit(cx);
        let to_fraction = if is_expanded { 1.0 } else { 0.0 };
        if self.animation.as_ref().map_or(self.expanded_fraction, |animation| animation.to_fraction) == to_fraction {
            return;
        }
        if should_animate {
            self.animation = Some(ExpandAnimation {
                started_at: Cx::monotonic_now(),
                from_fraction: self.expanded_fraction,
                to_fraction,
                next_frame: cx.new_next_frame(),
            });
        } else {
            self.animation = None;
            self.expanded_fraction = to_fraction;
        }
        self.update_button_visibility(cx);
        self.redraw(cx);
    }

    /// Returns whether the message input's empty text would still fit on one line beside the fully expanded row.
    fn does_expanded_row_fit(&mut self, cx: &mut Cx) -> bool {
        let spacing = self.child_refs(cx).row.borrow().map_or(0.0, |row| row.layout.spacing);
        let expanded_width = 3.0 * self.get_button_width(cx) + 2.0 * spacing;
        let narrower_by = expanded_width - self.view.area().rect(cx).size.x;
        self.input.does_empty_text_fit(cx, narrower_by)
    }

    /// Shows the "+" button unless the row is all the way out, and the others unless they're all tucked away.
    fn update_button_visibility(&mut self, cx: &mut Cx) {
        let is_animating = self.animation.is_some();
        let ChildRefs {
            open_popup_menu_button,
            upload_photo_video_button,
            upload_file_button,
            send_location_button,
            ..
        } = self.child_refs(cx).clone();
        open_popup_menu_button.set_visible(cx, is_animating || self.expanded_fraction < 1.0);
        for button in [upload_photo_video_button, upload_file_button, send_location_button] {
            button.set_visible(cx, is_animating || self.expanded_fraction > 0.0);
        }
    }
}

impl AttachmentButtonsRef {
    /// Sets the message input that we expand and collapse for.
    pub fn set_input(&self, input: SpeechTextInputRef) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.text_input = input.text_input_ref();
        inner.input = input;
    }

    /// Handles clicks on our buttons, and any focus or text changes in the message input.
    ///
    /// Returns what a click asked for, or `RoomInputPopupMenuAction::None`.
    pub fn handle_actions(&self, cx: &mut Cx, actions: &Actions) -> RoomInputPopupMenuAction {
        let Some(mut inner) = self.borrow_mut() else { return RoomInputPopupMenuAction::None };
        inner.handle_actions(cx, actions)
    }

    /// Expands or collapses the buttons to match the message input,
    /// e.g., after its text was set without the user typing.
    pub fn update_for_input(&self, cx: &mut Cx, should_animate: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.update_for_input(cx, should_animate);
    }}
