use std::{collections::BTreeMap, ops::DerefMut};

use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails};
use rangemap::RangeSet;

use crate::{app::AppState, home::{room_screen, room_search_result::{self, handle_search_input, SearchResultItem, SearchResultWidgetExt}, rooms_list::RoomsListRef}, shared::message_search_input_bar::MessageSearchAction, sliding_sync::{submit_async_request, MatrixRequest}};
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
    use crate::home::room_search_result::*;
    use crate::home::room_screen::*;
    pub MessageCard = <Message> {
        draw_bg: {
            instance highlight: 0.0
            instance hover: 0.0
            color: #ffffff  // default color
            instance border_radius: 4.0,
            instance border_size: 1.0,
            instance border_color: #000000,
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                // draw bg
                sdf.box(
                    self.border_size,
                    self.border_size,
                    self.rect_size.x - self.border_size * 2.0,
                    self.rect_size.y - self.border_size * 2.0,
                    max(1.0, self.border_radius)
                )
                sdf.fill(self.color);
                sdf.stroke(
                    self.border_color,
                    self.border_size
                )
                return sdf.result;
            }
        }
    }
    pub TimelineSearch = <View> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0.0} // center horizontally, align to top vertically
        flow: Overlay,
        list = <PortalList> {
            height: Fill,
            width: Fill
            flow: Down

            auto_tail: true, // set to `true` to lock the view to the last item.
            max_pull_down: 0.0, // set to `0.0` to disable the pulldown bounce animation.

            // Below, we must place all of the possible templates (views) that can be used in the portal list.
            Message = <Message> {}
            MessageCard = <MessageCard> {}
            ImageMessage = <ImageMessage> {}
            Empty = <Empty> {}
            RoomHeader = <Label> {
                margin: {left: 10},
                draw_text: {
                    text_style: <REGULAR_TEXT> {
                        font_size: 12.5,
                    },
                    color: #000,
                }
                text: "??"
            }
            NoMoreMessages = <Label> {
                margin: {left: 10, top: 30},
                draw_text: {
                    text_style: <REGULAR_TEXT> {
                        font_size: 16.5,
                    },
                    color: #000,
                }
                text: "??"
            }
            
        }
    }
    
    pub SearchScreen = {{SearchScreen}} {
        <View> {
            width: Fill,
            height: Fill,
            flow: Down,
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
                search_timeline = <TimelineSearch> {
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
        room_search_result::search_result_draw_walk(self, cx, scope, walk)
    }
}

pub fn search_result_draw_walk(
    room_screen: &mut SearchScreen,
    cx: &mut Cx2d,
    scope: &mut Scope,
    walk: Walk,
) -> DrawStep {
    let room_screen_widget_uid = room_screen.widget_uid();
    while let Some(subview) = room_screen.view.draw_walk(cx, scope, walk).step() {
        // We only care about drawing the portal list.
        let portal_list_ref = subview.as_portal_list();
        let Some(mut list_ref) = portal_list_ref.borrow_mut() else {
            error!("!!! RoomScreen::draw_walk(): BUG: expected a PortalList widget, but got something else");
            continue;
        };
        let tl_items = &room_screen.search_state.items;
        // Set the portal list's range based on the number of timeline items.
        let last_item_id = tl_items.len();
        let list = list_ref.deref_mut();
        list.set_item_range(cx, 0, last_item_id);

        while let Some(item_id) = list.next_visible_item(cx) {
            if item_id == 0 && room_screen.search_state.next_batch_token.is_none() && last_item_id > 0 {
                WidgetRef::new_from_ptr(cx, room_screen.no_more_template)
                    .as_label()
                    .draw_all(cx, &mut Scope::empty());
            }
        }
    }
    DrawStep::done()
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
                        0,
                        0.0,
                    );
                    // search_portal_list.set_first_id_and_scroll(
                    //     self.search_state.items.len().saturating_sub(1),
                    //     0.0,
                    // );
                    // search_portal_list.set_tail_range(true);
                    self.search_state.highlighted_strings = highlights;
                    self.search_state.next_batch_token = next_batch;
                    self.redraw(cx);
                }
                _ => {}
            }
            if self.view.button(id!(search_all_rooms_button)).clicked(actions) {
                let mut criteria = self.search_result(id!(search_result_plane)).get_search_criteria();
                self.search_result(id!(search_result_plane)).reset(cx);
                criteria.include_all_rooms = true;
                self.search_result(id!(search_result_plane)).set_search_criteria(cx, criteria.clone());
                self.search_state = SearchState::default();
                submit_async_request(MatrixRequest::SearchMessages { room_id: None, include_all_rooms: true, search_term: criteria.search_term, next_batch: None, abort_previous_search: true });
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