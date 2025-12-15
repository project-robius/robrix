//! The SpacesBar shows a scrollable strip of avatars,
//! one per space that the user has currently joined.
//!
//! Like the NavigationTabBar, this widget uses AdaptiveView to show:
//! 1. a narrow vertical strip, when in Desktop (widescreen) mode,
//! 2. a wide, short horizontal strip, when in Mobile (narrowscreen) mode.

use std::collections::HashMap;

use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::{RoomDisplayName, RoomState};
use ruma::{OwnedRoomAliasId, OwnedRoomId, room::JoinRuleSummary};

use crate::{
    home::navigation_tab_bar::{NavigationBarAction, SelectedTab}, room::{FetchedRoomAvatar, room_display_filter::{RoomDisplayFilter, RoomDisplayFilterBuilder, RoomFilterCriteria}}, shared::{avatar::AvatarWidgetRefExt, callout_tooltip::{CalloutTooltipOptions, TooltipAction, TooltipPosition}, room_filter_input_bar::RoomFilterAction}, utils::{self, RoomNameId}
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::*;

    // The duration of the animation when showing/hiding the SpacesBar (in Mobile view mode only).
    pub SPACES_BAR_ANIMATION_DURATION_SECS = 0.25

    // An entry in the list of all spaces, which shown the Space's avatar and name.
    SpacesBarEntry = {{SpacesBarEntry}}<RoundedView> {
        width: (NAVIGATION_TAB_BAR_SIZE - 5),
        height: (NAVIGATION_TAB_BAR_SIZE - 5),
        flow: Down
        padding: 5,
        margin: 3,
        align: {x: 0.5, y: 0.5}
        cursor: Hand

        show_bg: true
        draw_bg: {
            instance hover: 0.0
            instance active: 0.0

            color: #0000
            uniform color_hover: (COLOR_NAVIGATION_TAB_BG_HOVER)
            uniform color_active: (COLOR_NAVIGATION_TAB_BG_ACTIVE)

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

        avatar = <Avatar> {
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

        space_name = <Label> {
            width: Fill,
            // height: Fit
            height: 0,
            flow: RightWrap, // do not wrap
            padding: 0,
            align: {x: 0.5}
            draw_text: {
                instance active: 0.0
                instance hover: 0.0
                instance down: 0.0

                color: (COLOR_NAVIGATION_TAB_FG)
                uniform color_hover: (COLOR_NAVIGATION_TAB_FG_HOVER)
                uniform color_active: (COLOR_NAVIGATION_TAB_FG_ACTIVE)

                // text_style: <THEME_FONT_BOLD>{font_size: 9}
                text_style: <REGULAR_TEXT>{font_size: 9}
                wrap: Word,

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
        }

        animator: {
            hover = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.15}}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 0.0}
                        space_name = { draw_text: {down: [{time: 0.0, value: 0.0}], hover: 0.0} }
                    }
                }
                on = {
                    from: {all: Snap}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 1.0}
                        space_name = { draw_text: {down: [{time: 0.0, value: 0.0}], hover: 1.0} }
                    }
                }
                down = {
                    from: {all: Forward {duration: 0.2}}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 1.0}], hover: 1.0,}
                        space_name = { draw_text: {down: [{time: 0.0, value: 1.0}], hover: 1.0,} }
                    }
                }
            }
        }
    }

    StatusLabel = <View> {
        width: (NAVIGATION_TAB_BAR_SIZE),
        height: (NAVIGATION_TAB_BAR_SIZE),
        align: { x: 0.5, y: 0.5 }
        margin: {top: 9, left: 2, bottom: 5}
        // padding: 5.0,

        label = <Label> {
            padding: 0
            margin: 0
            width: Fill,
            height: Fill
            align: { x: 0.5, y: 0.5 }
            draw_text: {
                wrap: Word,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <REGULAR_TEXT>{font_size: 9}
            }
        }
    }

    SpacesList = <PortalList> {
        height: Fill,
        width: Fill,
        flow: Down,
        spacing: 0.0

        auto_tail: false, 
        max_pull_down: 0.0,
        scroll_bar: {  // hide the scroll bar
            bar_size: 0.0,
            min_handle_size: 0.0
        }

        SpacesBarEntry = <SpacesBarEntry> {}
        StatusLabel = <StatusLabel> {}
        BottomFiller = <View> {
            width: (NAVIGATION_TAB_BAR_SIZE)
            height: (NAVIGATION_TAB_BAR_SIZE)
        }
    }

    pub SpacesBar = {{SpacesBar}}<AdaptiveView> {
        Desktop = {
            align: {x: 0.5, y: 0.5}
            padding: 0,
            width: (NAVIGATION_TAB_BAR_SIZE), 
            height: Fill

            show_bg: false

            <CachedWidget> {
                spaces_list = <SpacesList> {
                    // Note: this doesn't work properly, so this is re-overwritten
                    // in the `SpacesBar::draw_walk()` function.
                    flow: Down,
                }
            }
        }

        Mobile = {
            align: {x: 0.5, y: 0.5}
            width: Fill,
            height: (NAVIGATION_TAB_BAR_SIZE)

            show_bg: false

            <CachedWidget> {
                spaces_list = <SpacesList> {
                    // Note: this doesn't work properly, so this is re-overwritten
                    // in the `SpacesBar::draw_walk()` function.
                    flow: Right,
                }
            }
        }
    }
}


/// Actions emitted by and handled by the SpacesBar widget (and its children).
#[derive(Clone, Debug, DefaultNone)]
pub enum SpacesBarAction {
    /// The user primary-clicked/tapped a space entry in the SpacesBar.
    ButtonClicked { space_name_id: RoomNameId },
    /// The user secondary-clicked/long-pressed a space entry in the SpacesBar.
    ButtonSecondaryClicked { space_name_id: RoomNameId },
    None,
}


#[derive(Live, LiveHook, Widget)]
pub struct SpacesBarEntry {
    #[deref] view: View,
    #[animator] animator: Animator,

    #[rust] space_name_id: Option<RoomNameId>,
}

impl Widget for SpacesBarEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        let area = self.draw_bg.area();
        let emit_hover_in_action = |this: &Self, cx: &mut Cx| {
            let is_desktop = cx.display_context.is_desktop();
            cx.widget_action(
                this.widget_uid(),
                &scope.path,
                TooltipAction::HoverIn {
                    widget_rect: area.rect(cx),
                    text: this.space_name_id.as_ref().map_or(
                        String::from("Unknown Space Name"),
                        |sni| sni.to_string(),
                    ),
                    options: CalloutTooltipOptions {
                        position: if is_desktop {
                            TooltipPosition::Right
                        } else {
                            TooltipPosition::Top
                        },
                        ..Default::default()
                    },
                },
            );
        };

        match event.hits(cx, area) {
            Hit::FingerHoverIn(_) => {
                self.animator_play(cx, ids!(hover.on));
                emit_hover_in_action(self, cx);
            }
            Hit::FingerHoverOver(_) => {
                emit_hover_in_action(self, cx);
            }
            Hit::FingerHoverOut(_) => {
                self.animator_play(cx, ids!(hover.off));
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    TooltipAction::HoverOut,
                );
            }
            Hit::FingerDown(fe) => {
                self.animator_play(cx, ids!(hover.down));
                if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                    if let Some(space_name_id) = self.space_name_id.clone() {
                        cx.widget_action(
                            self.widget_uid(),
                            &scope.path,
                            SpacesBarAction::ButtonSecondaryClicked { space_name_id },
                        );
                    }
                }
            }
            Hit::FingerLongPress(_lp) => {
                self.animator_play(cx, ids!(hover.down));
                emit_hover_in_action(self, cx);
                if let Some(space_name_id) = self.space_name_id.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path,
                        SpacesBarAction::ButtonSecondaryClicked { space_name_id },
                    );
                }
            }
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                self.animator_play(cx, ids!(hover.on));
                if let Some(space_name_id) = self.space_name_id.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path,
                        SpacesBarAction::ButtonClicked { space_name_id },
                    );
                }
            }
            Hit::FingerUp(fe) if !fe.is_over => {
                self.animator_play(cx, ids!(hover.off));
            }
            Hit::FingerMove(_fe) => { }
            _ => {}
        }
    }
    
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SpacesBarEntry {
    fn set_metadata(&mut self, cx: &mut Cx, space_name_id: RoomNameId, is_selected: bool) {
        self.space_name_id = Some(space_name_id);
        let active_val = is_selected as u8 as f64;
        self.apply_over(cx, live!{
            draw_bg: { active: (active_val) },
            space_name = { draw_text: { active: (active_val) } }
        });
    }
}
impl SpacesBarEntryRef {
    pub fn set_metadata(&self, cx: &mut Cx, space_name_id: RoomNameId, is_selected: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_metadata(cx, space_name_id, is_selected);
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

    /// Whether the list of `displayed_spaces` is currently filtered:
    /// `true` if filtered, `false` if showing everything.
    #[rust] is_filtered: bool,

    /// The ID of the currently-selected space in this SpacesBar.
    /// Only one space can be selected at once.
    #[rust] selected_space: Option<OwnedRoomId>,
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
                // The room filter input bar is also used to filter which spaces are visible.
                if let RoomFilterAction::Changed(keywords) = action.as_widget_action().cast() {
                    self.update_displayed_spaces(cx, &keywords);
                    continue;
                }

                // Update which space is currently selected.
                if let SpacesBarAction::ButtonClicked { space_name_id } = action.as_widget_action().cast() {
                    self.selected_space = Some(space_name_id.room_id().clone());
                    self.redraw(cx);
                    cx.action(NavigationBarAction::GoToSpace { space_name_id });
                    continue;
                }

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
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(widget_to_draw) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = widget_to_draw.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            // AdaptiveView + CachedWidget does not properly handle DSL-level style overrides,
            // so we must manually apply the different style choices here when drawing it.
            if cx.display_context.is_desktop() {
                list.apply_over(cx, live! {
                    flow: Down,
                });
            } else {
                list.apply_over(cx, live! {
                    flow: Right,
                });
            }

            let len = self.displayed_spaces.len();
            if len == 0 {
                list.set_item_range(cx, 0, 1);
                while let Some(portal_list_index) = list.next_visible_item(cx) {
                    let item = if portal_list_index == 0 {
                        let item = list.item(cx, portal_list_index, id!(StatusLabel));
                        item.label(ids!(label)).set_text(
                            cx,
                            if self.is_filtered {
                                "Found no\nmatching spaces."
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
                    let item = if let Some(space) = self.displayed_spaces
                        .get(portal_list_index)
                        .and_then(|space_id| self.all_joined_spaces.get(space_id))
                    {
                        let item = list.item(cx, portal_list_index, id!(SpacesBarEntry));
                        // Populate the space name and avatar (although this isn't visible by default).
                        let space_name = space.space_name_id.to_string();
                        item.label(ids!(space_name)).set_text(cx, &space_name);
                        let avatar_ref = item.avatar(ids!(avatar));
                        match &space.space_avatar {
                            FetchedRoomAvatar::Text(text) => {
                                avatar_ref.show_text(cx, None, None, text);
                            }
                            FetchedRoomAvatar::Image(image_data) => {
                                let res = avatar_ref.show_image(
                                    cx,
                                    None,
                                    |cx, img_ref| utils::load_png_or_jpg(&img_ref, cx, image_data),
                                );
                                if res.is_err() {
                                    avatar_ref.show_text(
                                        cx,
                                        None,
                                        None,
                                        &space_name,
                                    );
                                }
                            }
                        }
                        item.as_spaces_bar_entry().set_metadata(
                            cx,
                            space.space_name_id.clone(),
                            self.selected_space.as_ref().is_some_and(|id| id == space.space_name_id.room_id()),
                        );
                        item
                    }
                    else if portal_list_index == len {
                        let item = list.item(cx, portal_list_index, id!(StatusLabel));
                        let descriptor = if self.is_filtered { "matching" } else { "joined" }; 
                        let text = match len {
                            0      => format!("Found no\n{descriptor} spaces."),
                            1      => format!("Found 1\n{descriptor} space."),
                            2..100 => format!("Found {len}\n{descriptor} spaces."),
                            100..  => format!("Found 99+\n{descriptor} spaces."),
                        };
                        item.label(ids!(label)).set_text(cx, &text);
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
                        .position(|s| s == &space_id)
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
                    let space_id = joined_space.space_name_id.room_id().clone();
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
                        space.space_name_id = RoomNameId::new(
                            RoomDisplayName::Named(new_space_name),
                            space_id.clone(),
                        );
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
            self.redraw(cx);
        }
    }


    /// Updates the lists of displayed spaces based on the current search filter.
    fn update_displayed_spaces(&mut self, cx: &mut Cx, keywords: &str) {
        let portal_list = self.view.portal_list(ids!(spaces_list));
        if keywords.is_empty() {
            // Reset each of the displayed_* lists to show all rooms.
            self.is_filtered = false;
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
        self.is_filtered = true;

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
