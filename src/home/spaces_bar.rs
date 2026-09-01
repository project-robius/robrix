//! The SpacesBar shows a scrollable strip of avatars: first the spaces that the
//! user has been invited to, then the spaces they have currently joined.
//!
//! Like the NavigationTabBar, this widget uses AdaptiveView to show:
//! 1. a narrow vertical strip, when in Desktop (widescreen) mode,
//! 2. a wide, short horizontal strip, when in Mobile (narrowscreen) mode.

use std::{borrow::Cow, collections::HashMap};

use indexmap::IndexMap;

use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::{RoomDisplayName, RoomState};
use ruma::{OwnedRoomAliasId, OwnedRoomId, room::JoinRuleSummary};

use crate::{
    app::AppStateAction, home::{navigation_tab_bar::{NavigationBarAction, SelectedTab}, rooms_list::get_invited_rooms}, logout::logout_confirm_modal::LogoutAction, room::{FetchedRoomAvatar, room_display_filter::{RoomDisplayFilter, RoomDisplayFilterBuilder, RoomFilterCriteria, SortFn}}, settings::app_preferences::{AppPreferencesAction, ViewModeOverride}, shared::{avatar::AvatarWidgetExt, navigation_bar_button::NavigationBarButton, room_filter_input_bar::MainFilterAction, unread_badge::UnreadBadgeWidgetExt as _}, utils::{self, RoomNameId}
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // The duration of the animation when showing/hiding the SpacesBar (in Mobile view mode only).
    mod.widgets.SPACES_BAR_ANIMATION_DURATION_SECS = 0.25

    // An entry in the list of all spaces, which shows the Space's avatar and name.
    // Inherits hover/selected styling and click handling from
    // `mod.widgets.NavigationBarButton`. The space-name tooltip is set
    // dynamically per-entry from Rust via `set_metadata`.
    mod.widgets.SpacesBarEntry = set_type_default() do #(SpacesBarEntry::register_widget(vm)) {
        ..mod.widgets.NavigationBarButton

        // `height + (2 * margin)`` must equal NAVIGATION_TAB_BAR_SIZE to avoid clipping
        width: (NAVIGATION_TAB_BAR_SIZE - 4),
        height: (NAVIGATION_TAB_BAR_SIZE - 4),
        // Flow.Overlay (rather than Down) so that the invisible `space_name` Label
        // doesn't sit in the avatar's flow column and shift its centering.
        flow: Overlay
        padding: 4,
        margin: 2,
        align: Align{x: 0.5, y: 0.5}
        // Don't clip (cut-off) the invite badge's glow
        clip_x: false, clip_y: false

        avatar := Avatar {
            width: mod.widgets.NAVIGATION_TAB_BAR_AVATAR_SIZE
            height: mod.widgets.NAVIGATION_TAB_BAR_AVATAR_SIZE
            // If no avatar picture, use white text on a dark background.
            text_view +: {
                draw_bg.color: (COLOR_FG_DISABLED),
                text +: {
                    draw_text +: {
                        text_style: theme.font_regular { font_size: mod.widgets.NAVIGATION_TAB_BAR_AVATAR_FONT_SIZE },
                        color: (COLOR_PRIMARY),
                    }
                }
            }
        }

        space_name := Label {
            width: Fill,
            height: 0,
            flow: Flow.Right{wrap: false}, // do not wrap
            padding: 0,
            margin: 0,
            align: Align{x: 0.5}
            max_lines: 1
            text_overflow: Ellipsis
            draw_text +: {
                color: (COLOR_NAVIGATION_TAB_FG)
                text_style: REGULAR_TEXT {font_size: 9}
            }
        }

        // Places an unread badge at the top-right of the spaces bar entry/avatar.
        View {
            width: Fill, height: Fill
            align: Align{x: 1.0, y: 0.0}
            clip_x: false, clip_y: false
            invite_badge := UnreadBadge {
                margin: Inset{ top: -2, right: -5 }
            }
        }
    }

    mod.widgets.SpacesStatusLabel = View {
        // We allow the status label to take up 2 entries' worth of horizontal space
        // (only relevant in mobile view mode). 
        width: Fill { max: (NAVIGATION_TAB_BAR_SIZE * 2) },
        // Non-fixed height: let the label grow down (important on Desktop mode).
        height: Fit
        margin: 2,
        align: Align{ x: 0.5, y: 0.5 }
        padding: 4,

        label := Label {
            padding: 0
            margin: 0
            width: Fill,
            height: Fit,
            flow: Flow.Right{wrap: true},
            align: Align{ x: 0.5 }
            draw_text +: {
                color: (MESSAGE_TEXT_COLOR),
                text_style: REGULAR_TEXT {font_size: 8, line_spacing: 1.1}
            }
        }
    }

    mod.widgets.SpacesList = PortalList {
        height: Fill,
        width: Fill,
        spacing: 0
        align: Align{x: 0.5, y: 0.5}

        auto_tail: false,
        bounce_at_start: false,
        bounce_at_end: false,
        // Nothing here listens for scroll position changes.
        emit_scroll_actions: false,
        scroll_bar: ScrollBar {  // hide the scroll bar
            bar_size: 0.0,
            min_handle_size: 0.0
        }

        spaces_bar_entry := mod.widgets.SpacesBarEntry {}
        StatusLabel := mod.widgets.SpacesStatusLabel {}
        BottomFiller := View {
            width: (NAVIGATION_TAB_BAR_SIZE)
            height: (NAVIGATION_TAB_BAR_SIZE)
        }
    }

    mod.widgets.SpacesBar = #(SpacesBar::register_widget(vm)) {
        Desktop := View {
            align: Align{x: 0.5, y: 0.5}
            padding: 0,
            width: (NAVIGATION_TAB_BAR_SIZE), 
            height: Fill

            CachedWidget {
                spaces_list := mod.widgets.SpacesList { }
            }
        }

        Mobile := View {
            align: Align{x: 0.5, y: 0.5}
            padding: 0,
            width: Fill,
            height: (NAVIGATION_TAB_BAR_SIZE)

            CachedWidget {
                spaces_list := mod.widgets.SpacesList { }
            }
        }
    }
}


/// Actions emitted by and handled by the SpacesBar widget (and its children).
#[derive(Clone, Debug, Default)]
pub enum SpacesBarAction {
    /// The user primary-clicked/tapped a space entry in the SpacesBar.
    ButtonClicked { space_name_id: RoomNameId },
    /// The user clicked a space they've been invited to but haven't joined yet.
    InvitedSpaceClicked { space_name_id: RoomNameId },
    /// The user secondary-clicked/long-pressed a space entry in the SpacesBar.
    ButtonSecondaryClicked { space_name_id: RoomNameId },
    #[default]
    None,
}


/// An entry in the SpacesBar, displaying a single joined space's avatar and name.
///
/// `SpacesBarEntry` derefs into [`NavigationBarButton`], inheriting its hover
/// and selected background animations and click handling. The entry's tooltip
/// (the space's display name) is delivered via `NavigationBarButton`'s built-in
/// `tooltip_text`, which is set per-entry in [`SpacesBarEntry::set_metadata`].
#[derive(Script, ScriptHook, Widget)]
pub struct SpacesBarEntry {
    #[deref] inner: NavigationBarButton,

    #[rust] space_name_id: Option<RoomNameId>,
    #[rust] last_avatar: Option<FetchedRoomAvatar>,
    #[rust] is_invited: bool,
}

impl Widget for SpacesBarEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Forward to the inner NavigationBarButton, which handles hover/selected
        // animations, the built-in tooltip, and emits Clicked / SecondaryClicked
        // actions on tap and right-click / long-press.
        self.inner.handle_event(cx, event, scope);

        // Translate the inner button's generic click actions into
        // SpacesBarAction variants that include this entry's space identity.
        if let Event::Actions(actions) = event {
            if self.inner.clicked(actions) {
                if let Some(space_name_id) = self.space_name_id.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        match self.is_invited {
                            true => SpacesBarAction::InvitedSpaceClicked { space_name_id },
                            false => SpacesBarAction::ButtonClicked { space_name_id },
                        },
                    );
                }
            }
            if self.inner.secondary_clicked(actions) {
                if let Some(space_name_id) = self.space_name_id.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        SpacesBarAction::ButtonSecondaryClicked { space_name_id },
                    );
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.inner.draw_walk(cx, scope, walk)
    }
}

impl SpacesBarEntry {
    fn set_metadata(
        &mut self,
        cx: &mut Cx,
        space_name_id: RoomNameId,
        avatar: &FetchedRoomAvatar,
        is_selected: bool,
        is_invited: bool,
    ) {
        let space_name = space_name_id.display();
        // The name label isn't visible by default, but we populate it anyway.
        self.inner.view.label(cx, ids!(space_name)).set_text(cx, &space_name);

        // Only populate the avatar if it has actually changed.
        if self.last_avatar.as_ref() != Some(avatar) {
            let avatar_ref = self.inner.view.avatar(cx, ids!(avatar));
            match avatar {
                FetchedRoomAvatar::Text(text) => avatar_ref.show_text(cx, None, None, text),
                FetchedRoomAvatar::Image(image) => {
                    let res = avatar_ref.show_image(
                        cx,
                        None,
                        |cx, img_ref| utils::load_avatar_image(&img_ref, cx, image),
                    );
                    if res.is_err() {
                        avatar_ref.show_text(cx, None, None, &space_name);
                    }
                }
            }
            self.last_avatar = Some(avatar.clone());
        }

        self.inner.view
            .unread_badge(cx, ids!(invite_badge))
            .update_counts(false, is_invited as u64, 0);

        self.inner.set_tooltip_text(space_name);
        self.space_name_id = Some(space_name_id);
        self.is_invited = is_invited;
        self.inner.set_selected(cx, is_selected);
    }
}

impl SpacesBarEntryRef {
    pub fn set_metadata(
        &self,
        cx: &mut Cx,
        space_name_id: RoomNameId,
        avatar: &FetchedRoomAvatar,
        is_selected: bool,
        is_invited: bool,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_metadata(cx, space_name_id, avatar, is_selected, is_invited);
    }
}

pub struct JoinedSpaceInfo {
    /// The display name and ID of the space.
    pub space_name_id: RoomNameId,
    /// The canonical alias of the space, if any.
    pub canonical_alias: Option<OwnedRoomAliasId>,
    /// The topic of the space, if any.
    pub topic: Option<String>,
    /// The fully-fetched avatar for this space.
    pub space_avatar: FetchedRoomAvatar,
    /// The number of members joined to the space.
    pub num_joined_members: u64,
    /// The join rule of the space.
    pub join_rule: Option<JoinRuleSummary>,
    /// Whether the space may be viewed by users without joining.
    pub world_readable: Option<bool>,
    /// Whether guest users may join the space and participate in it.
    pub guest_can_join: bool,
    /// The number of children rooms this space has.
    pub children_count: u64,
}

struct InvitedSpaceInfo {
    space_name_id: RoomNameId,
    space_avatar: FetchedRoomAvatar,
}



/// The possible updates that should be displayed by the single list of all spaces.
///
/// These updates are enqueued by the `enqueue_spaces_list_update` function
/// (which is called from background async tasks that receive updates from the matrix server),
/// and then dequeued by the `SpacesList` widget's `handle_event` function.
pub enum SpacesListUpdate {
    /// Add a new space to the list of all spaces that the user has joined.
    AddJoinedSpace(JoinedSpaceInfo),
    /// Update the canonical alias for the given space.
    UpdateCanonicalAlias {
        space_id: OwnedRoomId,
        new_canonical_alias: Option<OwnedRoomAliasId>,
    },
    /// Update the displayable name for the given space.
    UpdateSpaceName {
        space_id: OwnedRoomId,
        new_space_name: String,
    },
    /// Update the topic for the given space.
    UpdateSpaceTopic {
        space_id: OwnedRoomId,
        topic: Option<String>,
    },
    /// Update the avatar for the given space.
    UpdateSpaceAvatar {
        space_id: OwnedRoomId,
        avatar: FetchedRoomAvatar,
    },
    /// Update the number of joined members for the given space.
    UpdateNumJoinedMembers {
        space_id: OwnedRoomId,
        num_joined_members: u64,
    },
    /// Update the join rule for the given space.
    UpdateJoinRule {
        space_id: OwnedRoomId,
        join_rule: Option<JoinRuleSummary>,
    },
    /// Update whether the given space is world-readable.
    UpdateWorldReadable {
        space_id: OwnedRoomId,
        world_readable: Option<bool>,
    },
    /// Update whether guest users can join the given space.
    UpdateGuestCanJoin {
        space_id: OwnedRoomId,
        guest_can_join: bool,
    },
    /// Update how many child rooms this space has.
    UpdateChildrenCount {
        space_id: OwnedRoomId,
        children_count: u64,
    },
    /// Remove the given space from the spaces list.
    RemoveSpace {
        space_id: OwnedRoomId,
        /// The new state of the space (which caused its removal).
        new_state: Option<RoomState>,
    },
    /// Clear all spaces in the list of all spaces.
    ClearSpaces,
    /// Scroll to the given space.
    ScrollToSpace(OwnedRoomId),
    /// The set of invited rooms changed, so we need to re-generate the list of invited spaces.
    InvitedSpacesChanged,
}


static PENDING_SPACE_UPDATES: SegQueue<SpacesListUpdate> = SegQueue::new();

/// Enqueue a new room update for the list of all spaces
/// and signals the UI that a new update is available to be handled.
pub fn enqueue_spaces_list_update(update: SpacesListUpdate) {
    PENDING_SPACE_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}


/// The tab bar with buttons that navigate through top-level app pages.
///
/// * In the "desktop" (wide) layout, this is a vertical bar on the left.
/// * In the "mobile" (narrow) layout, this is a horizontal bar on the bottom.
#[derive(Script, ScriptHook, Widget)]
pub struct SpacesBar {
    #[deref] view: AdaptiveView,

    /// The set of all joined spaces, keyed by space ID, with homeserver-based ordering.
    ///
    /// In the future we could allow the user to re-order this arbitrarily, or apply a sort fn.
    #[rust] all_joined_spaces: IndexMap<OwnedRoomId, JoinedSpaceInfo>,

    /// The currently-active filter function for the list of spaces.
    ///
    /// Note: for performance reasons, this does not get automatically applied
    /// when its value changes. Instead, you must manually invoke it on the set of
    /// `all_joined_spaces` in order to re-generate the set of `displayed_joined_spaces`.
    #[rust] display_filter: RoomDisplayFilter,

    /// The joined spaces currently displayed, in `all_joined_spaces` order.
    ///
    /// See `regenerate_displayed_joined_spaces()` for how this is populated.
    #[rust] displayed_joined_spaces: Vec<OwnedRoomId>,

    /// The sort applied to `displayed_joined_spaces` by the current search filter, if any.
    #[rust] sort_fn: Option<Box<SortFn>>,

    /// Spaces the user has been invited to, shown in front of `displayed_joined_spaces`.
    #[rust] displayed_invited_spaces: Vec<InvitedSpaceInfo>,

    /// Whether the list of `displayed_joined_spaces` is currently filtered:
    /// `true` if filtered, `false` if showing everything.
    #[rust] is_filtered: bool,

    /// The ID of the currently-selected space in this SpacesBar.
    /// Only one space can be selected at once.
    #[rust] selected_space: Option<OwnedRoomId>,

    /// The most recently applied view-mode override.
    #[rust] applied_view_mode: ViewModeOverride,
}

impl SpacesBar {
    fn apply_view_mode(&mut self, mode: ViewModeOverride) {
        self.view.set_variant_selector(mode.variant_selector());
        self.applied_view_mode = mode;
    }
}

impl Widget for SpacesBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Process all pending updates to the spaces list.
        if matches!(event, Event::Signal) {
            self.handle_spaces_list_updates(cx, event, scope);
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                // Only handle filter changes from the home screen's filter bar,
                // not from any other RoomFilterInputBar instance (e.g., SpaceLobbyScreen's).
                if let Some(MainFilterAction::Changed(keywords)) = action.downcast_ref() {
                    self.update_displayed_joined_spaces(cx, keywords);
                    continue;
                }

                // Update which space is currently selected.
                if let SpacesBarAction::ButtonClicked { space_name_id } = action.as_widget_action().cast() {
                    self.selected_space = Some(space_name_id.room_id().clone());
                    self.redraw(cx);
                    cx.action(NavigationBarAction::GoToSpace { space_name_id });
                    continue;
                }

                // Note: `SpacesBarAction::InvitedSpaceClicked` is not handled here.
                // The MainDesktopUI and HomeScreen handle that action.

                // If another widget programmatically selected a new tab,
                // we must unselect/deselect the currently-selected space.
                if let Some(NavigationBarAction::TabSelected(tab)) = action.downcast_ref() {
                    match tab {
                        SelectedTab::Space { space_name_id } => {
                            self.selected_space = Some(space_name_id.room_id().clone());
                            self.redraw(cx);
                        }
                        _ => {
                            self.selected_space = None;
                            self.redraw(cx);
                        }
                    }
                    continue;
                }

                // Handle a change to the view mode preference.
                if let Some(AppPreferencesAction::ViewModeChanged(new_mode)) = action.downcast_ref() {
                    if *new_mode != self.applied_view_mode {
                        self.apply_view_mode(*new_mode);
                        self.view.redraw(cx);
                    }
                    continue;
                }

                // Clear widget state upon logout.
                if let Some(LogoutAction::ClearAppState { .. }) = action.downcast_ref() {
                    self.all_joined_spaces.clear();
                    self.displayed_joined_spaces.clear();
                    self.displayed_invited_spaces.clear();
                    self.display_filter = Default::default();
                    self.selected_space = None;
                    self.is_filtered = false;
                    self.redraw(cx);
                    continue;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(widget_to_draw) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = widget_to_draw.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            // AdaptiveView + CachedWidget does not properly handle DSL-level style overrides,
            // so we must manually apply the correct portallist Flow when drawing it.
            let is_desktop = self.view.active_variant() == Some(live_id!(Desktop));
            list.set_flow(cx, if is_desktop { Flow::Down } else { Flow::right() });

            let num_invited = self.displayed_invited_spaces.len();
            let len = num_invited + self.displayed_joined_spaces.len();
            if len == 0 {
                list.set_item_range(cx, 0, 1);
                while let Some(portal_list_index) = list.next_visible_item(cx) {
                    let item = if portal_list_index == 0 {
                        let item = list.item(cx, portal_list_index, id!(StatusLabel));
                        item.label(cx, ids!(label)).set_text(
                            cx,
                            if self.is_filtered {
                                "No spaces\nmatch."
                            } else {
                                "Found no\njoined spaces."
                            }
                        );
                        item
                    } else {
                        list.item(cx, portal_list_index, id!(BottomFiller))
                    };
                    item.draw_all(cx, scope);
                }
            }
            else {
                list.set_item_range(cx, 0, len + 1);
                while let Some(portal_list_index) = list.next_visible_item(cx) {
                    let item = if let Some(invite) = self.displayed_invited_spaces.get(portal_list_index) {
                        let item = list.item(cx, portal_list_index, id!(spaces_bar_entry));
                        item.as_spaces_bar_entry().set_metadata(
                            cx,
                            invite.space_name_id.clone(),
                            &invite.space_avatar,
                            false,
                            true,
                        );
                        item
                    }
                    else if let Some(space) = portal_list_index.checked_sub(num_invited)
                        .and_then(|index| self.displayed_joined_spaces.get(index))
                        .and_then(|space_id| self.all_joined_spaces.get(space_id))
                    {
                        let item = list.item(cx, portal_list_index, id!(spaces_bar_entry));
                        item.as_spaces_bar_entry().set_metadata(
                            cx,
                            space.space_name_id.clone(),
                            &space.space_avatar,
                            self.selected_space.as_ref().is_some_and(|id| id == space.space_name_id.room_id()),
                            false,
                        );
                        item
                    }
                    else if portal_list_index == len {
                        let item = list.item(cx, portal_list_index, id!(StatusLabel));
                        let num_joined = self.displayed_joined_spaces.len();
                        let text: Cow<'static, str> = if self.is_filtered {
                            let total = self.all_joined_spaces.len();
                            format!("{num_joined} of {total} spaces").into()
                        } else {
                            match num_joined {
                                0   => "Found no joined spaces.".into(),
                                1   => "Found 1 joined space.".into(),
                                2.. => format!("Found {num_joined} joined spaces.").into(),
                            }
                        };
                        item.label(cx, ids!(label)).set_text(cx, &text);
                        item
                    }
                    else {
                        list.item(cx, portal_list_index, id!(BottomFiller))
                    };
                    item.draw_all(cx, scope);
                }
            }
        }

        DrawStep::done()
    }
}

impl SpacesBar {
     /// Handle all pending updates to the spaces list.
    fn handle_spaces_list_updates(&mut self, cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // The matrix SDK frequently issues `Clear` + `Append` updates in one batch,
        // so we optimize for that. One thing we do is save avatars across a Clear + Append
        // to avoid briefly flickering between the placeholder text avatar and the space's real avatar image.
        let mut cleared_space_avatars: HashMap<OwnedRoomId, FetchedRoomAvatar> = HashMap::new();

        let mut num_updates: usize = 0;
        let mut should_regenerate_displayed_spaces = false;
        let mut scroll_to_space: Option<OwnedRoomId> = None;
        while let Some(update) = PENDING_SPACE_UPDATES.pop() {
            num_updates += 1;
            match update {
                SpacesListUpdate::AddJoinedSpace(mut joined_space) => {
                    let space_id = joined_space.space_name_id.room_id().clone();
                    if let FetchedRoomAvatar::Text(_) = joined_space.space_avatar
                        && let Some(FetchedRoomAvatar::Image(image)) = self.all_joined_spaces
                            .get(&space_id)
                            .map(|s| &s.space_avatar)
                            // If a space got re-added (e.g., for ordering changes),
                            // try to recover its avatar from the known spaces (from before this update batch).
                            .or_else(|| cleared_space_avatars.get(&space_id))
                    {
                        joined_space.space_avatar = FetchedRoomAvatar::Image(image.clone());
                    }
                    self.all_joined_spaces.insert(space_id, joined_space);
                    should_regenerate_displayed_spaces = true;
                }

                SpacesListUpdate::UpdateCanonicalAlias { space_id, new_canonical_alias } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        space.canonical_alias = new_canonical_alias;
                        should_regenerate_displayed_spaces = true;
                    } else {
                        error!("Error: couldn't find space {space_id} to update space canonical alias");
                    }
                }

                SpacesListUpdate::UpdateSpaceName { space_id, new_space_name } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        space.space_name_id = RoomNameId::new(
                            RoomDisplayName::Named(new_space_name),
                            space_id.clone(),
                        );
                        cx.action(AppStateAction::RoomNameUpdated(space.space_name_id.clone()));
                        should_regenerate_displayed_spaces = true;
                    } else {
                        error!("Error: couldn't find space {space_id} to update space name");
                    }
                }

                SpacesListUpdate::UpdateSpaceTopic { space_id, topic } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        // We don't currently support filtering by topic.
                        space.topic = topic;
                    } else {
                        error!("Error: couldn't find space {space_id} to update space topic");
                    }
                }

                SpacesListUpdate::UpdateSpaceAvatar { space_id, avatar } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        space.space_avatar = avatar;
                    } else {
                        error!("Error: couldn't find space {space_id} to update space name");
                    }
                }

                SpacesListUpdate::UpdateNumJoinedMembers { space_id, num_joined_members } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        space.num_joined_members = num_joined_members;
                    } else {
                        error!("Error: couldn't find space {space_id} to update space num_joined_members");
                    }
                }

                SpacesListUpdate::UpdateJoinRule { space_id, join_rule } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        space.join_rule = join_rule;
                    } else {
                        error!("Error: couldn't find space {space_id} to update space join_rule");
                    }
                }

                SpacesListUpdate::UpdateWorldReadable { space_id, world_readable } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        space.world_readable = world_readable;
                    } else {
                        error!("Error: couldn't find space {space_id} to update space world_readable");
                    }
                }

                SpacesListUpdate::UpdateGuestCanJoin { space_id, guest_can_join } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        space.guest_can_join = guest_can_join;
                    } else {
                        error!("Error: couldn't find space {space_id} to update space guest_can_join");
                    }
                }

                SpacesListUpdate::UpdateChildrenCount { space_id, children_count } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        space.children_count = children_count;
                    } else {
                        error!("Error: couldn't find space {space_id} to update space children_count");
                    }
                }

                SpacesListUpdate::RemoveSpace { space_id, .. } => {
                    // Use `shift_remove` instead of `remove` to preserve the spaces' relative ordering
                    self.all_joined_spaces.shift_remove(&space_id);
                    should_regenerate_displayed_spaces = true;
                }

                SpacesListUpdate::ClearSpaces => {
                    cleared_space_avatars.extend(
                        self.all_joined_spaces.drain(..)
                            .map(|(space_id, space)| (space_id, space.space_avatar))
                    );
                    should_regenerate_displayed_spaces = true;
                }

                SpacesListUpdate::InvitedSpacesChanged => {
                    self.regenerate_invited_spaces(cx);
                }

                SpacesListUpdate::ScrollToSpace(space_id) => {
                    // Wait until we've handled all updates such that we scroll to the
                    // final position of the target space.
                    scroll_to_space = Some(space_id);
                }
            }
        }
        if should_regenerate_displayed_spaces {
            self.regenerate_displayed_joined_spaces();
        }
        if let Some(index) = scroll_to_space.and_then(|space_id|
            self.displayed_joined_spaces.iter().position(|s| s == &space_id)
        ) {
            // account for invited spaces at the beginning
            let index = index + self.displayed_invited_spaces.len();
            let speed = 40.0;
            self.view.portal_list(cx, ids!(spaces_list))
                .smooth_scroll_to(cx, index, speed, Some(10), 10.0);
        }
        if num_updates > 0 {
            self.redraw(cx);
        }
    }

    /// Re-generates the list of `displayed_joined_spaces` from `all_joined_spaces`,
    /// applying the `display_filter` and the `sort_fn`, if one is set.
    fn regenerate_displayed_joined_spaces(&mut self) {
        self.displayed_joined_spaces = self.all_joined_spaces.iter()
            .filter(|(_, space)| (self.display_filter)(*space))
            .map(|(space_id, _)| space_id.clone())
            .collect();
        if let Some(sort_fn) = self.sort_fn.as_deref() {
            let spaces = &self.all_joined_spaces;
            self.displayed_joined_spaces.sort_by(|a, b| sort_fn(&spaces[a], &spaces[b]));
        }
    }

    /// Re-generates the invited spaces shown at the beginning of the spaces bar
    /// using the invited rooms held by the rooms list.
    fn regenerate_invited_spaces(&mut self, cx: &mut Cx) {
        let invited_rooms = get_invited_rooms(cx);
        let invited_rooms = invited_rooms.borrow();
        self.displayed_invited_spaces = invited_rooms.values()
            .filter(|invite| invite.is_space
                && !self.all_joined_spaces.contains_key(invite.room_name_id.room_id())
                && (self.display_filter)(*invite)
            )
            .map(|invite| InvitedSpaceInfo {
                space_name_id: invite.room_name_id.clone(),
                space_avatar: invite.room_avatar.clone(),
            })
            .collect();
        // Use alphabetical ordering for invited spaces (HashMaps don't iterate predictably)
        self.displayed_invited_spaces.sort_by(|a, b|
            a.space_name_id.display().cmp(&b.space_name_id.display())
                .then_with(|| a.space_name_id.room_id().cmp(b.space_name_id.room_id()))
        );
    }

    /// Updates the lists of displayed spaces based on the current search filter.
    fn update_displayed_joined_spaces(&mut self, cx: &mut Cx, keywords: &str) {
        self.is_filtered = !keywords.is_empty();
        (self.display_filter, self.sort_fn) = match self.is_filtered {
            false => (RoomDisplayFilter::default(), None),
            true => RoomDisplayFilterBuilder::new()
                .set_keywords(keywords.to_owned())
                .set_filter_criteria(RoomFilterCriteria::All)
                .build(),
        };
        self.regenerate_displayed_joined_spaces();
        self.regenerate_invited_spaces(cx);
        self.view.portal_list(cx, ids!(spaces_list)).set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
    }
}
