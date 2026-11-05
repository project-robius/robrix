//! `EmojiPickerPopup`, the app-wide popup that shows an `EmojiPicker`.
//!
//! In the desktop view mode it's a popover beside whatever opened it,
//! and in the mobile view mode it's a sheet along the bottom of the window.
//! Either way, it's re-placed upon every draw so it always fits within the window.

use makepad_widgets::{makepad_platform::event::finger::TouchState, *};
use matrix_sdk_ui::timeline::ReactionsByKeyBySender;
use crate::{
    home::{home_screen::effective_is_desktop, new_message_context_menu::MessageDetails, room_screen::MessageAction},
    shared::{
        context_menu::{get_keyboard_height, get_popup_bounds, is_context_menu_cancelled, is_tap_outside},
        emoji_picker::{EmojiPickerAction, EmojiPickerMode, EmojiPickerRef, EmojiPickerWidgetExt, get_picked_emoji},
        recent_emojis::record_emoji_pick,
    },
    sliding_sync::current_user_id,
    utils::has_only_soft_keyboard,
};

/// The largest size of the popup, which fits 15 columns of emoji.
const MAX_SIZE: DVec2 = dvec2(616.0, 560.0);
/// The shortest the popover gets beside a message input, before it covers part of that input instead.
const MIN_POPOVER_HEIGHT: f64 = 300.0;
/// How much of the window's height the sheet takes up in the mobile view mode, at most.
const SHEET_HEIGHT_FRACTION: f64 = 0.6;
/// How far we keep the popup away from the edges of the window.
const EDGE_GAP: f64 = 8.0;
/// The space between the popover and the thing it's placed beside.
const ANCHOR_GAP: f64 = 6.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.EmojiPickerPopup = #(EmojiPickerPopup::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Overlay
        align: Align{x: 0.0, y: 0.0}

        // Dims everything behind a picker for reacting to a message, like a context menu does.
        backdrop := SolidView {
            visible: false
            width: Fill, height: Fill
            show_bg: true
            draw_bg.color: (COLOR_CONTEXT_MENU_OVERLAY_BG)
        }

        // We move and size this wrapper so that `main_content` can just fill it.
        popup_frame := View {
            width: Fit, height: Fit
            clip_x: false, clip_y: false

            main_content := mod.widgets.FloatingPopupContent {
                draw_bg +: { border_radius: 8.0 }
                picker := EmojiPicker {}
            }
        }
    }
}

/// What the emoji picked in the `EmojiPickerPopup` will be used for.
#[derive(Clone, Debug)]
enum EmojiPickerPurpose {
    /// Inserting it into the message input with the given widget UID,
    /// which gets an [`EmojiPickerAction::Picked`] widget action.
    Insert { owner: WidgetUid },
    /// Reacting to the given message.
    React { details: MessageDetails },
}

impl EmojiPickerPurpose {
    /// The widget that opened this picker.
    fn owner(&self) -> WidgetUid {
        match self {
            EmojiPickerPurpose::Insert { owner } => *owner,
            EmojiPickerPurpose::React { details } => details.room_screen_widget_uid,
        }
    }
}

/// What the popup is placed beside.
#[derive(Clone, Debug, Default)]
enum Anchor {
    /// A rect relative to the overlay container that we're drawn in, e.g., where a context menu was.
    /// The soft keyboard pans that whole container, so this stays put relative to the message it was for.
    InOverlay(Rect),
    /// A message input and its emoji button, which we place ourselves beside upon every draw,
    /// since the input can grow or move while we're open.
    Input { input: WidgetRef, button: WidgetRef },
    #[default]
    None,
}

/// The child widgets we use on every draw and event, looked up once.
#[derive(Clone)]
struct ChildRefs {
    popup_frame: ViewRef,
    main_content: ViewRef,
    picker: EmojiPickerRef,
}

#[derive(Script, Widget)]
pub struct EmojiPickerPopup {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// `Some` while the popup is open.
    #[rust] purpose: Option<EmojiPickerPurpose>,
    #[rust] anchor: Anchor,
    /// The window-absolute rect of the `anchor`, as of the last draw.
    #[rust] anchor_rect: Rect,
    /// The window-absolute position of the overlay container that we're drawn in, as of the last draw.
    #[rust] overlay_origin: DVec2,
    #[rust] is_sheet: bool,
    /// How much of the window the soft keyboard covers, which the sheet stays above.
    #[rust] keyboard_height: f64,
    #[rust] cancel_scope: Option<CancelScope>,
    #[rust] child_refs: Option<ChildRefs>,
}

impl ScriptHook for EmojiPickerPopup {
    fn on_after_reload(&mut self, _vm: &mut ScriptVm) {
        self.child_refs = None;
    }
}

impl Widget for EmojiPickerPopup {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // The keyboard often hides right after we close, so we keep track of it even while closed.
        if let Some(height) = get_keyboard_height(event) {
            self.keyboard_height = height;
            if self.purpose.is_some() {
                self.redraw(cx);
            }
        }
        if self.purpose.is_none() {
            return;
        }

        if is_context_menu_cancelled(cx, event, self.cancel_scope.as_ref()) {
            if let Some(EmojiPickerPurpose::Insert { owner }) = &self.purpose {
                cx.widget_action(*owner, EmojiPickerAction::Cancelled);
            }
            self.close(cx);
            return;
        }

        self.view.handle_event(cx, event, scope);

        let ChildRefs { main_content, picker, .. } = self.child_refs(cx).clone();
        if let Event::Actions(actions) = event {
            // We re-place the popup upon every draw, which keeps it within a resized window.
            if actions.iter().any(|a| matches!(a.as_widget_action().cast(), WindowAction::WindowGeomChange(_))) {
                self.redraw(cx);
            }
            if let Some((emoji, keep_open)) = get_picked_emoji(actions, picker.widget_uid())
                && let Some(purpose) = self.purpose.as_mut()
            {
                // A sheet for a message input works like a keyboard, so it stays open for more emoji.
                let keep_open = keep_open || (self.is_sheet && matches!(purpose, EmojiPickerPurpose::Insert { .. }));
                match purpose {
                    EmojiPickerPurpose::Insert { owner } => {
                        record_emoji_pick(cx, &emoji);
                        cx.widget_action(*owner, EmojiPickerAction::Picked { emoji, keep_open });
                    }
                    EmojiPickerPurpose::React { details } => {
                        if keep_open {
                            picker.toggle_own_reaction(cx, &emoji);
                        }
                        cx.widget_action(details.room_screen_widget_uid, MessageAction::React { details: details.clone(), reaction: emoji });
                    }
                }
                if !keep_open {
                    self.close(cx);
                    return;
                }
            }
        }

        // A picker with a backdrop takes every press, like a context menu,
        // while a popover lets a press outside of it close it and also reach whatever it pressed.
        if matches!(self.purpose, Some(EmojiPickerPurpose::React { .. })) {
            let hit = event.hits_with_capture_overload(cx, self.view.area(), true);
            if is_tap_outside(cx, &hit, main_content.area()) {
                self.close(cx);
            }
        } else {
            // Pressing what opened the popover (like the emoji button) is left to that to handle.
            let anchor = self.anchor_rect;
            let is_outside = |cx: &mut Cx, abs: DVec2| !main_content.area().rect(cx).contains(abs) && !anchor.contains(abs);
            let is_press_outside = match event {
                Event::MouseDown(e) => !e.button.is_back() && is_outside(cx, e.abs),
                Event::TouchUpdate(e) => e.touches.iter().any(|t| t.state == TouchState::Start && is_outside(cx, t.abs)),
                _ => false,
            };
            if is_press_outside {
                self.close(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.overlay_origin = cx.turtle().origin();
        if self.purpose.is_none() {
            return DrawStep::done();
        }
        self.place_popup_frame(cx);
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        let content_area = self.child_refs(cx).main_content.area();
        cx.block_scrolling_except_within(content_area);
        DrawStep::done()
    }
}

impl EmojiPickerPopup {
    fn child_refs(&mut self, cx: &mut Cx) -> &ChildRefs {
        self.child_refs.get_or_insert_with(|| ChildRefs {
            popup_frame: self.view.view(cx, ids!(popup_frame)),
            main_content: self.view.view(cx, ids!(main_content)),
            picker: self.view.emoji_picker(cx, ids!(main_content.picker)),
        })
    }

    fn show(&mut self, cx: &mut Cx, purpose: EmojiPickerPurpose, mode: EmojiPickerMode, anchor: Anchor) {
        let is_sheet = !effective_is_desktop(cx);
        // Only a reaction picker dims the rest of the app; the message an emoji goes into should stay in view.
        let has_backdrop = matches!(purpose, EmojiPickerPurpose::React { .. });
        // Our own scope is the newest one, so a Back press closes us rather than a pane behind us.
        self.cancel_scope = Some(cx.begin_widget_cancel_scope(self.widget_uid().0, CancelScopeKind::Both));
        self.anchor = anchor;
        self.is_sheet = is_sheet;
        self.purpose = Some(purpose);
        self.view.view(cx, ids!(backdrop)).set_visible(cx, has_backdrop);
        // A focused search would bring up a soft keyboard.
        let picker = self.child_refs(cx).picker.clone();
        picker.reset(cx, mode, !has_only_soft_keyboard(cx), !is_sheet);
        self.redraw(cx);
    }

    /// Moves and sizes the `popup_frame` to fit within the window, beside the anchor if it's a popover.
    fn place_popup_frame(&mut self, cx: &mut Cx2d) {
        let origin = self.overlay_origin;
        let window_size = cx.current_pass_size();
        let bounds = get_popup_bounds(cx, origin, self.keyboard_height, EDGE_GAP);
        let (left, top) = (bounds.pos.x, bounds.pos.y);
        let (right, bottom) = (left + bounds.size.x, top + bounds.size.y);
        let available = bounds.size;

        let anchor = match &self.anchor {
            Anchor::InOverlay(rect) => rect.translate(origin),
            // As wide as the emoji button, and as tall as the whole input.
            Anchor::Input { input, button } => {
                let (input_rect, button_rect) = (input.area().rect(cx), button.area().rect(cx));
                Rect { pos: dvec2(button_rect.pos.x, input_rect.pos.y), size: dvec2(button_rect.size.x, input_rect.size.y) }
            }
            Anchor::None => Rect::default(),
        };
        self.anchor_rect = anchor;
        let is_for_input = matches!(self.purpose, Some(EmojiPickerPurpose::Insert { .. }));
        let width = MAX_SIZE.x.min(available.x);
        let rect = if self.is_sheet {
            // A sheet for a message input sits right above it, so the message stays in view.
            let sheet_bottom = if is_for_input { (anchor.pos.y - ANCHOR_GAP).min(bottom) } else { bottom };
            let height = (sheet_bottom - top).max(0.0).min(window_size.y * SHEET_HEIGHT_FRACTION);
            let x = left + (available.x - width) / 2.0;
            Rect { pos: dvec2(x, sheet_bottom - height), size: dvec2(width, height) }
        } else if is_for_input {
            // A popover for a message input fills the space above or below it, lined up with the right edge of its button.
            let space_above = anchor.pos.y - ANCHOR_GAP - top;
            let space_below = bottom - (anchor.pos.y + anchor.size.y + ANCHOR_GAP);
            let is_above = space_above >= MAX_SIZE.y || space_above >= space_below;
            let space = if is_above { space_above } else { space_below };
            let (y, height) = if space >= MIN_POPOVER_HEIGHT {
                let height = space.min(MAX_SIZE.y);
                let y = if is_above { anchor.pos.y - ANCHOR_GAP - height } else { anchor.pos.y + anchor.size.y + ANCHOR_GAP };
                (y, height)
            } else {
                // In a small window, the popover covers part of the message input rather than being cramped.
                let height = MAX_SIZE.y.min(available.y);
                (if is_above { top } else { bottom - height }, height)
            };
            let x = (anchor.pos.x + anchor.size.x - width).clamp(left, (right - width).max(left));
            let y = y.clamp(top, (bottom - height).max(top));
            Rect { pos: dvec2(x, y), size: dvec2(width, height) }
        } else {
            // A popover for a reaction goes right where the context menu that opened it was.
            let size = dvec2(width, MAX_SIZE.y.min(available.y));
            let x = anchor.pos.x.clamp(left, (right - size.x).max(left));
            let y = anchor.pos.y.clamp(top, (bottom - size.y).max(top));
            Rect { pos: dvec2(x, y), size }
        };

        if let Some(mut popup_frame) = self.child_refs(cx).popup_frame.borrow_mut() {
            popup_frame.walk.margin = Inset {
                left: rect.pos.x - origin.x,
                top: rect.pos.y - origin.y,
                right: 0.0,
                bottom: 0.0,
            };
            popup_frame.walk.width = Size::Fixed(rect.size.x);
            popup_frame.walk.height = Size::Fixed(rect.size.y);
        }
    }

    fn close(&mut self, cx: &mut Cx) {
        if self.purpose.take().is_none() {
            return;
        }
        self.cancel_scope = None;
        self.anchor = Anchor::None;
        cx.unblock_scrolling();
        let picker = self.child_refs(cx).picker.clone();
        // The picker's search input took the key focus from whatever had it before, like a message input.
        if picker.has_search_focus(cx) {
            cx.revert_key_focus();
        }
        // This also ends any dictation into the search.
        picker.clear_search(cx);
        self.redraw(cx);
    }
}

impl EmojiPickerPopupRef {
    /// Shows the picker for inserting emoji into the message input with the given widget UID,
    /// beside that input's text input and emoji button.
    pub fn show_insert_picker(&self, cx: &mut Cx, owner: WidgetUid, input: WidgetRef, button: WidgetRef) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, EmojiPickerPurpose::Insert { owner }, EmojiPickerMode::Insert, Anchor::Input { input, button });
    }

    /// Shows the picker for reacting to the message with the given `details` and `reactions`,
    /// at the top-left corner of the given window-absolute `anchor` rect.
    pub fn show_reaction_picker(&self, cx: &mut Cx, details: MessageDetails, reactions: Option<&ReactionsByKeyBySender>, anchor: Rect) {
        let Some(mut inner) = self.borrow_mut() else { return };
        // The picker shows the fully-qualified form of each emoji, e.g., ❤️ for a ❤ that's missing its U+FE0F.
        let own_reactions = reactions.zip(current_user_id())
            .map(|(reactions, user_id)| reactions.iter()
                .filter(|(_, senders)| senders.contains_key(&user_id))
                .map(|(reaction, _)| emojis::get(reaction).map_or(reaction.as_str(), |emoji| emoji.as_str()).to_owned())
                .collect()
            )
            .unwrap_or_default();
        let anchor = Anchor::InOverlay(anchor.translate(-inner.overlay_origin));
        inner.show(cx, EmojiPickerPurpose::React { details }, EmojiPickerMode::React { own_reactions }, anchor);
    }

    pub fn is_open_for(&self, owner: WidgetUid) -> bool {
        self.borrow().is_some_and(|inner| inner.purpose.as_ref().is_some_and(|purpose| purpose.owner() == owner))
    }

    pub fn is_currently_shown(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.purpose.is_some())
    }

    /// Returns the window-absolute rect of the popup's content, as of the last draw.
    pub fn content_rect(&self, cx: &mut Cx) -> Rect {
        let Some(mut inner) = self.borrow_mut() else { return Rect::default() };
        inner.child_refs(cx).main_content.area().rect(cx)
    }

    pub fn close(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close(cx);
        }
    }

    /// Closes the popup if the widget it was opened for is hidden or gone.
    /// Call this at the very top of the app's event handling, before any widget handles the event (see [`Cx::widget_is_active`]).
    pub fn close_if_owner_inactive(&self, cx: &mut Cx) {
        let owner = self.borrow().and_then(|inner| inner.purpose.as_ref().map(EmojiPickerPurpose::owner));
        if owner.is_some_and(|owner| !cx.widget_is_active(owner)) {
            self.close(cx);
        }
    }
}

/// Registers the app-wide emoji picker popup as a global on `Cx`.
pub fn set_global_emoji_picker_popup(cx: &mut Cx, parent_ref: &WidgetRef) {
    Cx::set_global(cx, parent_ref.emoji_picker_popup(cx, ids!(emoji_picker_popup)));
}

/// Returns the app-wide emoji picker popup.
pub fn get_emoji_picker_popup(cx: &mut Cx) -> EmojiPickerPopupRef {
    if cx.has_global::<EmojiPickerPopupRef>() {
        cx.get_global::<EmojiPickerPopupRef>().clone()
    } else {
        EmojiPickerPopupRef::default()
    }
}
