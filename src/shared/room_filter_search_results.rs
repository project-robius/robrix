//! A FlatList-based search results widget for the room filter modal.
//!
//! This module provides:
//! - `RoomFilterSearchResultsList`: A widget containing a FlatList of search results
//! - `RoomFilterSearchResultItem`: Individual clickable search result items
//! - `RoomFilterResultAction`: Actions emitted when results are clicked

use makepad_widgets::*;
use matrix_sdk::{RoomDisplayName, ruma::{OwnedMxcUri, OwnedRoomId}};

use crate::{
    avatar_cache::{self, AvatarCacheEntry},
    profile::user_profile::UserProfile,
    room::FetchedRoomAvatar,
    shared::avatar::AvatarWidgetExt,
    utils::RoomNameId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // Individual search result item template
    mod.widgets.RoomFilterSearchResultItem = #(RoomFilterSearchResultItem::register_widget(vm)) {
        width: Fill
        height: 55
        flow: Overlay

        row := View {
            width: Fill
            height: Fill
            flow: Right
            align: Align{y: 0.5}
            spacing: 8
            padding: Inset{left: 8, right: 8, top: 5, bottom: 5}

            avatar := Avatar { width: 30, height: 30 }

            text_col := View {
                width: Fill
                height: Fit
                flow: Down
                spacing: 0

                name_label := Label {
                    width: Fill
                    height: Fit
                    draw_text +: {
                        color: (COLOR_TEXT)
                        text_style: REGULAR_TEXT {font_size: 10}
                    }
                }

                id_label := Label {
                    width: Fill
                    height: Fit
                    draw_text +: {
                        color: (COLOR_TEXT_INPUT_IDLE)
                        text_style: REGULAR_TEXT {font_size: 8.5}
                    }
                }
            }
        }

        click_button := RobrixNeutralIconButton {
            width: Fill
            height: Fill
            text: ""
            icon_walk: Walk{width: 0, height: 0}
            draw_bg +: {
                color: #0000
                color_hover: #FFFFFF22
                color_down: #FFFFFF11
            }
        }
    }

    // The FlatList container for search results
    mod.widgets.RoomFilterSearchResultsList = #(RoomFilterSearchResultsList::register_widget(vm)) {
        width: Fill
        height: Fit
        flow: Down
        spacing: 3

        list := FlatList {
            width: Fill
            height: Fit
            spacing: 3.0
            flow: Down

            grab_key_focus: false
            drag_scrolling: false
            scroll_bars +: { show_scroll_x: false, show_scroll_y: false }

            result_item := mod.widgets.RoomFilterSearchResultItem {}
        }
    }
}

/// The data for a single search result item, passed via Scope.
#[derive(Clone, Debug)]
pub enum RoomFilterResultTarget {
    LocalSpace { room_name_id: RoomNameId, avatar: FetchedRoomAvatar },
    LocalRoom { room_name_id: RoomNameId, avatar: FetchedRoomAvatar },
    RemoteSpace { space_name_id: RoomNameId, avatar_uri: Option<OwnedMxcUri> },
    RemoteRoom { room_name_id: RoomNameId, avatar_uri: Option<OwnedMxcUri> },
    RemoteUser(UserProfile),
}

impl RoomFilterResultTarget {
    /// Returns the display name and raw ID for this result.
    pub fn name_and_id(&self) -> (String, String) {
        match self {
            RoomFilterResultTarget::LocalSpace { room_name_id, .. }
            | RoomFilterResultTarget::LocalRoom { room_name_id, .. } => {
                (room_name_id.to_string(), room_name_id.room_id().to_string())
            }
            RoomFilterResultTarget::RemoteSpace { space_name_id, .. }
            | RoomFilterResultTarget::RemoteRoom { room_name_id: space_name_id, .. } => {
                (space_name_id.to_string(), space_name_id.room_id().to_string())
            }
            RoomFilterResultTarget::RemoteUser(user_profile) => {
                (user_profile.displayable_name().to_owned(), user_profile.user_id.to_string())
            }
        }
    }
}

/// Action emitted when a search result is clicked.
#[derive(Clone, Debug, Default)]
pub enum RoomFilterResultAction {
    #[default]
    None,
    Clicked(RoomFilterResultTarget),
}

/// Individual search result item widget.
#[derive(Script, ScriptHook, Widget)]
pub struct RoomFilterSearchResultItem {
    #[deref] view: View,
    #[rust] target: Option<RoomFilterResultTarget>,
}

impl Widget for RoomFilterSearchResultItem {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(click_button)).clicked(actions) {
                if let Some(target) = self.target.clone() {
                    cx.action(RoomFilterResultAction::Clicked(target));
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Get the target from scope props
        if let Some(target) = scope.props.get::<RoomFilterResultTarget>() {
            self.target = Some(target.clone());
            let (name, raw_id) = target.name_and_id();

            self.view.label(cx, ids!(row.text_col.name_label)).set_text(cx, &name);
            self.view.label(cx, ids!(row.text_col.id_label)).set_text(cx, &raw_id);

            let avatar_ref = self.view.avatar(cx, ids!(row.avatar));
            self.set_avatar(cx, &avatar_ref, &name, target);
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomFilterSearchResultItem {
    fn set_avatar(
        &self,
        cx: &mut Cx2d,
        avatar_ref: &crate::shared::avatar::AvatarRef,
        fallback_text: &str,
        target: &RoomFilterResultTarget,
    ) {
        match target {
            RoomFilterResultTarget::LocalSpace { avatar, .. }
            | RoomFilterResultTarget::LocalRoom { avatar, .. } => {
                match avatar {
                    FetchedRoomAvatar::Text(text) => {
                        avatar_ref.show_text(cx, None, None, text);
                    }
                    FetchedRoomAvatar::Image(image_data) => {
                        let res = avatar_ref.show_image(
                            cx,
                            None,
                            |cx, img_ref| crate::utils::load_png_or_jpg(&img_ref, cx, image_data),
                        );
                        if res.is_err() {
                            avatar_ref.show_text(cx, None, None, fallback_text);
                        }
                    }
                }
            }
            RoomFilterResultTarget::RemoteSpace { avatar_uri, .. }
            | RoomFilterResultTarget::RemoteRoom { avatar_uri, .. } => {
                if let Some(uri) = avatar_uri {
                    if let AvatarCacheEntry::Loaded(image_data) = avatar_cache::get_or_fetch_avatar(cx, uri) {
                        let res = avatar_ref.show_image(
                            cx,
                            None,
                            |cx, img_ref| crate::utils::load_png_or_jpg(&img_ref, cx, &image_data),
                        );
                        if res.is_ok() {
                            return;
                        }
                    }
                }
                avatar_ref.show_text(cx, None, None, fallback_text);
            }
            RoomFilterResultTarget::RemoteUser(user_profile) => {
                if let Some(image_data) = user_profile.avatar_state.data() {
                    let res = avatar_ref.show_image(
                        cx,
                        None,
                        |cx, img_ref| crate::utils::load_png_or_jpg(&img_ref, cx, image_data),
                    );
                    if res.is_ok() {
                        return;
                    }
                }
                if let Some(uri) = user_profile.avatar_state.uri() {
                    if let AvatarCacheEntry::Loaded(image_data) = avatar_cache::get_or_fetch_avatar(cx, uri) {
                        let res = avatar_ref.show_image(
                            cx,
                            None,
                            |cx, img_ref| crate::utils::load_png_or_jpg(&img_ref, cx, &image_data),
                        );
                        if res.is_ok() {
                            return;
                        }
                    }
                }
                avatar_ref.show_text(cx, None, None, fallback_text);
            }
        }
    }
}

/// The FlatList-based search results list widget.
#[derive(Script, ScriptHook, Widget)]
pub struct RoomFilterSearchResultsList {
    #[deref] view: View,
    #[rust] results: Vec<RoomFilterResultTarget>,
}

impl Widget for RoomFilterSearchResultsList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            let flat_list_ref = subview.as_flat_list();
            let Some(mut list) = flat_list_ref.borrow_mut() else {
                continue;
            };

            for (index, target) in self.results.iter().enumerate() {
                let item_id = LiveId(index as u64);
                let item = list.item(cx, item_id, id!(result_item)).unwrap();
                let mut scope = Scope::with_props(target);
                item.draw_all(cx, &mut scope);
            }
        }
        DrawStep::done()
    }
}

impl RoomFilterSearchResultsList {
    /// Set the search results to display.
    pub fn set_results(&mut self, cx: &mut Cx, results: Vec<RoomFilterResultTarget>) {
        self.results = results;
        self.view.redraw(cx);
    }

    /// Clear all search results.
    pub fn clear(&mut self, cx: &mut Cx) {
        self.results.clear();
        self.view.redraw(cx);
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Get the number of results.
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Populate with default test data for development/testing.
    #[allow(dead_code)]
    pub fn populate_test_data(&mut self, cx: &mut Cx) {
        let test_results = vec![
            RoomFilterResultTarget::LocalRoom {
                room_name_id: RoomNameId::new(
                    RoomDisplayName::Named("General Chat".to_string()),
                    OwnedRoomId::try_from("!abc123:127.0.0.1:8128").unwrap(),
                ),
                avatar: FetchedRoomAvatar::Text("GC".to_string()),
            },
            RoomFilterResultTarget::LocalRoom {
                room_name_id: RoomNameId::new(
                    RoomDisplayName::Named("Development".to_string()),
                    OwnedRoomId::try_from("!dev456:127.0.0.1:8128").unwrap(),
                ),
                avatar: FetchedRoomAvatar::Text("DE".to_string()),
            },
            RoomFilterResultTarget::LocalRoom {
                room_name_id: RoomNameId::new(
                    RoomDisplayName::Named("Random".to_string()),
                    OwnedRoomId::try_from("!rand789:127.0.0.1:8128").unwrap(),
                ),
                avatar: FetchedRoomAvatar::Text("RA".to_string()),
            },
            RoomFilterResultTarget::LocalSpace {
                room_name_id: RoomNameId::new(
                    RoomDisplayName::Named("Project Alpha".to_string()),
                    OwnedRoomId::try_from("!alpha:127.0.0.1:8128").unwrap(),
                ),
                avatar: FetchedRoomAvatar::Text("PA".to_string()),
            },
            RoomFilterResultTarget::LocalRoom {
                room_name_id: RoomNameId::new(
                    RoomDisplayName::Named("Support".to_string()),
                    OwnedRoomId::try_from("!support:127.0.0.1:8128").unwrap(),
                ),
                avatar: FetchedRoomAvatar::Text("SU".to_string()),
            },
        ];
        self.set_results(cx, test_results);
    }
}

impl RoomFilterSearchResultsListRef {
    /// Set the search results to display.
    pub fn set_results(&self, cx: &mut Cx, results: Vec<RoomFilterResultTarget>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_results(cx, results);
        }
    }

    /// Clear all search results.
    pub fn clear(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear(cx);
        }
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.borrow().map(|inner| inner.is_empty()).unwrap_or(true)
    }

    /// Get the number of results.
    pub fn len(&self) -> usize {
        self.borrow().map(|inner| inner.len()).unwrap_or(0)
    }

    /// Populate with default test data for development/testing.
    #[allow(dead_code)]
    pub fn populate_test_data(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.populate_test_data(cx);
        }
    }
}
