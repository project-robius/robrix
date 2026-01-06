use std::{
    borrow::Cow, cmp::Ordering, collections::{BTreeMap, HashSet}, ops::Deref
};
use bitflags::bitflags;
use matrix_sdk::{RoomDisplayName, ruma::{
    OwnedRoomAliasId, RoomAliasId, RoomId, events::tag::{TagName, Tags}
}};

use crate::{home::rooms_list::{InvitedRoomInfo, JoinedRoomInfo}, home::spaces_bar::JoinedSpaceInfo};

static EMPTY_TAGS: Tags = BTreeMap::new();

/// A trait that abstracts the common properties of a room used to filter/sort it.
pub trait FilterableRoom {
    fn room_id(&self) -> &RoomId;
    fn room_name(&self) -> Cow<'_, str>;
    fn unread_mentions(&self) -> u64;
    fn unread_messages(&self) -> u64;
    fn canonical_alias(&self) -> Option<Cow<'_, RoomAliasId>>;
    fn alt_aliases(&self) -> Cow<'_, [OwnedRoomAliasId]>;
    fn tags(&self) -> &Tags;
    fn is_direct(&self) -> bool;
}

impl FilterableRoom for JoinedRoomInfo {
    fn room_id(&self) -> &RoomId {
        self.room_name_id.room_id()
    }

    fn room_name(&self) -> Cow<'_, str> {
        Cow::Owned(self.room_name_id.to_string())
    }

    fn unread_mentions(&self) -> u64 {
        self.num_unread_mentions
    }

    fn unread_messages(&self) -> u64 {
        self.num_unread_messages
    }

    fn canonical_alias(&self) -> Option<Cow<'_, RoomAliasId>> {
        self.canonical_alias.as_deref().map(Cow::Borrowed)
    }

    fn alt_aliases(&self) -> Cow<'_, [OwnedRoomAliasId]> {
        Cow::Borrowed(&self.alt_aliases)
    }

    fn tags(&self) -> &Tags {
        &self.tags
    }

    fn is_direct(&self) -> bool {
        self.is_direct
    }
}

impl FilterableRoom for InvitedRoomInfo {
    fn room_id(&self) -> &RoomId {
        self.room_name_id.room_id()
    }

    fn room_name(&self) -> Cow<'_, str> {
        Cow::Owned(self.room_name_id.to_string())
    }

    fn unread_mentions(&self) -> u64 {
        1
    }

    fn unread_messages(&self) -> u64 {
        0
    }

    fn canonical_alias(&self) -> Option<Cow<'_, RoomAliasId>> {
        self.canonical_alias.as_deref().map(Cow::Borrowed)
    }

    fn alt_aliases(&self) -> Cow<'_, [OwnedRoomAliasId]> {
        Cow::Borrowed(&self.alt_aliases)
    }

    fn tags(&self) -> &Tags {
        &EMPTY_TAGS
    }

    fn is_direct(&self) -> bool {
        self.is_direct
    }
}

impl FilterableRoom for JoinedSpaceInfo {
    fn room_id(&self) -> &RoomId {
        self.space_name_id.room_id()
    }

    fn room_name(&self) -> Cow<'_, str> {
        match self.space_name_id.display_name() {
            RoomDisplayName::Aliased(name)
            | RoomDisplayName::Calculated(name)
            | RoomDisplayName::EmptyWas(name)
            | RoomDisplayName::Named(name) => name.into(),
            RoomDisplayName::Empty => self.space_name_id.to_string().into(),
        }
    }

    fn unread_mentions(&self) -> u64 {
        0 // TODO: calculate unread mentions for spaces
    }

    fn unread_messages(&self) -> u64 {
        0 // TODO: calculate unread messages for spaces
    }

    fn canonical_alias(&self) -> Option<Cow<'_, RoomAliasId>> {
        self.canonical_alias.as_deref().map(Cow::Borrowed)
    }

    fn alt_aliases(&self) -> Cow<'_, [OwnedRoomAliasId]> {
        (&[]).into()
    }

    fn tags(&self) -> &Tags {
        &EMPTY_TAGS
    }

    fn is_direct(&self) -> bool {
        false
    }
}


pub type RoomFilterFn = dyn Fn(&dyn FilterableRoom) -> bool;
pub type SortFn = dyn Fn(&dyn FilterableRoom, &dyn FilterableRoom) -> Ordering;

fn default_room_filter_fn(_: &dyn FilterableRoom) -> bool {
    true
}

/// A filter function that determines whether a given room should be displayed.
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
#[derive(Default)]
pub struct RoomDisplayFilter(Option<Box<RoomFilterFn>>);
impl Deref for RoomDisplayFilter {
    type Target = RoomFilterFn;
    fn deref(&self) -> &Self::Target {
        if let Some(rdf) = &self.0 {
            rdf.deref()
        } else {
            &default_room_filter_fn
        }
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
    fn default() -> Self {
        RoomFilterCriteria::All
    }
}

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
///         let name_a = a.room_name.as_ref().map_or("", |n| n.display_str());
///         let name_b = b.room_name.as_ref().map_or("", |n| n.display_str());
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
        F: Fn(&dyn FilterableRoom, &dyn FilterableRoom) -> Ordering + 'static,
    {
        self.sort_fn = Some(Box::new(sort_fn));
        self
    }

    fn matches_room_id(room: &dyn FilterableRoom, keywords: &str) -> bool {
        room.room_id().as_str().eq_ignore_ascii_case(keywords)
    }

    fn matches_room_name(room: &dyn FilterableRoom, keywords: &str) -> bool {
        room.room_name()
            .to_lowercase()
            .contains(keywords)
    }

    fn matches_room_alias(room: &dyn FilterableRoom, keywords: &str) -> bool {
        room.canonical_alias()
            .is_some_and(|alias| alias.as_str().eq_ignore_ascii_case(keywords))
        ||
        room.alt_aliases()
            .iter()
            .any(|alias| alias.as_str().eq_ignore_ascii_case(keywords))
    }

    fn matches_room_tags(room: &dyn FilterableRoom, keywords: &str) -> bool {
        fn is_tag_match(search_tag: &str, tag_name: &TagName) -> bool {
            match tag_name {
                TagName::Favorite => ["favourite", "favorite", "fav"].contains(&search_tag),
                TagName::LowPriority => {
                    ["low_priority", "low-priority", "lowpriority", "lowPriority"]
                        .contains(&search_tag)
                }
                TagName::ServerNotice => [
                    "server_notice",
                    "server-notice",
                    "servernotice",
                    "serverNotice",
                ]
                .contains(&search_tag),
                TagName::User(user_tag) => user_tag.as_ref().eq_ignore_ascii_case(search_tag),
                _ => false,
            }
        }

        let search_tags: HashSet<&str> = keywords
            .split_whitespace()
            .map(|tag| tag.trim_start_matches(':'))
            .collect();

        let tags = room.tags();
        search_tags.iter().all(|search_tag| {
            tags.iter()
                .any(|(tag_name, _)| is_tag_match(search_tag, tag_name))
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

    fn matches_filter(
        room: &dyn FilterableRoom,
        keywords: &str,
        filter_criteria: RoomFilterCriteria,
    ) -> bool {
        if filter_criteria.is_empty() {
            return false;
        }

        let (specific_type, cleaned_keywords) = Self::pre_match_filter_check(keywords);

        if specific_type != RoomFilterCriteria::All {
            // When using a special prefix, only check that specific type
            match specific_type {
                RoomFilterCriteria::RoomId
                    if filter_criteria.contains(RoomFilterCriteria::RoomId) =>
                {
                    Self::matches_room_id(room, cleaned_keywords)
                }
                RoomFilterCriteria::RoomAlias
                    if filter_criteria.contains(RoomFilterCriteria::RoomAlias) =>
                {
                    Self::matches_room_alias(room, cleaned_keywords)
                }
                RoomFilterCriteria::RoomTags
                    if filter_criteria.contains(RoomFilterCriteria::RoomTags) =>
                {
                    Self::matches_room_tags(room, cleaned_keywords)
                }
                _ => false,
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

        let filter = if keywords.is_empty() || filter_criteria.is_empty() {
            RoomDisplayFilter::default()
        } else {
            RoomDisplayFilter(Some(Box::new(
                move |room| {
                    let keywords = keywords.trim().to_lowercase();
                    Self::matches_filter(room, &keywords, self.filter_criteria)
                }
            )))
        };
        (filter, self.sort_fn)
    }
}

impl Default for RoomDisplayFilterBuilder {
    fn default() -> Self {
        Self::new()
    }
}
