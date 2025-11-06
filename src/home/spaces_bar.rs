//! The SpacesBar shows a scrollable strip of avatars,
//! one per space that the user has currently joined.
//!
//! Like the NavigationTabBar, this widget uses AdaptiveView to show:
//! 1. a narrow vertical strip, when in Desktop (widescreen) mode,
//! 2. a wide, short horizontal strip, when in Mobile (narrowscreen) mode.

use std::collections::HashMap;

use makepad_widgets::*;
use matrix_sdk::RoomState;
use matrix_sdk_ui::spaces::SpaceRoom;
use ruma::{OwnedRoomAliasId, OwnedRoomId, OwnedServerName, room::JoinRuleSummary};

use crate::{
    avatar_cache::{self, AvatarCacheEntry}, login::login_screen::LoginAction, logout::logout_confirm_modal::LogoutAction, profile::{
        user_profile::{AvatarState, UserProfile},
        user_profile_cache::{self, UserProfileUpdate},
    }, room::{FetchedRoomAvatar, room_display_filter::{RoomDisplayFilter, RoomDisplayFilterBuilder, RoomFilterCriteria}}, shared::{
        avatar::AvatarWidgetExt, callout_tooltip::TooltipAction, custom_radio_button::CustomRadioButton, jump_to_bottom_button::UnreadMessageCount, room_filter_input_bar::RoomFilterAction, styles::*, verification_badge::VerificationBadgeWidgetExt
    }, sliding_sync::current_user_id, utils
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::verification_badge::*;
    use crate::shared::avatar::*;
    use crate::shared::custom_radio_button::*;

    NAVIGATION_TAB_BAR_SIZE = 68

    COLOR_NAVIGATION_TAB_FG = #1C274C
    COLOR_NAVIGATION_TAB_FG_HOVER = #1C274C
    COLOR_NAVIGATION_TAB_FG_ACTIVE = #1C274C
    COLOR_NAVIGATION_TAB_BG        = (COLOR_SECONDARY)
    COLOR_NAVIGATION_TAB_BG_HOVER  = (COLOR_SECONDARY * 0.85)
    COLOR_NAVIGATION_TAB_BG_ACTIVE = (COLOR_SECONDARY * 0.5)

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")


    SpaceIcon = {{SpaceIcon}}<RoundedView> {
        width: Fill,
        height: (NAVIGATION_TAB_BAR_SIZE - 8)
        flow: Overlay
        align: { x: 0.5, y: 0.5 }
        cursor: Default,

        space_avatar = <Avatar> {
            width: 45, height: 45
            // If no avatar picture, use white text on a dark background.
            text_view = {
                draw_bg: {
                    background_color: (COLOR_FG_DISABLED),
                }
                text = { draw_text: {
                    text_style: { font_size: 16.0 },
                    color: (COLOR_PRIMARY),
                } }
            }
        }

        // TODO: add an unread badge for each space

        // <View> {
        //     align: { x: 0.5, y: 0.0 }
        //     margin: { left: 42 }
        //     verification_badge = <VerificationBadge> {}
        // }
    }

    // A CustomRadioButton styled to fit within the SpacesBar.
    pub SpaceRadioButton = {{SpaceRadioButton}}<CustomRadioButton> {
        width: Fill,
        height: (NAVIGATION_TAB_BAR_SIZE - 5),
        padding: 5,
        margin: 3, 
        align: {x: 0.5, y: 0.5}
        flow: Down,

        icon_walk: {margin: 0, width: (NAVIGATION_TAB_BAR_SIZE/2.2), height: Fit}
        // Fully hide the text with zero size, zero margin, and zero spacing
        label_walk: {margin: 0, width: 0, height: 0}
        spacing: 0,

        draw_bg: {
            radio_type: Tab,

            color: (COLOR_NAVIGATION_TAB_BG)
            color_hover: (COLOR_NAVIGATION_TAB_BG_HOVER)
            color_active: (COLOR_NAVIGATION_TAB_BG_ACTIVE)

            border_size: 0.0
            border_color: #0000
            uniform inset: vec4(0.0, 0.0, 0.0, 0.0)
            border_radius: 4.0

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color,
                        self.color_hover,
                        self.hover
                    ),
                    self.color_active,
                    self.active
                )
            }

            fn get_border_color(self) -> vec4 {
                return self.border_color
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.box(
                    self.inset.x + self.border_size,
                    self.inset.y + self.border_size,
                    self.rect_size.x - (self.inset.x + self.inset.z + self.border_size * 2.0),
                    self.rect_size.y - (self.inset.y + self.inset.w + self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.get_border_color(), self.border_size)
                }
                return sdf.result;
            }
        }

        draw_text: {
            color: (COLOR_NAVIGATION_TAB_FG)
            color_hover: (COLOR_NAVIGATION_TAB_FG_HOVER)
            color_active: (COLOR_NAVIGATION_TAB_FG_ACTIVE)

            text_style: <THEME_FONT_BOLD>{font_size: 9}

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color,
                        self.color_hover,
                        self.hover
                    ),
                    self.color_active,
                    self.active
                )
            }
        }

        draw_icon: {
            uniform color: (COLOR_NAVIGATION_TAB_FG)
            uniform color_hover: (COLOR_NAVIGATION_TAB_FG_HOVER)
            uniform color_active: (COLOR_NAVIGATION_TAB_FG_ACTIVE)
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color,
                        self.color_hover,
                        self.focus
                    ),
                    self.color_active,
                    self.active
                )
            }
        }
    }


    HomeButton = <NavigationTabButton> {
        draw_icon: { svg_file: (ICON_HOME) }
        animator: { active = { default: on } }
    }

    SettingsButton = <NavigationTabButton> {
        draw_icon: { svg_file: (ICON_SETTINGS) }
    }

    // This button is temporarily disabled until the AddRoomScreen is implemented.
    AddRoomButton = <NavigationTabButton> {
        draw_bg: {
            color: (COLOR_SECONDARY)
            color_hover: (COLOR_SECONDARY)
            color_active: (COLOR_SECONDARY)
        }
        draw_icon: {
            svg_file: (ICON_ADD),
            color: (COLOR_FG_DISABLED),
            color_hover: (COLOR_FG_DISABLED)
            color_active: (COLOR_FG_DISABLED)
        }
        animator: { disabled = { default: on } }
    }

    Separator = <LineH> { margin: 8 }

    StatusLabel = <View> {
        width: Fill, height: Fill,
        align: { x: 0.5, y: 0.5 }
        padding: 15.0,

        label = <Label> {
            padding: 0
            width: Fill,
            height: Fill
            align: { x: 0.5, y: 0.5 }
            draw_text: {
                wrap: Word,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <REGULAR_TEXT>{font_size: 9}
            }
            text: "Loading\nspaces..."
        }
    }

    SpacesList = <PortalList> {
        height: Fill,
        width: Fill,
        flow: Down,
        spacing: 0.0

        auto_tail: false, 
        max_pull_down: 0.0,

        SpaceRadioButton = <SpaceRadioButton> {}
        StatusLabel = <StatusLabel> {}
        BottomFiller = <View> {
            width: 80.0,
            height: 80.0,
        }
    }

    pub SpacesBar = {{SpacesBar}}<AdaptiveView> {
        Desktop = {
            flow: Down,
            align: {x: 0.5}
            padding: {top: 20., bottom: 20}
            width: (NAVIGATION_TAB_BAR_SIZE), 
            height: Fill

            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <CachedWidget> {
                spaces_list = <SpacesList> {
                    flow: Down,
                }
            }
        }

        Mobile = {
            flow: Right
            align: {x: 0.5, y: 0.5}
            width: Fill,
            height: (NAVIGATION_TAB_BAR_SIZE)

            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <CachedWidget> {
                spaces_list = <SpacesList> {
                    flow: Right,
                }
            }
        }
    }
}


#[derive(Live, LiveHook, Widget)]
pub struct SpaceRadioButton {
    #[deref] radio_button: CustomRadioButton,

    #[rust] index_in_portal_list: usize,
    #[rust] space_id: Option<OwnedRoomId>,
}
impl Widget for SpaceRadioButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.radio_button.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.radio_button.draw_walk(cx, scope, walk);
        DrawStep::done()
    }
}
impl SpaceRadioButton {
    fn set_index_and_id(&mut self, _cx: &mut Cx, index_in_portal_list: usize, space_id: OwnedRoomId) {
        self.index_in_portal_list = index_in_portal_list;
        self.space_id = Some(space_id);
    }
}
impl SpaceRadioButtonRef {
    fn set_index_and_id(&self, _cx: &mut Cx, index_in_portal_list: usize, space_id: OwnedRoomId) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_index_and_id(_cx, index_in_portal_list, space_id);
    }
}



pub struct JoinedSpaceInfo {
    /// The ID of the space.
    pub space_id: OwnedRoomId,
    /// The canonical alias of the space, if any.
    pub canonical_alias: Option<OwnedRoomAliasId>,
    /// Calculated display name based on the space's name, aliases, and members.
    pub display_name: String,
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
    /// Whether this space is currently selected in the SpacesBar UI.
    pub is_selected: bool,
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
#[derive(Live, LiveHook, Widget)]
pub struct SpacesBar {
    #[deref] view: AdaptiveView,

    /// The set of all joined spaces, keyed by the space ID.
    #[rust] all_joined_spaces: HashMap<OwnedRoomId, JoinedSpaceInfo>,

    /// The currently-active filter function for the list of spaces.
    ///
    /// Note: for performance reasons, this does not get automatically applied
    /// when its value changes. Instead, you must manually invoke it on the set of `all_joined_spaces`
    /// in order to update the set of `displayed_spaces` accordingly.
    #[rust] display_filter: RoomDisplayFilter,

    /// The list of spaces currently displayed in the UI, in order from top to bottom.
    /// This is a strict subset of the rooms in `all_joined_spaces`, and should be determined
    /// by applying the `display_filter` to the set of `all_joined_spaces`.
    #[rust] displayed_spaces: Vec<OwnedRoomId>,
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
                if let RoomFilterAction::Changed(keywords) = action.as_widget_action().cast() {
                    self.update_displayed_spaces(cx, &keywords);
                    continue;
                }
            }

            // Handle one of the radio buttons being clicked (selected).
            let radio_button_set = self.view.radio_button_set(ids_array!(
                home_button,
                // add_room_button,
                settings_button,
            ));
            match radio_button_set.selected(cx, actions) {
                Some(0) => cx.action(NavigationBarAction::GoToHome),
                // Some(1) => cx.action(NavigationBarAction::GoToAddRoom),
                Some(1) => cx.action(NavigationBarAction::OpenSettings),
                _ => { }
            }

            for action in actions {
                // If another widget programmatically selected a new tab,
                // update our radio buttons accordingly.
                if let Some(NavigationBarAction::TabSelected(tab)) = action.downcast_ref() {
                    match tab {
                        SelectedTab::Home     => self.view.radio_button(ids!(home_button)).select(cx, scope),
                        SelectedTab::AddRoom  => {
                            // self.view.radio_button(ids!(add_room_button)).select(cx, scope),
                        }
                        SelectedTab::Settings => self.view.radio_button(ids!(settings_button)).select(cx, scope),
                    }
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(widget_to_draw) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = widget_to_draw.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            let len = self.displayed_spaces.len();
            if len == 0 {
                list.set_item_range(cx, 0, 1);
                while let Some(portal_list_index) = list.next_visible_item(cx) {
                    let item = if portal_list_index == 0 {
                        let item = list.item(cx, portal_list_index, id!(StatusLabel));
                        item.set_text(
                            cx,
                            if self.all_joined_spaces.is_empty() {
                                "Joined spaces\nwill show here."
                            } else {
                                "No matching\nspaces found.",
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
                    let item = if let Some(space_room) = self.displayed_spaces
                        .get(portal_list_index)
                        .and_then(|space_id| self.all_joined_spaces.get(space_id))
                    {
                        let item = list.item(cx, portal_list_index, id!(collapsible_header));
                        item.as_space_radio_button().set_index_and_id(
                            cx,
                            portal_list_index, 
                            space_room.room_id.clone(),
                        );
                        item
                    }
                    else if portal_list_index == len {
                        let item = list.item(cx, portal_list_index, id!(StatusLabel));
                        item.set_text(
                            cx,
                            match len {
                                0 => "No matching\nspaces found.",
                                1 => "Found 1\nmatching space.",
                                n => &format!("Found {}\nmatching spaces.", n),
                            }
                        );
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
    fn handle_spaces_list_updates(&mut self, cx: &mut Cx, _event: &Event, scope: &mut Scope) {

        fn adjust_displayed_spaces(
            was_displayed: bool,
            should_display: bool,
            space_id: OwnedRoomId,
            displayed_spaces: &mut Vec<OwnedRoomId>,
        ) {
            match (was_displayed, should_display) {
                // No need to update anything
                (true, true) | (false, false) => { }
                // Space was displayed but should no longer be displayed.
                (true, false) => {
                    displayed_spaces.iter()
                        .position(|s| s == space_id)
                        .map(|index| displayed_spaces.remove(index));
                }
                // Space was not displayed but should now be displayed.
                (false, true) => {
                    displayed_spaces.push(space_id);
                }
            }
        }


        let mut num_updates: usize = 0;
        while let Some(update) = PENDING_SPACE_UPDATES.pop() {
            num_updates += 1;
            match update {
                SpacesListUpdate::AddJoinedSpace(joined_space) => {
                    let space_id = joined.space_id.clone();
                    let should_display = (self.display_filter)(&joined_space);
                    let replaced = self.all_joined_spaces.insert(space_id.clone(), joined_space);
                    if replaced.is_none() {
                        adjust_displayed_spaces(false, should_display, space_id, &mut self.displayed_spaces);
                    } else {
                        error!("BUG: Added joined space {space_id} that already existed");
                    }
                }

                SpacesListUpdate::UpdateCanonicalAlias { space_id, new_canonical_alias } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        let was_displayed = (self.display_filter)(space);
                        space.canonical_alias = new_canonical_alias;
                        let should_display = (self.display_filter)(space);
                        adjust_displayed_spaces(was_displayed, should_display, space_id, &mut self.displayed_spaces);
                    } else {
                        error!("Error: couldn't find space {space_id} to update space canonical alias");
                    }
                }

                SpacesListUpdate::UpdateSpaceName { space_id, new_space_name } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        let was_displayed = (self.display_filter)(space);
                        space.display_name = new_space_name;
                        let should_display = (self.display_filter)(space);
                        adjust_displayed_spaces(was_displayed, should_display, space_id, &mut self.displayed_spaces);
                    } else {
                        error!("Error: couldn't find space {space_id} to update space name");
                    }
                }

                SpacesListUpdate::UpdateSpaceTopic { space_id, topic } => {
                    if let Some(space) = self.all_joined_spaces.get_mut(&space_id) {
                        // We don't currently support filtering by topic.
                        // let was_displayed = (self.display_filter)(space);
                        space.topic = topic;
                        // let should_display = (self.display_filter)(space);
                        // adjust_displayed_spaces(was_displayed, should_display, space_id, &mut self.displayed_spaces);
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
                    self.all_joined_spaces.remove(&space_id);
                    adjust_displayed_spaces(true, false, space_id, &mut self.displayed_spaces);
                }

                SpacesListUpdate::ClearSpaces => {
                    self.all_joined_spaces.clear();
                    self.displayed_spaces.clear();
                }

                SpacesListUpdate::ScrollToSpace(space_id) => {
                    if let Some(index) = self.displayed_spaces.iter().position(|s| s == &space_id) {
                        let portal_list = self.view.portal_list(ids!(spaces_list));
                        let speed = 40.0;
                        // Scroll to just above the space to make it more visible.
                        portal_list.smooth_scroll_to(cx, index.saturating_sub(1), speed, Some(10));
                    }
                }
            }
        }
        if num_updates > 0 {
            // log!("SpacesBar: processed {} updates to the list of all space", num_updates);
            self.redraw(cx);
        }
    }


    /// Updates the lists of displayed spaces based on the current search filter.
    fn update_displayed_spaces(&mut self, cx: &mut Cx, keywords: &str) {
        let portal_list = self.view.portal_list(ids!(spaces_list));
        if keywords.is_empty() {
            // Reset each of the displayed_* lists to show all rooms.
            self.display_filter = RoomDisplayFilter::default();
            self.displayed_spaces = self.all_joined_spaces.keys().cloned().collect();
            portal_list.set_first_id_and_scroll(0, 0.0);
            self.redraw(cx);
            return;
        }

        // Create a new filter function based on the given keywords
        // and store it in this RoomsList such that we can apply it to newly-added rooms.
        let (filter, sort_fn) = RoomDisplayFilterBuilder::new()
            .set_keywords(keywords.to_owned())
            .set_filter_criteria(RoomFilterCriteria::All)
            .build();
        self.display_filter = filter;

        let filtered_spaces_iter = self.all_joined_spaces.iter()
            .filter(|(_, space)| (self.display_filter)(*space));

        self.displayed_spaces = if let Some(sort_fn) = sort_fn {
            let mut filtered_spaces = filtered_spaces_iter
                .collect::<Vec<_>>();
            filtered_spaces.sort_by(|(_, space_a), (_, space_b)| sort_fn(*space_a, *space_b));
            filtered_spaces
                .into_iter()
                .map(|(space_id, _)| space_id.clone()).collect()
        } else {
            filtered_spaces_iter.map(|(space_id, _)| space_id.clone()).collect()
        };

        portal_list.set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
    }
}
