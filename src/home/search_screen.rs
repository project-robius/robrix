use std::collections::BTreeMap;

use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails};
use rangemap::RangeSet;

use crate::{home::room_screen::{search_result::{self, handle_search_input, SearchResultWidgetExt}, SearchResultItem}, shared::message_search_input_bar::MessageSearchAction};
/// States that are necessary to display search results.
#[derive(Default)]
pub struct SearchState {
    /// The list of events in the search results.
    pub items: Vector<SearchResultItem>,
    /// The list of strings that should be highlighted in the search results.
    pub highlighted_strings: Vec<String>,
    /// See [`TimelineUiState.content_drawn_since_last_update`].
    pub content_drawn_since_last_update: RangeSet<usize>,
    /// Same as `content_drawn_since_last_update`, but for the event **profiles** (avatar, username).
    pub profile_drawn_since_last_update: RangeSet<usize>,
    /// All profile infos for the search results.
    pub profile_infos: BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
    pub fully_paginated: bool,
    /// The index of the timeline item that was most recently scrolled up past it.
    pub last_scrolled_index: usize,
    /// Token to be use for pagination of earlier search results.
    pub next_batch_token: Option<String>,
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::*;
    use crate::shared::message_search_input_bar::*;
    use crate::home::room_screen::search_result::*;
    use crate::home::room_screen::*;

    pub SearchScreen = {{SearchScreen}} {
        <View> {
            width: Fill,
            height: Fill,
            flow: Down,
            debug: true
            message_search_input_view = <View> {
                width: Fill, height: Fit,
                visible: true,
                <CachedWidget> {
                    message_search_input_bar = <MessageSearchInputBar> {
                        width: Fill,
                    }
                }
            }
            <View> {
                width: Fill,
                height: Fill,
                flow: Overlay,
                search_timeline = <Timeline> {
                    width: Fill,
                    height: Fill,
                }
                search_result_plane = <SearchResult> {
                    width: Fill,
                    height: Fill,
                    visible: true
                }
            }
        }
    }
}

/// The main widget that displays a single Matrix room.
#[derive(Live, LiveHook, Widget)]
pub struct SearchScreen {
    #[deref] 
    pub view: View,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    #[rust]
    pub search_state: SearchState,
    #[live]
    pub no_more_template: Option<LivePtr>,
    #[rust]
    pub room_id: Option<OwnedRoomId>,
}

impl Widget for SearchScreen {
    // Handle events and actions for the SearchScreen widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        search_result::search_result_draw_walk(self, cx, scope, walk)
    }
}

impl WidgetMatchEvent for SearchScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions, scope: &mut Scope) {
        for action in actions.iter() {
            handle_search_input(self, cx, action, scope);
            match action.downcast_ref().cloned() {
                Some(SearchResultAction::Ok(SearchResultReceived{
                    items,
                    profile_infos,
                    search_term,
                    count,
                    highlights,
                    next_batch
                })) => {
                    self.view
                        .search_result(id!(search_result_plane)).hide_top_space(cx);
                    let mut criteria = self.view
                        .search_result(id!(search_result_plane))
                        .get_search_criteria();
                    if criteria.search_term != *search_term {
                        self.search_state.items = Vector::new();
                    }
                    self.search_state.profile_infos = profile_infos.clone();
                    cx.action(MessageSearchAction::SetText(search_term.clone()));
                    criteria.search_term = search_term.clone();
                    self.view
                        .search_result(id!(search_result_plane))
                        .set_search_criteria(cx, criteria);
                    self.view
                        .search_result(id!(search_result_plane))
                        .set_result_count(cx, count);
                    self.view.view(id!(search_timeline)).set_visible(cx, true);
                    self.search_state
                        .content_drawn_since_last_update
                        .clear();
                    self.search_state
                        .profile_drawn_since_last_update
                        .clear();
                    for item in items {
                        self.search_state.items.push_front(item.clone());
                    }
                    let search_portal_list = self.portal_list(id!(search_timeline.list));
                    search_portal_list.set_first_id_and_scroll(
                        self.search_state.items.len().saturating_sub(1),
                        0.0,
                    );
                    search_portal_list.set_tail_range(true);
                    self.search_state.highlighted_strings = highlights;
                    self.search_state.next_batch_token = next_batch;
                    self.redraw(cx);
                }
                _ => {}
            }
        }
        
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum SearchResultAction{
    Ok(SearchResultReceived),
    None
}

#[derive(Default, Debug, Clone)]
pub struct SearchResultReceived {
    pub items: Vec<SearchResultItem>,
    pub profile_infos: BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
    pub count: u32,
    pub highlights: Vec<String>,
    pub search_term: String,
    pub next_batch: Option<String>,
}