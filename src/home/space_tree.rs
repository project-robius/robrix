//! `TreeEntry`, a row in the tree of rooms and subspaces within a space, and the `TreeLines` connecting those rows.
//! This is shared by the `SpaceLobbyScreen` and the `LeaveSpaceModal`.

use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::{RoomState, ruma::OwnedRoomId};
use matrix_sdk_ui::spaces::SpaceRoom;
use ruma::{OwnedRoomAliasId, room::JoinRuleSummary};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    shared::avatar::AvatarState,
    space_service_sync::SpaceRoomExt,
    utils::{RoomNameId, replace_linebreaks_separators},
};


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // A view that draws the hierarchical tree structure lines.
    let DrawTreeLine = set_type_default() do #(DrawTreeLine::script_shader(vm)){
        ..mod.draw.DrawQuad
    }

    mod.widgets.TreeLines = #(TreeLines::register_widget(vm)) {
        width: Fill, height: Fill { min: 32 }

        draw_bg: DrawTreeLine {
            indent_width: 44.0
            level: 0.0
            is_last: 0.0
            parent_mask: 0.0
            line_color: (COLOR_SPACE_LOBBY_TREE_LINES)
            // Offset to center each vertical line under the parent-level avatar.
            // Derived from: main_entry left padding (8) + expand_icon space (14)
            // + half avatar width (16) = 38.
            stem_offset: 38.0
            // Extend horizontal line to the center of the expand_icon:
            // spacer_end + left_padding(8) - expand_margin_left(6) + expand_width(16)/2 = +10
            elbow_end_offset: 10.0

            pixel: fn() {
                let pos = self.pos * self.rect_size;
                let indent = self.indent_width;
                // `pos` is in logical points, so one device pixel is 1/dpi of it.
                let dpi = max(self.draw_pass.dpi_factor, 0.0001);
                let px = 1.0 / dpi;
                let hp = px * 0.5;
                // Line thickness in whole device pixels, never less than one.
                let lw = max(floor(dpi + 0.5), 1.0);

                // Snap each line onto the device pixel grid, then measure how much of it
                // covers the sample's footprint. The old `abs(pos - k) < 0.5` test landed
                // exactly between two sample centers and lit neither whenever a row
                // started on a whole pixel, which is why these lines came and went.

                // Horizontal arm of the elbow, centered vertically in the row.
                let ay = floor((self.rect_size.y * 0.5 - 0.5 + self.rect_pos.y) * dpi + 0.5);
                let arm_top = ay / dpi - self.rect_pos.y;
                let arm_bot = (ay + lw) / dpi - self.rect_pos.y;
                let arm_y = clamp((min(arm_bot, pos.y + hp) - max(arm_top, pos.y - hp)) / px, 0.0, 1.0);

                // A last child's stem stops at the arm; everyone else runs off the
                // bottom edge so it meets the next row's stem with no seam.
                let stem_bot = mix(self.rect_size.y + px, arm_bot, self.is_last);
                let stem_y = clamp((min(stem_bot, pos.y + hp) - max(-px, pos.y - hp)) / px, 0.0, 1.0);

                let mut cov = 0.0;

                // Dumb approach, but it works.
                for i in 0..20 {
                    if f32(i) > self.level { break; }

                    // This level's vertical line, snapped to the pixel grid.
                    let vx = floor((f32(i) * indent + self.stem_offset - 0.5 + self.rect_pos.x) * dpi + 0.5);
                    let stem_l = vx / dpi - self.rect_pos.x;
                    let stem_r = (vx + lw) / dpi - self.rect_pos.x;
                    let stem_x = clamp((min(stem_r, pos.x + hp) - max(stem_l, pos.x - hp)) / px, 0.0, 1.0);

                    if f32(i) < self.level {
                        // Check mask for parent levels
                        let mask_bit = modf(floor(self.parent_mask / pow(2.0, f32(i))), 2.0);
                        if mask_bit > 0.5 {
                            // Full-height continuation line
                            cov = max(cov, stem_x);
                        }
                    } else {
                        // Current level: connection to self.
                        let arm_end = (f32(i) + 1.0) * indent + self.elbow_end_offset;
                        let arm_x = clamp((min(arm_end, pos.x + hp) - max(stem_l, pos.x - hp)) / px, 0.0, 1.0);
                        // max() rather than first-hit-wins, so the corner pixel doesn't
                        // get whichever arm we happened to test first.
                        cov = max(cov, max(stem_x * stem_y, arm_x * arm_y));
                    }
                }

                // Premultiplied, since the blend is ONE / 1-SRC_ALPHA. cov 0 leaves the
                // row's hover background alone.
                return vec4(self.line_color.rgb * cov, cov);
            }
        }
    }
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawTreeLine {
    #[deref] draw_super: DrawQuad,
    #[live] indent_width: f32,
    #[live] level: f32,
    #[live] is_last: f32,
    #[live] parent_mask: f32,
    #[live] line_color: Vec4f,
    #[live] stem_offset: f32,
    #[live] elbow_end_offset: f32,
}

#[derive(Script, ScriptHook, Widget)]
pub struct TreeLines {
    #[uid] uid: WidgetUid,
    #[redraw] #[live] draw_bg: DrawTreeLine,
    #[walk] walk: Walk,
}

impl Widget for TreeLines {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) { }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let mut walk = walk;
        // When used in a non-Overlay flow (e.g., SubspaceLoadingEntry's Right flow),
        // set the width to the indent area. When width is Fill (Overlay case),
        // the shader handles clipping via indent_width.
        if !walk.width.is_fill() {
            let indent_pixel = (self.draw_bg.level + 1.0) * self.draw_bg.indent_width;
            walk.width = Size::Fixed(indent_pixel as f64);
        }
        // Use the parent's resolved height so tree lines span the full row,
        // even when our height is Fill inside a Fit parent.
        let parent_h = cx.turtle().height();
        if parent_h.is_finite() && parent_h > 0.0 {
            walk.height = Size::Fixed(parent_h);
        }
        self.draw_bg.draw_walk(cx, walk);
        DrawStep::done()
    }
}

/// The subset of info in [`SpaceRoom`] that we display for each room/space.
#[derive(Debug)]
pub struct SpaceRoomInfo {
    pub room_name_id: RoomNameId,
    pub canonical_alias: Option<OwnedRoomAliasId>,
    pub topic: Option<String>,
    pub avatar: AvatarState,
    pub num_joined_members: u64,
    pub state: Option<RoomState>,
    #[allow(unused)]
    pub join_rule: Option<JoinRuleSummary>,
    /// If `Some`, this is a space. If `None`, it's a room.
    pub children_count: Option<u64>,
    /// Whether the room is suggested by the space administrators.
    pub suggested: bool,
}
impl SpaceRoomInfo {
    pub fn is_space(&self) -> bool {
        self.children_count.is_some()
    }
}
impl From<&SpaceRoom> for SpaceRoomInfo {
    fn from(space_room: &SpaceRoom) -> Self {
        SpaceRoomInfo {
            room_name_id: RoomNameId::from_space_room(space_room),
            canonical_alias: space_room.canonical_alias.clone(),
            topic: space_room.topic.as_ref().map(|t| {
                replace_linebreaks_separators(t.trim(), false).into_owned()
            }),
            avatar: AvatarState::Known(space_room.avatar_url.clone()),
            num_joined_members: space_room.num_joined_members,
            state: space_room.state,
            join_rule: space_room.join_rule.clone(),
            children_count: space_room.is_space().then_some(space_room.children_count),
            suggested: space_room.suggested,
        }
    }
}
impl From<SpaceRoom> for SpaceRoomInfo {
    fn from(space_room: SpaceRoom) -> Self {
        SpaceRoomInfo {
            room_name_id: RoomNameId::from_space_room(&space_room),
            children_count: space_room.is_space().then_some(space_room.children_count),
            canonical_alias: space_room.canonical_alias,
            topic: space_room.topic.map(|t| {
                replace_linebreaks_separators(t.trim(), false).into_owned()
            }),
            avatar: AvatarState::Known(space_room.avatar_url),
            num_joined_members: space_room.num_joined_members,
            state: space_room.state,
            join_rule: space_room.join_rule,
            suggested: space_room.suggested,
        }
    }
}

/// An entry in the tree to be displayed.
#[allow(clippy::large_enum_variant)]
pub enum TreeEntry {
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

/// Decides what a tree built by [`build_filtered_tree()`] shows.
pub struct TreeFilter<'a> {
    /// Returns whether the given room or space should be shown.
    pub matches: &'a dyn Fn(&SpaceRoomInfo) -> bool,
    /// Whether to show everything within a matching space, even if it doesn't match.
    pub shows_all_within_matches: bool,
}

/// Returns the given children with spaces first and then rooms, both alphabetically.
/// This skips the given `ancestors`, since a space can end up inside of itself.
fn sort_children<'a>(children: &'a Vector<SpaceRoom>, ancestors: &[OwnedRoomId]) -> Vec<&'a SpaceRoom> {
    let mut sorted_children: Vec<_> = children.iter()
        .filter(|child| !ancestors.contains(&child.room_id))
        .collect();
    sorted_children.sort_by(|a, b| {
        match (a.is_space(), b.is_space()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()),
        }
    });
    sorted_children
}

/// Returns the `parent_mask` for the children of an entry at the given `level`.
fn get_child_parent_mask(parent_mask: u32, level: usize, is_last: bool) -> u32 {
    // Calculate mask for children:
    // If we are NOT the last child, our level needs a continuation line for our children.
    // If we ARE the last child, our level does NOT need a line.
    // Parent levels are preserved.
    if is_last {
        parent_mask
    } else {
        // Spaces can nest deeper than the mask has bits for, so those levels just get no line.
        parent_mask | 1u32.checked_shl(level as u32).unwrap_or(0)
    }
}

/// Recursively build a filtered tree that includes only entries matching
/// the `filter`, plus any ancestor spaces needed to preserve the hierarchy.
///
/// Returns `true` if any matching entry was added within this subtree.
#[allow(clippy::too_many_arguments)]
pub fn build_filtered_tree(
    children_cache: &FxHashMap<OwnedRoomId, Vector<SpaceRoom>>,
    tree_entries: &mut Vec<TreeEntry>,
    ancestors: &mut Vec<OwnedRoomId>,
    space_id: &OwnedRoomId,
    filter: &TreeFilter,
    level: usize,
    parent_mask: u32,
) -> bool {
    let Some(children) = children_cache.get(space_id) else { return false };
    ancestors.push(space_id.clone());

    // Sort identically to the unfiltered tree: spaces first, then rooms, both alphabetically.
    let sorted_children = sort_children(children, ancestors);

    // First pass: determine which children have matches (self or descendants)
    // so we can correctly compute `is_last` for tree line drawing.
    let matched_indices: Vec<usize> = sorted_children.iter().enumerate().filter_map(|(i, child)| {
        let info = SpaceRoomInfo::from(*child);
        let self_matches = (filter.matches)(&info);
        let has_matching_descendants = child.is_space()
            && children_cache.contains_key(&child.room_id)
            && subtree_has_match(children_cache, &child.room_id, filter, &mut FxHashSet::default());
        if self_matches || has_matching_descendants {
            Some(i)
        } else {
            None
        }
    }).collect();

    // Second pass: emit entries for matched children, preserving hierarchy.
    for (pos, &child_idx) in matched_indices.iter().enumerate() {
        let child = sorted_children[child_idx];
        let is_last = pos == matched_indices.len() - 1;
        let info = SpaceRoomInfo::from(child);
        let self_matches = (filter.matches)(&info);

        let child_mask = get_child_parent_mask(parent_mask, level, is_last);

        if child.is_space() && children_cache.contains_key(&child.room_id) {
            // For spaces: always include if self matches or descendants match.
            tree_entries.push(TreeEntry::Item {
                info,
                level,
                is_last,
                parent_mask,
            });
            // Recurse into child space: if the space itself matches,
            // show ALL of its children (unfiltered); otherwise show only
            // the matching descendants.
            if self_matches && filter.shows_all_within_matches {
                // Show all children of a matching space (no further filtering).
                build_tree_for_space_ignoring_expansion(
                    children_cache,
                    tree_entries,
                    ancestors,
                    &child.room_id,
                    level + 1,
                    child_mask,
                );
            } else {
                // Space doesn't match, but some descendant does — recurse with filter.
                build_filtered_tree(
                    children_cache,
                    tree_entries,
                    ancestors,
                    &child.room_id,
                    filter,
                    level + 1,
                    child_mask,
                );
            }
        } else if self_matches {
            // Non-space room or space without cached children: include only if it matches.
            tree_entries.push(TreeEntry::Item {
                info,
                level,
                is_last,
                parent_mask,
            });
        }
    }

    ancestors.pop();
    !matched_indices.is_empty()
}

/// Returns `true` if any entry in the subtree rooted at `space_id` matches the `filter`.
fn subtree_has_match<'a>(
    children_cache: &'a FxHashMap<OwnedRoomId, Vector<SpaceRoom>>,
    space_id: &'a OwnedRoomId,
    filter: &TreeFilter,
    visited: &mut FxHashSet<&'a OwnedRoomId>,
) -> bool {
    if !visited.insert(space_id) { return false }
    let Some(children) = children_cache.get(space_id) else { return false };
    children.iter().any(|child| {
        let info = SpaceRoomInfo::from(child);
        (filter.matches)(&info)
            || (child.is_space() && subtree_has_match(children_cache, &child.room_id, filter, visited))
    })
}

/// Like [`build_tree_for_space`] but ignores expansion state — shows all children.
/// Used to display the full contents of a space that itself matched the filter.
fn build_tree_for_space_ignoring_expansion(
    children_cache: &FxHashMap<OwnedRoomId, Vector<SpaceRoom>>,
    tree_entries: &mut Vec<TreeEntry>,
    ancestors: &mut Vec<OwnedRoomId>,
    space_id: &OwnedRoomId,
    level: usize,
    parent_mask: u32,
) {
    let Some(children) = children_cache.get(space_id) else { return };
    ancestors.push(space_id.clone());

    let sorted_children = sort_children(children, ancestors);
    let count = sorted_children.len();
    for (i, child) in sorted_children.into_iter().enumerate() {
        let is_last = i == count - 1;
        tree_entries.push(TreeEntry::Item {
            info: SpaceRoomInfo::from(child),
            level,
            is_last,
            parent_mask,
        });

        if child.is_space() && children_cache.contains_key(&child.room_id) {
            build_tree_for_space_ignoring_expansion(
                children_cache,
                tree_entries,
                ancestors,
                &child.room_id,
                level + 1,
                get_child_parent_mask(parent_mask, level, is_last),
            );
        }
    }
    ancestors.pop();
}

/// Recursively build the tree of spaces and their expanded children such that they
/// can be displayed in the SpaceLobbyScreen's PortalList.
#[allow(clippy::too_many_arguments)]
pub fn build_tree_for_space(
    children_cache: &FxHashMap<OwnedRoomId, Vector<SpaceRoom>>,
    expanded_spaces: &FxHashSet<OwnedRoomId>,
    loading_subspaces: &FxHashSet<OwnedRoomId>,
    tree_entries: &mut Vec<TreeEntry>,
    ancestors: &mut Vec<OwnedRoomId>,
    space_id: &OwnedRoomId,
    level: usize,
    parent_mask: u32,
) {
    let Some(children) = children_cache.get(space_id) else { return };
    ancestors.push(space_id.clone());

    // Sort: spaces first, then rooms, both alphabetically
    let sorted_children = sort_children(children, ancestors);

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
            let child_mask = get_child_parent_mask(parent_mask, level, is_last);

            if children_cache.contains_key(&child.room_id) {
                build_tree_for_space(
                    children_cache,
                    expanded_spaces,
                    loading_subspaces,
                    tree_entries,
                    ancestors,
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
    ancestors.pop();
}

/// Positions the given row within the tree, using its `tree_lines` child and its indent spacer.
/// A negative `line_level` draws no lines for this row.
pub fn set_tree_row_position(
    row: &WidgetRef,
    indent_spacer: &[LiveId],
    line_level: isize,
    is_last: bool,
    parent_mask: u32,
    indent_width: f64,
    indent_pixel: f64,
) {
    if let Some(mut lines) = row.child_by_path(ids!(tree_lines)).borrow_mut::<TreeLines>() {
        lines.draw_bg.level = line_level as f32;
        lines.draw_bg.is_last = if is_last { 1.0 } else { 0.0 };
        lines.draw_bg.parent_mask = parent_mask as f32;
        lines.draw_bg.indent_width = indent_width as f32;
    }
    // Set the indent spacer width to match the tree indentation.
    if let Some(mut spacer) = row.child_by_path(indent_spacer).borrow_mut::<View>() {
        spacer.walk.width = Size::Fixed(indent_pixel);
    }
}
