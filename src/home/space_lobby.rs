//! Contains two widgets related to the top-level view of a space.
//!
//! 1. `SpaceLobby`: shows details about a space, including its name, avatar,
//!    members, topic, and the full list of rooms and subspaces within it.
//! 2. `SpaceLobbyEntry`: the button that can be shown in a RoomsList
//!    that allows the user to click on it to show the `SpaceLobby`.
//!

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::{RoomState, ruma::OwnedRoomId};
use matrix_sdk_ui::spaces::SpaceRoom;
use ruma::room::JoinRuleSummary;
use tokio::sync::mpsc::UnboundedSender;
use crate::shared::avatar::AvatarState;
use crate::utils::replace_linebreaks_separators;
use crate::{
    avatar_cache::{self, AvatarCacheEntry},
    home::rooms_list::RoomsListRef,
    shared::avatar::{AvatarWidgetExt, AvatarWidgetRefExt},
    space_service_sync::{SpaceRequest, SpaceRoomExt, SpaceRoomListAction},
    utils::{self, RoomNameId},
};


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::*;

    ICON_COLLAPSE = dep("crate://self/resources/icons/triangle_fill.svg")

    // An entry in the RoomsList that will show the SpaceLobby when clicked.
    pub SpaceLobbyEntry = {{SpaceLobbyEntry}}<RoundedView> {
        visible: false, // only visible when a space is selected
        width: Fill,
        height: 35, // same as CollapsibleHeader
        flow: Right,
        align: {y: 0.5}
        padding: 5,
        margin: {top: 10, bottom: 0}
        cursor: Hand

        show_bg: true
        draw_bg: {
            instance hover: 0.0
            instance active: 0.0

            color: (COLOR_NAVIGATION_TAB_BG)
            uniform color_hover: (COLOR_NAVIGATION_TAB_BG_HOVER)
            uniform color_active: (COLOR_ACTIVE_PRIMARY)

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

        icon = <Icon> {
            width: 25,
            height: 25,
            margin: {left: 5, right: 3}
            align: {x: 0.5, y: 0.5}
            draw_icon: {
                svg_file: (ICON_HIERARCHY)

                instance active: 0.0
                instance hover: 0.0
                instance down: 0.0

                color: (COLOR_TEXT)
                uniform color_hover: (COLOR_TEXT)
                uniform color_active: (COLOR_PRIMARY)

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
            icon_walk: { width: 25, height: 20, margin: {left: 0, bottom: 0} }
        }

        space_lobby_label = <Label> {
            width: Fill, height: Fit
            flow: Right,
            padding: 0,

            draw_text: {
                instance active: 0.0
                instance hover: 0.0
                instance down: 0.0

                color: (COLOR_TEXT)
                uniform color_hover: (COLOR_TEXT)
                uniform color_active: (COLOR_PRIMARY)

                text_style: <REGULAR_TEXT>{font_size: 11},
                wrap: Ellipsis,

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
            text: "Explore this Space"
        }

        animator: {
            hover = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.15}}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 0.0}
                        space_lobby_label = { draw_text: {down: [{time: 0.0, value: 0.0}], hover: 0.0} }
                        icon = { draw_icon: {down: [{time: 0.0, value: 0.0}], hover: 0.0} }
                    }
                }
                on = {
                    from: {all: Snap}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 1.0}
                        space_lobby_label = { draw_text: {down: [{time: 0.0, value: 0.0}], hover: 1.0} }
                        icon = { draw_icon: {down: [{time: 0.0, value: 0.0}], hover: 1.0} }
                    }
                }
                down = {
                    from: {all: Forward {duration: 0.2}}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 1.0}], hover: 1.0,}
                        space_lobby_label = { draw_text: {down: [{time: 0.0, value: 1.0}], hover: 1.0,} }
                        icon = { draw_icon: {down: [{time: 0.0, value: 1.0}], hover: 1.0,} }
                    }
                }
            }
        }
    }

    // A view that draws the hierarchical tree structure lines.
    DrawTreeLine = {{DrawTreeLine}} { }

    TreeLines = {{TreeLines}} {
        width: 0, height: Fill
        draw_bg: {
            indent_width: 44.0
            level: 0.0
            is_last: 0.0
            parent_mask: 0.0

            fn pixel(self) -> vec4 {
                let pos = self.pos * self.rect_size;
                let indent = self.indent_width;
                // Yes, this should be 0.5, but 0.6 makes it line up nicely
                // with the middle of the parent-level avatar, which is better.
                let half_indent = indent * 0.6;
                let line_width = 1.0;
                let half_line = 0.5;

                let c = vec4(0.0);

                // Dumb approach, but it works.
                for i in 0..20 {
                    if float(i) > self.level { break; }
                    
                    if float(i) < self.level {
                        // Check mask for parent levels
                        let mask_bit = mod(floor(self.parent_mask / pow(2.0, float(i))), 2.0);
                        if mask_bit > 0.5 {
                             // Draw full vertical line
                             if abs(pos.x - (float(i) * indent + half_indent)) < half_line && pos.y < self.rect_size.y {
                                  return vec4(0.8, 0.8, 0.8, 1.0);
                             }
                        }
                    } else {
                        // Current level: connection to self
                        
                        // Horizontal line to content
                        let hy = self.rect_size.y * 0.5;
                        if abs(pos.y - hy) < half_line && pos.x > (float(i) * indent + half_indent) {
                             return vec4(0.8, 0.8, 0.8, 1.0);
                        }
                        
                        // Vertical line (L shape)
                        if abs(pos.x - (float(i) * indent + half_indent)) < half_line && pos.y < (self.rect_size.y * (1.0 - 0.5 * self.is_last)) {
                              return vec4(0.8, 0.8, 0.8, 1.0);
                        }
                    }
                }
                return c;
            }
        }
    }

    // Entry for a child subspace (can be expanded)
    pub SubspaceEntry = {{SubspaceEntry}} {
        width: Fill,
        height: 44,
        flow: Right,
        align: {y: 0.5}
        padding: {left: 8, right: 12}
        cursor: Hand

        show_bg: true
        draw_bg: {
            instance hover: 0.0
            color: #fff
            uniform color_hover: #f5f5f5
            fn pixel(self) -> vec4 {
                return mix(self.color, self.color_hover, self.hover);
            }
        }

        // The connecting hierarchical lines on the left.
        tree_lines = <TreeLines> {}

        // Expand/collapse icon
        expand_icon = <IconRotated> {
            width: 16,
            height: 16,
            margin: { top: 7, left: -8, right: 2 }
            draw_icon: {
                svg_file: (ICON_COLLAPSE)
                rotation_angle: 90.0
                color: #888
            }
            icon_walk: { width: 10, height: 10 }
        }

        avatar = <Avatar> { width: 32, height: 32, margin: {right: 8} }

        content = <View> {
            width: Fill, height: Fit, flow: Down, spacing: 5,
            name_label = <Label> {
                margin: 0
                padding: 0
                width: Fill, height: Fit,
                draw_text: { text_style: <REGULAR_TEXT>{font_size: 10.5}, color: #1a1a1a, wrap: Ellipsis }
            }
            info_label = <Label> {
                margin: 0
                padding: 0
                width: Fit, height: Fit,
                flow: Right
                draw_text: { text_style: <REGULAR_TEXT>{font_size: 8.5}, color: #737373, wrap: Ellipsis }
            }
        }

        animator: {
            hover = {
                default: off
                off = { from: {all: Forward {duration: 0.1}}, apply: { draw_bg: {hover: 0.0} } }
                on = { from: {all: Snap}, apply: { draw_bg: {hover: 1.0} } }
            }
        }
    }

    // Entry for a child room within a space, which cannot be expanded.
    pub RoomEntry = {{RoomEntry}}<SubspaceEntry> {
        cursor: Default

        expand_icon = <View> {
            width: 10
            height: 16
        }
    }

    StatusLabel = <View> {
        width: Fill, height: Fit,
        flow: Right,
        align: { x: 0.5, y: 0.5 }
        padding: 20.0,

        loading_spinner = <LoadingSpinner> {
            width: 18,
            height: 18,
            draw_bg: {
                color: (COLOR_ACTIVE_PRIMARY)
                border_size: 2.5,
            }
        }

        label = <Label> {
            padding: {left: 10}
            width: Fit,
            flow: RightWrap,
            align: { x: 0.5, y: 0.5 }
            draw_text: {
                wrap: Word,
                color: #737373,
                text_style: <REGULAR_TEXT>{font_size: 10}
            }
            text: "Loading rooms and spaces..."
        }
    }

    // Small loading indicator shown inline when loading subspace children
    SubspaceLoadingEntry = <View> {
        width: Fill, height: 36,
        flow: Right,
        align: { y: 0.5 }
        padding: {left: 8, right: 12}

        // Tree lines replace the spacer
        tree_lines = <TreeLines> {}

        loading_spinner = <LoadingSpinner> {
            width: 14,
            height: 14,
            margin: {left: 8, right: 10}
            draw_bg: {
                color: (COLOR_ACTIVE_PRIMARY)
                border_size: 2.0,
            }
        }

        label = <Label> {
            width: Fit,
            height: Fit,
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 9},
                color: #888,
            }
            text: "Loading..."
        }
    }

    // The main view that shows the lobby (homepage) for a space.
    pub SpaceLobbyScreen = {{SpaceLobbyScreen}} {
        width: Fill, height: Fill,
        flow: Down,

        show_bg: true
        draw_bg: {
            color: #fff
        }

        // Header with parent space info
        header = <View> {
            width: Fill,
            height: Fit,
            flow: Down,
            padding: {left: 16, right: 16, top: 16, bottom: 8}

            show_bg: true,
            draw_bg: {
                color: #fafafa
            }

            title = <Label> {
                width: Fill,
                height: Fit,
                draw_text: {
                    text_style: <REGULAR_TEXT>{font_size: 9},
                    color: #737373,
                    wrap: Word,
                }
                text: "Rooms and Spaces in"
            }
            
            // Parent space row with avatar and name
            parent_space_row = <View> {
                width: Fill,
                height: Fit,
                flow: Right,
                align: { y: 0.5 }
                padding: { top: 8 }
                
                parent_avatar = <Avatar> {
                    width: 36,
                    height: 36,
                    margin: { right: 12 }
                }
                
                parent_name = <Label> {
                    width: Fill,
                    height: Fit,
                    draw_text: {
                        text_style: <TITLE_TEXT>{font_size: 14},
                        color: #1a1a1a,
                        wrap: Ellipsis,
                    }
                    text: ""
                }
            }
        }

        // The hierarchical tree list
        tree_list = <PortalList> {
            keep_invisible: false,
            max_pull_down: 0.0,
            auto_tail: false,
            width: Fill, height: Fill
            flow: Down,
            spacing: 0.0

            subspace_entry = <SubspaceEntry> {}
            room_entry = <RoomEntry> {}
            subspace_loading = <SubspaceLoadingEntry> {}
            status_label = <StatusLabel> {}
            bottom_filler = <View> {
                width: Fill,
                height: 80.0,
            }
        }
    }
}


thread_local! {
    /// A cache of UI states for each SpaceLobbyScreen, keyed by the space's room ID.
    /// This allows preserving the expanded/collapsed state of subspaces across screen changes.
    static SPACE_LOBBY_STATES: RefCell<BTreeMap<OwnedRoomId, SpaceLobbyUiState>> = const {
        RefCell::new(BTreeMap::new())
    };
}

/// The UI-side state of a SpaceLobbyScreen that should persist across hide/show cycles.
#[derive(Default)]
struct SpaceLobbyUiState {
    /// The set of space IDs that are currently expanded (showing their children).
    expanded_spaces: HashSet<OwnedRoomId>,
}


/// A clickable entry shown in the RoomsList that will show the space lobby when clicked.
#[derive(Live, LiveHook, Widget)]
pub struct SpaceLobbyEntry {
    #[deref] view: View,
    #[animator] animator: Animator,
}

impl Widget for SpaceLobbyEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        let area = self.draw_bg.area();
        match event.hits(cx, area) {
            Hit::FingerHoverIn(_) => {
                self.animator_play(cx, ids!(hover.on));
            }
            Hit::FingerHoverOut(_) => {
                self.animator_play(cx, ids!(hover.off));
            }
            Hit::FingerDown(_fe) => {
                self.animator_play(cx, ids!(hover.down));
            }
            Hit::FingerLongPress(_lp) => {
                self.animator_play(cx, ids!(hover.down));
            }
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                self.animator_play(cx, ids!(hover.on));
                cx.action(SpaceLobbyAction::SpaceLobbyEntryClicked);
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


#[derive(Debug)]
pub enum SpaceLobbyAction {
    SpaceLobbyEntryClicked,
}

#[derive(Live, LiveHook, LiveRegister)]
#[repr(C)]
pub struct DrawTreeLine {
    #[deref] draw_super: DrawQuad,
    #[live] indent_width: f32,
    #[live] level: f32,
    #[live] is_last: f32,
    #[live] parent_mask: f32,
}

#[derive(Live, LiveHook, Widget)]
pub struct TreeLines {
    #[redraw] #[live] draw_bg: DrawTreeLine,
    #[walk] walk: Walk,
}

impl Widget for TreeLines {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) { }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let indent_pixel = (self.draw_bg.level + 1.0) * self.draw_bg.indent_width;
        let mut walk = walk;
        walk.width = Size::Fixed(indent_pixel as f64);
        
        self.draw_bg.draw_walk(cx, walk);
        DrawStep::done()
    }
}

/// A clickable entry for a child subspace.
#[derive(Live, LiveHook, Widget)]
pub struct SubspaceEntry {
    #[deref] view: View,
    #[animator] animator: Animator,
    #[rust] space_id: Option<OwnedRoomId>,
}

/// A clickable entry for a child room.
#[derive(Live, LiveHook, Widget)]
pub struct RoomEntry {
    #[deref] view: View,
    #[animator] animator: Animator,
    #[rust] room_id: Option<OwnedRoomId>,
}

/// Action emitted when a subspace entry is clicked.
#[derive(Clone, Debug, DefaultNone)]
pub enum SubspaceEntryAction {
    Clicked { space_id: OwnedRoomId },
    None,
}

/// Action emitted when a room entry is clicked.
#[derive(Clone, Debug, DefaultNone)]
pub enum RoomEntryAction {
    Clicked { room_id: OwnedRoomId },
    None,
}

impl Widget for SubspaceEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        match event.hits(cx, self.view.area()) {
            Hit::FingerHoverIn(_) => { self.animator_play(cx, ids!(hover.on)); }
            Hit::FingerHoverOut(_) => { self.animator_play(cx, ids!(hover.off)); }
            Hit::FingerDown(_) => { cx.set_key_focus(self.view.area()); }
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                if let Some(space_id) = self.space_id.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path, 
                        SubspaceEntryAction::Clicked { space_id },
                    );
                }
            }
            _ => {}
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl Widget for RoomEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        match event.hits(cx, self.view.area()) {
            Hit::FingerHoverIn(_) => { self.animator_play(cx, ids!(hover.on)); }
            Hit::FingerHoverOut(_) => { self.animator_play(cx, ids!(hover.off)); }
            Hit::FingerDown(_) => { cx.set_key_focus(self.view.area()); }
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                if let Some(room_id) = self.room_id.clone() {
                    cx.widget_action(self.widget_uid(), &scope.path,
                        RoomEntryAction::Clicked { room_id });
                }
            }
            _ => {}
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

/// The subset of info in [`SpaceRoom`] that we display for each room/space.
struct SpaceRoomInfo {
    id: OwnedRoomId,
    name: String,
    topic: Option<String>,
    avatar: AvatarState,
    num_joined_members: u64,
    state: Option<RoomState>,
    #[allow(unused)]
    join_rule: Option<JoinRuleSummary>,
    /// If `Some`, this is a space. If `None`, it's a room.
    children_count: Option<u64>,
}
impl SpaceRoomInfo {
    fn is_space(&self) -> bool {
        self.children_count.is_some()
    }
}
impl From<&SpaceRoom> for SpaceRoomInfo {
    fn from(space_room: &SpaceRoom) -> Self {
        SpaceRoomInfo {
            id: space_room.room_id.clone(),
            name: space_room.display_name.clone(),
            topic: space_room.topic.as_ref().map(|t| replace_linebreaks_separators(t.trim())),
            avatar: AvatarState::Known(space_room.avatar_url.clone()),
            num_joined_members: space_room.num_joined_members,
            state: space_room.state,
            join_rule: space_room.join_rule.clone(),
            children_count: space_room.is_space().then_some(space_room.children_count),
        }
    }
}
impl From<SpaceRoom> for SpaceRoomInfo {
    fn from(space_room: SpaceRoom) -> Self {
        SpaceRoomInfo {
            children_count: space_room.is_space().then_some(space_room.children_count),
            id: space_room.room_id,
            name: space_room.display_name,
            topic: space_room.topic.map(|t| replace_linebreaks_separators(t.trim())),
            avatar: AvatarState::Known(space_room.avatar_url),
            num_joined_members: space_room.num_joined_members,
            state: space_room.state,
            join_rule: space_room.join_rule,
        }
    }
}

/// An entry in the tree to be displayed.
#[allow(clippy::large_enum_variant)]
enum TreeEntry {
    /// A regular space or room entry.
    Item {
        /// The info needed to display this space or room.
        info: SpaceRoomInfo,
        /// The nesting level (0 = direct child of the displayed space).
        level: usize,
        /// Whether this entry is the last child of its parent.
        is_last: bool,
        /// Bitmask of which parent levels need continuation lines.
        parent_mask: u32,
    },
    /// A loading indicator for a subspace that's still loading.
    Loading {
        /// The nesting level for proper indentation.
        level: usize,
        /// Bitmask of which parent levels need continuation lines.
        parent_mask: u32,
    },
}

/// The view showing the lobby/homepage for a given space.
#[derive(Live, LiveHook, Widget)]
pub struct SpaceLobbyScreen {
    #[deref] view: View,

    /// The space that is currently being displayed.
    #[rust] space_name_id: Option<RoomNameId>,

    /// The sender channel to submit space requests to the background service.
    #[rust] space_request_sender: Option<UnboundedSender<SpaceRequest>>,

    /// Cache of detailed children for each space we've fetched.
    /// Key is the space_id, value is the list of its direct children.
    #[rust] children_cache: HashMap<OwnedRoomId, Vector<SpaceRoom>>,

    /// The set of space IDs that are currently expanded (showing their children).
    #[rust] expanded_spaces: HashSet<OwnedRoomId>,

    /// The ordered list of children to display in the space tree.
    #[rust] tree_entries: Vec<TreeEntry>,

    /// The set of space IDs that are currently loading their children.
    #[rust] loading_subspaces: HashSet<OwnedRoomId>,

    /// Whether we are currently loading the initial data.
    #[rust] is_loading: bool,
}

impl Widget for SpaceLobbyScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Handle Signal events for avatar cache updates
        if let Event::Signal = event {
            // Process any pending avatar updates
            avatar_cache::process_avatar_updates(cx);
            self.redraw(cx);
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(SpaceRoomListAction::DetailedChildren { space_id, children, .. }) = action.downcast_ref() {
                    self.update_children_in_space(cx, space_id, children);
                }

                // Handle SubspaceEntry clicks
                if let SubspaceEntryAction::Clicked { space_id: room_id } = action.as_widget_action().cast() {
                    self.toggle_space_expansion(cx, &room_id);
                }

                // Handle RoomEntry clicks
                if let RoomEntryAction::Clicked { room_id: _ } = action.as_widget_action().cast() {
                    // TODO: Navigate to the room
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(widget_to_draw) = self.view.draw_walk(cx, scope, walk).step() {
            let portal_list_ref = widget_to_draw.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            let entry_count = self.tree_entries.len();
            let total_count = if self.is_loading || entry_count == 0 {
                2 // status label + filler
            } else {
                entry_count + 1 // entries + filler
            };

            list.set_item_range(cx, 0, total_count);

            while let Some(item_id) = list.next_visible_item(cx) {
                // Draw loading indicator
                let item = if self.is_loading && item_id == 0 {
                    let item = list.item(cx, item_id, id!(status_label));
                    item.label(ids!(label)).set_text(cx, "Loading rooms and spaces...");
                    item
                }
                // No entries found
                else if entry_count == 0 && item_id == 0 {
                    let item = list.item(cx, item_id, id!(status_label));
                    item.label(ids!(label)).set_text(cx, "No rooms or spaces found.");
                    item.view(ids!(loading_spinner)).apply_over(cx, live! { visible: false });
                    item
                }
                // Draw a regular entrty
                else if let Some(entry) = self.tree_entries.get_mut(item_id) {
                    match entry {
                        TreeEntry::Item { info, level, is_last, parent_mask } => {
                            let item = if info.is_space() {
                                let item = list.item(cx, item_id, id!(subspace_entry));
                                if let Some(mut inner) = item.borrow_mut::<SubspaceEntry>() {
                                    inner.space_id = Some(info.id.clone());
                                }
                                // Expand icon
                                let is_expanded = self.expanded_spaces.contains(&info.id);
                                let angle = if is_expanded { 180.0 } else { 90.0 };
                                item.icon(ids!(expand_icon)).apply_over(cx, live! {
                                    draw_icon: { rotation_angle: (angle) }
                                });
                                item
                            } else {
                                let item = list.item(cx, item_id, id!(room_entry));
                                if let Some(mut inner) = item.borrow_mut::<RoomEntry>() {
                                    inner.room_id = Some(info.id.clone());
                                }
                                item
                            };

                            // Below, draw things that are common to child rooms and subspaces.
                            item.label(ids!(content.name_label)).set_text(cx, &info.name);

                            // Display avatar from stored data, or fetch from cache, or show initials
                            let avatar_ref = item.avatar(ids!(avatar));
                            let first_char = utils::user_name_first_letter(&info.name);
                            let mut drew_avatar = false;

                            match &info.avatar {
                                AvatarState::Loaded(data) => {
                                    drew_avatar = avatar_ref.show_image(
                                        cx,
                                        None,
                                        |cx, img| utils::load_png_or_jpg(&img, cx, data),
                                    ).is_ok();
                                }
                                AvatarState::Known(Some(uri)) => {
                                    match avatar_cache::get_or_fetch_avatar(cx, uri.to_owned()) {
                                        AvatarCacheEntry::Loaded(data) => {
                                            drew_avatar = avatar_ref.show_image(
                                                cx,
                                                None,
                                                |cx, img| utils::load_png_or_jpg(&img, cx, &data),
                                            ).is_ok();
                                            info.avatar = AvatarState::Loaded(data);
                                        }
                                        AvatarCacheEntry::Failed => {
                                            info.avatar = AvatarState::Failed;
                                        }
                                        AvatarCacheEntry::Requested => { }
                                    }
                                }
                                _ => { }
                            };
                            // Fallback to text initials.
                            if !drew_avatar {
                                avatar_ref.show_text(cx, None, None, first_char.unwrap_or("#"));
                            }

                            if let Some(mut lines) = item.widget(ids!(tree_lines)).borrow_mut::<TreeLines>() {
                                lines.draw_bg.level = *level as f32;
                                lines.draw_bg.is_last = if *is_last { 1.0 } else { 0.0 };
                                lines.draw_bg.parent_mask = *parent_mask as f32;
                                lines.draw_bg.indent_width = 44.0; // Hardcoded to match
                            }

                            // Build the info label with join status, member count, and topic
                            // Note: Public/Private is intentionally not shown per-item to reduce clutter
                            let info_label = item.label(ids!(content.info_label));
                            let mut info_parts = Vec::new();

                            // Add join status for rooms we haven't joined
                            if let Some(state) = &info.state {
                                match state {
                                    RoomState::Joined => info_parts.push("✅ Joined".to_string()),
                                    RoomState::Left => info_parts.push("Left".to_string()),
                                    RoomState::Invited => info_parts.push("Invited".to_string()),
                                    RoomState::Knocked => info_parts.push("Knocked".to_string()),
                                    RoomState::Banned => info_parts.push("Banned".to_string()),
                                }
                            }

                            // Add member count
                            info_parts.push(format!(
                                "{} {}",
                                info.num_joined_members,
                                if info.num_joined_members == 1 { "member" } else { "members" }
                            ));

                            // Add children count for spaces
                            if let Some(c) = info.children_count {
                                if c > 0 {
                                    info_parts.push(format!(
                                        "~{} {}",
                                        c,
                                        if c == 1 { "room" } else { "rooms" }
                                    ));
                                }
                            }

                            // Add topic if available (Label handles truncation via wrap: Ellipsis)
                            if let Some(topic) = &info.topic {
                                info_parts.push(topic.to_string());
                            }

                            info_label.set_text(cx, &info_parts.join("  |  "));

                            item
                        }
                        TreeEntry::Loading { level, parent_mask } => {
                            // Draw loading indicator for subspace
                            let item = list.item(cx, item_id, id!(subspace_loading));
                            // Configure tree lines
                            if let Some(mut lines) = item.widget(ids!(tree_lines)).borrow_mut::<TreeLines>() {
                                lines.draw_bg.level = *level as f32;
                                lines.draw_bg.is_last = 1.0; 
                                lines.draw_bg.parent_mask = *parent_mask as f32;
                                lines.draw_bg.indent_width = 44.0;
                            }
                            item
                        }
                    }
                } else {
                    list.item(cx, item_id, id!(bottom_filler))
                };
                item.draw_all(cx, scope);
            }
        }

        DrawStep::done()
    }
}

impl SpaceLobbyScreen {

    /// Handle receiving detailed children for a space.
    fn update_children_in_space(&mut self, cx: &mut Cx, space_id: &OwnedRoomId, children: &Vector<SpaceRoom>) {
        self.children_cache.insert(space_id.clone(), children.clone());
        self.loading_subspaces.remove(space_id);

        // If this is for our displayed space, mark as loaded and rebuild tree
        if self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == space_id) {
            self.is_loading = false;
            // Auto-expand the top-level space (we don't show it, just its children)
            self.expanded_spaces.insert(space_id.clone());
        }

        self.rebuild_tree_entries();
        self.redraw(cx);
    }

    /// Toggle the expansion state of a space.
    fn toggle_space_expansion(&mut self, cx: &mut Cx, space_id: &OwnedRoomId) {
        if self.expanded_spaces.contains(space_id) {
            self.expanded_spaces.remove(space_id);
            self.loading_subspaces.remove(space_id);
        } else {
            self.expanded_spaces.insert(space_id.clone());

            // Request children if we don't have them yet
            if !self.children_cache.contains_key(space_id) {
                self.loading_subspaces.insert(space_id.clone());
                if let Some(sender) = &self.space_request_sender {
                    let parent_chain = cx.get_global::<RoomsListRef>()
                        .get_space_parent_chain(space_id)
                        .unwrap_or_default();
                    let _ = sender.send(SpaceRequest::GetDetailedChildren {
                        space_id: space_id.clone(),
                        parent_chain,
                    });
                }
            }
        }

        self.rebuild_tree_entries();
        self.redraw(cx);
    }

    /// Rebuild the flattened tree entries based on the current expansion state.
    fn rebuild_tree_entries(&mut self) {
        let Some(space_name_id) = &self.space_name_id else { return };
        let root_space_id = space_name_id.room_id().clone();
        // Build tree starting from root
        let mut new_tree_entries = Vec::new();
        Self::build_tree_for_space(
            &self.children_cache,
            &self.expanded_spaces,
            &self.loading_subspaces,
            &mut new_tree_entries,
            &root_space_id,
            0,
            0,
        );
        self.tree_entries = new_tree_entries;
    }

    /// Recursively build the tree of spaces and their expanded children such that they
    /// can be displayed in the SpaceLobbyScreen's PortalList.
    //
    // Note: this is intentionally *not* a method (it doesn't take &mut self),
    // in order to make it possible to recursively call it while immutably borrowing
    // only select fields of `Self`.
    fn build_tree_for_space(
        children_cache: &HashMap<OwnedRoomId, Vector<SpaceRoom>>,
        expanded_spaces: &HashSet<OwnedRoomId>,
        loading_subspaces: &HashSet<OwnedRoomId>,
        tree_entries: &mut Vec<TreeEntry>,
        space_id: &OwnedRoomId,
        level: usize,
        parent_mask: u32,
    ) {
        let Some(children) = children_cache.get(space_id) else { return };

        // Sort: spaces first, then rooms, both alphabetically
        let mut sorted_children: Vec<_> = children.iter().collect();
        sorted_children.sort_by(|a, b| {
            match (a.is_space(), b.is_space()) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()),
            }
        });

        
        let count = sorted_children.len();
        for (i, child) in sorted_children.into_iter().enumerate() {
            let is_last = i == count - 1;
            
            tree_entries.push(TreeEntry::Item {
                info: SpaceRoomInfo::from(child),
                level,
                is_last,
                parent_mask,
            });

            // If this is an expanded space, recursively add its children or a loading indicator
            if child.is_space() && expanded_spaces.contains(&child.room_id) {
                // Calculate mask for children:
                // If we are NOT the last child, our level needs a continuation line for our children.
                // If we ARE the last child, our level does NOT need a line.
                // Parent levels are preserved.
                let child_mask = if is_last {
                    parent_mask
                } else {
                    parent_mask | (1 << level)
                };

                if children_cache.contains_key(&child.room_id) {
                    Self::build_tree_for_space(
                        children_cache,
                        expanded_spaces,
                        loading_subspaces,
                        tree_entries,
                        &child.room_id,
                        level + 1,
                        child_mask,
                    );
                } else if loading_subspaces.contains(&child.room_id) {
                    // Show loading indicator
                    tree_entries.push(TreeEntry::Loading { 
                        level: level + 1,
                        parent_mask: child_mask,
                    });
                }
            }
        }
    }

    /// Saves the current UI state to the cache. Call this when the screen is being hidden.
    pub fn save_current_state(&mut self) {
        if let Some(current_space) = &self.space_name_id {
            SPACE_LOBBY_STATES.with_borrow_mut(|states| {
                states.insert(
                    current_space.room_id().clone(),
                    SpaceLobbyUiState {
                        expanded_spaces: self.expanded_spaces.clone(),
                    },
                );
            });
        }
    }

    pub fn set_displayed_space(&mut self, cx: &mut Cx, space_name_id: &RoomNameId) {
        let space_name = space_name_id.to_string();
        let parent_name = self.view.label(ids!(header.parent_space_row.parent_name));
        parent_name.set_text(cx, &space_name);

        // If this space is already being displayed, then the only thing we may need to do
        // is update its name in the top-level header (already done above).
        if self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == space_name_id.room_id()) {
            return;
        }

        // Save the current UI state before switching to a new space
        self.save_current_state();

        self.space_name_id = Some(space_name_id.clone());
        let rooms_list_ref = cx.get_global::<RoomsListRef>();
        if let Some(sender) = rooms_list_ref.get_space_request_sender() {
            // Request detailed children for this space so we can start populating it.
            let parent_chain_opt = rooms_list_ref.get_space_parent_chain(space_name_id.room_id());
            let _ = sender.send(SpaceRequest::GetDetailedChildren {
                space_id: space_name_id.room_id().clone(),
                parent_chain: parent_chain_opt.unwrap_or_default(),
            });
            self.space_request_sender = Some(sender);
        }

        self.tree_entries.clear();
        self.is_loading = true;

        // Restore UI state if we've viewed this space before, otherwise start fresh
        self.expanded_spaces = SPACE_LOBBY_STATES.with_borrow(|states| {
            states
                .get(space_name_id.room_id())
                .map(|state| state.expanded_spaces.clone())
                .unwrap_or_default()
        });

        // TODO: move avatar setting to `draw_walk()`
        // Set parent avatar
        let avatar_ref = self.view.avatar(ids!(header.parent_space_row.parent_avatar));
        let first_char = utils::user_name_first_letter(&space_name);
        avatar_ref.show_text(cx, None, None, first_char.unwrap_or("#"));
        self.redraw(cx);
    }
}

impl SpaceLobbyScreenRef {
    pub fn set_displayed_space(&self, cx: &mut Cx, space_name_id: &RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_displayed_space(cx, space_name_id);
    }

    /// Saves the current UI state. Call this when the screen is being hidden or destroyed.
    pub fn save_current_state(&self) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.save_current_state();
    }
}
