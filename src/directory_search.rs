//! Searching for rooms, spaces, and users by name, alias, ID, or Matrix link.
//!
//! A [`SearchQuery`] is parsed leniently, so a partial ID like `#robrix` or `alice` is completed with
//! our own homeserver's name, and we also search our homeserver's room and user directories for it,
//! plus each other room directory that the user has turned on in the settings.

use std::ops::{Deref, DerefMut};

use futures_util::future::join_all;
use makepad_widgets::Cx;
use matrix_sdk::{Client, Room, RoomDisplayName, RoomState, room_preview::RoomPreview};
use ruma::{
    OwnedRoomId, OwnedRoomOrAliasId, OwnedServerName, OwnedUserId, RoomAliasId, RoomId, ServerName, UInt, UserId,
    api::{client::{alias::get_alias, directory::get_public_rooms_filtered, profile::{AvatarUrl, DisplayName}, user_directory}, error::ErrorKind},
    directory::{PublicRoomsChunk, RoomTypeFilter}, matrix_uri::MatrixId, owned_server_name, room::RoomType,
};
use serde::{Deserialize, Serialize};

use crate::{profile::user_profile::UserProfile, room::FetchedRoomPreview, settings::app_preferences::AppPreferencesGlobal, shared::avatar::AvatarState, sliding_sync::current_user_id, utils::{self, MatchQuality, RoomNameId}};

/// How many results we ask for at a time.
pub const SEARCH_PAGE_SIZE: u32 = 20;

/// Another server's room directory that we can search along with our own homeserver's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomDirectory {
    pub server: OwnedServerName,
    /// Whether the user wants us to search this room directory.
    pub is_enabled: bool,
}

/// The other room directories that we can search, in the order that the settings list them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoomDirectories(Vec<RoomDirectory>);

impl Default for RoomDirectories {
    fn default() -> Self {
        Self(
            [owned_server_name!("matrix.org"), owned_server_name!("activematrixrooms.com"), owned_server_name!("matrixrooms.info")]
                .into_iter()
                .map(|server| RoomDirectory { server, is_enabled: true })
                .collect()
        )
    }
}

impl Deref for RoomDirectories {
    type Target = Vec<RoomDirectory>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RoomDirectories {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl RoomDirectories {
    /// Returns the room directories other than our own homeserver's, which we always search anyway.
    pub fn iter_except(&self, own_server: Option<&ServerName>) -> impl Iterator<Item = &RoomDirectory> {
        self.0.iter().filter(move |directory| Some(&*directory.server) != own_server)
    }
}

/// Returns the servers whose room directories we search: `None` for our own homeserver, then each other enabled one.
pub fn get_room_directory_servers(cx: &mut Cx) -> Vec<Option<OwnedServerName>> {
    let own_server = current_user_id().map(|user_id| user_id.server_name().to_owned());
    let other_servers = cx.global::<AppPreferencesGlobal>().0.room_directories
        .iter_except(own_server.as_deref())
        .filter(|directory| directory.is_enabled)
        .map(|directory| Some(directory.server.clone()));
    std::iter::once(None).chain(other_servers).collect()
}

/// Whether a room directory search is for rooms or for spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    Room,
    Space,
}

/// The text in a search box, parsed leniently such that it can be a partial ID.
#[derive(Clone, Debug, PartialEq)]
pub enum SearchQuery {
    /// A link to a room, a space, or an event in a room.
    RoomLink {
        room_or_alias_id: OwnedRoomOrAliasId,
        via: Vec<OwnedServerName>,
    },
    /// A link to a user.
    UserLink(OwnedUserId),
    /// Text starting with `#` or `!`, i.e., a room alias or room ID, maybe without its server name.
    Room(String),
    /// Text starting with `@`, i.e., a user ID, maybe without its server name.
    User(String),
    /// Any other text, which could be the name, alias, or ID of a room or a user.
    Text(String),
}

impl SearchQuery {
    /// Parses the given search text, returning `None` if it's blank.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        // ruma only accepts a matrix.to link that starts with exactly `https://matrix.to/#/`,
        // so we also accept one that's missing its `https://`, has an extra `www.`, or isn't lowercase.
        let lowercase = text.to_ascii_lowercase();
        let without_scheme = ["https://", "http://"].iter()
            .find_map(|scheme| lowercase.strip_prefix(scheme))
            .unwrap_or(&lowercase);
        let without_www = without_scheme.strip_prefix("www.").unwrap_or(without_scheme);
        let matrix_link = match without_www.strip_prefix("matrix.to/#/") {
            // An ASCII-lowercase string has the same length as the original, so this keeps the IDs' case.
            Some(ids) => utils::parse_matrix_link(&format!("https://matrix.to/#/{}", &text[text.len() - ids.len()..])),
            None => utils::parse_matrix_link(text),
        };
        if let Some((matrix_id, via)) = matrix_link {
            match matrix_id {
                MatrixId::Room(room_id) => return Some(Self::RoomLink { room_or_alias_id: room_id.into(), via }),
                MatrixId::RoomAlias(alias) => return Some(Self::RoomLink { room_or_alias_id: alias.into(), via }),
                MatrixId::Event(room_or_alias_id, _) => return Some(Self::RoomLink { room_or_alias_id, via }),
                MatrixId::User(user_id) => return Some(Self::UserLink(user_id)),
                _ => {}
            }
        }
        Some(match text.as_bytes()[0] {
            b'#' | b'!' => Self::Room(text.to_owned()),
            b'@' => Self::User(text.to_owned()),
            _ => Self::Text(text.to_owned()),
        })
    }

    /// Returns whether this query could match a room or space.
    pub fn can_match_rooms(&self) -> bool {
        !matches!(self, Self::User(_) | Self::UserLink(_))
    }

    /// Returns whether this query could match a user.
    pub fn can_match_users(&self) -> bool {
        !matches!(self, Self::Room(_) | Self::RoomLink { .. })
    }

    /// Returns the address of the room that this query fully spells out.
    ///
    /// That's a room link or a room ID with its server name, but not an alias, whose case might not match.
    pub fn full_room_address(&self) -> Option<(OwnedRoomOrAliasId, Vec<OwnedServerName>)> {
        match self {
            Self::RoomLink { room_or_alias_id, via } => Some((room_or_alias_id.clone(), via.clone())),
            Self::Room(text) if text.starts_with('!') && text.contains(':') => {
                OwnedRoomOrAliasId::try_from(text.as_str()).ok().map(|room_id| (room_id, Vec::new()))
            }
            _ => None,
        }
    }

    /// Returns whether this query is a link, or an alias or ID with its server name,
    /// such that it can only be one room or user, unlike a partial ID or a name.
    pub fn is_fully_qualified(&self) -> bool {
        match self {
            Self::RoomLink { .. } => true,
            Self::Room(text) => text.contains(':') && OwnedRoomOrAliasId::try_from(text.as_str()).is_ok(),
            Self::UserLink(_) | Self::User(_) => self.full_user_id().is_some(),
            Self::Text(_) => false,
        }
    }

    /// Returns the room aliases and IDs that this query could exactly be,
    /// using `own_server` for any that are missing their server name.
    fn room_candidates(&self, own_server: Option<&ServerName>) -> Vec<(OwnedRoomOrAliasId, Vec<OwnedServerName>)> {
        let ids = match self {
            Self::RoomLink { room_or_alias_id, via } => return vec![(room_or_alias_id.clone(), via.clone())],
            Self::Room(text) if text.starts_with('!') => {
                // Room IDs are case sensitive, and newer ones don't have a server name at all.
                if text.len() == 1 || text.chars().any(char::is_whitespace) {
                    return Vec::new();
                }
                let mut ids = vec![text.clone()];
                if !text.contains(':') && let Some(own_server) = own_server {
                    ids.push(format!("{text}:{own_server}"));
                }
                ids
            }
            Self::Room(text) => complete_ids('#', &text[1..], own_server),
            Self::Text(text) => complete_ids('#', text, own_server),
            Self::User(_) | Self::UserLink(_) => return Vec::new(),
        };
        ids.into_iter()
            .filter_map(|id| OwnedRoomOrAliasId::try_from(id).ok())
            .map(|id| (id, Vec::new()))
            .collect()
    }

    /// Returns the user IDs that this query could exactly be,
    /// using `own_server` for any that are missing their server name.
    fn user_candidates(&self, own_server: Option<&ServerName>) -> Vec<OwnedUserId> {
        let ids = match self {
            Self::UserLink(user_id) => return vec![user_id.clone()],
            Self::User(text) => complete_ids('@', &text[1..], own_server),
            Self::Text(text) => complete_ids('@', text, own_server),
            Self::Room(_) | Self::RoomLink { .. } => return Vec::new(),
        };
        ids.into_iter().filter_map(|id| UserId::parse(id).ok()).collect()
    }

    /// Returns the user ID that this query exactly spells out, if any.
    fn full_user_id(&self) -> Option<OwnedUserId> {
        match self {
            Self::UserLink(user_id) => Some(user_id.clone()),
            Self::User(text) => UserId::parse(text).ok(),
            _ => None,
        }
    }

    /// Returns the text to search for in the room directory, if any.
    fn room_search_term(&self) -> Option<&str> {
        match self {
            // The directory matches an alias's localpart, so we leave out the sigil and server.
            Self::Room(text) if text.starts_with('#') => text[1..].split(':').next().filter(|t| !t.is_empty()),
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Returns the text to search for in the user directory, if any.
    fn user_search_term(&self) -> Option<&str> {
        match self {
            Self::User(text) => Some(text[1..].trim_end_matches(':')).filter(|t| !t.is_empty()),
            Self::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// A room or space found by a search.
#[derive(Clone, Debug)]
pub struct RoomSearchResult {
    /// Everything we know about this room, so we can show its preview as soon as the user picks it.
    pub preview: FetchedRoomPreview,
    /// Whether this room is exactly what was entered, rather than a room directory match.
    pub is_exact_match: bool,
    /// How well this room's name matches what was entered.
    pub name_match: MatchQuality,
    /// Whether this room's alias is exactly what was entered.
    pub is_alias_match: bool,
}

impl RoomSearchResult {
    pub fn room_id(&self) -> &OwnedRoomId {
        self.preview.room_name_id.room_id()
    }

    pub fn is_space(&self) -> bool {
        self.preview.room_type == Some(RoomType::Space)
    }

    /// Returns the alias to show for this room, if it has one.
    pub fn alias(&self) -> Option<&str> {
        self.preview.canonical_alias.as_ref().or(self.preview.alias.as_ref()).map(|alias| alias.as_str())
    }

    /// Converts the preview of a room that we looked up by the given alias or ID,
    /// ranking it by how well it matches the given lowercase search term.
    fn from_preview(client: &Client, preview: RoomPreview, address: OwnedRoomOrAliasId, via: Vec<OwnedServerName>, term: Option<&str>) -> Self {
        let room_avatar = utils::avatar_from_room_name(preview.name.as_deref());
        let alias = <&RoomId>::try_from(&*address).err().map(ToOwned::to_owned);
        let preview = FetchedRoomPreview::from(client, preview, room_avatar, via, alias);
        let (name_match, is_alias_match) = get_name_and_alias_match(&preview, term);
        Self { preview, is_exact_match: true, name_match, is_alias_match }
    }

    /// Converts a room listed in the given server's room directory,
    /// ranking it by how well it matches the given lowercase search term.
    ///
    /// If we already know this room, e.g., because we've joined it, its preview says so, like a fetched one does.
    async fn from_directory(public_room: PublicRoomsChunk, directory_server: Option<&ServerName>, known_room: Option<Room>, term: Option<&str>) -> Self {
        // A directory like activematrixrooms.com isn't in the rooms it lists, so it can't help us join them.
        // Thus, we first try the servers of the room's alias and ID.
        let mut via: Vec<OwnedServerName> = Vec::new();
        let via_candidates = public_room.canonical_alias.as_deref().map(RoomAliasId::server_name)
            .into_iter()
            .chain(public_room.room_id.server_name())
            .chain(directory_server);
        for server in via_candidates {
            if !via.iter().any(|s| s == server) {
                via.push(server.to_owned());
            }
        }
        let (state, is_direct, local_name) = match &known_room {
            Some(room) => {
                let state = room.state();
                // Like a fetched preview, we only use our own copy of the name of a room that we're in or banned from.
                let local_name = match state {
                    RoomState::Joined | RoomState::Banned => room.display_name().await.ok(),
                    _ => None,
                };
                (Some(state), room.is_direct().await.ok(), local_name)
            }
            None => (None, None, None),
        };
        let display_name = local_name
            .or_else(|| public_room.name.clone().map(RoomDisplayName::Named))
            .unwrap_or(RoomDisplayName::Empty);
        let room_name_id = RoomNameId::new(display_name, public_room.room_id);
        let preview = FetchedRoomPreview {
            room_avatar: utils::avatar_from_room_name(room_name_id.name_for_avatar()),
            room_name_id,
            via,
            alias: None,
            // The room directory doesn't say which rooms a restricted join rule allows.
            is_in_allowed_room: false,
            canonical_alias: public_room.canonical_alias,
            topic: public_room.topic,
            avatar_url: public_room.avatar_url.filter(|url| url.is_valid()),
            num_joined_members: public_room.num_joined_members.into(),
            num_active_members: None,
            room_type: public_room.room_type,
            join_rule: Some(public_room.join_rule.into()),
            is_world_readable: Some(public_room.world_readable),
            state,
            is_direct,
            heroes: None,
        };
        let (name_match, is_alias_match) = get_name_and_alias_match(&preview, term);
        Self { preview, is_exact_match: false, name_match, is_alias_match }
    }
}

/// A user found by a search.
#[derive(Clone, Debug)]
pub struct UserSearchResult {
    pub user_profile: UserProfile,
    /// Whether this is exactly who was entered, rather than a user directory match.
    ///
    /// It's `false` for an entered user ID whose profile we couldn't see, since they might not exist.
    pub is_exact_match: bool,
}

/// The results of a search, posted by the functions in this module.
#[derive(Debug)]
pub enum DirectorySearchAction {
    /// The rooms and spaces we've joined whose name or alias contains the given `query`.
    JoinedRooms {
        query: String,
        rooms: Vec<RoomSearchResult>,
    },
    /// The rooms and spaces that the given `query` could exactly be, e.g., its full or completed alias.
    ExactRooms {
        query: String,
        rooms: Vec<RoomSearchResult>,
    },
    /// The users that the given `query` could exactly be, e.g., their full or completed user ID.
    ExactUsers {
        query: String,
        users: Vec<UserSearchResult>,
    },
    /// One room directory's matches for the given `query`, either rooms or spaces.
    DirectoryRooms {
        query: String,
        kind: RoomKind,
        /// The server whose room directory this is, or `None` for our own homeserver's.
        server: Option<OwnedServerName>,
        rooms: Vec<RoomSearchResult>,
        /// The token for getting the next page of matches, if there are more.
        next_batch: Option<String>,
        /// Whether these rooms are another page of matches, rather than the first page.
        is_next_page: bool,
        error: Option<String>,
    },
    /// The user directory's matches for the given `query`.
    DirectoryUsers {
        query: String,
        users: Vec<UserSearchResult>,
        /// The maximum number of users that we asked the user directory for.
        limit: u32,
        /// Whether the user directory has more matching users than it gave us.
        is_limited: bool,
        error: Option<String>,
    },
}

/// Finds the rooms and spaces we've joined that match the given query text, and the ones it could exactly be,
/// posting them as a [`DirectorySearchAction::JoinedRooms`] and then a [`DirectorySearchAction::ExactRooms`].
pub async fn resolve_room_search_query(client: Client, query: String) {
    let Some(search_query) = SearchQuery::parse(&query) else { return };
    let lowercase_term = search_query.room_search_term().map(str::to_lowercase);
    let term = lowercase_term.as_deref();
    let mut joined_rooms = Vec::new();
    if let Some(term) = term {
        for room in client.joined_rooms() {
            // An alias's server name would match every room on that server, so we only match its localpart.
            let is_match = room.name().is_some_and(|name| name.to_lowercase().contains(term))
                || room.canonical_alias().into_iter().chain(room.alt_aliases())
                    .any(|alias| alias.alias().to_lowercase().contains(term));
            // The other search box finds people.
            if !is_match || room.is_direct().await.unwrap_or(false) {
                continue;
            }
            // We're in this room, so its preview comes from our own copy of it.
            let address = OwnedRoomOrAliasId::from(room.room_id().to_owned());
            if let Ok(preview) = client.get_room_preview(&address, Vec::new()).await {
                joined_rooms.push(RoomSearchResult { is_exact_match: false, ..RoomSearchResult::from_preview(&client, preview, address, Vec::new(), Some(term)) });
            }
        }
    }
    // We post these first, so whoever has the exact matches has these too.
    Cx::post_action(DirectorySearchAction::JoinedRooms { query: query.clone(), rooms: joined_rooms });

    let own_server = client.user_id().map(|user_id| user_id.server_name().to_owned());
    // Like `RequestConfig::short_retry()`, but this keeps our client's longer timeout.
    let request_config = client.request_config().retry_limit(3);
    let exact_rooms = join_all(search_query.room_candidates(own_server.as_deref()).into_iter().map(|(address, via)| {
        let client = &client;
        async move {
            // The SDK would resolve the alias with no limit on its retries, which could take many minutes.
            if let Ok(alias) = <&RoomAliasId>::try_from(&*address) {
                client.send(get_alias::v3::Request::new(alias.to_owned()))
                    .with_request_config(request_config)
                    .await
                    .ok()?;
            }
            let preview = client.get_room_preview(&address, via.clone()).await.ok()?;
            Some(RoomSearchResult::from_preview(client, preview, address, via, term))
        }
    })).await;

    let mut rooms: Vec<RoomSearchResult> = Vec::new();
    // An alias and its lowercase version can belong to the same room.
    for room in exact_rooms.into_iter().flatten() {
        if !rooms.iter().any(|r| r.room_id() == room.room_id()) {
            rooms.push(room);
        }
    }
    Cx::post_action(DirectorySearchAction::ExactRooms { query, rooms });
}

/// Looks up the users that the given query text could exactly be,
/// posting them as a [`DirectorySearchAction::ExactUsers`].
pub async fn resolve_user_search_query(client: Client, query: String) {
    let Some(search_query) = SearchQuery::parse(&query) else { return };
    let own_server = client.user_id().map(|user_id| user_id.server_name().to_owned());
    let full_user_id = search_query.full_user_id();
    let exact_users = join_all(search_query.user_candidates(own_server.as_deref()).into_iter().map(|user_id| {
        let client = &client;
        // Candidates have a lowercase server name.
        let is_full_user_id = full_user_id.as_ref().is_some_and(|full| {
            full.localpart() == user_id.localpart()
                && full.server_name().as_str().eq_ignore_ascii_case(user_id.server_name().as_str())
        });
        async move {
            match client.account().fetch_user_profile_of(&user_id).await {
                Ok(response) => Some(UserSearchResult {
                    user_profile: UserProfile {
                        username: response.get_static::<DisplayName>().ok().flatten(),
                        avatar_state: response.get_static::<AvatarUrl>().ok().map_or(AvatarState::Unknown, AvatarState::Known),
                        user_id,
                    },
                    is_exact_match: true,
                }),
                // A homeserver may refuse to show the profile of someone who doesn't share a room with us,
                // so we still show an entered user ID, though we can't tell whether that user exists.
                Err(e) if is_full_user_id && matches!(e.client_api_error_kind(), Some(ErrorKind::Forbidden)) => {
                    Some(UserSearchResult {
                        user_profile: UserProfile { user_id, username: None, avatar_state: AvatarState::Unknown },
                        is_exact_match: false,
                    })
                }
                Err(_) => None,
            }
        }
    })).await;
    let users = exact_users.into_iter().flatten().collect();
    Cx::post_action(DirectorySearchAction::ExactUsers { query, users });
}

/// Searches the given server's room directory (or our own homeserver's if `None`) for rooms or spaces
/// matching the query, posting a [`DirectorySearchAction::DirectoryRooms`].
///
/// If `next_batch` is given, this gets that next page of matches.
pub async fn search_room_directory(client: Client, query: String, kind: RoomKind, server: Option<OwnedServerName>, next_batch: Option<String>) {
    let Some(search_query) = SearchQuery::parse(&query) else { return };
    let directory_server = server.clone().or_else(|| client.user_id().map(|user_id| user_id.server_name().to_owned()));
    let lowercase_term = search_query.room_search_term().map(str::to_lowercase);
    let is_next_page = next_batch.is_some();
    let result = match search_query.room_search_term() {
        Some(term) => {
            let mut request = get_public_rooms_filtered::v3::Request::new();
            request.server = server.clone();
            request.filter.generic_search_term = Some(term.to_owned());
            request.filter.room_types = vec![match kind {
                RoomKind::Room => RoomTypeFilter::Default,
                RoomKind::Space => RoomTypeFilter::Space,
            }];
            request.limit = Some(UInt::from(SEARCH_PAGE_SIZE));
            request.since = next_batch;
            client.send(request)
                .with_request_config(client.request_config().retry_limit(3))
                .await
                .map(|response| (response.chunk, response.next_batch))
        }
        None => Ok((Vec::new(), None)),
    };
    let (rooms, next_batch, error) = match result {
        Ok((public_rooms, next_batch)) => {
            let mut rooms = Vec::with_capacity(public_rooms.len());
            // Not every room directory filters by room type, so we make sure.
            let public_rooms = public_rooms.into_iter()
                .filter(|public_room| (public_room.room_type == Some(RoomType::Space)) == (kind == RoomKind::Space));
            for public_room in public_rooms {
                let known_room = client.get_room(&public_room.room_id);
                rooms.push(RoomSearchResult::from_directory(public_room, directory_server.as_deref(), known_room, lowercase_term.as_deref()).await);
            }
            (rooms, next_batch, None)
        }
        Err(e) => (Vec::new(), None, Some(format!(
            "Couldn't search the room directory of {}: {}",
            directory_server.as_ref().map_or("your homeserver", |server| server.as_str()),
            utils::stringify_matrix_error(&e.into()),
        ))),
    };
    Cx::post_action(DirectorySearchAction::DirectoryRooms { query, kind, server, rooms, next_batch, is_next_page, error });
}

/// Searches our homeserver's user directory for the given query text, getting up to `limit` users,
/// and posts the results as a [`DirectorySearchAction::DirectoryUsers`].
pub async fn search_user_directory(client: Client, query: String, limit: u32) {
    let Some(search_query) = SearchQuery::parse(&query) else { return };
    let result = match search_query.user_search_term() {
        Some(term) => {
            let mut request = user_directory::search_users::v3::Request::new(term.to_owned());
            request.limit = UInt::from(limit);
            client.send(request)
                .with_request_config(client.request_config().retry_limit(3))
                .await
                .map(|response| {
                    // Synapse gives at most 50 users but still says there are more, so we only offer more
                    // if we got more users than the previous, smaller search could have given us.
                    let is_limited = response.limited
                        && response.results.len() > (limit as usize).saturating_sub(SEARCH_PAGE_SIZE as usize);
                    (response.results, is_limited)
                })
        }
        None => Ok((Vec::new(), false)),
    };
    let (users, is_limited, error) = match result {
        Ok((users, is_limited)) => (
            users.into_iter()
                .map(|user| UserSearchResult {
                    user_profile: UserProfile {
                        avatar_state: AvatarState::Known(user.avatar_url.filter(|url| url.is_valid())),
                        username: user.display_name,
                        user_id: user.user_id,
                    },
                    is_exact_match: false,
                })
                .collect(),
            is_limited,
            None,
        ),
        Err(e) => (Vec::new(), false, Some(format!(
            "Couldn't search the user directory: {}",
            utils::stringify_matrix_error(&e.into()),
        ))),
    };
    Cx::post_action(DirectorySearchAction::DirectoryUsers { query, users, limit, is_limited, error });
}

/// Returns how well the given room's name matches the given lowercase search term,
/// and whether its alias is exactly that term.
fn get_name_and_alias_match(preview: &FetchedRoomPreview, term: Option<&str>) -> (MatchQuality, bool) {
    let Some(term) = term else { return (MatchQuality::None, false) };
    let name_match = preview.room_name_id.name_for_avatar()
        .map_or(MatchQuality::None, |name| MatchQuality::of(&name.to_lowercase(), term));
    let is_alias_match = preview.canonical_alias.iter().chain(&preview.alias)
        .any(|alias| alias.alias().to_lowercase() == term);
    (name_match, is_alias_match)
}

/// Returns the full IDs that `localpart_and_server` could be with the given `sigil`, using `own_server` if needed.
///
/// IDs are case sensitive but almost always lowercase, so we try the lowercase one too.
fn complete_ids(sigil: char, localpart_and_server: &str, own_server: Option<&ServerName>) -> Vec<String> {
    let (localpart, server) = localpart_and_server.split_once(':').unwrap_or((localpart_and_server, ""));
    let server = match (server, own_server) {
        ("", Some(own_server)) => own_server.as_str().to_owned(),
        ("", None) => return Vec::new(),
        (server, _) => server.to_ascii_lowercase(),
    };
    if localpart.is_empty()
        || localpart_and_server.chars().any(char::is_whitespace)
        || ServerName::parse(&server).is_err()
    {
        return Vec::new();
    }
    let id = format!("{sigil}{localpart}:{server}");
    let lowercase_id = format!("{sigil}{}:{server}", localpart.to_lowercase());
    if lowercase_id == id { vec![id] } else { vec![id, lowercase_id] }
}

#[cfg(test)]
mod tests_search_query {
    use super::*;

    fn own_server() -> Option<&'static ServerName> {
        Some(<&ServerName>::try_from("matrix.org").unwrap())
    }

    fn room_ids(query: &str) -> Vec<String> {
        SearchQuery::parse(query).unwrap()
            .room_candidates(own_server())
            .into_iter()
            .map(|(id, _)| id.to_string())
            .collect()
    }

    fn user_ids(query: &str) -> Vec<String> {
        SearchQuery::parse(query).unwrap()
            .user_candidates(own_server())
            .into_iter()
            .map(|id| id.to_string())
            .collect()
    }

    #[test]
    fn parses_kinds_of_queries() {
        assert_eq!(SearchQuery::parse("  "), None);
        assert_eq!(SearchQuery::parse(" #robrix "), Some(SearchQuery::Room("#robrix".into())));
        assert_eq!(SearchQuery::parse("!abc"), Some(SearchQuery::Room("!abc".into())));
        assert_eq!(SearchQuery::parse("@alice"), Some(SearchQuery::User("@alice".into())));
        assert_eq!(SearchQuery::parse("Robrix Dev"), Some(SearchQuery::Text("Robrix Dev".into())));
        assert!(matches!(
            SearchQuery::parse("https://matrix.to/#/#robrix:matrix.org"),
            Some(SearchQuery::RoomLink { room_or_alias_id, .. }) if room_or_alias_id.as_str() == "#robrix:matrix.org"
        ));
        assert!(matches!(
            SearchQuery::parse("matrix:u/alice:matrix.org"),
            Some(SearchQuery::UserLink(id)) if id.as_str() == "@alice:matrix.org"
        ));
    }

    #[test]
    fn parses_lenient_matrix_to_links() {
        assert!(matches!(
            SearchQuery::parse("matrix.to/#/@Alice:matrix.org"),
            Some(SearchQuery::UserLink(id)) if id.as_str() == "@Alice:matrix.org"
        ));
        let Some(SearchQuery::RoomLink { room_or_alias_id, via }) = SearchQuery::parse("HTTP://WWW.Matrix.To/#/!abc:matrix.org?via=example.org") else {
            panic!("expected a room link");
        };
        assert_eq!(room_or_alias_id.as_str(), "!abc:matrix.org");
        assert_eq!(via.iter().map(|s| s.as_str()).collect::<Vec<_>>(), ["example.org"]);
        // A link to an event gives us the event's room.
        assert!(matches!(
            SearchQuery::parse("https://matrix.to/#/#robrix:matrix.org/$event:matrix.org"),
            Some(SearchQuery::RoomLink { room_or_alias_id, .. }) if room_or_alias_id.as_str() == "#robrix:matrix.org"
        ));
        // Something that's not quite a link is just searched for as text.
        assert!(matches!(SearchQuery::parse("matrix.to/#/robrix"), Some(SearchQuery::Text(_))));
    }

    #[test]
    fn says_which_sections_apply() {
        let room = SearchQuery::parse("#robrix").unwrap();
        assert!(room.can_match_rooms() && !room.can_match_users());
        let user = SearchQuery::parse("https://matrix.to/#/@alice:matrix.org").unwrap();
        assert!(!user.can_match_rooms() && user.can_match_users());
        let text = SearchQuery::parse("alice").unwrap();
        assert!(text.can_match_rooms() && text.can_match_users());
    }

    #[test]
    fn finds_full_room_addresses() {
        let address = |query: &str| SearchQuery::parse(query).unwrap().full_room_address().map(|(id, _)| id.to_string());
        assert_eq!(address("!abc:matrix.org").as_deref(), Some("!abc:matrix.org"));
        assert_eq!(address("https://matrix.to/#/#robrix:matrix.org").as_deref(), Some("#robrix:matrix.org"));
        assert_eq!(address("!abc"), None);
        assert_eq!(address("#robrix:matrix.org"), None);
        assert_eq!(address("robrix"), None);
    }

    #[test]
    fn finds_fully_qualified_queries() {
        let is_fully_qualified = |query: &str| SearchQuery::parse(query).unwrap().is_fully_qualified();
        assert!(is_fully_qualified("#robrix:matrix.org"));
        assert!(is_fully_qualified("!abc:matrix.org"));
        assert!(is_fully_qualified("@alice:matrix.org"));
        assert!(is_fully_qualified("https://matrix.to/#/#robrix:matrix.org"));
        assert!(is_fully_qualified("matrix:u/alice:matrix.org"));
        assert!(!is_fully_qualified("#robrix"));
        assert!(!is_fully_qualified("#robrix:"));
        assert!(!is_fully_qualified("!abc"));
        assert!(!is_fully_qualified("@alice"));
        assert!(!is_fully_qualified("robrix"));
        assert!(!is_fully_qualified("robrix:matrix.org"));
    }

    #[test]
    fn completes_partial_room_aliases() {
        assert_eq!(room_ids("#robrix"), ["#robrix:matrix.org"]);
        assert_eq!(room_ids("#robrix:"), ["#robrix:matrix.org"]);
        assert_eq!(room_ids("robrix"), ["#robrix:matrix.org"]);
        assert_eq!(room_ids("robrix:example.org"), ["#robrix:example.org"]);
        assert_eq!(room_ids("#Robrix:Example.org"), ["#Robrix:example.org", "#robrix:example.org"]);
        assert!(room_ids("#").is_empty());
        assert!(room_ids("#robrix:not a server").is_empty());
        assert!(room_ids("Robrix Dev").is_empty());
        assert!(room_ids("@alice").is_empty());
    }

    #[test]
    fn completes_partial_room_ids() {
        assert_eq!(room_ids("!abc"), ["!abc", "!abc:matrix.org"]);
        assert_eq!(room_ids("!AbC:example.org"), ["!AbC:example.org"]);
        assert!(room_ids("!").is_empty());
        assert!(room_ids("!abc def").is_empty());
        assert_eq!(room_ids("https://matrix.to/#/!abc:example.org?via=example.org"), ["!abc:example.org"]);
    }

    #[test]
    fn completes_partial_user_ids() {
        assert_eq!(user_ids("@alice"), ["@alice:matrix.org"]);
        assert_eq!(user_ids("alice"), ["@alice:matrix.org"]);
        assert_eq!(user_ids("alice:Example.org"), ["@alice:example.org"]);
        assert_eq!(user_ids("@Alice"), ["@Alice:matrix.org", "@alice:matrix.org"]);
        assert_eq!(user_ids("matrix:u/bob:example.org"), ["@bob:example.org"]);
        assert!(user_ids("@").is_empty());
        assert!(user_ids("alice smith").is_empty());
        assert!(user_ids("#robrix").is_empty());
        assert!(SearchQuery::User("@alice".into()).user_candidates(None).is_empty());
    }

    #[test]
    fn finds_full_user_ids() {
        let full_user_id = |query: &str| SearchQuery::parse(query).unwrap().full_user_id().map(|id| id.to_string());
        assert_eq!(full_user_id("@alice:example.org").as_deref(), Some("@alice:example.org"));
        assert_eq!(full_user_id("https://matrix.to/#/@alice:example.org").as_deref(), Some("@alice:example.org"));
        assert_eq!(full_user_id("@alice"), None);
        assert_eq!(full_user_id("alice:example.org"), None);
    }

    #[test]
    fn picks_directory_search_terms() {
        let terms = |query: &str| {
            let query = SearchQuery::parse(query).unwrap();
            (query.room_search_term().map(str::to_owned), query.user_search_term().map(str::to_owned))
        };
        assert_eq!(terms("Robrix Dev"), (Some("Robrix Dev".into()), Some("Robrix Dev".into())));
        assert_eq!(terms("#robrix:matrix.org"), (Some("robrix".into()), None));
        assert_eq!(terms("#"), (None, None));
        assert_eq!(terms("!abc:matrix.org"), (None, None));
        assert_eq!(terms("@alice:"), (None, Some("alice".into())));
        assert_eq!(terms("@alice:matrix.org"), (None, Some("alice:matrix.org".into())));
        assert_eq!(terms("https://matrix.to/#/@alice:matrix.org"), (None, None));
    }
}
