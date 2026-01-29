# Tillandsia Social Features Implementation Plan

## Overview

This document outlines the implementation plan for adding social media features to Robrix based on the [Matrix-Based Social Media Platform Architecture](../../tillandsia/docs/matrix-protocol/social-media-architecture.md). The implementation follows a modular, feature-gated approach to minimize conflicts with upstream Robrix development.

**Key Principles:**
- All features isolated behind `--features social` Cargo feature flag
- Separate modules and files - no modifications to existing upstream code
- Custom event types in dedicated workspace crate
- Strict security validation at all boundaries
- CI/CD for multi-platform builds

---

## Architecture Overview

### Module Structure

```
robrix/
├── Cargo.toml                          # Add social feature flag
├── robrix-social-events/               # NEW: Workspace crate for event types
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── profile.rs                  # org.social.profile
│       ├── event.rs                    # org.social.event
│       ├── rsvp.rs                     # org.social.rsvp
│       ├── link_preview.rs             # org.social.link_preview
│       └── caption.rs                  # org.social.caption
│
├── src/
│   ├── social/                         # NEW: [FEATURE: social] Full implementation
│   │   ├── mod.rs                      # Module root, live_design registration
│   │   ├── profile_room.rs             # Profile room management
│   │   ├── feed_room.rs                # Feed room (public/friends/close)
│   │   ├── post.rs                     # Post creation and display
│   │   ├── reactions.rs                # Reactions aggregation
│   │   ├── events/                     # Event gatherings
│   │   │   ├── mod.rs
│   │   │   ├── event_room.rs           # Event room creation/management
│   │   │   └── rsvp.rs                 # RSVP system
│   │   ├── friends/                    # Friend network
│   │   │   ├── mod.rs
│   │   │   ├── friends_space.rs        # Friends space management
│   │   │   └── friend_request.rs       # Knock/invite flow
│   │   ├── newsfeed/                   # Aggregated timeline
│   │   │   ├── mod.rs
│   │   │   ├── feed_aggregator.rs      # Multi-room aggregation
│   │   │   └── feed_filter.rs          # Sync filters
│   │   ├── discovery/                  # Profile/event discovery
│   │   │   ├── mod.rs
│   │   │   └── public_directory.rs
│   │   ├── privacy/                    # Privacy controls
│   │   │   ├── mod.rs
│   │   │   └── sharing_guard.rs        # Cross-posting safeguards
│   │   └── widgets/                    # UI components
│   │       ├── mod.rs
│   │       ├── post_composer.rs
│   │       ├── post_card.rs
│   │       ├── profile_page.rs
│   │       ├── event_card.rs
│   │       ├── feed_view.rs
│   │       └── friend_list.rs
│   │
│   └── social_dummy/                   # NEW: [NO FEATURE] Placeholder
│       └── mod.rs                      # Empty widgets, hidden UI elements
│
├── .github/workflows/
│   ├── build-social.yml                # NEW: CI for social features
│   └── release-social.yml              # NEW: Release workflow
│
└── tests/
    └── social_integration/             # NEW: Integration tests
        ├── mod.rs
        ├── profile_tests.rs
        ├── feed_tests.rs
        ├── friend_tests.rs
        └── event_tests.rs
```

### Feature Flag Pattern

Following Robrix's existing TSP feature pattern:

```rust
// In src/lib.rs
#[cfg(feature = "social")]
pub mod social;

#[cfg(not(feature = "social"))]
pub mod social_dummy;

// In App::live_register()
#[cfg(feature = "social")] {
    crate::social::live_design(cx);
    cx.link(id!(social_link), id!(social_enabled));
}
#[cfg(not(feature = "social"))] {
    crate::social_dummy::live_design(cx);
    cx.link(id!(social_link), id!(social_disabled));
}
```

---

## Phase 1: Foundation Infrastructure

### 1.1 Cargo.toml Updates

**File**: `Cargo.toml`

```toml
[features]
default = []
social = ["robrix-social-events"]
tsp = [...]  # Existing
full = ["social", "tsp"]  # Combined feature

[dependencies]
robrix-social-events = { path = "./robrix-social-events", optional = true }

[workspace]
members = [".", "robrix-social-events"]
```

### 1.2 Custom Event Types Crate

**Crate**: `robrix-social-events`

This separate crate defines all custom Matrix event types used by the social features. Keeping it separate allows:
- Independent versioning and publishing
- Reuse by other Matrix clients
- Clear API boundary

#### Cargo.toml

```toml
[package]
name = "robrix-social-events"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
description = "Custom Matrix event types for social media features"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
ruma = { version = "0.12", features = ["events"] }
thiserror = "2.0"
url = { version = "2.5", features = ["serde"] }
```

#### Profile Event Type

**File**: `robrix-social-events/src/profile.rs`

```rust
use ruma::events::macros::EventContent;
use serde::{Deserialize, Serialize};

/// Custom profile data stored as room state in a user's profile room.
/// Event type: `org.social.profile`
#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "org.social.profile", kind = State, state_key_type = EmptyStateKey)]
#[serde(deny_unknown_fields)]
pub struct SocialProfileEventContent {
    /// User's biography/about text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,

    /// User's location (city, country, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,

    /// User's website URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<url::Url>,

    /// Cover/banner image MXC URI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<ruma::OwnedMxcUri>,

    /// Additional custom fields (for extensibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<serde_json::Value>,
}
```

#### Event (Gathering) Type

**File**: `robrix-social-events/src/event.rs`

```rust
use ruma::events::macros::EventContent;
use serde::{Deserialize, Serialize};

/// Event/gathering details stored as room state.
/// Event type: `org.social.event`
#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "org.social.event", kind = State, state_key_type = EmptyStateKey)]
#[serde(deny_unknown_fields)]
pub struct SocialEventEventContent {
    /// Event title
    pub title: String,

    /// Event description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Start time (Unix timestamp in milliseconds)
    pub start_time: u64,

    /// End time (Unix timestamp in milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<u64>,

    /// Event location
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<EventLocation>,

    /// Cover image MXC URI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<ruma::OwnedMxcUri>,

    /// Visibility level
    pub visibility: EventVisibility,

    /// RSVP deadline (Unix timestamp in milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rsvp_deadline: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventLocation {
    /// Human-readable location name
    pub name: String,

    /// Full address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,

    /// Geo URI (e.g., "geo:40.7829,-73.9654")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventVisibility {
    Public,
    Private,
}
```

#### RSVP Event Type

**File**: `robrix-social-events/src/rsvp.rs`

```rust
use ruma::events::macros::EventContent;
use serde::{Deserialize, Serialize};

/// RSVP status for an event.
/// Event type: `org.social.rsvp`
///
/// SECURITY: The state_key MUST equal the sender's user ID.
/// Clients MUST validate this and ignore events where they don't match.
#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "org.social.rsvp", kind = State, state_key_type = ruma::OwnedUserId)]
#[serde(deny_unknown_fields)]
pub struct SocialRsvpEventContent {
    /// RSVP status
    pub status: RsvpStatus,

    /// Number of guests (including the user)
    #[serde(default = "default_guests")]
    pub guests: u32,

    /// Optional note (e.g., "Bringing potato salad!")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn default_guests() -> u32 {
    1
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RsvpStatus {
    Going,
    Interested,
    NotGoing,
}
```

#### Link Preview Type

**File**: `robrix-social-events/src/link_preview.rs`

```rust
use serde::{Deserialize, Serialize};

/// Rich link preview data embedded in message content.
/// Field name: `org.social.link_preview`
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LinkPreview {
    /// Original URL
    pub url: url::Url,

    /// Page title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Page description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Preview image MXC URI (uploaded to homeserver)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ruma::OwnedMxcUri>,

    /// Site name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_name: Option<String>,
}
```

### 1.3 Feature-Gated Module Root

**File**: `src/social/mod.rs`

```rust
//! Social media features for Robrix.
//!
//! This module implements the Matrix-based social media architecture,
//! providing profile pages, feeds, friend networks, and events.

use makepad_widgets::*;

pub mod profile_room;
pub mod feed_room;
pub mod post;
pub mod reactions;
pub mod events;
pub mod friends;
pub mod newsfeed;
pub mod discovery;
pub mod privacy;
pub mod widgets;

mod actions;
mod requests;

pub use actions::*;
pub use requests::*;

/// Register all social feature UI components.
pub fn live_design(cx: &mut Cx) {
    // Register all widget designs
    widgets::live_design(cx);
}
```

### 1.4 Dummy Module (Feature Disabled)

**File**: `src/social_dummy/mod.rs`

```rust
//! Placeholder module when social features are disabled.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;

    // Empty placeholder widgets that render nothing
    pub SocialFeedView = {{SocialFeedView}} {}
    pub SocialProfilePage = {{SocialProfilePage}} {}
    pub SocialPostComposer = {{SocialPostComposer}} {}
    pub SocialEventCard = {{SocialEventCard}} {}
    pub SocialFriendList = {{SocialFriendList}} {}
}

#[derive(Live, LiveHook, Widget)]
pub struct SocialFeedView {
    #[deref] view: View,
}

impl Widget for SocialFeedView {
    fn draw_walk(&mut self, _cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        DrawStep::done()
    }
}

// Similar empty implementations for other widgets...
```

---

## Phase 2: Profile Rooms

### 2.1 Profile Room Service

**File**: `src/social/profile_room.rs`

```rust
//! Profile room management for social profiles.
//!
//! Each user has a dedicated "profile room" that stores their extended
//! profile information as state events.

use matrix_sdk::{
    room::Room,
    ruma::{
        api::client::room::create_room::v3::Request as CreateRoomRequest,
        events::room::{
            join_rules::{JoinRule, RoomJoinRulesEventContent},
            history_visibility::{HistoryVisibility, RoomHistoryVisibilityEventContent},
        },
        OwnedRoomAliasId, OwnedRoomId, OwnedUserId,
    },
    Client,
};
use robrix_social_events::profile::SocialProfileEventContent;

/// Profile room configuration
pub struct ProfileRoomConfig {
    /// Room alias format: #profile_{localpart}:{server}
    pub alias_prefix: &'static str,
    /// Default join rules for profile rooms
    pub default_join_rule: JoinRule,
    /// Default history visibility
    pub default_history_visibility: HistoryVisibility,
}

impl Default for ProfileRoomConfig {
    fn default() -> Self {
        Self {
            alias_prefix: "profile_",
            default_join_rule: JoinRule::Public,
            default_history_visibility: HistoryVisibility::WorldReadable,
        }
    }
}

/// Service for managing user profile rooms
pub struct ProfileRoomService {
    client: Client,
    config: ProfileRoomConfig,
}

impl ProfileRoomService {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            config: ProfileRoomConfig::default(),
        }
    }

    /// Create a profile room for the current user
    pub async fn create_profile_room(
        &self,
        initial_profile: SocialProfileEventContent,
    ) -> Result<OwnedRoomId, ProfileRoomError> {
        let user_id = self.client.user_id()
            .ok_or(ProfileRoomError::NotLoggedIn)?;

        let alias = self.profile_alias_for_user(user_id)?;

        // Check if room already exists
        if let Some(room_id) = self.find_profile_room(user_id).await? {
            return Err(ProfileRoomError::AlreadyExists(room_id));
        }

        let request = CreateRoomRequest::new();
        // Configure room creation...

        todo!("Implement room creation with state events")
    }

    /// Find a user's profile room by alias
    pub async fn find_profile_room(
        &self,
        user_id: &UserId,
    ) -> Result<Option<OwnedRoomId>, ProfileRoomError> {
        let alias = self.profile_alias_for_user(user_id)?;

        match self.client.resolve_room_alias(&alias).await {
            Ok(response) => Ok(Some(response.room_id)),
            Err(e) if e.is_not_found() => Ok(None),
            Err(e) => Err(ProfileRoomError::MatrixError(e)),
        }
    }

    /// Update the profile in an existing profile room
    pub async fn update_profile(
        &self,
        room_id: &RoomId,
        profile: SocialProfileEventContent,
    ) -> Result<(), ProfileRoomError> {
        let room = self.client.get_room(room_id)
            .ok_or(ProfileRoomError::RoomNotFound)?;

        room.send_state_event(profile).await
            .map_err(ProfileRoomError::MatrixError)?;

        Ok(())
    }

    /// Get profile alias for a user
    fn profile_alias_for_user(&self, user_id: &UserId) -> Result<OwnedRoomAliasId, ProfileRoomError> {
        let localpart = user_id.localpart();
        let server = user_id.server_name();
        let alias = format!("#{}{}:{}", self.config.alias_prefix, localpart, server);
        alias.try_into().map_err(|_| ProfileRoomError::InvalidAlias)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileRoomError {
    #[error("Not logged in")]
    NotLoggedIn,
    #[error("Profile room already exists: {0}")]
    AlreadyExists(OwnedRoomId),
    #[error("Room not found")]
    RoomNotFound,
    #[error("Invalid room alias")]
    InvalidAlias,
    #[error("Matrix error: {0}")]
    MatrixError(#[from] matrix_sdk::Error),
}
```

### 2.2 Profile Page Widget

**File**: `src/social/widgets/profile_page.rs`

```rust
//! Profile page widget displaying user's social profile.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::Avatar;

    pub SocialProfilePage = {{SocialProfilePage}} {
        width: Fill,
        height: Fill,
        flow: Down,

        // Cover photo banner
        cover_container = <View> {
            width: Fill,
            height: 200,

            cover_image = <Image> {
                width: Fill,
                height: Fill,
                fit: Cover,
            }
        }

        // Profile info section
        profile_section = <View> {
            width: Fill,
            padding: 16,
            flow: Down,
            spacing: 12,

            // Avatar overlapping cover
            avatar_row = <View> {
                margin: { top: -50 },

                avatar = <Avatar> {
                    width: 100,
                    height: 100,
                }
            }

            // Name and username
            name_label = <Label> {
                text: "",
                draw_text: {
                    text_style: <TITLE_TEXT> {},
                    color: #000,
                }
            }

            username_label = <Label> {
                text: "",
                draw_text: {
                    text_style: <REGULAR_TEXT> {},
                    color: #666,
                }
            }

            // Bio
            bio_label = <Label> {
                text: "",
                draw_text: {
                    text_style: <REGULAR_TEXT> {},
                    color: #333,
                    wrap: Word,
                }
            }

            // Location and website
            meta_row = <View> {
                flow: Right,
                spacing: 16,

                location_label = <Label> {
                    text: "",
                    draw_text: {
                        text_style: <REGULAR_TEXT> {},
                        color: #666,
                    }
                }

                website_link = <LinkLabel> {
                    text: "",
                }
            }

            // Action buttons
            action_row = <View> {
                flow: Right,
                spacing: 8,

                follow_button = <Button> {
                    text: "Follow",
                }

                friend_request_button = <Button> {
                    text: "Add Friend",
                }

                message_button = <Button> {
                    text: "Message",
                }
            }
        }

        // Posts feed
        posts_section = <View> {
            width: Fill,
            height: Fill,

            // Will embed SocialFeedView here
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct SocialProfilePage {
    #[deref] view: View,

    #[rust] user_id: Option<OwnedUserId>,
    #[rust] profile: Option<LoadedProfile>,
}

// Implementation...
```

---

## Phase 3: Posts and Feed Rooms

### 3.1 Feed Room Service

**File**: `src/social/feed_room.rs`

```rust
//! Feed room management for user posts.
//!
//! Each user maintains up to three feed rooms:
//! - Public feed: Anyone can read
//! - Friends feed: Only friends can read (restricted join)
//! - Close friends feed: Invite-only

use matrix_sdk::{
    room::Room,
    ruma::{
        events::room::{
            join_rules::{AllowRule, JoinRule, RoomJoinRulesEventContent, RoomMembership},
            history_visibility::{HistoryVisibility, RoomHistoryVisibilityEventContent},
            power_levels::RoomPowerLevelsEventContent,
        },
        OwnedRoomId, OwnedUserId, Int,
    },
    Client,
};

/// Feed privacy level
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FeedPrivacy {
    /// Anyone can read, public room directory
    Public,
    /// Only members of user's friends space can join
    Friends,
    /// Invite-only for close friends
    CloseFriends,
}

impl FeedPrivacy {
    /// Get the Matrix join rule for this privacy level
    pub fn join_rule(&self, friends_space_id: Option<&RoomId>) -> JoinRule {
        match self {
            Self::Public => JoinRule::Public,
            Self::Friends => {
                if let Some(space_id) = friends_space_id {
                    JoinRule::Restricted(vec![
                        AllowRule::RoomMembership(RoomMembership::new(space_id.to_owned()))
                    ])
                } else {
                    // Fallback to invite if no friends space
                    JoinRule::Invite
                }
            }
            Self::CloseFriends => JoinRule::Invite,
        }
    }

    /// Get the history visibility for this privacy level
    pub fn history_visibility(&self) -> HistoryVisibility {
        match self {
            Self::Public => HistoryVisibility::WorldReadable,
            Self::Friends | Self::CloseFriends => HistoryVisibility::Shared,
        }
    }
}

/// Power levels for feed rooms (only owner can post)
pub fn feed_room_power_levels(owner: &UserId) -> RoomPowerLevelsEventContent {
    let mut power_levels = RoomPowerLevelsEventContent::new();

    // Only owner can send messages
    power_levels.events_default = Int::new(50).unwrap();
    power_levels.users.insert(owner.to_owned(), Int::new(100).unwrap());

    // Default users can react and read
    power_levels.users_default = Int::new(0).unwrap();

    power_levels
}

/// Service for managing feed rooms
pub struct FeedRoomService {
    client: Client,
}

impl FeedRoomService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Create a feed room with the specified privacy level
    pub async fn create_feed_room(
        &self,
        privacy: FeedPrivacy,
        friends_space_id: Option<&RoomId>,
    ) -> Result<OwnedRoomId, FeedRoomError> {
        let user_id = self.client.user_id()
            .ok_or(FeedRoomError::NotLoggedIn)?;

        // Build room creation request with appropriate settings
        todo!("Implement feed room creation")
    }

    /// Get all feed rooms for a user
    pub async fn get_user_feeds(&self, user_id: &UserId) -> Result<UserFeeds, FeedRoomError> {
        todo!("Implement feed discovery")
    }
}

/// Collection of a user's feed rooms
pub struct UserFeeds {
    pub public: Option<OwnedRoomId>,
    pub friends: Option<OwnedRoomId>,
    pub close_friends: Option<OwnedRoomId>,
}
```

### 3.2 Post Creation

**File**: `src/social/post.rs`

```rust
//! Post creation and management.
//!
//! Posts are standard Matrix messages with optional social extensions.

use matrix_sdk::{
    room::Room,
    ruma::{
        events::room::message::{
            ImageMessageEventContent, MessageType, RoomMessageEventContent,
            TextMessageEventContent, VideoMessageEventContent,
        },
        OwnedEventId, OwnedMxcUri,
    },
};
use robrix_social_events::link_preview::LinkPreview;

/// A social media post
pub struct Post {
    /// The message content
    pub content: PostContent,
    /// Target feed rooms to post to
    pub targets: Vec<OwnedRoomId>,
}

/// Post content types
pub enum PostContent {
    /// Text-only post
    Text {
        body: String,
        formatted_body: Option<String>,
        mentions: Vec<OwnedUserId>,
    },
    /// Photo post with optional caption
    Photo {
        mxc_uri: OwnedMxcUri,
        caption: Option<String>,
        thumbnail_uri: Option<OwnedMxcUri>,
        width: u32,
        height: u32,
    },
    /// Video post
    Video {
        mxc_uri: OwnedMxcUri,
        caption: Option<String>,
        thumbnail_uri: Option<OwnedMxcUri>,
        duration_ms: Option<u64>,
    },
    /// Link share with preview
    Link {
        url: url::Url,
        comment: Option<String>,
        preview: Option<LinkPreview>,
    },
}

impl PostContent {
    /// Convert to Matrix message content
    pub fn to_message_content(&self) -> RoomMessageEventContent {
        match self {
            Self::Text { body, formatted_body, mentions } => {
                let mut content = if let Some(html) = formatted_body {
                    RoomMessageEventContent::text_html(body, html)
                } else {
                    RoomMessageEventContent::text_plain(body)
                };

                // Add v1.17 compliant mentions
                if !mentions.is_empty() {
                    content.mentions = Some(ruma::events::room::message::Mentions {
                        user_ids: mentions.iter().cloned().collect(),
                        room: false,
                    });
                }

                content
            }
            Self::Photo { mxc_uri, caption, .. } => {
                let mut content = ImageMessageEventContent::plain(
                    caption.clone().unwrap_or_default(),
                    mxc_uri.clone(),
                );
                // Add social caption extension
                // content.custom.insert("org.social.caption", caption);
                RoomMessageEventContent::new(MessageType::Image(content))
            }
            // ... other content types
            _ => todo!("Implement other post types"),
        }
    }
}

/// Service for creating and managing posts
pub struct PostService {
    client: Client,
}

impl PostService {
    /// Create a new post in the specified feed rooms
    pub async fn create_post(&self, post: Post) -> Result<Vec<OwnedEventId>, PostError> {
        let mut event_ids = Vec::new();
        let content = post.content.to_message_content();

        for room_id in &post.targets {
            let room = self.client.get_room(room_id)
                .ok_or_else(|| PostError::RoomNotFound(room_id.clone()))?;

            let response = room.send(content.clone()).await
                .map_err(PostError::MatrixError)?;

            event_ids.push(response.event_id);
        }

        Ok(event_ids)
    }
}
```

### 3.3 Post Composer Widget

**File**: `src/social/widgets/post_composer.rs`

```rust
//! Post composer widget for creating new posts.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::mentionable_text_input::MentionableTextInput;

    pub SocialPostComposer = {{SocialPostComposer}} {
        width: Fill,
        padding: 16,
        flow: Down,
        spacing: 12,

        // Header with avatar and audience selector
        header_row = <View> {
            flow: Right,
            spacing: 12,
            align: { y: 0.5 },

            user_avatar = <Avatar> {
                width: 40,
                height: 40,
            }

            audience_dropdown = <DropDown> {
                labels: ["Public", "Friends", "Close Friends", "Public + Friends"],
                selected_item: 0,
            }
        }

        // Text input area
        text_input = <MentionableTextInput> {
            width: Fill,
            height: Fit,
            placeholder: "What's on your mind?",
        }

        // Media preview area (shown when media attached)
        media_preview = <View> {
            visible: false,
            width: Fill,
            height: 200,

            preview_image = <Image> {
                width: Fill,
                height: Fill,
                fit: Contain,
            }

            remove_media_button = <IconButton> {
                icon: dep("crate://self/resources/icons/close.svg"),
            }
        }

        // Link preview (shown when URL detected)
        link_preview = <View> {
            visible: false,
            width: Fill,
            padding: 12,

            link_title = <Label> { text: "" }
            link_description = <Label> { text: "" }
            link_image = <Image> {}
        }

        // Action bar
        action_bar = <View> {
            flow: Right,
            spacing: 8,
            align: { y: 0.5 },

            attach_photo_button = <IconButton> {
                icon: dep("crate://self/resources/icons/photo.svg"),
            }

            attach_video_button = <IconButton> {
                icon: dep("crate://self/resources/icons/video.svg"),
            }

            attach_location_button = <IconButton> {
                icon: dep("crate://self/resources/icons/location.svg"),
            }

            <Filler> {}

            post_button = <Button> {
                text: "Post",
                enabled: false,
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct SocialPostComposer {
    #[deref] view: View,

    #[rust] selected_audience: Vec<FeedPrivacy>,
    #[rust] attached_media: Option<AttachedMedia>,
    #[rust] detected_link: Option<url::Url>,
}

pub enum AttachedMedia {
    Photo { path: PathBuf, mxc_uri: Option<OwnedMxcUri> },
    Video { path: PathBuf, mxc_uri: Option<OwnedMxcUri> },
}

// Widget implementation...
```

---

## Phase 4: Aggregated Newsfeed

### 4.1 Feed Aggregator Service

**File**: `src/social/newsfeed/feed_aggregator.rs`

```rust
//! Newsfeed aggregation across multiple feed rooms.
//!
//! The newsfeed is the union of all joined feed rooms, sorted
//! chronologically or by engagement.

use matrix_sdk::{
    ruma::{
        api::client::sync::sync_events::v3::Filter,
        events::TimelineEventType,
        OwnedRoomId,
    },
    Client,
};
use std::collections::BTreeMap;

/// Sync filter optimized for feed rooms
pub fn create_feed_sync_filter() -> Filter {
    let mut filter = Filter::default();

    // Only fetch message events, reactions, and redactions
    filter.room.timeline.types = Some(vec![
        TimelineEventType::RoomMessage.to_string(),
        TimelineEventType::Reaction.to_string(),
        TimelineEventType::RoomRedaction.to_string(),
    ]);
    filter.room.timeline.limit = Some(10u32.into());

    // Lazy load members for performance
    filter.room.state.lazy_load_members = true;

    // Minimal state events
    filter.room.state.types = Some(vec![
        "m.room.name".to_string(),
        "m.room.avatar".to_string(),
    ]);

    filter
}

/// Sort order for the newsfeed
#[derive(Clone, Copy, Debug, Default)]
pub enum FeedSortOrder {
    /// Most recent first (Twitter-style)
    #[default]
    Chronological,
    /// By engagement (reactions + comments)
    Engagement,
    /// Grouped by author
    GroupedByAuthor,
}

/// Aggregated feed item
pub struct FeedItem {
    /// Source room ID
    pub room_id: OwnedRoomId,
    /// Event ID
    pub event_id: OwnedEventId,
    /// Author user ID
    pub sender: OwnedUserId,
    /// Timestamp
    pub origin_server_ts: MilliSecondsSinceUnixEpoch,
    /// Message content
    pub content: PostContent,
    /// Reaction counts
    pub reactions: BTreeMap<String, u32>,
    /// Comment/reply count
    pub comment_count: u32,
}

/// Service for aggregating feed items
pub struct FeedAggregator {
    client: Client,
    /// IDs of feed rooms to aggregate
    feed_rooms: Vec<OwnedRoomId>,
    /// Current sort order
    sort_order: FeedSortOrder,
}

impl FeedAggregator {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            feed_rooms: Vec::new(),
            sort_order: FeedSortOrder::default(),
        }
    }

    /// Add a feed room to the aggregation
    pub fn add_feed_room(&mut self, room_id: OwnedRoomId) {
        if !self.feed_rooms.contains(&room_id) {
            self.feed_rooms.push(room_id);
        }
    }

    /// Remove a feed room from aggregation
    pub fn remove_feed_room(&mut self, room_id: &RoomId) {
        self.feed_rooms.retain(|id| id != room_id);
    }

    /// Get aggregated feed items
    pub async fn get_feed(&self, limit: usize) -> Result<Vec<FeedItem>, FeedError> {
        let mut all_items = Vec::new();

        for room_id in &self.feed_rooms {
            if let Some(room) = self.client.get_room(room_id) {
                // Fetch recent timeline items
                let items = self.fetch_room_items(&room, limit).await?;
                all_items.extend(items);
            }
        }

        // Sort according to current order
        self.sort_items(&mut all_items);

        // Limit total results
        all_items.truncate(limit);

        Ok(all_items)
    }

    fn sort_items(&self, items: &mut Vec<FeedItem>) {
        match self.sort_order {
            FeedSortOrder::Chronological => {
                items.sort_by(|a, b| b.origin_server_ts.cmp(&a.origin_server_ts));
            }
            FeedSortOrder::Engagement => {
                items.sort_by(|a, b| {
                    let a_engagement = a.reactions.values().sum::<u32>() + a.comment_count;
                    let b_engagement = b.reactions.values().sum::<u32>() + b.comment_count;
                    b_engagement.cmp(&a_engagement)
                });
            }
            FeedSortOrder::GroupedByAuthor => {
                items.sort_by(|a, b| {
                    a.sender.cmp(&b.sender)
                        .then_with(|| b.origin_server_ts.cmp(&a.origin_server_ts))
                });
            }
        }
    }
}
```

### 4.2 Feed View Widget

**File**: `src/social/widgets/feed_view.rs`

```rust
//! Feed view widget displaying aggregated newsfeed.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use crate::social::widgets::post_card::SocialPostCard;

    pub SocialFeedView = {{SocialFeedView}} {
        width: Fill,
        height: Fill,

        // Pull-to-refresh indicator
        refresh_indicator = <View> {
            visible: false,
            height: 50,
            align: { x: 0.5, y: 0.5 },

            spinner = <RotatedImage> {
                source: dep("crate://self/resources/icons/spinner.svg"),
            }
        }

        // New posts indicator
        new_posts_banner = <View> {
            visible: false,
            width: Fill,
            height: 40,
            align: { x: 0.5, y: 0.5 },

            <Button> {
                text: "New posts",
            }
        }

        // Sort/filter bar
        toolbar = <View> {
            width: Fill,
            height: 44,
            flow: Right,
            padding: { left: 16, right: 16 },
            align: { y: 0.5 },

            sort_dropdown = <DropDown> {
                labels: ["Recent", "Popular", "Grouped"],
                selected_item: 0,
            }

            <Filler> {}

            filter_button = <IconButton> {
                icon: dep("crate://self/resources/icons/filter.svg"),
            }
        }

        // Scrollable feed
        feed_list = <PortalList> {
            width: Fill,
            height: Fill,

            PostCard = <SocialPostCard> {}
        }

        // Empty state
        empty_state = <View> {
            visible: false,
            width: Fill,
            height: Fill,
            align: { x: 0.5, y: 0.5 },

            <Label> {
                text: "No posts yet. Follow some users to see their posts here!",
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct SocialFeedView {
    #[deref] view: View,

    #[rust] items: Vec<FeedItem>,
    #[rust] is_loading: bool,
    #[rust] has_new_posts: bool,
}

// Widget implementation with infinite scroll, pull-to-refresh...
```

---

## Phase 5: Friend Network

### 5.1 Friends Space Service

**File**: `src/social/friends/friends_space.rs`

```rust
//! Friends space management for social graph.
//!
//! Each user maintains a private space containing their friends' feed rooms.

use matrix_sdk::{
    room::Room,
    ruma::{
        events::{
            room::join_rules::{JoinRule, RoomJoinRulesEventContent},
            space::child::SpaceChildEventContent,
        },
        OwnedRoomId, OwnedUserId,
    },
    Client,
};

/// Service for managing the friends space
pub struct FriendsSpaceService {
    client: Client,
    /// The user's friends space room ID
    space_id: Option<OwnedRoomId>,
}

impl FriendsSpaceService {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            space_id: None,
        }
    }

    /// Create or get the user's friends space
    pub async fn get_or_create_friends_space(&mut self) -> Result<OwnedRoomId, FriendsError> {
        if let Some(id) = &self.space_id {
            return Ok(id.clone());
        }

        // Try to find existing friends space
        if let Some(id) = self.find_friends_space().await? {
            self.space_id = Some(id.clone());
            return Ok(id);
        }

        // Create new friends space
        let id = self.create_friends_space().await?;
        self.space_id = Some(id.clone());
        Ok(id)
    }

    /// Add a friend's feed room to the space
    pub async fn add_friend(&mut self, friend_feed_room: &RoomId) -> Result<(), FriendsError> {
        let space_id = self.get_or_create_friends_space().await?;
        let space = self.client.get_room(&space_id)
            .ok_or(FriendsError::SpaceNotFound)?;

        // Add as space child
        let content = SpaceChildEventContent::new(vec![]);
        space.send_state_event_for_key(friend_feed_room.to_owned(), content).await
            .map_err(FriendsError::MatrixError)?;

        Ok(())
    }

    /// Remove a friend from the space
    pub async fn remove_friend(&mut self, friend_feed_room: &RoomId) -> Result<(), FriendsError> {
        let space_id = self.get_or_create_friends_space().await?;
        let space = self.client.get_room(&space_id)
            .ok_or(FriendsError::SpaceNotFound)?;

        // Remove space child by sending empty content
        // (Matrix spec: empty content removes the relationship)
        todo!("Implement friend removal")
    }

    /// Get list of friends (feed room IDs in the space)
    pub async fn get_friends(&self) -> Result<Vec<OwnedRoomId>, FriendsError> {
        let space_id = self.space_id.as_ref()
            .ok_or(FriendsError::SpaceNotFound)?;

        let space = self.client.get_room(space_id)
            .ok_or(FriendsError::SpaceNotFound)?;

        // Get space children
        todo!("Implement friends list retrieval")
    }

    /// Check if a user is a friend (bidirectional membership check)
    pub async fn is_mutual_friend(&self, user_id: &UserId) -> Result<bool, FriendsError> {
        todo!("Implement mutual friend check")
    }

    async fn find_friends_space(&self) -> Result<Option<OwnedRoomId>, FriendsError> {
        // Look for space with specific naming convention or tag
        todo!("Implement friends space discovery")
    }

    async fn create_friends_space(&self) -> Result<OwnedRoomId, FriendsError> {
        let user_id = self.client.user_id()
            .ok_or(FriendsError::NotLoggedIn)?;

        // Create private space
        // Name: "{user}'s Friends"
        // Join rules: invite only
        todo!("Implement friends space creation")
    }
}
```

### 5.2 Friend Request Flow

**File**: `src/social/friends/friend_request.rs`

```rust
//! Friend request flow using Matrix knock mechanism.

use matrix_sdk::{
    room::Room,
    ruma::{
        events::room::member::{MembershipState, RoomMemberEventContent},
        OwnedRoomId, OwnedUserId,
    },
    Client,
};

/// Friend request state
#[derive(Clone, Debug)]
pub enum FriendRequestState {
    /// No relationship
    None,
    /// Pending outgoing request (we knocked)
    PendingOutgoing,
    /// Pending incoming request (they knocked)
    PendingIncoming,
    /// Accepted (mutual friends)
    Friends,
    /// Blocked
    Blocked,
}

/// Service for handling friend requests
pub struct FriendRequestService {
    client: Client,
}

impl FriendRequestService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Send a friend request by knocking on their friends-only feed
    pub async fn send_friend_request(
        &self,
        target_friends_feed: &RoomId,
    ) -> Result<(), FriendRequestError> {
        // Knock on the room
        self.client.knock(target_friends_feed).await
            .map_err(FriendRequestError::MatrixError)?;

        Ok(())
    }

    /// Accept a friend request (invite them to our friends feed)
    pub async fn accept_friend_request(
        &self,
        requester: &UserId,
        our_friends_feed: &RoomId,
    ) -> Result<(), FriendRequestError> {
        let room = self.client.get_room(our_friends_feed)
            .ok_or(FriendRequestError::RoomNotFound)?;

        // Invite the requester
        room.invite_user_by_id(requester).await
            .map_err(FriendRequestError::MatrixError)?;

        Ok(())
    }

    /// Reject a friend request
    pub async fn reject_friend_request(
        &self,
        requester: &UserId,
        our_friends_feed: &RoomId,
    ) -> Result<(), FriendRequestError> {
        let room = self.client.get_room(our_friends_feed)
            .ok_or(FriendRequestError::RoomNotFound)?;

        // Kick the knock (if they're in knock state)
        room.kick_user(requester, Some("Friend request declined")).await
            .map_err(FriendRequestError::MatrixError)?;

        Ok(())
    }

    /// Get pending incoming friend requests
    pub async fn get_pending_requests(&self) -> Result<Vec<PendingFriendRequest>, FriendRequestError> {
        // Look for knock events in our friends-only feed rooms
        todo!("Implement pending request retrieval")
    }
}

/// A pending friend request
pub struct PendingFriendRequest {
    /// The user who sent the request
    pub requester: OwnedUserId,
    /// The room they knocked on
    pub room_id: OwnedRoomId,
    /// When the request was sent
    pub timestamp: MilliSecondsSinceUnixEpoch,
    /// Requester's profile info
    pub profile: Option<UserProfile>,
}
```

---

## Phase 6: Events (Gatherings)

### 6.1 Event Room Service

**File**: `src/social/events/event_room.rs`

```rust
//! Event/gathering room management.

use matrix_sdk::{
    room::Room,
    ruma::{
        events::room::{
            join_rules::{JoinRule, RoomJoinRulesEventContent},
            power_levels::RoomPowerLevelsEventContent,
        },
        Int, OwnedRoomId, OwnedUserId,
    },
    Client,
};
use robrix_social_events::event::{EventVisibility, SocialEventEventContent};

/// Power level roles for events
#[derive(Clone, Copy, Debug)]
pub enum EventRole {
    /// Full control (PL 100)
    Creator,
    /// Can edit event, moderate (PL 50)
    CoHost,
    /// Can chat, RSVP, potentially invite (PL 0)
    Guest,
}

impl EventRole {
    pub fn power_level(&self) -> Int {
        match self {
            Self::Creator => Int::new(100).unwrap(),
            Self::CoHost => Int::new(50).unwrap(),
            Self::Guest => Int::new(0).unwrap(),
        }
    }
}

/// Power levels for event rooms
pub fn event_room_power_levels(creator: &UserId, guests_can_invite: bool) -> RoomPowerLevelsEventContent {
    let mut power_levels = RoomPowerLevelsEventContent::new();

    // Creator has full control
    power_levels.users.insert(creator.to_owned(), EventRole::Creator.power_level());

    // State events require co-host level
    power_levels.state_default = EventRole::CoHost.power_level();

    // Event details require co-host level
    power_levels.events.insert(
        "org.social.event".into(),
        EventRole::CoHost.power_level(),
    );

    // RSVP can be set by anyone (their own)
    power_levels.events.insert(
        "org.social.rsvp".into(),
        EventRole::Guest.power_level(),
    );

    // Chat is open to all
    power_levels.events_default = EventRole::Guest.power_level();

    // Invite permission depends on event settings
    power_levels.invite = if guests_can_invite {
        EventRole::Guest.power_level()
    } else {
        EventRole::CoHost.power_level()
    };

    // Moderation requires co-host
    power_levels.kick = EventRole::CoHost.power_level();
    power_levels.ban = EventRole::CoHost.power_level();
    power_levels.redact = EventRole::CoHost.power_level();

    power_levels
}

/// Service for managing event rooms
pub struct EventRoomService {
    client: Client,
}

impl EventRoomService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Create a new event room
    pub async fn create_event(
        &self,
        event_details: SocialEventEventContent,
        guests_can_invite: bool,
    ) -> Result<OwnedRoomId, EventRoomError> {
        let user_id = self.client.user_id()
            .ok_or(EventRoomError::NotLoggedIn)?;

        let join_rule = match event_details.visibility {
            EventVisibility::Public => JoinRule::Public,
            EventVisibility::Private => JoinRule::Invite,
        };

        let power_levels = event_room_power_levels(user_id, guests_can_invite);

        // Create room with event state
        todo!("Implement event room creation")
    }

    /// Update event details
    pub async fn update_event(
        &self,
        room_id: &RoomId,
        event_details: SocialEventEventContent,
    ) -> Result<(), EventRoomError> {
        let room = self.client.get_room(room_id)
            .ok_or(EventRoomError::RoomNotFound)?;

        room.send_state_event(event_details).await
            .map_err(EventRoomError::MatrixError)?;

        Ok(())
    }

    /// Add a co-host to an event
    pub async fn add_cohost(
        &self,
        room_id: &RoomId,
        cohost: &UserId,
    ) -> Result<(), EventRoomError> {
        todo!("Implement co-host promotion")
    }
}
```

### 6.2 RSVP System

**File**: `src/social/events/rsvp.rs`

```rust
//! RSVP system for events.
//!
//! SECURITY: This module includes critical validation to prevent
//! RSVP spoofing attacks.

use matrix_sdk::{
    room::Room,
    ruma::{
        events::AnySyncStateEvent,
        OwnedEventId, OwnedUserId,
    },
    Client,
};
use robrix_social_events::rsvp::{RsvpStatus, SocialRsvpEventContent};

/// RSVP validation result
#[derive(Debug)]
pub enum RsvpValidation {
    Valid,
    /// state_key doesn't match sender - potential spoofing attempt
    SenderMismatch { claimed: OwnedUserId, actual: OwnedUserId },
    /// Invalid RSVP content
    InvalidContent(String),
}

/// Validate an RSVP event for security
///
/// CRITICAL: Matrix does NOT enforce that state_key matches sender.
/// Clients MUST perform this validation to prevent impersonation.
pub fn validate_rsvp_event(
    event: &AnySyncStateEvent,
    sender: &UserId,
) -> RsvpValidation {
    // For org.social.rsvp events, state_key must equal sender
    if event.event_type().to_string() == "org.social.rsvp" {
        let state_key = event.state_key();

        // Parse state_key as user ID
        match OwnedUserId::try_from(state_key.to_string()) {
            Ok(claimed_user) => {
                if claimed_user.as_ref() != sender {
                    return RsvpValidation::SenderMismatch {
                        claimed: claimed_user,
                        actual: sender.to_owned(),
                    };
                }
            }
            Err(_) => {
                return RsvpValidation::InvalidContent(
                    format!("Invalid state_key: {}", state_key)
                );
            }
        }
    }

    RsvpValidation::Valid
}

/// Service for managing RSVPs
pub struct RsvpService {
    client: Client,
}

impl RsvpService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Set the current user's RSVP for an event
    pub async fn set_rsvp(
        &self,
        room_id: &RoomId,
        status: RsvpStatus,
        guests: u32,
        note: Option<String>,
    ) -> Result<OwnedEventId, RsvpError> {
        let user_id = self.client.user_id()
            .ok_or(RsvpError::NotLoggedIn)?;

        let room = self.client.get_room(room_id)
            .ok_or(RsvpError::RoomNotFound)?;

        let content = SocialRsvpEventContent {
            status,
            guests,
            note,
        };

        // Send state event with our user ID as state_key
        let response = room
            .send_state_event_for_key(user_id.to_owned(), content)
            .await
            .map_err(RsvpError::MatrixError)?;

        Ok(response.event_id)
    }

    /// Get all RSVPs for an event
    pub async fn get_rsvps(
        &self,
        room_id: &RoomId,
    ) -> Result<Vec<ValidatedRsvp>, RsvpError> {
        let room = self.client.get_room(room_id)
            .ok_or(RsvpError::RoomNotFound)?;

        let mut rsvps = Vec::new();

        // Fetch all org.social.rsvp state events
        // IMPORTANT: Validate each one before including
        todo!("Implement RSVP retrieval with validation")
    }

    /// Get aggregated RSVP counts
    pub async fn get_rsvp_counts(
        &self,
        room_id: &RoomId,
    ) -> Result<RsvpCounts, RsvpError> {
        let rsvps = self.get_rsvps(room_id).await?;

        let mut counts = RsvpCounts::default();
        for rsvp in rsvps {
            match rsvp.status {
                RsvpStatus::Going => {
                    counts.going += 1;
                    counts.total_guests += rsvp.guests;
                }
                RsvpStatus::Interested => counts.interested += 1,
                RsvpStatus::NotGoing => counts.not_going += 1,
            }
        }

        Ok(counts)
    }
}

/// A validated RSVP (passed security checks)
pub struct ValidatedRsvp {
    pub user_id: OwnedUserId,
    pub status: RsvpStatus,
    pub guests: u32,
    pub note: Option<String>,
}

/// Aggregated RSVP counts
#[derive(Default)]
pub struct RsvpCounts {
    pub going: u32,
    pub interested: u32,
    pub not_going: u32,
    pub total_guests: u32,
}
```

---

## Phase 7: Privacy and Security Safeguards

### 7.1 Sharing Guard

**File**: `src/social/privacy/sharing_guard.rs`

```rust
//! Privacy safeguards for cross-posting and sharing.
//!
//! This module prevents accidental privacy leaks when sharing
//! content from private rooms to public rooms.

use matrix_sdk::ruma::OwnedRoomId;

/// Privacy level of content
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrivacyLevel {
    /// Publicly visible (world_readable)
    Public = 0,
    /// Friends only (restricted join)
    Friends = 1,
    /// Close friends (invite only)
    CloseFriends = 2,
    /// Private/DM
    Private = 3,
}

impl PrivacyLevel {
    /// Check if content at this level can be shared to target level
    pub fn can_share_to(&self, target: PrivacyLevel) -> bool {
        // Can only share to equal or more private levels
        target >= *self
    }
}

/// Result of share validation
#[derive(Debug)]
pub enum ShareValidation {
    /// Sharing is allowed
    Allowed,
    /// Sharing is blocked due to privacy leak
    BlockedPrivacyLeak {
        source: PrivacyLevel,
        target: PrivacyLevel,
        message: String,
    },
    /// Sharing requires user confirmation
    RequiresConfirmation {
        warning: String,
    },
    /// Mentioned users not in target room
    MissingMentions {
        missing_users: Vec<OwnedUserId>,
    },
}

/// Service for validating share actions
pub struct SharingGuard;

impl SharingGuard {
    /// Validate a share/cross-post action
    pub fn validate_share(
        source_room: &RoomId,
        source_privacy: PrivacyLevel,
        target_room: &RoomId,
        target_privacy: PrivacyLevel,
        mentioned_users: &[OwnedUserId],
        target_members: &[OwnedUserId],
    ) -> ShareValidation {
        // Check privacy levels
        if !source_privacy.can_share_to(target_privacy) {
            return ShareValidation::BlockedPrivacyLeak {
                source: source_privacy,
                target: target_privacy,
                message: format!(
                    "Cannot share {} content to {} audience",
                    privacy_level_name(source_privacy),
                    privacy_level_name(target_privacy),
                ),
            };
        }

        // Check if mentioned users are in target room
        let missing: Vec<_> = mentioned_users
            .iter()
            .filter(|u| !target_members.contains(u))
            .cloned()
            .collect();

        if !missing.is_empty() {
            return ShareValidation::MissingMentions { missing_users: missing };
        }

        // Warn when sharing from semi-private to public
        if source_privacy == PrivacyLevel::Friends && target_privacy == PrivacyLevel::Public {
            return ShareValidation::RequiresConfirmation {
                warning: "You are about to share friends-only content publicly. \
                         The original author may not have intended this content \
                         to be shared publicly.".to_string(),
            };
        }

        ShareValidation::Allowed
    }

    /// Check if a quote/reply leaks private content
    pub fn validate_quote(
        original_room_privacy: PrivacyLevel,
        reply_room_privacy: PrivacyLevel,
    ) -> ShareValidation {
        if original_room_privacy > reply_room_privacy {
            ShareValidation::RequiresConfirmation {
                warning: "Your reply quotes content from a more private room. \
                         This may expose private information.".to_string(),
            }
        } else {
            ShareValidation::Allowed
        }
    }
}

fn privacy_level_name(level: PrivacyLevel) -> &'static str {
    match level {
        PrivacyLevel::Public => "public",
        PrivacyLevel::Friends => "friends-only",
        PrivacyLevel::CloseFriends => "close friends",
        PrivacyLevel::Private => "private",
    }
}
```

### 7.2 Input Validation

**File**: `src/social/privacy/mod.rs`

```rust
//! Privacy and security module.

pub mod sharing_guard;

mod validation;

pub use sharing_guard::*;
pub use validation::*;

/// Maximum allowed sizes for various content types
pub mod limits {
    /// Maximum bio length in characters
    pub const MAX_BIO_LENGTH: usize = 500;
    /// Maximum post text length
    pub const MAX_POST_LENGTH: usize = 10_000;
    /// Maximum event description length
    pub const MAX_EVENT_DESCRIPTION: usize = 5_000;
    /// Maximum RSVP note length
    pub const MAX_RSVP_NOTE_LENGTH: usize = 200;
    /// Maximum number of mentions per post
    pub const MAX_MENTIONS_PER_POST: usize = 50;
    /// Maximum link preview description length
    pub const MAX_LINK_PREVIEW_DESCRIPTION: usize = 500;
}

/// Sanitize user input for safety
pub fn sanitize_user_input(input: &str, max_length: usize) -> String {
    // Trim whitespace
    let trimmed = input.trim();

    // Truncate to max length (at char boundary)
    let truncated: String = trimmed.chars().take(max_length).collect();

    // Basic HTML entity encoding for display safety
    truncated
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Validate MXC URI format
pub fn validate_mxc_uri(uri: &str) -> Result<(), ValidationError> {
    if !uri.starts_with("mxc://") {
        return Err(ValidationError::InvalidMxcUri("Must start with mxc://".into()));
    }

    // Basic format check: mxc://server/media_id
    let parts: Vec<_> = uri.strip_prefix("mxc://").unwrap().split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(ValidationError::InvalidMxcUri("Invalid format".into()));
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid MXC URI: {0}")]
    InvalidMxcUri(String),
    #[error("Content too long: {field} exceeds {max} characters")]
    ContentTooLong { field: String, max: usize },
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}
```

---

## Phase 8: CI/CD for Multi-Platform Builds

### 8.1 Build Workflow

**File**: `.github/workflows/build-social.yml`

```yaml
name: Build Social Features

on:
  push:
    branches: [main, develop]
    paths:
      - 'src/social/**'
      - 'src/social_dummy/**'
      - 'robrix-social-events/**'
      - 'Cargo.toml'
      - '.github/workflows/build-social.yml'
  pull_request:
    branches: [main]
    paths:
      - 'src/social/**'
      - 'src/social_dummy/**'
      - 'robrix-social-events/**'

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  # Security audit
  security-audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install cargo-audit
        run: cargo install cargo-audit --locked

      - name: Security audit
        run: cargo audit

      - name: Dependency review
        uses: actions/dependency-review-action@v4
        if: github.event_name == 'pull_request'

  # Lint and format check
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Clippy (without social)
        run: cargo clippy --all-targets -- -D warnings

      - name: Clippy (with social)
        run: cargo clippy --all-targets --features social -- -D warnings

  # Test suite
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install dependencies (Linux)
        run: |
          sudo apt-get update
          sudo apt-get install -y libssl-dev pkg-config

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Run tests (without social)
        run: cargo test --lib

      - name: Run tests (with social)
        run: cargo test --lib --features social

      - name: Run social integration tests
        run: cargo test --features social --test social_integration

  # Build matrix for desktop platforms
  build-desktop:
    needs: [security-audit, lint, test]
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: robrix-social-linux-x64
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: robrix-social-macos-x64
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: robrix-social-macos-arm64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: robrix-social-windows-x64

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install dependencies (Linux)
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libssl-dev pkg-config libxcb-shape0-dev libxcb-xfixes0-dev

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-${{ matrix.target }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Build release (with social features)
        run: cargo build --release --features social --target ${{ matrix.target }}

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: |
            target/${{ matrix.target }}/release/robrix
            target/${{ matrix.target }}/release/robrix.exe
          if-no-files-found: error

  # Mobile builds
  build-mobile:
    needs: [security-audit, lint, test]
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: android
            os: ubuntu-latest
          - platform: ios
            os: macos-latest

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install cargo-makepad
        run: cargo install cargo-makepad --locked

      - name: Setup Android SDK
        if: matrix.platform == 'android'
        uses: android-actions/setup-android@v3

      - name: Setup Android NDK
        if: matrix.platform == 'android'
        run: |
          sdkmanager --install "ndk;25.2.9519653"
          echo "ANDROID_NDK_HOME=$ANDROID_HOME/ndk/25.2.9519653" >> $GITHUB_ENV

      - name: Setup Xcode
        if: matrix.platform == 'ios'
        uses: maxim-lobanov/setup-xcode@v1
        with:
          xcode-version: latest-stable

      - name: Build ${{ matrix.platform }}
        run: cargo makepad ${{ matrix.platform }} build --release --features social

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: robrix-social-${{ matrix.platform }}
          path: |
            target/makepad-${{ matrix.platform }}/release/*
          if-no-files-found: warn
```

### 8.2 Release Workflow

**File**: `.github/workflows/release-social.yml`

```yaml
name: Release with Social Features

on:
  release:
    types: [published]
  workflow_dispatch:
    inputs:
      version:
        description: 'Version to release'
        required: true
        type: string

env:
  CARGO_TERM_COLOR: always

jobs:
  build-and-release:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            suffix: linux-x64.tar.gz
            asset_content_type: application/gzip
          - os: macos-latest
            target: x86_64-apple-darwin
            suffix: macos-x64.tar.gz
            asset_content_type: application/gzip
          - os: macos-latest
            target: aarch64-apple-darwin
            suffix: macos-arm64.tar.gz
            asset_content_type: application/gzip
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            suffix: windows-x64.zip
            asset_content_type: application/zip

    runs-on: ${{ matrix.os }}
    permissions:
      contents: write

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install dependencies (Linux)
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libssl-dev pkg-config libxcb-shape0-dev libxcb-xfixes0-dev

      - name: Build release
        run: cargo build --profile distribution --features social --target ${{ matrix.target }}

      - name: Package (Unix)
        if: matrix.os != 'windows-latest'
        run: |
          cd target/${{ matrix.target }}/distribution
          tar czvf ../../../robrix-social-${{ matrix.suffix }} robrix

      - name: Package (Windows)
        if: matrix.os == 'windows-latest'
        run: |
          cd target/${{ matrix.target }}/distribution
          Compress-Archive -Path robrix.exe -DestinationPath ../../../robrix-social-${{ matrix.suffix }}

      - name: Upload release asset
        uses: softprops/action-gh-release@v2
        with:
          files: robrix-social-${{ matrix.suffix }}
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  build-mobile-release:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: android
            os: ubuntu-latest
          - platform: ios
            os: macos-latest

    runs-on: ${{ matrix.os }}
    permissions:
      contents: write

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install cargo-makepad
        run: cargo install cargo-makepad --locked

      - name: Setup Android
        if: matrix.platform == 'android'
        uses: android-actions/setup-android@v3

      - name: Setup Android NDK
        if: matrix.platform == 'android'
        run: |
          sdkmanager --install "ndk;25.2.9519653"
          echo "ANDROID_NDK_HOME=$ANDROID_HOME/ndk/25.2.9519653" >> $GITHUB_ENV

      - name: Build release
        run: cargo makepad ${{ matrix.platform }} build --profile distribution --features social

      - name: Upload release asset
        uses: softprops/action-gh-release@v2
        with:
          files: target/makepad-${{ matrix.platform }}/distribution/*
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

---

## Phase 9: Testing Strategy

### 9.1 Unit Tests

**File**: `tests/social_integration/mod.rs`

```rust
//! Integration tests for social features.

#[cfg(feature = "social")]
mod profile_tests;

#[cfg(feature = "social")]
mod feed_tests;

#[cfg(feature = "social")]
mod friend_tests;

#[cfg(feature = "social")]
mod event_tests;

#[cfg(feature = "social")]
mod privacy_tests;
```

### 9.2 Privacy Validation Tests

**File**: `tests/social_integration/privacy_tests.rs`

```rust
use robrix::social::privacy::{PrivacyLevel, SharingGuard, ShareValidation};

#[test]
fn test_privacy_level_ordering() {
    assert!(PrivacyLevel::Public < PrivacyLevel::Friends);
    assert!(PrivacyLevel::Friends < PrivacyLevel::CloseFriends);
    assert!(PrivacyLevel::CloseFriends < PrivacyLevel::Private);
}

#[test]
fn test_cannot_share_private_to_public() {
    assert!(!PrivacyLevel::Friends.can_share_to(PrivacyLevel::Public));
    assert!(!PrivacyLevel::CloseFriends.can_share_to(PrivacyLevel::Public));
    assert!(!PrivacyLevel::Private.can_share_to(PrivacyLevel::Public));
}

#[test]
fn test_can_share_public_anywhere() {
    assert!(PrivacyLevel::Public.can_share_to(PrivacyLevel::Public));
    assert!(PrivacyLevel::Public.can_share_to(PrivacyLevel::Friends));
    assert!(PrivacyLevel::Public.can_share_to(PrivacyLevel::CloseFriends));
    assert!(PrivacyLevel::Public.can_share_to(PrivacyLevel::Private));
}

#[test]
fn test_sharing_guard_blocks_leak() {
    let result = SharingGuard::validate_share(
        &"!source:example.org".try_into().unwrap(),
        PrivacyLevel::Friends,
        &"!target:example.org".try_into().unwrap(),
        PrivacyLevel::Public,
        &[],
        &[],
    );

    assert!(matches!(result, ShareValidation::BlockedPrivacyLeak { .. }));
}
```

### 9.3 RSVP Validation Tests

**File**: `tests/social_integration/event_tests.rs`

```rust
use robrix::social::events::rsvp::{validate_rsvp_event, RsvpValidation};

#[test]
fn test_rsvp_sender_validation() {
    // Test that mismatched sender/state_key is detected
    // This is critical for preventing RSVP spoofing

    // Create mock event where state_key != sender
    // ... test implementation
}

#[test]
fn test_valid_rsvp_passes() {
    // Test that matching sender/state_key passes validation
    // ... test implementation
}
```

---

## Implementation Timeline

| Phase | Components | Estimated Effort | Dependencies |
|-------|------------|------------------|--------------|
| **1** | Feature gate, event types crate, module structure | 1 week | None |
| **2** | Profile rooms | 1 week | Phase 1 |
| **3** | Posts and feed rooms | 2 weeks | Phase 1, 2 |
| **4** | Newsfeed aggregation | 1 week | Phase 3 |
| **5** | Friend network | 2 weeks | Phase 1, 3 |
| **6** | Events/gatherings | 2 weeks | Phase 1, 5 |
| **7** | Privacy safeguards | 1 week | Phase 3, 5 |
| **8** | CI/CD pipelines | 1 week | Phase 1 (can parallel) |
| **9** | Testing and polish | 2 weeks | All phases |

**Total estimated effort**: 10-12 weeks

---

## Security Considerations

### Critical Security Requirements

1. **RSVP Validation**: Always validate `state_key === sender` for `org.social.rsvp` events
2. **Privacy Guards**: Block cross-posting from restricted to public rooms
3. **Input Sanitization**: All user input sanitized before display
4. **MXC URI Validation**: Validate all media URIs before use
5. **Power Level Enforcement**: Proper power levels prevent unauthorized modifications
6. **Mention Validation**: Verify mentioned users exist in target rooms

### Security Audit Checklist

- [ ] All `serde` structs use `#[serde(deny_unknown_fields)]`
- [ ] RSVP events validated for sender/state_key match
- [ ] Privacy level checks on all share operations
- [ ] Input length limits enforced
- [ ] HTML content sanitized before rendering
- [ ] MXC URIs validated before media requests
- [ ] No hardcoded credentials or secrets
- [ ] Dependency audit passes (`cargo audit`)

---

## Compatibility Notes

### Room Version Requirements

| Feature | Minimum Room Version |
|---------|---------------------|
| Restricted join rules | Room Version 8 |
| All features | Room Version 11 (recommended) |

### Matrix Spec Compliance

- All custom event types use `org.social.*` namespace
- v1.17 compliant `m.mentions` for user mentions
- Standard `m.room.message` types for posts
- Standard `m.reaction` for engagement

### Upstream Compatibility

This implementation:
- Creates **new files only** - no modifications to existing Robrix code
- Uses **feature flags** - social features completely disabled by default
- Follows **existing patterns** - mirrors TSP feature structure
- **No patches** to matrix-sdk or other dependencies

---

## Appendix: Custom Event Type Schemas

### org.social.profile

```json
{
  "type": "org.social.profile",
  "state_key": "",
  "content": {
    "bio": "string (optional, max 500 chars)",
    "location": "string (optional)",
    "website": "URL (optional)",
    "cover_image": "mxc:// URI (optional)",
    "custom": "object (optional, for extensibility)"
  }
}
```

### org.social.event

```json
{
  "type": "org.social.event",
  "state_key": "",
  "content": {
    "title": "string (required)",
    "description": "string (optional, max 5000 chars)",
    "start_time": "integer (required, Unix ms)",
    "end_time": "integer (optional, Unix ms)",
    "location": {
      "name": "string (required)",
      "address": "string (optional)",
      "geo": "geo: URI (optional)"
    },
    "cover_image": "mxc:// URI (optional)",
    "visibility": "public | private (required)",
    "rsvp_deadline": "integer (optional, Unix ms)"
  }
}
```

### org.social.rsvp

```json
{
  "type": "org.social.rsvp",
  "state_key": "@user:server (MUST match sender)",
  "content": {
    "status": "going | interested | not_going (required)",
    "guests": "integer (default 1)",
    "note": "string (optional, max 200 chars)"
  }
}
```

### org.social.link_preview

```json
{
  "org.social.link_preview": {
    "url": "URL (required)",
    "title": "string (optional)",
    "description": "string (optional, max 500 chars)",
    "image": "mxc:// URI (optional)",
    "site_name": "string (optional)"
  }
}
```
