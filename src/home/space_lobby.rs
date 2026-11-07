//! Contains two widgets related to the top-level view of a space.
//!
//! 1. `SpaceLobby`: shows details about a space, including its name, avatar,
//!    members, topic, and the full list of rooms and subspaces within it.
//! 2. `SpaceLobbyEntry`: the button that can be shown in a RoomsList
//!    that allows the user to click on it to show the `SpaceLobby`.
//!

use std::cell::RefCell;
use std::fmt::Write as _;
use std::collections::BTreeMap;
use imbl::Vector;
use makepad_widgets::*;
use makepad_widgets::animator::Animate;
use matrix_sdk::{RoomDisplayName, RoomState, ruma::OwnedRoomId};
use matrix_sdk_ui::spaces::SpaceRoom;
use ruma::room::JoinRuleSummary;
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::mpsc::UnboundedSender;
use crate::shared::avatar::{AvatarImage, AvatarState};
use crate::shared::expand_arrow::ExpandArrow;
use crate::shared::hover_highlight::handle_hover_hit_with_test;
/// The horizontal indent width (in pixels) per tree level.
const TREE_INDENT_WIDTH: f64 = 44.0;

use crate::{
    app::AppStateAction,
    avatar_cache::{self, AvatarCacheEntry},
    home::{
        invite_modal::InviteModalAction,
        leave_space_modal::LeaveSpaceModalAction,
        rooms_list::RoomsListRef,
        space_tree::{SpaceRoomInfo, TreeEntry, TreeFilter, build_filtered_tree, build_tree_for_space, set_tree_row_position},
    },
    join_leave_room_modal::{JoinLeaveModalKind, JoinLeaveRoomModalAction},
    profile::user_profile::UserProfileSlidingPaneWidgetExt,
    room::{
        BasicRoomDetails,
        pane_dock::{RoomPaneDockAction, RoomPaneDockWidgetExt, RoomPaneDockWidgetRefExt, SavedRoomPane},
        room_members_list::{RoomMembersChanged, RoomMembersListAction, show_member_profile},
        room_pane::RoomPaneKind,
    },
    shared::{
        avatar::{AvatarWidgetExt, AvatarWidgetRefExt},
        collapsible_html::handle_link_clicked,
        room_filter_input_bar::RoomFilterInputBarWidgetExt,
        styles::apply_lavender_button_style,
    },
    sliding_sync::{MatrixRequest, UserPowerLevelsUpdated, submit_async_request},
    space_service_sync::{SpaceRequest, SpaceRoomListAction},
    utils::{self, RoomNameId},
};


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // An entry in the RoomsList that will show the SpaceLobby when clicked.
    mod.widgets.SpaceLobbyEntry = #(SpaceLobbyEntry::register_widget(vm)) {
        visible: false, // only visible when a space is selected
        width: Fill,
        height: Fit,
        flow: Right,
        align: Align{y: 0.5}
        padding: 5,
        margin: Inset{top: 10, bottom: 0}
        spacing: 5
        cursor: MouseCursor.Hand

        show_bg: true
        draw_bg +: {
            hover: instance(0.0)
            active: instance(0.0)

            color: instance(COLOR_NAVIGATION_TAB_BG)
            color_hover: instance(COLOR_NAVIGATION_TAB_BG_HOVER)
            color_active: instance(COLOR_ACTIVE_PRIMARY)
            border_size: uniform(0.0)
            border_color: instance(COLOR_TRANSPARENT)
            border_radius: uniform(4.0)
            border_inset: uniform(vec4(0.0))

            get_color: fn() -> vec4 {
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

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(
                    self.border_inset.x + self.border_size,
                    self.border_inset.y + self.border_size,
                    self.rect_size.x - (self.border_inset.x + self.border_inset.z + self.border_size * 2.0),
                    self.rect_size.y - (self.border_inset.y + self.border_inset.w + self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.border_color, self.border_size)
                }
                return sdf.result;
            }
        }

        icon := Icon {
            width: 25,
            height: 25,
            margin: Inset{left: 5, right: 3}
            align: Align{x: 0.5, y: 0.5}
            draw_icon +: {
                svg: (ICON_HIERARCHY)

                active: instance(0.0)
                hover: instance(0.0)
                down: instance(0.0)

                color: (COLOR_TEXT)
                color_hover: instance(COLOR_TEXT)
                color_active: instance(COLOR_SPACE_LOBBY_ENTRY_TEXT_ACTIVE)

                get_color: fn() -> vec4 {
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
            icon_walk: Walk{ width: 25, height: 20, margin: Inset{top: 2} }
        }

        space_lobby_label := Label {
            width: Fill, height: Fit
            flow: Flow.Right { wrap: true },
            padding: 0,

            draw_text +: {
                active: instance(0.0)
                hover: instance(0.0)
                down: instance(0.0)

                color: (COLOR_TEXT)
                color_hover: instance(COLOR_TEXT)
                color_active: instance(COLOR_SPACE_LOBBY_ENTRY_TEXT_ACTIVE)

                text_style: REGULAR_TEXT {font_size: 11},

                get_color: fn() -> vec4 {
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

        animator: Animator{
            bg_hover: {
                default: @off
                off: AnimatorState{
                    from: {all: Forward {duration: 0.15}}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 0.0}
                        space_lobby_label: { draw_text: {down: [{time: 0.0, value: 0.0}], hover: 0.0} }
                        icon: { draw_icon: {down: [{time: 0.0, value: 0.0}], hover: 0.0} }
                    }
                }
                on: AnimatorState{
                    from: {all: Snap}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 1.0}
                        space_lobby_label: { draw_text: {down: [{time: 0.0, value: 0.0}], hover: 1.0} }
                        icon: { draw_icon: {down: [{time: 0.0, value: 0.0}], hover: 1.0} }
                    }
                }
                down: AnimatorState{
                    from: {all: Forward {duration: 0.2}}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 1.0}], hover: 1.0,}
                        space_lobby_label: { draw_text: {down: [{time: 0.0, value: 1.0}], hover: 1.0,} }
                        icon: { draw_icon: {down: [{time: 0.0, value: 1.0}], hover: 1.0,} }
                    }
                }
            }
        }
    }

    // Entry for a child subspace (can be expanded)
    mod.widgets.SubspaceEntry = #(SubspaceEntry::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Overlay
        align: Align{x: 1.0, y: 0.5}

        show_bg: true
        draw_bg +: {
            hover: instance(0.0)
            color: instance(COLOR_PRIMARY)
            color_hover: instance(COLOR_LIST_ITEM_BG_HOVER)
            pixel: fn() {
                return mix(self.color, self.color_hover, self.hover);
            }
        }

        main_entry := View {
            width: Fill,
            height: Fit,
            flow: Right
            align: Align{x: 0, y: 0.5}
            padding: Inset{top: 8, bottom: 8, left: 8, right: 12}
            cursor: MouseCursor.Hand

            // Invisible spacer whose width is set dynamically to match
            // the tree indent level, replacing tree_lines' layout role.
            indent_spacer := View { width: 0, height: Fit }

            // Expand/collapse arrow (animated triangle)
            expand_icon := mod.widgets.ExpandArrow {
                width: 16,
                height: 16,
                margin: Inset{ left: -6, right: 4 }
                draw_bg.color: (COLOR_TEXT_FAINT)
                draw_bg.border_radius: 1.5 // less rounded
            }

            avatar := Avatar { width: 32, height: 32, margin: Inset{right: 8} }

            content := View {
                width: Fill
                height: Fit
                flow: Down
                align: Align { y: 0.5 }
                spacing: 5,

                name_label := Label {
                    width: Fill, height: Fit,
                    margin: 0
                    padding: 0
                    flow: Flow.Right{wrap: true}
                    max_lines: 2
                    text_overflow: Ellipsis
                    draw_text +: { text_style: REGULAR_TEXT {font_size: 10.5}, color: (COLOR_SPACE_LOBBY_NAME_TEXT) }
                }

                suggested_tag := RoundedView {
                    visible: false
                    width: Fit, height: Fit,
                    padding: Inset { left: 6, right: 6, top: 3, bottom: 3 }
                    show_bg: true
                    draw_bg +: {
                        color: (COLOR_BG_INFO_BLUE)
                        border_radius: 3.0
                        border_size: 0.75
                        border_color: (COLOR_INFO_BLUE)
                    }
                    suggested_label := Label {
                        padding: 0
                        margin: 0
                        width: Fit, height: Fit,
                        text: "Suggested"
                        draw_text +: { text_style: REGULAR_TEXT {font_size: 8.5}, color: (COLOR_FG_INFO_BLUE) }
                    }
                }

                info_label := Label {
                    width: Fill, height: Fit,
                    margin: 0
                    padding: 0
                    flow: Flow.Right{wrap: true}
                    max_lines: 2
                    text_overflow: Ellipsis
                    draw_text +: { text_style: REGULAR_TEXT {font_size: 8.5}, color: (COLOR_TEXT_SUBTLE) }
                }
            }
        }

        buttons_view := RoundedView {
            visible: false
            width: Fit,
            height: Fit,
            flow: Right,
            spacing: 8,
            padding: Inset { left: 8, right: 8, top: 4, bottom: 4 }
            align: Align{x: 1.0, y: 0.5}
            margin: Inset{right: 16}

            show_bg: true
            draw_bg +: {
                color: (COLOR_LIST_ITEM_BG_HOVER)
                border_radius: 4.0
                border_size: 0
            }

            join_button := RobrixPositiveIconButton {
                width: Fit,
                padding: 8
                spacing: 0
                icon_walk: Walk{width: 0, height: 0}
                draw_text.text_style: REGULAR_TEXT {font_size: 9.5}
                text: "Join"
            }

            view_button := RobrixIconButton {
                width: Fit,
                padding: 8
                spacing: 0
                icon_walk: Walk{width: 0, height: 0}
                draw_text.text_style: REGULAR_TEXT {font_size: 9.5}
                text: "View"
            }

            leave_button := RobrixNegativeIconButton {
                width: Fit,
                padding: 8
                spacing: 0
                icon_walk: Walk{width: 0, height: 0}
                draw_text.text_style: REGULAR_TEXT {font_size: 9.5}
                text: "Leave"
            }
        }

        // The connecting hierarchical lines on the left, placed last in
        // the Overlay so the parent's Fit height (from main_entry) is
        // already resolved when tree_lines is laid out.
        tree_lines := mod.widgets.TreeLines {}

        animator: Animator{
            bg_hover: {
                default: @off
                off: AnimatorState{ from: {all: Forward {duration: 0.1}}, apply: { draw_bg: {hover: 0.0} } }
                on: AnimatorState{ from: {all: Snap}, apply: { draw_bg: {hover: 1.0} } }
            }
        }
    }

    // Entry for a child room within a space, which cannot be expanded.
    mod.widgets.RoomEntry = mod.widgets.SubspaceEntry {
        main_entry +: {
            cursor: MouseCursor.Default
            expand_icon := View {
                width: 10
                height: 16
            }
        }
    }

    mod.widgets.SpaceLobbyStatusLabel = View {
        width: Fill, height: Fit,
        flow: Right,
        align: Align{ x: 0.5, y: 0.5 }
        padding: 20.0,

        loading_spinner := LoadingSpinner {
            width: 18,
            height: 18,
            margin: Inset{right: 10}
            draw_bg +: {
                color: (COLOR_ACTIVE_PRIMARY)
                stroke_width: 2.5
            }
        }

        label := Label {
            padding: 0
            width: Fit{max: FitBound.Rel{base: Base.Line, factor: 1.0}},
            text_overflow: Ellipsis,
            flow: Flow.Right{wrap: true},
            align: Align{ x: 0, y: 0 }
            draw_text +: {
                color: (COLOR_TEXT_SUBTLE),
                text_style: REGULAR_TEXT {font_size: 10}
            }
            text: "Loading rooms and spaces..."
        }
    }

    // Small loading indicator shown inline when loading subspace children.
    // Uses the same Overlay + spacer pattern as SubspaceEntry so tree lines
    // span the full row height and the content is indented correctly.
    mod.widgets.SubspaceLoadingEntry = View {
        width: Fill, height: 36,
        flow: Overlay,
        align: Align{ x: 0, y: 0.5 }

        loading_content := View {
            width: Fill, height: Fit,
            flow: Right,
            align: Align{ x: 0, y: 0.5 }
            padding: Inset{left: 8, right: 12}

            // Spacer for tree indent (width set dynamically in draw_item)
            indent_spacer := View { width: 0, height: Fit }

            loading_spinner := LoadingSpinner {
                width: 14,
                height: 14,
                margin: Inset{left: 10, right: 4}
                draw_bg +: {
                    color: (COLOR_ACTIVE_PRIMARY)
                    stroke_width: 2.0
                }
            }

            label := Label {
                width: Fit,
                height: Fit,
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 9},
                    color: (COLOR_TEXT_FAINT),
                }
                text: "Loading..."
            }
        }

        // Tree lines drawn last so parent height is resolved
        tree_lines := mod.widgets.TreeLines {}
    }

    // The main view that shows the lobby (homepage) for a space.
    mod.widgets.SpaceLobbyScreen = set_type_default() do #(SpaceLobbyScreen::register_widget(vm)) {
        ..mod.widgets.SolidView

        width: Fill, height: Fill,
        flow: Down,

        show_bg: true
        draw_bg +: {
            color: COLOR_PRIMARY
        }

        // Header with parent space info. Its content wraps onto more lines rather than shrinking.
        header := SolidView {
            width: Fill,
            height: Fit,
            flow: Down,
            spacing: 10,
            padding: Inset{left: 16, right: 16, top: 12, bottom: 10}

            show_bg: true,
            draw_bg.color: (COLOR_BG_PREVIEW)

            // The buttons stay on the right side while they fit beside the space's name.
            parent_space_row := View {
                width: Fill,
                height: Fit,
                flow: Flow.Right { wrap: true, row_align: RowAlign.Center },
                distribute: Distribute.SpaceBetween,
                spacing: 16,
                wrap_spacing: 10,

                parent_space_title := View {
                    width: Fit,
                    height: Fit,
                    flow: Right,
                    spacing: 12,
                    align: Align{ y: 0.5 }

                    parent_avatar := Avatar {
                        width: 36,
                        height: 36,
                    }

                    parent_space_info := View {
                        width: Fit,
                        height: Fit,
                        flow: Down,
                        spacing: 3,

                        parent_name := Label {
                            width: Fit{max: FitBound.Rel{base: Base.Line, factor: 1.0}},
                            height: Fit,
                            padding: 0,
                            flow: Flow.Right { wrap: true },
                            text_overflow: Ellipsis,
                            draw_text +: {
                                text_style: TITLE_TEXT {font_size: 14},
                                color: (COLOR_SPACE_LOBBY_NAME_TEXT),
                            }
                            text: ""
                        }

                        space_info_label := Label {
                            width: Fit{max: FitBound.Rel{base: Base.Line, factor: 1.0}},
                            height: Fit,
                            padding: 0,
                            flow: Flow.Right { wrap: true },
                            text_overflow: Ellipsis,
                            draw_text +: {
                                text_style: REGULAR_TEXT {font_size: 10},
                                color: (COLOR_TEXT_SUBTLE),
                            }
                            text: ""
                        }
                    }
                }

                // Only a definite width can wrap, so this is just wide enough for all of these buttons.
                parent_space_buttons := View {
                    width: "min(282px, 100%)",
                    height: Fit,
                    flow: Flow.Right { wrap: true },
                    spacing: 8,
                    wrap_spacing: 8,

                    members_button := RobrixNeutralIconButton {
                        width: 102
                        height: 35
                        align: Align{x: 0.5, y: 0.5}
                        padding: Inset{left: 12, right: 12}
                        draw_bg +: {
                            color: (COLOR_BG_LAVENDER)
                            color_hover: (COLOR_BG_LAVENDER_HOVER)
                            color_down: (COLOR_BG_LAVENDER_DOWN)
                        }
                        draw_icon +: {
                            svg: (ICON_MEMBERS)
                            color: (ROOM_NAME_TEXT_COLOR)
                        }
                        icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                        draw_text +: {
                            color: (ROOM_NAME_TEXT_COLOR)
                            color_hover: (ROOM_NAME_TEXT_COLOR)
                            color_down: (ROOM_NAME_TEXT_COLOR)
                        }
                        text: "Members"
                    }

                    invite_button := RobrixPositiveIconButton {
                        width: 82
                        height: 35
                        align: Align{x: 0.5, y: 0.5}
                        padding: Inset{left: 12, right: 12}
                        draw_icon.svg: (ICON_ADD_USER)
                        icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                        text: "Invite"
                    }

                    leave_button := RobrixNegativeIconButton {
                        width: 82
                        height: 35
                        align: Align{x: 0.5, y: 0.5}
                        padding: Inset{left: 12, right: 12}
                        draw_icon.svg: (ICON_LOGOUT)
                        icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                        text: "Leave"
                    }
                }
            }

            // The space's topic is shown using linkified HTML (links in topics are common).
            // Wrapped in a view so we can hide it.
            space_topic_view := View {
                visible: false,
                width: Fill, height: Fit,

                space_topic := mod.widgets.CollapsibleHtml {
                    html +: {
                        font_size: 10
                        font_color: (COLOR_TEXT_SUBTLE)
                    }
                }
            }

            // Filter input bar for searching rooms/spaces in this space
            filter_bar := mod.widgets.RoomFilterInputBar {
                input +: {
                    text_input +: { empty_text: "Filter this space..." }
                }
            }
        }

        View {
            width: Fill, height: Fill
            flow: Overlay

            // The hierarchical tree list, surrounded by any of this space's panes that are docked around it.
            room_pane_dock := mod.widgets.RoomPaneDock {
                body +: { mid +: { center +: {
                    tree_list := PortalList {
                        keep_invisible: false,
                        bounce_at_start: true,
                        bounce_at_end: true,
                        // Nothing here listens for scroll position changes.
                        emit_scroll_actions: false,
                        auto_tail: false,
                        width: Fill, height: Fill
                        flow: Down,
                        spacing: 0.0

                        subspace_entry := mod.widgets.SubspaceEntry {}
                        room_entry := mod.widgets.RoomEntry {}
                        subspace_loading := mod.widgets.SubspaceLoadingEntry {}
                        status_label := mod.widgets.SpaceLobbyStatusLabel {}
                        bottom_filler := View {
                            width: Fill,
                            height: 80.0,
                        }
                    }
                }}}
            }

            // Shown when clicking on a member, on top of the tree list and panes.
            user_profile_sliding_pane := mod.widgets.UserProfileSlidingPane { }
        }
    }
}


thread_local! {
    /// A cache of UI states for each SpaceLobbyScreen, keyed by the space's room ID.
    /// This allows preserving its rooms, expanded subspaces, and scroll position across screen changes.
    static SPACE_LOBBY_STATES: RefCell<BTreeMap<OwnedRoomId, SpaceLobbyUiState>> = const {
        RefCell::new(BTreeMap::new())
    };
}

/// The UI-side state of a SpaceLobbyScreen that should persist across hide/show cycles.
#[derive(Default)]
struct SpaceLobbyUiState {
    /// The details of the space that its header shows.
    space_details: Option<SpaceRoom>,
    room_panes: Vec<SavedRoomPane>,
    /// The set of space IDs that are currently expanded (showing their children).
    expanded_spaces: FxHashSet<OwnedRoomId>,
    /// The children of the space and of its expanded subspaces, so we can show them again right away.
    children: FxHashMap<OwnedRoomId, Vector<SpaceRoom>>,
    /// The first item shown in the tree list, and how far it was scrolled.
    first_id_and_scroll: (usize, f64),
}

/// Forgets the saved state of every space lobby, e.g., upon logout.
pub fn clear_space_lobby_states(_cx: &mut Cx) {
    SPACE_LOBBY_STATES.with_borrow_mut(BTreeMap::clear);
}


/// A clickable entry shown in the RoomsList that will show the space lobby when clicked.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct SpaceLobbyEntry {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
}

impl Widget for SpaceLobbyEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.visible {
            return;
        }
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        if let Event::ClearHover = event {
            self.animator_play(cx, ids!(bg_hover.off));
        }

        let area = self.draw_bg.area();
        match event.hits(cx, area) {
            Hit::FingerHoverIn(_) => {
                self.animator_play(cx, ids!(bg_hover.on));
            }
            Hit::FingerHoverOut(_) => {
                self.animator_play(cx, ids!(bg_hover.off));
            }
            Hit::FingerDown(_fe) => {
                self.animator_play(cx, ids!(bg_hover.down));
            }
            Hit::FingerLongPress(_lp) => {
                self.animator_play(cx, ids!(bg_hover.down));
            }
            Hit::FingerMove(fe) if !fe.is_over => {
                self.animator_play(cx, ids!(bg_hover.off));
            }
            Hit::FingerUp(fe) => {
                if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                    cx.action(SpaceLobbyAction::SpaceLobbyEntryClicked);
                }
                // A finger-up clears all hovers unless the mouse is still over us.
                self.animator_toggle(cx, fe.device.has_hovers() && fe.is_over, Animate::Yes, ids!(bg_hover.on), ids!(bg_hover.off));
            }
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

/// A clickable entry for a child subspace.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct SubspaceEntry {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    #[rust] room_id: Option<OwnedRoomId>,
    #[rust] is_space: bool,
    #[rust] show_buttons_view: bool,
    /// Whether `show_buttons_view` was set by a tap (touch) rather than mouse hover.
    /// On mobile (no hover events), tapping toggles button visibility;
    /// on desktop, hover handles it and taps fire the normal action.
    #[rust] buttons_shown_by_tap: bool,
    #[rust] is_expanded: bool,
}

/// Actions emitted when a `SubspaceEntry` or its buttons are clicked.
///
/// These *are* all widget actions.
#[derive(Clone, Debug, Default)]
pub enum SubspaceEntryAction {
    SpaceClicked { space_id: OwnedRoomId },
    RoomClicked  { room_id: OwnedRoomId },
    JoinClicked  { room_id: OwnedRoomId, is_space: bool },
    LeaveClicked { room_id: OwnedRoomId, is_space: bool },
    ViewClicked  { room_id: OwnedRoomId },
    #[default]
    None,
}

impl ActionDefaultRef for SubspaceEntryAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: SubspaceEntryAction = SubspaceEntryAction::None;
        &DEFAULT
    }
}

impl Widget for SubspaceEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        // NOTE: Use child_by_path instead of widget tree-based lookups
        //       (e.g., self.view.view(), self.view.button()) because these
        //       fail for portal list items.
        let buttons_view_rect = self.view.child_by_path(ids!(buttons_view)).area().clipped_rect(cx);
        let are_buttons_visible = self.show_buttons_view;
        let claim_before = event.pointer_claimed_area();

        // We exclude the buttons from the hit test so that they can handle their own hits.
        let hit = handle_hover_hit_with_test(
            self,
            cx,
            event,
            self.view.area(),
            claim_before,
            self.buttons_shown_by_tap,
            |abs, rect, _| rect.contains(abs) && !(are_buttons_visible && buttons_view_rect.contains(abs)),
        );

        // Show buttons based on the mouse hover (if they're not shown via a prior touch tap).
        if matches!(event, Event::MouseMove(_) | Event::ClearHover)
            && !self.buttons_shown_by_tap
        {
            let is_hovered = self.animator_in_state(cx, ids!(bg_hover.on));
            if is_hovered != self.show_buttons_view {
                self.show_buttons_view = is_hovered;
                self.view.child_by_path(ids!(buttons_view)).set_visible(cx, is_hovered);
                self.redraw(cx);
            }
        }

        match hit {
            Hit::FingerDown(fe) if !fe.device.is_touch() => {
                cx.set_key_focus(self.view.area());
            }
            Hit::FingerUp(fe) => {
                let is_tap = fe.is_over && fe.is_primary_hit() && fe.was_tap();
                let is_within_buttons_view = self.show_buttons_view
                    && self.view.child_by_path(ids!(buttons_view)).area().rect(cx).contains(fe.abs);
                if !is_tap {
                    // if it's not a tap, do nothing.
                }
                else if is_within_buttons_view {
                    // Let individual button handlers deal with taps on the buttons.
                }
                // On touch devices, tapping on the avatar or to its left
                // always expands/collapses a space (bypasses button toggle).
                else if fe.is_touch() && self.is_space {
                    let avatar_rect = self.view.child_by_path(ids!(main_entry.avatar)).area().rect(cx);
                    let tap_in_expand_region = fe.abs.x <= avatar_rect.pos.x + avatar_rect.size.x;
                    if tap_in_expand_region {
                        self.is_expanded = !self.is_expanded;
                        if let Some(mut arrow) = self.view.child_by_path(ids!(main_entry.expand_icon)).borrow_mut::<ExpandArrow>() {
                            arrow.set_is_open(cx, self.is_expanded, Animate::Yes);
                        }
                        if let Some(room_id) = self.room_id.as_ref() {
                            cx.widget_action(
                                self.widget_uid(),
                                SubspaceEntryAction::SpaceClicked { space_id: room_id.clone() },
                            );
                        }
                    } else {
                        // Touch tap on the text area: toggle buttons visibility.
                        self.toggle_buttons_for_tap(cx);
                    }
                }
                // On touch devices for rooms (not spaces): tap toggles buttons.
                else if fe.is_touch() {
                    self.toggle_buttons_for_tap(cx);
                }
                // Non-touch (desktop): fire the normal entry action,
                // since hover already handles button visibility.
                else if let Some(room_id) = self.room_id.as_ref() {
                    if self.is_space {
                        self.is_expanded = !self.is_expanded;
                        if let Some(mut arrow) = self.view.child_by_path(ids!(main_entry.expand_icon)).borrow_mut::<ExpandArrow>() {
                            arrow.set_is_open(cx, self.is_expanded, Animate::Yes);
                        }
                        cx.widget_action(
                            self.widget_uid(),
                            SubspaceEntryAction::SpaceClicked { space_id: room_id.clone() },
                        );
                    } else {
                        cx.widget_action(
                            self.widget_uid(),
                            SubspaceEntryAction::RoomClicked { room_id: room_id.clone() },
                        );
                    }
                }
                // A finger-up clears all hovers, unless the mouse is still over us (including the button view).
                if !self.buttons_shown_by_tap {
                    let still_over = fe.is_over || is_within_buttons_view;
                    self.animator_toggle(cx, fe.device.has_hovers() && still_over, Animate::Yes, ids!(bg_hover.on), ids!(bg_hover.off));
                }
            }
            _ => {}
        }

        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            let join_button = self.view.child_by_path(ids!(buttons_view.join_button)).as_button();
            let leave_button = self.view.child_by_path(ids!(buttons_view.leave_button)).as_button();
            let view_button = self.view.child_by_path(ids!(buttons_view.view_button)).as_button();

            if join_button.clicked(actions) {
                if let Some(room_id) = self.room_id.clone() {
                    join_button.reset_hover(cx);
                    cx.widget_action(
                        self.widget_uid(),
                        SubspaceEntryAction::JoinClicked { room_id, is_space: self.is_space },
                    );
                }
            }
            if leave_button.clicked(actions) {
                if let Some(room_id) = self.room_id.clone() {
                    leave_button.reset_hover(cx);
                    cx.widget_action(
                        self.widget_uid(),
                        SubspaceEntryAction::LeaveClicked { room_id, is_space: self.is_space },
                    );
                }
            }
            if view_button.clicked(actions) {
                if let Some(room_id) = self.room_id.clone() {
                    view_button.reset_hover(cx);
                    cx.widget_action(
                        self.widget_uid(),
                        SubspaceEntryAction::ViewClicked { room_id },
                    );
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SubspaceEntry {
    /// Clears the hover highlight and hides the buttons.
    /// Toggles the buttons_view visibility for a touch tap.
    fn toggle_buttons_for_tap(&mut self, cx: &mut Cx) {
        if self.show_buttons_view {
            self.animator_play(cx, ids!(bg_hover.off));
            self.show_buttons_view = false;
            self.buttons_shown_by_tap = false;
            self.view.child_by_path(ids!(buttons_view)).set_visible(cx, false);
            self.redraw(cx);
        } else {
            self.animator_play(cx, ids!(bg_hover.on));
            self.show_buttons_view = true;
            self.buttons_shown_by_tap = true;
            self.view.child_by_path(ids!(buttons_view)).set_visible(cx, true);
            self.redraw(cx);
        }
    }
}

/// The view showing the lobby/homepage for a given space.
#[derive(Script, Widget)]
pub struct SpaceLobbyScreen {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// The space that is currently being displayed.
    #[rust] space_name_id: Option<RoomNameId>,
    #[rust] space_avatar_state: AvatarState,
    #[rust] space_details: Option<SpaceRoom>,

    /// The sender channel to submit space requests to the background service.
    #[rust] space_request_sender: Option<UnboundedSender<SpaceRequest>>,

    /// Cache of detailed children for each space we've fetched.
    /// Key is the space_id, value is the list of its direct children.
    #[rust] children_cache: FxHashMap<OwnedRoomId, Vector<SpaceRoom>>,

    /// The set of space IDs that are currently expanded (showing their children).
    #[rust] expanded_spaces: FxHashSet<OwnedRoomId>,

    /// The ordered list of children to display in the space tree.
    #[rust] tree_entries: Vec<TreeEntry>,

    /// The set of space IDs that are currently loading their children.
    #[rust] loading_subspaces: FxHashSet<OwnedRoomId>,

    /// Whether we are currently loading the initial data.
    #[rust] is_loading: bool,

    /// Where to scroll the tree list to once the space's children have loaded.
    #[rust] pending_scroll: Option<(usize, f64)>,

    /// The current filter keywords entered by the user, if any.
    #[rust] filter_keywords: String,
}

impl Drop for SpaceLobbyScreen {
    fn drop(&mut self) {
        self.save_current_state();
    }
}

impl ScriptHook for SpaceLobbyScreen {
    fn on_after_apply(&mut self, vm: &mut ScriptVm, apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        if !apply.is_reload() {
            return;
        }
        // A reload resets the size and colors that we gave the header's buttons.
        vm.with_cx_mut(|cx| {
            self.fit_header_buttons(cx);
            let is_members_pane_shown = self.view.room_pane_dock(cx, ids!(room_pane_dock)).has_pane(&RoomPaneKind::Members);
            self.highlight_members_button(cx, is_members_pane_shown);
        });
    }
}

impl Widget for SpaceLobbyScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let profile_pane = self.view.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane));
        // Capture the actions from our child widgets so we can handle clicked links and members
        let mut inner_actions = cx.capture_actions(|cx| {
            // While the profile pane is shown, it gets all of the user's input.
            if profile_pane.is_currently_shown(cx) && utils::is_interactive_hit_event(event) {
                profile_pane.handle_event(cx, event, scope);
            } else {
                self.view.handle_event(cx, event, scope);
            }
        });
        inner_actions.retain(|action| {
            if let RoomMembersListAction::MemberClicked { room_name_id, member } = action.as_widget_action().cast() {
                show_member_profile(cx, &profile_pane, &room_name_id, member);
                // There's no timeline here in which to jump to a read receipt.
                profile_pane.button(cx, ids!(jump_to_read_receipt_button)).set_visible(cx, false);
                return false;
            }
            !handle_link_clicked(action)
        });
        cx.extend_actions(inner_actions);

        if let Event::Actions(actions) = event {
            let room_pane_dock_uid = self.view.room_pane_dock(cx, ids!(room_pane_dock)).widget_uid();
            for action in actions {
                // Highlight the members button while the members pane is shown.
                if let RoomPaneDockAction::ShownPanesChanged(kinds) = action.as_widget_action().widget_uid_eq(room_pane_dock_uid).cast() {
                    self.highlight_members_button(cx, kinds.contains(&RoomPaneKind::Members));
                    continue;
                }

                // Just like the rooms list header, handle updates to a space's name.
                if let Some(AppStateAction::RoomNameUpdated(new_room_name)) = action.downcast_ref()
                    && self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == new_room_name.room_id())
                {
                    self.set_displayed_space(cx, new_room_name);
                    self.redraw(cx);
                    continue;
                }

                if let Some(UserPowerLevelsUpdated { room_id, user_power_levels }) = action.downcast_ref()
                    && self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == room_id)
                {
                    self.view.button(cx, ids!(header.parent_space_row.parent_space_buttons.invite_button))
                        .set_visible(cx, user_power_levels.can_invite());
                    self.fit_header_buttons(cx);
                    self.redraw(cx);
                    continue;
                }
                // We may have been promoted or demoted in this space.
                if let Some(RoomMembersChanged { room_id }) = action.downcast_ref()
                    && self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == room_id)
                {
                    submit_async_request(MatrixRequest::GetRoomPowerLevels { room_id: room_id.clone() });
                    continue;
                }

                match action.downcast_ref() {
                    Some(SpaceRoomListAction::DetailedChildren { space_id, children, .. }) => {
                        self.update_children_in_space(cx, space_id, children);
                    }

                    // Handle receiving space details (join rule, member count).
                    Some(SpaceRoomListAction::SpaceDetails(sr))
                        if self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == &sr.room_id) =>
                    {
                        self.show_space_details(cx, sr);
                    }

                    // Handle a change to the set of children in this space or any of its child subspaces.
                    Some(SpaceRoomListAction::UpdatedChildren { space_id, parent_chain, .. })
                        if self.space_name_id.as_ref().is_some_and(|sni|
                            sni.room_id() == space_id
                            || parent_chain.iter().any(|ancestor_id| sni.room_id() == ancestor_id)
                        ) =>
                    {
                        if let Some(sender) = &self.space_request_sender {
                            let _ = sender.send(SpaceRequest::GetDetailedChildren {
                                space_name_id: self.known_space_name_id(space_id),
                                parent_chain: parent_chain.clone(),
                            });
                        }
                    }
                    _ => { }
                }

                // Handle SubspaceEntry clicks
                match action.as_widget_action().cast_ref() {
                    SubspaceEntryAction::SpaceClicked { space_id } => {
                        self.toggle_space_expansion(cx, space_id);
                    }
                    SubspaceEntryAction::RoomClicked { room_id: _ } => {
                        // TODO: highlight the room, such that on mobile devices
                        //       it will behave just like we hovered-in on desktop platforms.
                    }
                    SubspaceEntryAction::JoinClicked { room_id, is_space } => {
                        cx.action(JoinLeaveRoomModalAction::Open {
                            kind: JoinLeaveModalKind::JoinRoom {
                                details: self.basic_room_details_for(room_id),
                                is_space: *is_space,
                                // The space's own entry for this room says which servers to ask about it.
                                via: self.children_cache.values()
                                    .flat_map(|children| children.iter())
                                    .find(|child| &child.room_id == room_id)
                                    .map_or_else(Vec::new, |child| child.via.clone()),
                            },
                            show_tip: false,
                        });
                    }
                    SubspaceEntryAction::LeaveClicked { room_id, is_space } => {
                        if *is_space {
                            cx.action(LeaveSpaceModalAction::Open(self.known_space_name_id(room_id)));
                        } else {
                            cx.action(JoinLeaveRoomModalAction::Open {
                                kind: JoinLeaveModalKind::LeaveRoom(
                                    self.basic_room_details_for(room_id)
                                ),
                                show_tip: false,
                            });
                        }
                    }
                    SubspaceEntryAction::ViewClicked { room_id } => {
                        cx.action(AppStateAction::NavigateToRoom {
                            room_to_close: None,
                            destination_room: self.basic_room_details_for(room_id),
                        });
                    }
                    SubspaceEntryAction::None => { }
                }
            }

            // Handle the members button being clicked in the header, which shows or hides the members pane.
            if self.view.button(cx, ids!(header.parent_space_row.parent_space_buttons.members_button)).clicked(actions) {
                self.view.room_pane_dock(cx, ids!(room_pane_dock)).toggle(cx, RoomPaneKind::Members);
            }

            // Handle the invite button being clicked in the header.
            if self.view.button(cx, ids!(header.parent_space_row.parent_space_buttons.invite_button)).clicked(actions) {
                if let Some(space_name_id) = self.space_name_id.as_ref() {
                    cx.action(InviteModalAction::Open(space_name_id.clone()));
                }
            }

            // Handle the leave button being clicked in the header.
            let leave_button = self.view.button(cx, ids!(header.parent_space_row.parent_space_buttons.leave_button));
            if leave_button.clicked(actions) {
                if let Some(space_name_id) = self.space_name_id.as_ref() {
                    // The modal covers this button, so it never gets the hover-out event.
                    leave_button.reset_hover(cx);
                    cx.action(LeaveSpaceModalAction::Open(space_name_id.clone()));
                }
            }

            // Handle changes to this screen's own filter input bar.
            if let Some(keywords) = self.view.room_filter_input_bar(cx, ids!(filter_bar)).changed(actions) {
                self.filter_keywords = keywords;
                self.rebuild_tree_entries();
                // Reset scroll to the top when filter changes.
                let portal_list = self.view.portal_list(cx, ids!(tree_list));
                portal_list.set_first_id_and_scroll(0, 0.0);
                self.redraw(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Draw parent avatar from the SpaceRoom's avatar URL, or show initials.
        let parent_avatar_ref = self.view.avatar(cx, ids!(parent_avatar));
        if self.space_avatar_state.update_from_cache(cx).is_none_or(|image| {
            parent_avatar_ref.show_image(
                cx,
                None,
                |cx, img| utils::load_avatar_image(&img, cx, image),
            ).is_err()
        }) {
            let first_char = self.space_name_id.as_ref().and_then(|sni| sni.name_for_avatar())
                .and_then(|name| utils::user_name_first_letter(name));
            parent_avatar_ref.show_text(cx, None, None, first_char.unwrap_or("S"));
        }
        
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
                // NOTE: Use child_by_path instead of widget tree-based lookups
                //       (e.g., item.label(), item.avatar(), item.widget())
                //       because WidgetRef::widget() fails for portal list items.

                // Draw loading indicator
                let item = if self.is_loading && item_id == 0 {
                    let item = list.item(cx, item_id, id!(status_label));
                    item.child_by_path(ids!(label)).as_label().set_text(cx, "Loading rooms and spaces...");
                    item.child_by_path(ids!(loading_spinner)).set_visible(cx, true);
                    item
                }
                // No entries found
                else if entry_count == 0 && item_id == 0 {
                    let item = list.item(cx, item_id, id!(status_label));
                    let msg = if self.filter_keywords.is_empty() {
                        "No rooms or spaces found."
                    } else {
                        "No matching rooms or spaces."
                    };
                    item.child_by_path(ids!(label)).as_label().set_text(cx, msg);
                    item.child_by_path(ids!(loading_spinner)).set_visible(cx, false);
                    item
                }
                // Draw a regular entry
                else if let Some(entry) = self.tree_entries.get_mut(item_id) {
                    match entry {
                        TreeEntry::Item { info, level, is_last, parent_mask } => {
                            let show_join_button = !matches!(info.state, Some(RoomState::Joined));
                            let show_leave_button = !show_join_button;
                            let show_view_button = show_leave_button && !info.is_space();
                            let item = if info.is_space() {
                                let item = list.item(cx, item_id, id!(subspace_entry));
                                let is_expanded = self.expanded_spaces.contains(&info.id);
                                let mut show_buttons_view = false;
                                let mut need_snap = false;
                                if let Some(mut inner) = item.borrow_mut::<SubspaceEntry>() {
                                    let id_changed = inner.room_id.as_ref() != Some(&info.id);
                                    need_snap = id_changed || inner.is_expanded != is_expanded;
                                    inner.room_id = Some(info.id.clone());
                                    inner.is_space = true;
                                    inner.is_expanded = is_expanded;
                                    if id_changed {
                                        inner.show_buttons_view = false;
                                        inner.buttons_shown_by_tap = false;
                                        inner.animator_cut(cx, ids!(bg_hover.off));
                                    }
                                    show_buttons_view = inner.show_buttons_view;
                                }
                                item.child_by_path(ids!(buttons_view)).set_visible(cx, show_buttons_view);
                                // Snap expand arrow to correct state without animation
                                // when item is reused or state changed externally
                                if need_snap {
                                    if let Some(mut arrow) = item.child_by_path(ids!(main_entry.expand_icon)).borrow_mut::<ExpandArrow>() {
                                        arrow.set_is_open(cx, is_expanded, Animate::No);
                                    }
                                }
                                item
                            } else {
                                let item = list.item(cx, item_id, id!(room_entry));
                                let mut show_buttons_view = false;
                                if let Some(mut inner) = item.borrow_mut::<SubspaceEntry>() {
                                    let id_changed = inner.room_id.as_ref() != Some(&info.id);
                                    inner.room_id = Some(info.id.clone());
                                    inner.is_space = false;
                                    if id_changed {
                                        inner.show_buttons_view = false;
                                        inner.buttons_shown_by_tap = false;
                                        inner.animator_cut(cx, ids!(bg_hover.off));
                                    }
                                    show_buttons_view = inner.show_buttons_view;
                                }
                                item.child_by_path(ids!(buttons_view)).set_visible(cx, show_buttons_view);
                                item
                            };

                            item.child_by_path(ids!(buttons_view.join_button)).set_visible(cx, show_join_button);
                            item.child_by_path(ids!(buttons_view.leave_button)).set_visible(cx, show_leave_button);
                            item.child_by_path(ids!(buttons_view.view_button)).set_visible(cx, show_view_button);

                            // Below, draw things that are common to child rooms and subspaces.
                            item.child_by_path(ids!(main_entry.content.name_label)).as_label().set_text(cx, &info.name);

                            // Display avatar from stored data, or fetch from cache, or show initials
                            let avatar_ref = item.child_by_path(ids!(main_entry.avatar)).as_avatar();
                            let first_char = utils::user_name_first_letter(&info.name);
                            let mut drew_avatar = false;

                            match &info.avatar {
                                AvatarState::Loaded(image) => {
                                    drew_avatar = avatar_ref.show_image(
                                        cx,
                                        None,
                                        |cx, img| utils::load_avatar_image(&img, cx, image),
                                    ).is_ok();
                                }
                                AvatarState::Known(Some(uri)) => {
                                    match avatar_cache::get_or_fetch_avatar(cx, uri) {
                                        AvatarCacheEntry::Loaded(data) => {
                                            let image = AvatarImage::from((uri.clone(), data));
                                            drew_avatar = avatar_ref.show_image(
                                                cx,
                                                None,
                                                |cx, img| utils::load_avatar_image(&img, cx, &image),
                                            ).is_ok();
                                            info.avatar = AvatarState::Loaded(image);
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

                            set_tree_row_position(
                                &item,
                                ids!(main_entry.indent_spacer),
                                *level as isize,
                                *is_last,
                                *parent_mask,
                                TREE_INDENT_WIDTH,
                                (*level as f64 + 1.0) * TREE_INDENT_WIDTH,
                            );

                            // Show "Suggested" tag if recommended and not already joined
                            let show_suggested = info.suggested
                                && !matches!(info.state, Some(RoomState::Joined));
                            item.child_by_path(ids!(main_entry.content.suggested_tag))
                                .set_visible(cx, show_suggested);

                            // Build the info label with join status, member count, and topic
                            // Note: Public/Private is intentionally not shown per-item to reduce clutter
                            let info_label = item.child_by_path(ids!(main_entry.content.info_label)).as_label();
                            let mut info_text = String::new();

                            // Add join status for rooms we haven't joined
                            if let Some(state) = &info.state {
                                info_text.push_str(match state {
                                    RoomState::Joined => "✅ Joined",
                                    RoomState::Left => "Left",
                                    RoomState::Invited => "Invited",
                                    RoomState::Knocked => "Knocked",
                                    RoomState::Banned => "Banned",
                                });
                                info_text.push_str("  |  ");
                            }

                            // Add member count
                            let _ = write!(
                                info_text,
                                "{} {}",
                                info.num_joined_members,
                                if info.num_joined_members == 1 { "member" } else { "members" },
                            );

                            // Add children count for spaces
                            if let Some(c) = info.children_count.filter(|c| *c > 0) {
                                let _ = write!(info_text, "  |  ~{} {}", c, if c == 1 { "room" } else { "rooms" });
                            }

                            // Add topic if available (the label truncates it at 2 lines)
                            if let Some(topic) = info.topic.as_deref().filter(|t| !t.is_empty()) {
                                info_text.push_str("  |  ");
                                info_text.push_str(topic);
                            }

                            info_label.set_text(cx, &info_text);

                            item
                        }
                        TreeEntry::Loading { level, parent_mask } => {
                            // Draw loading indicator for subspace
                            let item = list.item(cx, item_id, id!(subspace_loading));
                            set_tree_row_position(
                                &item,
                                ids!(loading_content.indent_spacer),
                                *level as isize,
                                true,
                                *parent_mask,
                                TREE_INDENT_WIDTH,
                                (*level as f64 + 1.0) * TREE_INDENT_WIDTH,
                            );
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
    /// Finds the given room/space ID in the tree and returns its basic details (including name).
    fn basic_room_details_for(&self, room_id: &OwnedRoomId) -> BasicRoomDetails {
        let room_name = self.tree_entries.iter().find_map(|entry| match entry {
            TreeEntry::Item { info, .. } if &info.id == room_id => Some(info.name.clone()),
            _ => None,
        });
        let room_name_id: RoomNameId = room_name
            .map(|name| RoomNameId::new(RoomDisplayName::Named(name), room_id.clone()))
            .unwrap_or_else(|| RoomNameId::empty(room_id.clone()));
        BasicRoomDetails::Name(room_name_id)
    }

    /// Handle receiving detailed children for a space.
    fn update_children_in_space(&mut self, cx: &mut Cx, space_id: &OwnedRoomId, children: &Vector<SpaceRoom>) {
        self.children_cache.insert(space_id.clone(), children.clone());
        self.loading_subspaces.remove(space_id);

        // If this is for our displayed space, mark as loaded and rebuild tree
        if self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == space_id) {
            self.is_loading = false;
            // Auto-expand the top-level space (we don't show it, just its children)
            self.expanded_spaces.insert(space_id.clone());
            if let Some((first_id, first_scroll)) = self.pending_scroll.take() {
                self.view.portal_list(cx, ids!(tree_list)).set_first_id_and_scroll(first_id, first_scroll);
            }
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
                        space_name_id: self.known_space_name_id(space_id),
                        parent_chain,
                    });
                }
            }
        }

        self.rebuild_tree_entries();
        self.redraw(cx);
    }

    /// Rebuild the flattened tree entries based on the current expansion state,
    /// and then apply the current filter keywords (if any).
    fn rebuild_tree_entries(&mut self) {
        let Some(space_name_id) = &self.space_name_id else { return };
        let root_space_id = space_name_id.room_id().clone();
        let mut new_tree_entries = Vec::new();

        if self.filter_keywords.is_empty() {
            // No filter: build tree respecting expansion state.
            build_tree_for_space(
                &self.children_cache,
                &self.expanded_spaces,
                &self.loading_subspaces,
                &mut new_tree_entries,
                &mut Vec::new(),
                &root_space_id,
                0,
                0,
            );
        } else {
            // Filter active: build tree showing all matching entries
            // plus their ancestor spaces to preserve hierarchy context.
            let kw = self.filter_keywords.to_lowercase();
            build_filtered_tree(
                &self.children_cache,
                &mut new_tree_entries,
                &mut Vec::new(),
                &root_space_id,
                &TreeFilter {
                    matches: &|info| Self::matches_filter(info, &kw),
                    shows_all_within_matches: true,
                },
                0,
                0,
            );
        }

        self.tree_entries = new_tree_entries;
    }

    /// Returns whether the given [`SpaceRoomInfo`] matches the filter keywords.
    fn matches_filter(info: &SpaceRoomInfo, keywords: &str) -> bool {
        info.name.to_lowercase().contains(keywords)
            || info.id.as_str().to_lowercase().contains(keywords)
            || info.canonical_alias.as_ref()
                .is_some_and(|a| a.as_str().to_lowercase().contains(keywords))
            || info.topic.as_ref()
                .is_some_and(|t| t.to_lowercase().contains(keywords))
    }

    /// Saves the current UI state to the cache. Call this when the screen is being hidden.
    pub fn save_current_state(&mut self) {
        if let Some(current_space) = &self.space_name_id {
            let tree_list = self.view.child_by_path(ids!(tree_list)).as_portal_list();
            let first_id_and_scroll = self.pending_scroll.unwrap_or_else(|| (tree_list.first_id(), tree_list.scroll_position()));
            // The space itself is always expanded, so these are all the children that we show.
            let children = self.children_cache.iter()
                .filter(|(space_id, _)| self.expanded_spaces.contains(*space_id))
                .map(|(space_id, children)| (space_id.clone(), children.clone()))
                .collect();
            SPACE_LOBBY_STATES.with_borrow_mut(|states| {
                states.insert(
                    current_space.room_id().clone(),
                    SpaceLobbyUiState {
                        space_details: self.space_details.clone(),
                        room_panes: self.view.child_by_path(ids!(room_pane_dock)).as_room_pane_dock().save_state(),
                        expanded_spaces: self.expanded_spaces.clone(),
                        children,
                        first_id_and_scroll,
                    },
                );
            });
        }
    }

    /// Makes the header's row of buttons only as wide as the buttons that are shown,
    /// since it needs a definite width to wrap them onto more lines.
    fn fit_header_buttons(&mut self, cx: &mut Cx) {
        let row = self.view.view(cx, ids!(header.parent_space_row.parent_space_buttons));
        let Some(spacing) = row.borrow().map(|row| row.layout.spacing) else { return };
        let mut widths = Vec::new();
        for id in [ids!(members_button), ids!(invite_button), ids!(leave_button)] {
            let button = row.button(cx, id);
            if button.visible() && let Some(width) = button.walk(cx).width.to_fixed() {
                widths.push(width);
            }
        }
        let width = widths.iter().sum::<f64>() + spacing * widths.len().saturating_sub(1) as f64;
        if let Ok(SizeExprSimple::Compound(expr_id)) = cx.global::<SizeExprStore>().intern(&format!("min({width}px, 100%)"))
            && let Some(mut row) = row.borrow_mut()
        {
            row.walk.width = Size::Expr(expr_id);
        }
    }

    /// Makes the members button purple while the members pane is shown, like a room's action bar does.
    fn highlight_members_button(&self, cx: &mut Cx, is_members_pane_shown: bool) {
        let mut button = self.view.button(cx, ids!(header.parent_space_row.parent_space_buttons.members_button));
        apply_lavender_button_style(cx, &mut button, is_members_pane_shown);
    }

    /// Shows the given details of our space in the header, e.g., its avatar, member count, and topic.
    fn show_space_details(&mut self, cx: &mut Cx, sr: &SpaceRoom) {
        self.space_avatar_state = AvatarState::Known(sr.avatar_url.clone());
        self.space_avatar_state.update_from_cache(cx); // prefetch the avatar image
        self.view.label(cx, ids!(header.parent_space_row.parent_space_title.parent_space_info.space_info_label)).set_text(cx, &format!(
            "{}  ·  {} {}",
            match sr.join_rule {
                Some(JoinRuleSummary::Public) => "🌐  Public space",
                _ => "🔒  Private space",
            },
            sr.num_joined_members,
            if sr.num_joined_members == 1 { "member" } else { "members" }
        ));
        self.set_space_topic(cx, sr.topic.as_deref());
        self.space_details = Some(sr.clone());
        self.redraw(cx);
    }

    /// Shows the given topic in the header as linkified HTML, or hides it if there is none.
    fn set_space_topic(&mut self, cx: &mut Cx, topic: Option<&str>) {
        let space_topic_view = self.view.view(cx, ids!(header.space_topic_view));
        match topic.map(str::trim).filter(|t| !t.is_empty()) {
            Some(topic) => {
                self.view.widget(cx, ids!(header.space_topic_view.space_topic)).set_text(cx, topic);
                space_topic_view.set_visible(cx, true);
            }
            None => space_topic_view.set_visible(cx, false),
        }
    }

    /// Returns the name of the given space, using the parent's name for this space as a fallback
    /// if its name is not already immediately known.
    fn known_space_name_id(&self, space_id: &OwnedRoomId) -> RoomNameId {
        if let Some(sni) = self.space_name_id.as_ref().filter(|sni| sni.room_id() == space_id) {
            return sni.clone();
        }
        self.children_cache.values()
            .flatten()
            .find(|sr| &sr.room_id == space_id)
            .map_or_else(
                || RoomNameId::empty(space_id.clone()),
                |sr| RoomNameId::new(RoomDisplayName::Named(sr.display_name.clone()), space_id.clone()),
            )
    }

    /// Requests details about the given space.
    fn send_initial_space_requests(&mut self, cx: &mut Cx, space_name_id: &RoomNameId) {
        let rooms_list_ref = cx.get_global::<RoomsListRef>();
        let Some(sender) = rooms_list_ref.get_space_request_sender() else { return };
        // Request detailed children for this space so we can start populating it,
        // which also starts the backend space service subscriber so we get notified of any future changes.
        let parent_chain_opt = rooms_list_ref.get_space_parent_chain(space_name_id.room_id());
        let _ = sender.send(SpaceRequest::GetDetailedChildren {
            space_name_id: space_name_id.clone(),
            parent_chain: parent_chain_opt.unwrap_or_default(),
        });
        let _ = sender.send(SpaceRequest::GetSpaceDetails {
            space_id: space_name_id.room_id().clone(),
        });
        self.space_request_sender = Some(sender);
    }

    pub fn set_displayed_space(&mut self, cx: &mut Cx, space_name_id: &RoomNameId) {
        let space_name = space_name_id.display();
        let parent_name = self.view.label(cx, ids!(header.parent_space_row.parent_space_title.parent_space_info.parent_name));
        parent_name.set_text(cx, &space_name);

        // If this space is already being displayed, then update its name here in case it changed.
        if self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == space_name_id.room_id()) {
            self.space_name_id = Some(space_name_id.clone());
            // If the space service wasn't running yet, start the subscriber for this space
            if self.space_request_sender.is_none() {
                self.send_initial_space_requests(cx, space_name_id);
            }
            return;
        }

        // Save the current UI state before switching to a new space
        self.save_current_state();

        self.space_name_id = Some(space_name_id.clone());
        self.send_initial_space_requests(cx, space_name_id);

        // Restore UI state if we've viewed this space before, otherwise start fresh
        let saved_state = SPACE_LOBBY_STATES.with_borrow_mut(|states| states.remove(space_name_id.room_id()))
            .unwrap_or_default();
        self.view.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane)).reset(cx);
        self.view.room_pane_dock(cx, ids!(room_pane_dock)).show_space(cx, space_name_id, saved_state.room_panes);

        // We show the space's details that we already know of until we receive the async space info responses.
        match saved_state.space_details.as_ref() {
            Some(space_details) => self.show_space_details(cx, space_details),
            None => {
                self.view.label(cx, ids!(header.parent_space_row.parent_space_title.parent_space_info.space_info_label)).set_text(cx, "");
                // Also clear the avatar, so we don't show the previous space's avatar.
                self.space_avatar_state = AvatarState::Unknown;
                self.set_space_topic(cx, None);
                self.space_details = None;
            }
        }
        // Hide the invite button until we know whether the user can invite others to this space.
        self.view.button(cx, ids!(header.parent_space_row.parent_space_buttons.invite_button)).set_visible(cx, false);
        self.fit_header_buttons(cx);
        submit_async_request(MatrixRequest::GetRoomPowerLevels { room_id: space_name_id.room_id().clone() });

        // Clear the filter bar when switching to a new space.
        self.filter_keywords.clear();
        self.view.room_filter_input_bar(cx, ids!(filter_bar)).clear(cx);

        self.expanded_spaces = saved_state.expanded_spaces;
        // Children that we've received since saving them are newer.
        for (space_id, children) in saved_state.children {
            self.children_cache.entry(space_id).or_insert(children);
        }
        // We show the rooms that we already know of until the space service sends us the latest ones.
        self.is_loading = !self.children_cache.contains_key(space_name_id.room_id());
        self.rebuild_tree_entries();
        let (first_id, first_scroll) = saved_state.first_id_and_scroll;
        self.view.portal_list(cx, ids!(tree_list)).set_first_id_and_scroll(first_id, first_scroll);
        // A loading list is too short to stay there, so we scroll there again once it's loaded.
        self.pending_scroll = self.is_loading.then_some(saved_state.first_id_and_scroll);

        // TODO: move avatar setting to `draw_walk()`
        // Set parent avatar
        let avatar_ref = self.view.avatar(cx, ids!(header.parent_space_row.parent_space_title.parent_avatar));
        let first_char = utils::user_name_first_letter(&space_name);
        avatar_ref.show_text(cx, None, None, first_char.unwrap_or("#"));

        self.redraw(cx);
    }

    pub fn hide_displayed_space(&mut self, cx: &mut Cx) {
        self.save_current_state();
        self.space_name_id = None;
        self.space_avatar_state = AvatarState::Unknown;
        self.space_details = None;
        self.space_request_sender = None;
        self.tree_entries.clear();
        self.loading_subspaces.clear();
        self.is_loading = false;
        self.pending_scroll = None;
        self.filter_keywords.clear();
        self.view.room_filter_input_bar(cx, ids!(filter_bar)).clear(cx);
        self.set_space_topic(cx, None);
        self.view.room_pane_dock(cx, ids!(room_pane_dock)).clear(cx);
        self.view.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane)).reset(cx);
        self.redraw(cx);
    }
}

impl SpaceLobbyScreenRef {
    pub fn set_displayed_space(&self, cx: &mut Cx, space_name_id: &RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_displayed_space(cx, space_name_id);
    }

    pub fn hide_displayed_space(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide_displayed_space(cx);
    }

    /// Saves the current UI state. Call this when the screen is being hidden or destroyed.
    pub fn save_current_state(&self) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.save_current_state();
    }
}
