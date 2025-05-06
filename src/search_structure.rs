use std::{collections::HashMap, sync::Arc};

use makepad_widgets::*;
use matrix_sdk_ui::timeline::TimelineItem;
use ruma::OwnedEventId;

use crate::home::{room_screen::{RoomScreen, TimelineUiState}, room_search_result::{SearchResultWidgetExt, SearchResultWidgetRefExt}};
#[derive(Clone)]
pub enum EventType {
    Context(Arc<TimelineItem>),
    Main(Arc<TimelineItem>),
    DateDivider(Arc<TimelineItem>)
}
pub struct ProcessedSearchResult {
    pub event: OwnedEventId,
    pub context_before: Option<OwnedEventId>,
    pub context_after: Option<OwnedEventId>
}
pub struct SearchStructure {
    pub events: Vec<ProcessedSearchResult>,
    pub highlighted_strings: Vec<String>,
    pub next_batch_token: Option<String>
}
pub fn append_to_tl_state(room_screen_view: &View,cx: &mut Cx, tl: &mut TimelineUiState, search_structure_len: usize, done_loading: &mut bool, count: u32, highlights: Vec<String>){
    room_screen_view.search_result(id!(search_result_plane)).set_result_count(cx, count);
    room_screen_view.view(id!(search_timeline)).set_visible(cx, true);
    *done_loading = true;
    tl.search_result_state.highlighted_strings = highlights;
    let mut main_event_indexes: Vec<usize> = Vec::with_capacity(search_structure_len);
    for i in 0..tl.search_result_state.pre_processed_items.len() {
        if let Some(items) = tl.search_result_state.pre_processed_items.get(&(tl.search_result_state.pre_processed_items.len() - i)) {
            for item in items.iter() {
                match item {
                    EventType::Main(timeline) => {
                        main_event_indexes.push(tl.search_result_state.items.len());
                        tl.search_result_state.items.push_back(timeline.clone());
                    },
                    EventType::Context(timeline) | EventType::DateDivider(timeline) => {
                        tl.search_result_state.items.push_back(timeline.clone());
                    },
                }
            }
        }
    }
    tl.search_result_state.main_event_indexes = main_event_indexes;
    tl.search_result_state.pre_processed_items = HashMap::new();
}