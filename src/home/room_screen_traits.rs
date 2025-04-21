use std::sync::Arc;

use imbl::Vector;
use rangemap::RangeSet;

use super::room_screen::{Eventable, TimelineUiState};

pub trait Stateable <T:Eventable>{
    fn get_content_drawn_since_last_update(&mut self) -> &mut RangeSet<usize>;
    fn get_profile_drawn_since_last_update(&mut self) -> &mut RangeSet<usize>;
    fn get_fully_paginated(&mut self) -> &mut bool;
    fn get_items(&mut self) -> &mut Vector<T>;
    fn room_id(&self) -> &ruma::RoomId;
    fn get_prev_first_index(&mut self) -> &mut Option<usize>;
    fn get_scrolled_past_read_marker(&mut self) -> &mut bool;
}
