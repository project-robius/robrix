use std::{cmp::Ordering, collections::HashSet, ops::Deref};
use bitflags::bitflags;
use matrix_sdk::ruma::events::tag::TagName;

use crate::home::rooms_list::JoinedRoomInfo;


/// A filter function that is called for each room to determine whether it should be displayed.
///
/// If the function returns `true`, the room is displayed; otherwise, it is not shown.
/// The default value is a filter function that always returns `true`.
///
/// ## Example
/// The following example shows how to create and apply a filter function
/// that only displays rooms that have a displayable name starting with the letter "M":
/// ```rust,norun
/// rooms_list.display_filter = RoomDisplayFilter(Box::new(
///     |room| room.room_name.as_ref().is_some_and(|n| n.starts_with("M"))
/// ));
/// rooms_list.displayed_rooms = rooms_list.all_joined_rooms.iter()
///    .filter(|(_, room)| (rooms_list.display_filter)(room))
///    .collect();
/// // Then redraw the rooms_list widget.
/// ```
pub struct RoomDisplayFilter(Box<dyn Fn(&JoinedRoomInfo) -> bool>);
impl Default for RoomDisplayFilter {
    fn default() -> Self {
        RoomDisplayFilter(Box::new(|_| true))
    }
}
impl Deref for RoomDisplayFilter {
    type Target = Box<dyn Fn(&JoinedRoomInfo) -> bool>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

bitflags! {
    /// The criteria that can be used to filter rooms in the `RoomDisplayFilter`.
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub struct RoomFilterCriteria: u8 {
        const RoomId    = 0b0000_0001;
        const RoomName  = 0b0000_0010;
        const RoomAlias = 0b0000_0100;
        const RoomTags  = 0b0000_1000;
        const All       = Self::RoomId.bits() | Self::RoomName.bits() | Self::RoomAlias.bits() | Self::RoomTags.bits();
    }
}

impl Default for RoomFilterCriteria {
    fn default() -> Self { RoomFilterCriteria::All }
}

type SortFn = dyn Fn(&JoinedRoomInfo, &JoinedRoomInfo) -> Ordering;

/// A builder for creating a `RoomDisplayFilter` with a specific set of filter types and a sorting function.
pub struct RoomDisplayFilterBuilder {
    keywords: String,
    filter_criteria: RoomFilterCriteria,
    sort_fn: Option<Box<SortFn>>,
}
/// ## Example
/// You can create any combination of filters and sorting functions using the `RoomDisplayFilterBuilder`.
/// ```rust,norun
///   let (filter, sort_fn) = RoomDisplayFilterBuilder::new()
///     .set_keywords(keywords)
///     .by_room_id()
///     .by_room_name()
///     .sort_by(|a, b| {
///         let name_a = a.room_name.as_ref().map_or("", |n| n.as_str());
///         let name_b = b.room_name.as_ref().map_or("", |n| n.as_str());
///         name_a.cmp(name_b)
///     })
///     .build();
/// ```
impl RoomDisplayFilterBuilder {
    pub fn new() -> Self {
        Self {
            keywords: String::new(),
            filter_criteria: RoomFilterCriteria::default(),
            sort_fn: None,
        }
    }

    pub fn set_keywords(mut self, keywords: String) -> Self {
        self.keywords = keywords;
        self
    }

    pub fn set_filter_criteria(mut self, filter_criteria: RoomFilterCriteria) -> Self {
        self.filter_criteria = filter_criteria;
        self
    }

    pub fn sort_by<F>(mut self, sort_fn: F) -> Self
    where
        F: Fn(&JoinedRoomInfo, &JoinedRoomInfo) -> Ordering + 'static
    {
        self.sort_fn = Some(Box::new(sort_fn));
        self
    }

    fn matches_room_id(room: &JoinedRoomInfo, keywords: &str) -> bool {
        room.room_id.to_string().eq_ignore_ascii_case(keywords)
    }

    fn matches_room_name(room: &JoinedRoomInfo, keywords: &str) -> bool {
        room.room_name
            .as_ref()
            .is_some_and(|name| name.to_lowercase().contains(keywords))
    }

    fn matches_room_alias(room: &JoinedRoomInfo, keywords: &str) -> bool {
        let matches_canonical_alias = room.canonical_alias
            .as_ref()
            .is_some_and(|alias| alias.as_str().eq_ignore_ascii_case(keywords));
        let matches_alt_aliases = room.alt_aliases
            .iter()
            .any(|alias| alias.as_str().eq_ignore_ascii_case(keywords));

        matches_canonical_alias || matches_alt_aliases
    }

    fn matches_room_tags(room: &JoinedRoomInfo, keywords: &str) -> bool {
        let search_tags: HashSet<&str> = keywords
            .split_whitespace()
            .map(|tag| tag.trim_start_matches(':'))
            .collect();

        fn is_tag_match(search_tag: &str, tag_name: &TagName) -> bool {
            match tag_name {
                TagName::Favorite => ["favourite", "favorite"].contains(&search_tag),
                TagName::LowPriority => ["low_priority", "low-priority", "lowpriority", "lowPriority"].contains(&search_tag),
                TagName::ServerNotice => ["server_notice", "server-notice", "servernotice", "serverNotice"].contains(&search_tag),
                TagName::User(user_tag) => user_tag.as_ref().eq_ignore_ascii_case(search_tag),
                _ => false,
            }
        }

        room.tags.as_ref().is_some_and(|room_tags| {
            search_tags.iter().all(|search_tag| {
                room_tags.iter().any(|(tag_name, _)| is_tag_match(search_tag, tag_name))
            })
        })
    }

    // Check if the keywords have a special prefix that indicates a pre-match filter check.
    fn pre_match_filter_check(keywords: &str) -> (RoomFilterCriteria, &str) {
        match keywords.chars().next() {
            Some('!') => (RoomFilterCriteria::RoomId, keywords),
            Some('#') => (RoomFilterCriteria::RoomAlias, keywords),
            Some(':') => (RoomFilterCriteria::RoomTags, keywords),
            _ => (RoomFilterCriteria::All, keywords),
        }
    }

    fn matches_filter(room: &JoinedRoomInfo, keywords: &str, filter_criteria: RoomFilterCriteria) -> bool {
        if filter_criteria.is_empty() {
            return false;
        }

        let (specific_type, cleaned_keywords) = Self::pre_match_filter_check(keywords);

        if specific_type != RoomFilterCriteria::All {
            // When using a special prefix, only check that specific type
            match specific_type {
                RoomFilterCriteria::RoomId if filter_criteria.contains(RoomFilterCriteria::RoomId) => {
                    Self::matches_room_id(room, cleaned_keywords)
                }
                RoomFilterCriteria::RoomAlias if filter_criteria.contains(RoomFilterCriteria::RoomAlias) => {
                    Self::matches_room_alias(room, cleaned_keywords)
                }
                RoomFilterCriteria::RoomTags if filter_criteria.contains(RoomFilterCriteria::RoomTags) => {
                    Self::matches_room_tags(room, cleaned_keywords)
                }
                _ => false
            }
        } else {
            // No special prefix, check all enabled filter types
            let mut matches = false;

            if filter_criteria.contains(RoomFilterCriteria::RoomId) {
                matches |= Self::matches_room_id(room, cleaned_keywords);
            }
            if filter_criteria.contains(RoomFilterCriteria::RoomName) {
                matches |= Self::matches_room_name(room, cleaned_keywords);
            }
            if filter_criteria.contains(RoomFilterCriteria::RoomAlias) {
                matches |= Self::matches_room_alias(room, cleaned_keywords);
            }
            if filter_criteria.contains(RoomFilterCriteria::RoomTags) {
                matches |= Self::matches_room_tags(room, cleaned_keywords);
            }

            matches
        }
    }

    pub fn build(self) -> (RoomDisplayFilter, Option<Box<SortFn>>) {
        let keywords = self.keywords;
        let filter_criteria = self.filter_criteria;

        let filter = RoomDisplayFilter(Box::new(move |room| {
            if keywords.is_empty() || filter_criteria.is_empty() {
                return true;
            }
            let keywords = keywords.trim().to_lowercase();
            Self::matches_filter(room, &keywords, self.filter_criteria)
        }));

        (filter, self.sort_fn)
    }

}

impl Default for RoomDisplayFilterBuilder {
    fn default() -> Self {
        Self::new()
    }
}
