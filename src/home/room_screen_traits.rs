use std::sync::Arc;

use imbl::Vector;
use matrix_sdk_ui::timeline::{EventTimelineItem, TimelineItem};
use rangemap::RangeSet;
use ruma::{OwnedEventId, OwnedRoomId};

use super::{room_screen::{Eventable, TimelineUiState}, room_search_result::{SearchTimelineItem, SearchTimelineItemKind}};

pub trait Stateable <T: TimelineItemAble>{
    fn get_content_drawn_since_last_update(&mut self) -> &mut RangeSet<usize>;
    fn get_profile_drawn_since_last_update(&mut self) -> &mut RangeSet<usize>;
    fn get_fully_paginated(&mut self) -> &mut bool;
    fn get_items(&mut self) -> &mut Vector<T>;
    fn room_id(&self) -> Option<OwnedRoomId>;
    fn get_prev_first_index(&mut self) -> &mut Option<usize>;
    fn get_scrolled_past_read_marker(&mut self) -> &mut bool;
    fn get_backward_pagination(&mut self) -> &mut Option<String>;
    fn get_batch_list(&mut self) -> &mut Vec<String>;
}

pub trait TimelineItemAble {
    fn event_id(&self) -> Option<OwnedEventId>;
}
#[derive(Clone)]
pub struct TimelineItemWrapper(pub Arc<TimelineItem>);
impl TimelineItemAble for TimelineItemWrapper {
    fn event_id(&self) -> Option<OwnedEventId>{
        self.0.as_event().and_then(|f|f.event_id().and_then(|f| Some(f.to_owned())))
    }
}

impl TimelineItemAble for SearchTimelineItem{
    fn event_id(&self) -> Option<OwnedEventId>{
        match &self.kind {
            SearchTimelineItemKind::Event(e) | SearchTimelineItemKind::ContextEvent(e) => Some(e.event_id().to_owned()),
            _ => None
        }
    }
}
impl TimelineItemWrapper {
    pub fn as_event(&self) -> Option<&EventTimelineItem>{
        self.0.as_event()
    }
}