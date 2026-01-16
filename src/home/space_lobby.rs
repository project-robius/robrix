//! Contains two widgets related to the top-level view of a space.
//!
//! 1. `SpaceLobby`: shows details about a space, including its name, avatar,
//!    members, topic, and the full list of rooms and subspaces within it.
//! 2. `SpaceLobbyEntry`: the button that can be shown in a RoomsList
//!    that allows the user to click on it to show the `SpaceLobby`.
//!

use std::{collections::{HashMap, HashSet}, sync::Arc};
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use tokio::sync::mpsc::UnboundedSender;
use crate::{
    home::rooms_list::RoomsListRef,
    shared::avatar::{AvatarWidgetExt, AvatarWidgetRefExt},
    space_service_sync::{ParentChain, SpaceRequest, SpaceRoomInfo, SpaceRoomListAction},
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
        margin: {top: 5, bottom: 10}
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
    DrawTreeLine = {{DrawTreeLine}} {
        
    }

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
                let half_indent = indent * 0.5;
                let line_width = 1.0;
                let half_line = 0.5;

                let c = vec4(0.0);

                // Iterate for parent levels and current level
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

        // Tree lines replace the spacer
        tree_lines = <TreeLines> {}

        // Expand/collapse icon
        expand_icon = <IconRotated> {
            width: 16,
            height: 16,
            margin: { right: 4 }
            draw_icon: {
                svg_file: (ICON_COLLAPSE)
                rotation_angle: 90.0
                color: #888
            }
            icon_walk: { width: 10, height: 10 }
        }

        avatar = <Avatar> { width: 32, height: 32, margin: {right: 12} }

        content = <View> {
            width: Fill, height: Fit, flow: Down, spacing: 0,
            name_label = <Label> {
                width: Fill, height: Fit,
                draw_text: { text_style: <REGULAR_TEXT>{font_size: 10.5}, color: #1a1a1a, wrap: Ellipsis }
            }
            info_label = <Label> {
                width: Fill, height: Fit,
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

    // Entry for a child room (leaf node, no expand icon)
    pub RoomEntry = {{RoomEntry}} {
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

        // Tree lines replace the spacer
        tree_lines = <TreeLines> {}

        avatar = <Avatar> { width: 32, height: 32, margin: {right: 12} }

        content = <View> {
            width: Fill, height: Fit, flow: Down, spacing: 0,
            name_label = <Label> {
                width: Fill, height: Fit,
                draw_text: { text_style: <REGULAR_TEXT>{font_size: 10.5}, color: #1a1a1a, wrap: Ellipsis }
            }
            info_label = <Label> {
                width: Fill, height: Fit,
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
                color: #999
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
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        // No interaction needed
        let _ = event;
        let _ = cx;
    }

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
    #[rust] room_id: Option<OwnedRoomId>,
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
    Clicked { room_id: OwnedRoomId },
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
                if let Some(room_id) = &self.room_id {
                    cx.widget_action(self.widget_uid(), &scope.path, 
                        SubspaceEntryAction::Clicked { room_id: room_id.clone() });
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
                if let Some(room_id) = &self.room_id {
                    cx.widget_action(self.widget_uid(), &scope.path,
                        RoomEntryAction::Clicked { room_id: room_id.clone() });
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


/// An entry in the flattened tree to be displayed.
#[derive(Clone, Debug)]
enum TreeEntry {
    /// A regular space or room entry.
    Item {
        /// The detailed info about this space or room.
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
    #[rust] children_cache: HashMap<OwnedRoomId, Arc<Vec<SpaceRoomInfo>>>,

    /// The set of space IDs that are currently expanded (showing their children).
    #[rust] expanded_spaces: HashSet<OwnedRoomId>,

    /// The flattened list of tree entries to display.
    #[rust] tree_entries: Vec<TreeEntry>,

    /// The set of space IDs that are currently loading their children.
    #[rust] loading_subspaces: HashSet<OwnedRoomId>,

    /// Whether we are currently loading the initial data.
    #[rust] is_loading: bool,
}

impl Widget for SpaceLobbyScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Listen for actions from the space service.
        if let Event::Actions(actions) = event {
            for action in actions {
                // Handle detailed children response
                if let Some(SpaceRoomListAction::DetailedChildren { space_id, children, .. }) = action.downcast_ref() {
                    self.handle_detailed_children(cx, space_id, children);
                }

                // Handle SubspaceEntry clicks
                if let SubspaceEntryAction::Clicked { room_id } = action.as_widget_action().cast() {
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
                let mut item_scope = Scope::empty();

                if self.is_loading && item_id == 0 {
                    // Draw loading indicator
                    let item = list.item(cx, item_id, id!(status_label));
                    item.label(ids!(label)).set_text(cx, "Loading rooms and spaces...");
                    item.draw_all(cx, &mut item_scope);
                } else if entry_count == 0 && item_id == 0 {
                    // No entries found
                    let item = list.item(cx, item_id, id!(status_label));
                    item.label(ids!(label)).set_text(cx, "No rooms or spaces found.");
                    item.view(ids!(loading_spinner)).apply_over(cx, live! { visible: false });
                    item.draw_all(cx, &mut item_scope);
                } else if let Some(entry) = self.tree_entries.get(item_id) {
                    match entry {
                        TreeEntry::Item { info, level, is_last, parent_mask } => {

                            
                            if info.is_space {
                                let item = list.item(cx, item_id, id!(subspace_entry));
                                if let Some(mut inner) = item.borrow_mut::<SubspaceEntry>() {
                                    inner.room_id = Some(info.room_id.clone());
                                }
                                
                                // Configure tree lines
                                if let Some(mut lines) = item.widget(ids!(tree_lines)).borrow_mut::<TreeLines>() {
                                    lines.draw_bg.level = *level as f32;
                                    lines.draw_bg.is_last = if *is_last { 1.0 } else { 0.0 };
                                    lines.draw_bg.parent_mask = *parent_mask as f32;
                                    lines.draw_bg.indent_width = 44.0; // Hardcoded to match
                                }
                                
                                // Expand icon
                                let is_expanded = self.expanded_spaces.contains(&info.room_id);
                                let angle = if is_expanded { 180.0 } else { 90.0 };
                                item.icon(ids!(expand_icon)).apply_over(cx, live! {
                                    draw_icon: { rotation_angle: (angle) }
                                });
                                
                                // Avatar
                                let avatar_ref = item.avatar(ids!(avatar));
                                let first_char = utils::user_name_first_letter(&info.display_name);
                                avatar_ref.show_text(cx, None, None, first_char.as_deref().unwrap_or("#"));
                                
                                // Text
                                item.label(ids!(content.name_label)).set_text(cx, &info.display_name);
                                let info_text = if info.children_count > 0 {
                                    format!("{} members · {} rooms", info.num_joined_members, info.children_count)
                                } else {
                                    format!("{} members", info.num_joined_members)
                                };
                                item.label(ids!(content.info_label)).set_text(cx, &info_text);
                                
                                item.draw_all(cx, &mut item_scope);
                            } else {
                                let item = list.item(cx, item_id, id!(room_entry));
                                if let Some(mut inner) = item.borrow_mut::<RoomEntry>() {
                                    inner.room_id = Some(info.room_id.clone());
                                }
                                
                                // Configure tree lines
                                if let Some(mut lines) = item.widget(ids!(tree_lines)).borrow_mut::<TreeLines>() {
                                    lines.draw_bg.level = *level as f32;
                                    lines.draw_bg.is_last = if *is_last { 1.0 } else { 0.0 };
                                    lines.draw_bg.parent_mask = *parent_mask as f32;
                                    lines.draw_bg.indent_width = 44.0;
                                }
                                
                                // Avatar
                                let avatar_ref = item.avatar(ids!(avatar));
                                let first_char = utils::user_name_first_letter(&info.display_name);
                                avatar_ref.show_text(cx, None, None, first_char.as_deref().unwrap_or("#"));
                                
                                // Text
                                item.label(ids!(content.name_label)).set_text(cx, &info.display_name);
                                let info_text = format!("{} members", info.num_joined_members);
                                item.label(ids!(content.info_label)).set_text(cx, &info_text);
                                
                                item.draw_all(cx, &mut item_scope);
                            }
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

                            item.draw_all(cx, &mut item_scope);
                        }
                    }
                } else {
                    // Bottom filler
                    let item = list.item(cx, item_id, id!(bottom_filler));
                    item.draw_all(cx, &mut item_scope);
                }
            }
        }

        DrawStep::done()
    }
}

impl SpaceLobbyScreen {

    /// Handle receiving detailed children for a space.
    fn handle_detailed_children(&mut self, cx: &mut Cx, space_id: &OwnedRoomId, children: &Arc<Vec<SpaceRoomInfo>>) {
        // Store in cache and remove from loading set
        self.children_cache.insert(space_id.clone(), Arc::clone(children));
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
                    let _ = sender.send(SpaceRequest::GetDetailedChildren {
                        space_id: space_id.clone(),
                        parent_chain: ParentChain::new(),
                    });
                }
            }
        }

        self.rebuild_tree_entries();
        self.redraw(cx);
    }

    /// Rebuild the flattened tree entries based on the current expansion state.
    fn rebuild_tree_entries(&mut self) {
        self.tree_entries.clear();

        let Some(space_name_id) = &self.space_name_id else { return };
        let root_space_id = space_name_id.room_id().clone();

        // Build tree starting from root
        self.build_tree_for_space(&root_space_id, 0, 0);
    }

    /// Build tree entries for a space and its expanded children.
    fn build_tree_for_space(&mut self, space_id: &OwnedRoomId, level: usize, parent_mask: u32) {
        let children = match self.children_cache.get(space_id) {
            Some(c) => Arc::clone(c),
            None => return,
        };

        // Sort: spaces first, then rooms, both alphabetically
        let mut sorted_children: Vec<_> = children.iter().collect();
        sorted_children.sort_by(|a, b| {
            match (a.is_space, b.is_space) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()),
            }
        });

        
        let count = sorted_children.len();
        for (i, child) in sorted_children.iter().enumerate() {
            let is_last = i == count - 1;
            
            self.tree_entries.push(TreeEntry::Item {
                info: (*child).clone(),
                level,
                is_last,
                parent_mask,
            });

            // If this is an expanded space, recursively add its children or a loading indicator
            if child.is_space && self.expanded_spaces.contains(&child.room_id) {
                // Calculate mask for children:
                // If we are NOT the last child, our level needs a continuation line for our children.
                // If we ARE the last child, our level does NOT need a line.
                // Parent levels are preserved.
                let child_mask = if is_last {
                    parent_mask
                } else {
                    parent_mask | (1 << level)
                };

                if self.children_cache.contains_key(&child.room_id) {
                    self.build_tree_for_space(&child.room_id, level + 1, child_mask);
                } else if self.loading_subspaces.contains(&child.room_id) {
                    // Show loading indicator
                    self.tree_entries.push(TreeEntry::Loading { 
                        level: level + 1,
                        parent_mask: child_mask,
                    });
                }
            }
        }
    }

    pub fn set_displayed_space(&mut self, cx: &mut Cx, space_name_id: &RoomNameId) {
        // If this space is already being displayed, then do nothing.
        if self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == space_name_id.room_id()) {
            return;
        }

        // Clear previous state
        self.tree_entries.clear();
        self.expanded_spaces.clear();
        self.is_loading = true;

        // Set the new space
        self.space_name_id = Some(space_name_id.clone());

        // Update parent space header
        let space_name = space_name_id.to_string();
        self.view.label(ids!(header.parent_space_row.parent_name)).set_text(cx, &space_name);
        
        // Set parent avatar
        let avatar_ref = self.view.avatar(ids!(header.parent_space_row.parent_avatar));
        let first_char = utils::user_name_first_letter(&space_name);
        avatar_ref.show_text(cx, None, None, first_char.as_deref().unwrap_or("#"));

        // Request detailed children for this space
        if let Some(sender) = &self.space_request_sender {
            let _ = sender.send(SpaceRequest::GetDetailedChildren {
                space_id: space_name_id.room_id().clone(),
                parent_chain: ParentChain::new(),
            });
        }

        self.redraw(cx);
    }

    /// Set the space request sender channel.
    pub fn set_space_request_sender(&mut self, sender: UnboundedSender<SpaceRequest>) {
        self.space_request_sender = Some(sender);
    }
}

impl SpaceLobbyScreenRef {
    pub fn set_displayed_space(&self, cx: &mut Cx, space_name_id: &RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else { return };

        // Get the space request sender from RoomsList via Cx global if needed
        if inner.space_request_sender.is_none() {
            if let Some(sender) = cx.get_global::<RoomsListRef>().get_space_request_sender() {
                inner.space_request_sender = Some(sender);
            }
        }

        inner.set_displayed_space(cx, space_name_id);
    }
}
