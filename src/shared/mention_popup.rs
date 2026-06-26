//! The autocomplete popup for selecting something to insert into a MentionableTextInput.
//!
//! See MentionableTextInput for more info for how it works with that.
//! The main widget is an overlay so it can display anywhere within the app window.
//!
//! Currently it can show 3 kinds of content in the popup, each rendered by a `MentionRow`:
//! 1. Users to mention within a given room.
//! 2. Rooms or spaces to insert a link to.
//! 3. Slash commands.
//!

use std::sync::Arc;
use makepad_widgets::*;
use crate::{
    avatar_cache::{get_or_fetch_avatar, process_avatar_updates, AvatarCacheEntry},
    home::rooms_list::RoomsListRef,
    room::FetchedRoomAvatar,
    shared::{avatar::AvatarWidgetRefExt, slash_commands::SlashCommand, styles::*},
    utils::{self, RoomNameId},
};
use matrix_sdk::ruma::{OwnedMxcUri, OwnedRoomAliasId, OwnedRoomId, OwnedUserId};

// Note: most of the layout dimensions of the popup have to be calc'd in Rust code,
//       so they're defined here up front for clarity.
const POPUP_MAX_WIDTH: f64 = 600.0;
const HEADER_HEIGHT: f64 = 48.0;
const ROW_HEIGHT: f64 = 52.0;
const MAX_VISIBLE_ROWS: f64 = 7.0;
/// Padding around the list of suggestions to make it look a bit nicer within the popup.
const LIST_PADDING: f64 = 6.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // The row shown for each match within the popup, e.g., a user, room/space, or command.
    mod.widgets.MentionRow = RoundedView {
        width: Fill, height: #(ROW_HEIGHT), flow: Right, spacing: 9, align: Align{y: 0.5}
        padding: Inset{left: 10, right: 10}
        show_bg: true, cursor: MouseCursor.Hand
        draw_bg +: { color: #00000000, border_radius: 5.0 }
        avatar := Avatar { width: 30, height: 30 }
        info := View {
            width: Fill, height: Fit, flow: Down, spacing: 1
            title := Label {
                width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                draw_text +: { color: (COLOR_TEXT), text_style: theme.font_bold {font_size: 11, line_spacing: 1.0} }
            }
            subtitle := Label {
                width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                draw_text +: { color: #555, text_style: theme.font_regular {font_size: 9.5, line_spacing: 1.0} }
            }
        }
    }

    // The row shown while matches are still loading.
    mod.widgets.MentionLoadingRow = View {
        width: Fill, height: #(ROW_HEIGHT), flow: Right, spacing: 10, align: Align{x: 0.5, y: 0.5}
        loading_spinner := LoadingSpinner {
            width: 20, height: 20
            draw_bg +: { color: (COLOR_ACTIVE_PRIMARY), border_size: 2.5 }
        }
        loading_label := Label {
            height: Fit
            draw_text +: { color: #555, text_style: theme.font_regular {font_size: 10.5} }
        }
    }

    // The row shown when there are no matches.
    mod.widgets.MentionEmptyRow = View {
        width: Fill, height: #(ROW_HEIGHT), align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 12, right: 12}
        empty_label := Label {
            width: Fill, height: Fit, max_lines: 2, text_overflow: Ellipsis
            align: Align{x: 0.5}
            draw_text +: { color: #555, text_style: theme.font_regular {font_size: 10.5} }
        }
    }

    mod.widgets.MentionablePopup = #(MentionablePopup::register_widget(vm)) {
        width: Fill
        height: Fill
        flow: Overlay
        align: Align{x: 0.0, y: 0.0}

        color_focus: #xB6D3F2
        color_hover: #xEAEFF5

        // So the way this works is that we move the popup_frame wrapper view,
        // which allows the `main_content` to just behave like a regular Fill/Fill view.
        popup_frame := View {
            clip_x: false, clip_y: false
            width: Fit, height: Fit

            main_content := RoundedShadowView {
                width: Fill, height: Fill
                flow: Down
                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    border_radius: 5.0
                    border_size: 1.0
                    border_color: (COLOR_SECONDARY)
                    shadow_color: #0006
                    shadow_radius: 12.0
                    shadow_offset: vec2(0.0, 3.0)
                }

                header_view := RoundedView {
                    width: Fill, height: #(HEADER_HEIGHT)
                    flow: Right
                    align: Align{y: 0.5}
                    padding: Inset{left: 16, right: 16}
                    show_bg: true
                    draw_bg +: {
                        color: (COLOR_ROBRIX_PURPLE)
                        border_radius: 5.0
                    }
                    header_label := Label {
                        width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                        draw_text +: {
                            color: (COLOR_PRIMARY)
                            text_style: theme.font_bold {font_size: 13.0, line_spacing: 1.0}
                        }
                    }
                }

                list_container := View {
                    width: Fill, height: Fill
                    padding: #(LIST_PADDING)
                    list := PortalList {
                        width: Fill, height: Fill
                        flow: Down
                        row := mod.widgets.MentionRow {}
                        command_row := mod.widgets.MentionRow {
                            avatar := View { width: 0, height: 0 }
                        }
                        loading_row := mod.widgets.MentionLoadingRow {}
                        empty_row := mod.widgets.MentionEmptyRow {}
                    }
                }
            }
        }
    }
}

/// A match suggested in the room/space mention popup.
#[derive(Clone, Debug)]
pub struct RoomMentionCandidate {
    pub room_name_id: RoomNameId,
    pub alias: Option<OwnedRoomAliasId>,
    pub avatar_url: Option<OwnedMxcUri>,
    pub is_space: bool,
}

/// A matching user, room/space, or slash command.
#[derive(Clone, Debug)]
pub enum MentionItem {
    User {
        user_id: OwnedUserId,
        display_name: String,
        avatar_url: Option<OwnedMxcUri>,
    },
    NotifyRoom { room_name: String },
    Room(RoomMentionCandidate),
    Command(&'static SlashCommand),
}

#[derive(Clone, Debug, Default)]
enum MentionablePopupAction {
    /// The suggested match at the given item index was clicked.
    ClickedItem(usize),
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct MentionablePopup {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    #[live] color_focus: Vec4f,
    #[live] color_hover: Vec4f,

    #[rust] is_open: bool,
    /// The location of the text input cursor that this popup is positioned near.
    #[rust] anchor_rect: Rect,
    /// The MentionableTextInput widget instance that opened and is controlling this popup.
    #[rust] owner: Option<WidgetUid>,
    /// The starting byte of the trigger token within this text input's content.
    #[rust] trigger_start_byte: Option<usize>,

    /// The actual matches that we should show, in ranked order.
    #[rust] items: Arc<Vec<MentionItem>>,
    /// Message shown when there were no matches for the current query.
    #[rust] empty_message: String,
    /// The message shown while loading more matches.
    #[rust] loading_message: String,
    #[rust] is_loading: bool,

    /// The index in `items` of which item we're focusing on via keyboard navigation.
    #[rust] keyboard_focus_index: Option<usize>,
    /// The index in `items` of which item we're focusing on via mouse hover.
    #[rust] pointer_hover_index: Option<usize>,
    /// The last-drawn height of the list itself; used to help align the item selected
    /// via keyboard nav to the bottom of the viewport.
    #[rust] list_viewport_height: f64,
    /// Whether all of the avatars in the currently-visible rows have been fully drawn.
    /// If `false`, we'll try to update the avatar cache and re-draw avatars upon a UI Signal.
    #[rust] is_fully_drawn: bool,
}

impl Widget for MentionablePopup {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.is_open {
            return;
        }

        self.view.handle_event(cx, event, scope);

        if !self.is_fully_drawn && matches!(event, Event::Signal) {
            process_avatar_updates(cx);
            self.redraw(cx);
        }

        if let Event::Actions(actions) = event {
            let list = self.portal_list(cx, ids!(main_content.list_container.list));

            if actions.iter().any(|a| matches!(a.as_widget_action().cast(), WindowAction::WindowGeomChange(_))) {
                self.redraw(cx);
            }

            let mut clicked_item_idx = None;
            let mut should_redraw = false;
            for (index, widget) in list.items_with_actions(actions) {
                let item = widget.as_view();
                // Don't treat a touch that drags (a scroll motion) as a regular tap/click.
                if !list.was_scrolling() {
                    if let Some(fe) = item.finger_up(actions) {
                        if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                            clicked_item_idx = Some(index);
                        }
                    }
                }
                if item.finger_hover_in(actions).is_some() {
                    self.pointer_hover_index = Some(index);
                    self.keyboard_focus_index = Some(index);
                    should_redraw = true;
                }
                if item.finger_hover_out(actions).is_some() && self.pointer_hover_index == Some(index) {
                    self.pointer_hover_index = None;
                    should_redraw = true;
                }
            }
            if let Some(index) = clicked_item_idx {
                if index < self.items.len() {
                    cx.widget_action(self.widget_uid(), MentionablePopupAction::ClickedItem(index));
                }
            }
            if should_redraw {
                self.redraw(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.is_open {
            return DrawStep::done();
        }

        self.position_content(cx);

        // We treat the whole widget as fully drawn initially. If any avatars aren't available yet,
        // then we set it to false so that future avatar updates can be grabbed and redrawn.
        let mut fully_drawn = true;

        while let Some(widget) = self.view.draw_walk(cx, scope, walk).step() {
            let portal_list = widget.as_portal_list();
            let Some(mut list) = portal_list.borrow_mut() else { continue };
            // With no matches we draw a single loading or empty row.
            let count = self.items.len().max(1);
            list.set_item_range(cx, 0, count);
            while let Some(index) = list.next_visible_item(cx) {
                if index >= count {
                    continue;
                }
                let row = match self.items.get(index) {
                    Some(mention) => {
                        let mut row_widget = build_row(cx, &mut list, index, mention, &mut fully_drawn);
                        let color = if self.keyboard_focus_index == Some(index) {
                            self.color_focus
                        } else if self.pointer_hover_index == Some(index) {
                            self.color_hover
                        } else {
                            Vec4f::default()
                        };
                        script_apply_eval!(cx, row_widget, { draw_bg.color: #(color) });
                        row_widget
                    }
                    None if self.is_loading => {
                        let loading_row = list.item(cx, index, id!(loading_row));
                        loading_row.label(cx, ids!(loading_label)).set_text(cx, &self.loading_message);
                        loading_row
                    }
                    None => {
                        let empty_row = list.item(cx, index, id!(empty_row));
                        empty_row.label(cx, ids!(empty_label)).set_text(cx, &self.empty_message);
                        empty_row
                    }
                };
                row.draw_all(cx, scope);
            }
        }
        self.is_fully_drawn = fully_drawn;

        // Block scrolling everywhere except inside the box.
        let content_area = self.view.view(cx, ids!(main_content)).area();
        cx.block_scrolling_except_within(content_area);
        DrawStep::done()
    }
}

impl MentionablePopup {
    /// Recalculates and updates the position of the `popup_frame`.
    fn position_content(&mut self, cx: &mut Cx2d) {
        let window_size = cx.current_pass_size();
        let edge = 10.0;
        let gap = 16.0; // space between the current line of text and the popup border
        let anchor = self.anchor_rect;
        // The anchor is in absolute coords (relative to the whole app window),
        // but the margins are relatively to the overlay container.
        let overlay = cx.turtle().origin();

        // Align the left side of the popup with the current textinput's cursor location,
        // and then ensure that the width is either limited by the app window size or a fixed max value.
        let width = (window_size.x - 2.0 * edge).clamp(0.0, POPUP_MAX_WIDTH);
        let left_win = anchor.pos.x.clamp(edge, (window_size.x - edge - width).max(edge));

        // Measure space inside the overlay body, not the window: its top is at
        // overlay.y, so the popup never spills up into the caption.
        let top_limit = overlay.y + edge;
        let bottom_limit = window_size.y - edge;
        // Must match header_view's fixed height in the DSL.
        let header_h = HEADER_HEIGHT;
        let space_above = (anchor.pos.y - gap - top_limit).max(0.0);
        let space_below = (bottom_limit - anchor.pos.y - anchor.size.y - gap).max(0.0);
        let is_above_text_input = space_above >= space_below;
        let available_space = if is_above_text_input { space_above } else { space_below };
        // Shrink the list so header + list fits the space we have, capped to
        // MAX_VISIBLE_ROWS. Anything past that scrolls.
        let list_cap = (MAX_VISIBLE_ROWS * ROW_HEIGHT + 2.0 * LIST_PADDING)
            .min((available_space - header_h).max(0.0));
        let row_count = self.items.len().max(1) as f64;
        let list_height = (row_count * ROW_HEIGHT + 2.0 * LIST_PADDING).min(list_cap).max(0.0);
        self.list_viewport_height = (list_height - 2.0 * LIST_PADDING).max(0.0);
        let box_height = header_h + list_height;

        // Box top: above or below the caret line, clamped to stay inside the body.
        let top_win = if is_above_text_input {
            (anchor.pos.y - gap - box_height).max(top_limit)
        } else {
            (anchor.pos.y + anchor.size.y + gap).min(bottom_limit - box_height)
        };

        // As with context menus, we use a margin to position the popup_frame.
        if let Some(mut popup_frame) = self.view.view(cx, ids!(popup_frame)).borrow_mut() {
            let margin = Inset {
                left: left_win - overlay.x,
                top: top_win - overlay.y,
                right: 0.0,
                bottom: 0.0,
            };
            popup_frame.walk.abs_pos = None;
            popup_frame.walk.margin = margin;
            popup_frame.walk.width = Size::Fixed(width);
            popup_frame.walk.height = Size::Fixed(box_height);
        }
    }

    fn content_rect(&self, cx: &mut Cx) -> Rect {
        self.view.view(cx, ids!(main_content)).area().rect(cx)
    }
}

impl MentionablePopupRef {
    /// Shows the popup for the given `owner`, anchored near the text cursor.
    ///
    /// ## Arguments
    /// * `owner`: the `MentionableTextInput` that has opened and is controlling this popup.
    /// * `anchor_rect`: the cursor rect (current line of text) that the popup is positioned next to.
    /// * `trigger_start_byte`: byte offset of the trigger char within the currently-entered text.
    /// * `header`: the title text shown at the top of the popup.
    /// * `loading_message`: text shown next to the loading spinner while matches are loading.
    pub fn show(
        &self,
        cx: &mut Cx,
        owner: WidgetUid,
        anchor_rect: Rect,
        trigger_start_byte: usize,
        header: &str,
        loading_message: &str,
    ) {
        {
            let Some(mut inner) = self.borrow_mut() else { return };
            let is_first_anchor = inner.owner != Some(owner)
                || inner.trigger_start_byte != Some(trigger_start_byte);
            inner.owner = Some(owner);
            if is_first_anchor {
                inner.anchor_rect = anchor_rect;
                inner.trigger_start_byte = Some(trigger_start_byte);
            }
            inner.is_open = true;
            inner.view.label(cx, ids!(main_content.header_view.header_label)).set_text(cx, header);
            inner.loading_message = loading_message.to_string();
            inner.redraw(cx);
        }
        // Always reset the scroll position when showing a new popup.
        self.portal_list(cx, ids!(main_content.list_container.list)).set_first_id_and_scroll(0, 0.0);
    }

    /// Replaces the currently-shown matches in an existing popup.
    ///
    /// If the `owner` doesn't match the current owner of the already-shown popup, this does nothing.
    ///
    /// ## Arguments
    /// * `owner`: the `MentionableTextInput` that has opened and is controlling this popup.
    /// * `items`: the items that matched the previous query, ranked in order of priority.
    /// * `is_loading`: whether or not the items are still loading.
    /// * `empty_message`: the text to show if no matches were found.
    pub fn set_results(
        &self,
        cx: &mut Cx,
        owner: WidgetUid,
        items: Arc<Vec<MentionItem>>,
        is_loading: bool,
        empty_message: &str,
    ) {
        {
            let Some(mut inner) = self.borrow_mut() else { return };
            if inner.owner != Some(owner) {
                return;
            }
            inner.items = items;
            inner.is_loading = is_loading;
            inner.empty_message = empty_message.to_string();
            // These are fresh results, so move the focus back to the first item/row.
            inner.keyboard_focus_index = (!inner.items.is_empty()).then_some(0);
            inner.pointer_hover_index = None;
            inner.redraw(cx);
        }
        self.portal_list(cx, ids!(main_content.list_container.list)).set_first_id_and_scroll(0, 0.0);
    }

    pub fn hide(&self, cx: &mut Cx, owner: WidgetUid) {
        if self.is_open_for(owner) {
            self.cancel(cx);
        }
    }

    /// Closes the popup and resets its state.
    pub fn cancel(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            if !inner.is_open {
                return;
            }
            inner.is_open = false;
            inner.owner = None;
            inner.trigger_start_byte = None;
            inner.items = Default::default();
            inner.keyboard_focus_index = None;
            inner.pointer_hover_index = None;
            cx.unblock_scrolling();
            inner.redraw(cx);
        }
    }

    pub fn is_open_for(&self, owner: WidgetUid) -> bool {
        self.borrow().is_some_and(|inner| inner.is_open && inner.owner == Some(owner))
    }

    /// Updates the anchor rect, and thus the location, of this popup without changing its content.
    ///
    /// This is useful for events like the window being resized.
    pub fn set_anchor(&self, cx: &mut Cx, owner: WidgetUid, anchor: Rect) {
        if let Some(mut inner) = self.borrow_mut() {
            if inner.is_open && inner.owner == Some(owner) && inner.anchor_rect != anchor {
                inner.anchor_rect = anchor;
                inner.redraw(cx);
            }
        }
    }

    pub fn content_rect(&self, cx: &mut Cx) -> Rect {
        self.borrow().map_or(Rect::default(), |inner| inner.content_rect(cx))
    }

    /// Moves the keyboard focus by `delta` and returns the newly-selected item index.
    ///
    /// It scrolls and repositions the view nicely to ensure the selected item is now visible.
    pub fn move_focus(&self, cx: &mut Cx, delta: i32) -> Option<usize> {
        let (new_index, viewport) = {
            let mut inner = self.borrow_mut()?;
            if inner.items.is_empty() {
                return None;
            }
            let last = inner.items.len() - 1;
            let new_index = match inner.keyboard_focus_index {
                Some(idx) => idx.saturating_add_signed(delta as isize).min(last),
                None if delta > 0 => 0,
                None => last,
            };
            inner.keyboard_focus_index = Some(new_index);
            inner.pointer_hover_index = None;
            (new_index, inner.list_viewport_height)
        };

        // Ensure the selected row is fully visible and "snapped" to a row boundary,
        // otherwise it looks kinda shitty to have a keyboard selection only partially visible.
        let list = self.portal_list(cx, ids!(main_content.list_container.list));
        // Number of fully-visible rows from the actual drawn viewport.
        let viewport_height = list.area().rect(cx).size.y.max(viewport);
        let visible_rows = ((viewport_height / ROW_HEIGHT).floor() as usize).max(1);
        let first = list.first_id();
        let new_first = if new_index < first {
            new_index
        } else if new_index + 1 > first + visible_rows {
            new_index + 1 - visible_rows
        } else {
            first
        };
        list.set_first_id_and_scroll(new_first, 0.0);
        self.redraw(cx);
        Some(new_index)
    }

    /// Returns the item currently selected/focused by the keyboard navigation.
    pub fn focused_item(&self) -> Option<MentionItem> {
        let inner = self.borrow()?;
        inner.keyboard_focus_index.and_then(|i| inner.items.get(i).cloned())
    }

    pub fn item_at(&self, index: usize) -> Option<MentionItem> {
        self.borrow().and_then(|inner| inner.items.get(index).cloned())
    }

    pub fn clicked_item(&self, actions: &Actions) -> Option<MentionItem> {
        let uid = self.widget_uid();
        let index = actions.iter()
            .filter_map(|a| a.as_widget_action())
            .filter(|a| a.widget_uid == uid)
            .find_map(|a| match a.cast() {
                MentionablePopupAction::ClickedItem(idx) => Some(idx),
                _ => None,
            })?;
        self.item_at(index)
    }
}

/// Registers the app-level popup as a global on Cx.
pub fn set_global_mention_popup(cx: &mut Cx, parent_ref: &WidgetRef) {
    Cx::set_global(cx, parent_ref.mentionable_popup(cx, ids!(mention_popup)));
}

fn build_row(cx: &mut Cx, list: &mut PortalList, index: usize, item: &MentionItem, fully_drawn: &mut bool) -> WidgetRef {
    match item {
        MentionItem::User { user_id, display_name, avatar_url } => {
            let new_widget = list.item(cx, index, id!(row));
            new_widget.label(cx, ids!(info.title)).set_text(cx, display_name);
            new_widget.label(cx, ids!(info.subtitle)).set_text(cx, user_id.as_str());
            *fully_drawn &= set_user_avatar(cx, &new_widget, avatar_url.as_ref(), display_name);
            new_widget
        }
        MentionItem::NotifyRoom { room_name } => {
            let new_widget = list.item(cx, index, id!(row));
            new_widget.label(cx, ids!(info.title)).set_text(cx, "Notify the entire room");
            new_widget.label(cx, ids!(info.subtitle)).set_text(cx, "@room");
            let first = room_name.chars().next().map(|c| c.to_string()).unwrap_or_else(|| "@".into());
            new_widget.avatar(cx, ids!(avatar)).show_text(cx, Some(COLOR_ROBRIX_PURPLE), None, &first);
            new_widget
        }
        MentionItem::Room(candidate) => {
            let new_widget = list.item(cx, index, id!(row));
            let name = if candidate.is_space {
                format!("[Space] {}", candidate.room_name_id)
            } else {
                candidate.room_name_id.to_string()
            };
            new_widget.label(cx, ids!(info.title)).set_text(cx, &name);
            let alias = candidate.alias.as_ref().map(|a| a.as_str()).unwrap_or("");
            new_widget.label(cx, ids!(info.subtitle)).set_text(cx, alias);
            *fully_drawn &= set_room_avatar(cx, &new_widget, candidate.room_name_id.room_id(), candidate.avatar_url.as_ref(), candidate.room_name_id.name_for_avatar());
            new_widget
        }
        MentionItem::Command(cmd) => {
            let new_widget = list.item(cx, index, id!(command_row));
            new_widget.label(cx, ids!(info.title)).set_text(cx, &format!("/{}", cmd.name));
            new_widget.label(cx, ids!(info.subtitle)).set_text(cx, cmd.description);
            new_widget
        }
    }
}

/// Returns `true` once the avatar is fully drawn, `false` if it's still being fetched.
fn set_user_avatar(cx: &mut Cx, row: &WidgetRef, avatar_url: Option<&OwnedMxcUri>, display: &str) -> bool {
    let avatar = row.avatar(cx, ids!(avatar));
    match avatar_url {
        Some(mxc) => match get_or_fetch_avatar(cx, mxc) {
            AvatarCacheEntry::Loaded(data) => {
                let _ = avatar.show_image(cx, None, |cx, img| utils::load_image(&img, cx, &data));
                true
            }
            AvatarCacheEntry::Requested => {
                avatar.show_text(cx, None, None, display);
                false
            }
            AvatarCacheEntry::Failed => {
                avatar.show_text(cx, None, None, display);
                true
            }
        },
        None => {
            avatar.show_text(cx, None, None, display);
            true
        }
    }
}

/// Resolves a room's avatar using the rooms list's metadata or the avatar cache.
fn set_room_avatar(cx: &mut Cx, row: &WidgetRef, room_id: &OwnedRoomId, avatar_url: Option<&OwnedMxcUri>, name_for_avatar: Option<&str>) -> bool {
    let avatar = row.avatar(cx, ids!(avatar));
    if cx.has_global::<RoomsListRef>() {
        if let Some(FetchedRoomAvatar::Image(data)) = cx.get_global::<RoomsListRef>().get_room_avatar(room_id) {
            let _ = avatar.show_image(cx, None, |cx, img| utils::load_image(&img, cx, &data));
            return true;
        }
    }
    let mut fully_drawn = true;
    if let Some(mxc) = avatar_url {
        match get_or_fetch_avatar(cx, mxc) {
            AvatarCacheEntry::Loaded(data) => {
                let _ = avatar.show_image(cx, None, |cx, img| utils::load_image(&img, cx, &data));
                return true;
            }
            AvatarCacheEntry::Requested => fully_drawn = false,
            AvatarCacheEntry::Failed => {}
        }
    }
    if let FetchedRoomAvatar::Text(fallback) = utils::avatar_from_room_name(name_for_avatar) {
        avatar.show_text(cx, Some(COLOR_UNKNOWN_ROOM_AVATAR), None, &fallback);
    }
    fully_drawn
}
