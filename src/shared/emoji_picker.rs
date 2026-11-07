//! `EmojiPicker`, a panel for searching and picking emoji, with the emoji grouped into categories.
//!
//! The app-wide `EmojiPickerPopup` shows it, both to react to messages and to insert emoji into them.
//!
//! The grid of emoji is a `PortalList` of `EmojiGridRow`s, each of which draws all of its emoji
//! in one batch, since the grid can show a hundred or more emoji at once.

use emojis::{Emoji, Group, SkinTone};
use makepad_widgets::{makepad_platform::event::finger::TAP_COUNT_DISTANCE, *};
use rustc_hash::FxHashSet;
use crate::{
    room::room_action_bar::RoomActionTooltip,
    shared::{
        emoji_catalog::{get_emoji_catalog, get_shortcode, group_title, with_skin_tone, without_skin_tone},
        list_rows::handle_row_actions,
        recent_emojis::{get_frequently_used_emojis, get_pick_counts, get_skin_tone, set_skin_tone},
        room_filter_input_bar::{RoomFilterInputBarRef, RoomFilterInputBarWidgetExt},
        styles::set_view_bg_color,
    },
    utils::has_only_soft_keyboard,
};

/// The width and height of each emoji's cell in the grid, though cells widen to fill each row.
const CELL_SIZE: f64 = 44.0;
const HEADER_ROW_HEIGHT: f64 = 30.0;
const TEXT_REACTION_ROW_HEIGHT: f64 = 40.0;
/// The category buttons and the skin tone buttons take turns in this space, so they're the same height.
const TABS_HEIGHT: f64 = 36.0;
/// How many rows of frequently used emoji we show at the top of the grid.
const FREQUENTLY_USED_ROWS: usize = 3;

/// The sections of the grid that the category bar's buttons jump to, in the order of those buttons.
const CATEGORY_SECTIONS: [Section; 10] = [
    Section::FrequentlyUsed,
    Section::Group(Group::SmileysAndEmotion),
    Section::Group(Group::PeopleAndBody),
    Section::Group(Group::AnimalsAndNature),
    Section::Group(Group::FoodAndDrink),
    Section::Group(Group::TravelAndPlaces),
    Section::Group(Group::Activities),
    Section::Group(Group::Objects),
    Section::Group(Group::Symbols),
    Section::Group(Group::Flags),
];

/// The skin tones that users can choose, in the order of the skin tone buttons.
const SKIN_TONES: [SkinTone; 6] = [
    SkinTone::Default,
    SkinTone::Light,
    SkinTone::MediumLight,
    SkinTone::Medium,
    SkinTone::MediumDark,
    SkinTone::Dark,
];

/// The tooltips of the skin tone buttons, in the order of those buttons.
const SKIN_TONE_TOOLTIPS: [&str; SKIN_TONES.len()] = [
    "Default skin tone",
    "Light skin tone",
    "Medium-light skin tone",
    "Medium skin tone",
    "Medium-dark skin tone",
    "Dark skin tone",
];

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.EmojiGridRow = #(EmojiGridRow::register_widget(vm)) {
        width: Fill, height: #(CELL_SIZE)
        draw_cell: mod.draw.DrawColor {
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(1.0, 1.0, self.rect_size.x - 2.0, self.rect_size.y - 2.0, 6.0)
                return sdf.fill(self.color)
            }
        }
        draw_emoji +: {
            ink_centered: false
            text_style: EMOJI_TEXT {font_size: 20}
        }
        color_hover: (COLOR_FLOATING_ITEM_BG_HOVER)
        color_focus: (COLOR_LIST_ITEM_BG_FOCUS)
        color_own_reaction: (COLOR_BG_ACCENT_TINT)
        color_own_reaction_hover: (COLOR_BG_ACCENT_TINT_HOVER)
    }

    // A button in the category bar, which jumps to that category's emoji.
    mod.widgets.EmojiCategoryButton = RobrixIconButton {
        width: 40, height: #(TABS_HEIGHT)
        margin: 0, padding: 0, spacing: 0
        align: Align{x: 0.5, y: 0.5}
        // Tapping it shouldn't take the key focus away from a message input, which hides the soft keyboard.
        grab_key_focus: false
        // A long press shows its tooltip.
        enable_long_press: true
        icon_walk: Walk{width: 22, height: 22}
        draw_bg +: {
            color: (COLOR_TRANSPARENT)
            color_hover: (COLOR_FLOATING_ITEM_BG_HOVER)
            color_down: (COLOR_FLOATING_ITEM_BG_SELECTED)
            border_radius: 6.0
        }
        draw_icon.color: (COLOR_TEXT_MUTED)
    }

    mod.widgets.EmojiButton = RobrixIconButton {
        width: 34, height: 32
        padding: 0, spacing: 0
        align: Align{x: 0.5, y: 0.5}
        grab_key_focus: false
        // A long press shows its tooltip.
        enable_long_press: true
        draw_bg +: {
            color: (COLOR_TRANSPARENT)
            color_hover: (COLOR_FLOATING_ITEM_BG_HOVER)
            color_down: (COLOR_FLOATING_ITEM_BG_SELECTED)
            border_radius: 6.0
        }
        draw_text +: {
            ink_centered: false
            text_style: EMOJI_TEXT {font_size: 14}
        }
    }

    mod.widgets.EmojiPicker = #(EmojiPicker::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Down

        color_category_icon: (COLOR_TEXT_MUTED)
        color_category_bg: (COLOR_TRANSPARENT)
        color_active_category_icon: (COLOR_ACTIVE_PRIMARY_DARKER)
        color_active_category_bg: (COLOR_BG_ACCENT_TINT)
        color_text_reaction: (COLOR_TRANSPARENT)
        color_text_reaction_hover: (COLOR_FLOATING_ITEM_BG_HOVER)
        color_text_reaction_focus: (COLOR_LIST_ITEM_BG_FOCUS)
        color_text_reaction_own: (COLOR_BG_ACCENT_TINT)
        color_text_reaction_own_hover: (COLOR_BG_ACCENT_TINT_HOVER)

        search_row := View {
            width: Fill, height: Fit
            flow: Right, spacing: 4
            align: Align{y: 0.5}
            padding: Inset{top: 8, left: 8, right: 6, bottom: 4}

            search_bar := RoomFilterInputBar {
                input +: {
                    has_mic: false
                    text_input +: { empty_text: "Search emoji..." }
                }
            }
            skin_tone_button := mod.widgets.EmojiButton {}
        }

        // The skin tone buttons take the category bar's place while they're shown.
        tabs := View {
            width: Fill, height: Fit
            flow: Overlay
            // Clips the scrolling buttons just inside the popup's border, which is 2pt wide.
            padding: Inset{left: 2, right: 2, bottom: 4}

            // Scrolls sideways when the picker's too narrow to fit all of its buttons.
            category_bar := ScrollXView {
                width: Fill, height: #(TABS_HEIGHT)
                flow: Right
                align: Align{x: 0.5}
                padding: Inset{left: 5}
                scroll_bars +: {
                    scroll_bar_x +: {
                        show_handle: false
                        smoothing: 0.15
                        // Lets a mouse wheel scroll it sideways.
                        use_vertical_finger_scroll: true
                        hide_keyboard_on_scroll: false
                    }
                }
                frequently_used := mod.widgets.EmojiCategoryButton { draw_icon +: { svg: (ICON_HISTORY) } }
                smileys_and_emotion := mod.widgets.EmojiCategoryButton { draw_icon +: { svg: (ICON_EMOJI) } }
                people_and_body := mod.widgets.EmojiCategoryButton { draw_icon +: { svg: (ICON_HAND_WAVE) } }
                animals_and_nature := mod.widgets.EmojiCategoryButton { draw_icon +: { svg: (ICON_ANIMAL_CAT) } }
                food_and_drink := mod.widgets.EmojiCategoryButton { draw_icon +: { svg: (ICON_FOOD_PIZZA) } }
                travel_and_places := mod.widgets.EmojiCategoryButton { draw_icon +: { svg: (ICON_VEHICLE_CAR) } }
                activities := mod.widgets.EmojiCategoryButton { draw_icon +: { svg: (ICON_SPORT_SOCCER) } }
                objects := mod.widgets.EmojiCategoryButton { draw_icon +: { svg: (ICON_LIGHTBULB) } }
                symbols := mod.widgets.EmojiCategoryButton { draw_icon +: { svg: (ICON_SYMBOLS) } }
                // A scroll view's right padding doesn't count towards how far it scrolls, but this does.
                flags := mod.widgets.EmojiCategoryButton { margin: Inset{right: 5}, draw_icon +: { svg: (ICON_FLAG) } }
            }

            skin_tones := View {
                visible: false
                width: Fill, height: #(TABS_HEIGHT)
                flow: Right, spacing: 4
                align: Align{x: 0.5, y: 0.5}
                skin_tone_0 := mod.widgets.EmojiButton {}
                skin_tone_1 := mod.widgets.EmojiButton {}
                skin_tone_2 := mod.widgets.EmojiButton {}
                skin_tone_3 := mod.widgets.EmojiButton {}
                skin_tone_4 := mod.widgets.EmojiButton {}
                skin_tone_5 := mod.widgets.EmojiButton {}
            }
        }

        LineH { draw_bg.color: (COLOR_DIVIDER) }

        grid := PortalList {
            width: Fill, height: Fill
            flow: Down
            // The right side also holds the scroll bar.
            padding: Inset{left: 6, right: 10, bottom: 6}
            reuse_items: true
            hide_keyboard_on_scroll: false

            section_header := View {
                width: Fill, height: #(HEADER_ROW_HEIGHT)
                align: Align{y: 1.0}
                padding: Inset{left: 6, bottom: 5}
                label := Label {
                    width: Fill, height: Fit, padding: 0
                    draw_text +: {
                        color: (COLOR_TEXT_MUTED)
                        text_style: theme.font_bold {font_size: 9}
                    }
                }
            }
            emoji_row := mod.widgets.EmojiGridRow {}
            // Reacts with the search text itself, since a reaction can be any text.
            text_reaction := RoundedView {
                width: Fill, height: #(TEXT_REACTION_ROW_HEIGHT)
                flow: Right, spacing: 8
                align: Align{y: 0.5}
                padding: Inset{left: 8, right: 8}
                show_bg: true
                cursor: MouseCursor.Hand
                grab_key_focus: false
                draw_bg +: { color: (COLOR_TRANSPARENT), border_radius: 6.0 }
                Icon {
                    draw_icon +: {
                        svg: (ICON_ADD_REACTION)
                        color: (COLOR_ACTIVE_PRIMARY_DARKER)
                    }
                    icon_walk: Walk{width: 18, height: 18}
                }
                label := Label {
                    width: Fill, height: Fit, padding: 0
                    max_lines: 1, text_overflow: Ellipsis
                    draw_text +: {
                        color: (COLOR_ACTIVE_PRIMARY_DARKER)
                        text_style: REGULAR_TEXT {font_size: 10.5}
                    }
                }
            }
            no_results := View {
                width: Fill, height: Fit
                align: Align{x: 0.5}
                padding: Inset{top: 24, bottom: 16, left: 10, right: 10}
                label := Label {
                    width: Fill, height: Fit, padding: 0
                    align: Align{x: 0.5}
                    draw_text +: {
                        color: (COLOR_TEXT_MUTED)
                        text_style: REGULAR_TEXT {font_size: 10.5}
                    }
                    text: "No matching emoji"
                }
            }
        }

        // Shows the name and shortcode of the emoji under the mouse or focused via the keyboard.
        preview := View {
            width: Fill, height: Fit
            flow: Down

            LineH { draw_bg.color: (COLOR_DIVIDER) }
            View {
                width: Fill, height: 46
                flow: Right, spacing: 10
                align: Align{y: 0.5}
                padding: Inset{left: 12, right: 12}

                preview_emoji := Label {
                    width: Fit, height: Fit, padding: 0
                    draw_text +: {
                        ink_centered: false
                        text_style: EMOJI_TEXT {font_size: 20}
                    }
                }
                View {
                    width: Fill, height: Fit
                    flow: Down, spacing: 2
                    preview_name := Label {
                        width: Fill, height: Fit, padding: 0
                        max_lines: 1, text_overflow: Ellipsis
                        draw_text +: {
                            color: (COLOR_TEXT)
                            text_style: theme.font_bold {font_size: 10}
                        }
                    }
                    preview_shortcode := Label {
                        width: Fill, height: Fit, padding: 0
                        max_lines: 1, text_overflow: Ellipsis
                        draw_text +: {
                            color: (COLOR_TEXT_MUTED)
                            text_style: REGULAR_TEXT {font_size: 9}
                        }
                    }
                }
            }
        }
    }
}

/// What the emoji that the user picks will be used for.
#[derive(Clone, Debug, Default)]
pub enum EmojiPickerMode {
    /// Inserting it into a message.
    #[default]
    Insert,
    /// Reacting to a message, which the user already reacted to with `own_reactions`
    /// (each emoji in its fully-qualified form). Picking one of those again removes it, so we highlight them.
    React { own_reactions: FxHashSet<String> },
}

/// Actions emitted by an `EmojiPicker`, or by the `EmojiPickerPopup` to the message input that opened it.
#[derive(Clone, Debug, Default)]
pub enum EmojiPickerAction {
    /// The user picked the given emoji, or the given text to react with.
    ///
    /// `keep_open` is true if they held Shift to pick more than one.
    Picked { emoji: String, keep_open: bool },
    /// The `EmojiPickerPopup` was closed with Escape or Back, which it tells the message input that opened it.
    Cancelled,
    #[default]
    None,
}

/// A part of the grid that starts with a header and can be jumped to from the category bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    FrequentlyUsed,
    Group(Group),
    SearchResults,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Section::FrequentlyUsed => "Frequently Used",
            Section::Group(group) => group_title(group),
            Section::SearchResults => "Search Results",
        }
    }
}

#[derive(Clone, Debug)]
enum GridRow {
    SectionHeader(Section),
    /// The emoji in the cells from the given start index up to the given end index.
    Emojis { start: usize, end: usize },
    /// Reacts with the search text itself, which comes before the search results.
    TextReaction,
    /// Shown in place of search results when nothing matched.
    NoResults,
}

impl GridRow {
    /// Every row of a given kind has the same height, so we can tell which row is at the top without drawing.
    fn height(&self) -> f64 {
        match self {
            GridRow::SectionHeader(_) => HEADER_ROW_HEIGHT,
            GridRow::Emojis { .. } => CELL_SIZE,
            GridRow::TextReaction => TEXT_REACTION_ROW_HEIGHT,
            // Nothing comes after this row, so its height never matters.
            GridRow::NoResults => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct GridCell {
    emoji: &'static Emoji,
    is_own_reaction: bool,
}

/// The child widgets we use on every draw and event, looked up once.
#[derive(Clone)]
struct ChildRefs {
    search_bar: RoomFilterInputBarRef,
    search_input: TextInputRef,
    skin_tone_button: ButtonRef,
    skin_tone_buttons: [ButtonRef; SKIN_TONES.len()],
    category_bar: ViewRef,
    skin_tones: ViewRef,
    category_buttons: [ButtonRef; CATEGORY_SECTIONS.len()],
    grid: PortalListRef,
    preview: ViewRef,
}

#[derive(Script, Widget)]
pub struct EmojiPicker {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    #[live] color_category_icon: Vec4f,
    #[live] color_category_bg: Vec4f,
    #[live] color_active_category_icon: Vec4f,
    #[live] color_active_category_bg: Vec4f,
    #[live] color_text_reaction: Vec4f,
    #[live] color_text_reaction_hover: Vec4f,
    #[live] color_text_reaction_focus: Vec4f,
    #[live] color_text_reaction_own: Vec4f,
    #[live] color_text_reaction_own_hover: Vec4f,

    #[rust] mode: EmojiPickerMode,
    #[rust] query: String,
    #[rust] cells: Vec<GridCell>,
    #[rust] rows: Vec<GridRow>,
    /// The index in `rows` of each section's header.
    #[rust] section_rows: Vec<(Section, usize)>,
    /// How many emoji fit in each row, as of the last draw.
    #[rust] columns: usize,
    #[rust] needs_rebuild: bool,
    #[rust] grid_viewport_height: f64,
    /// The index in `cells` of the emoji under the mouse.
    #[rust] hovered_cell: Option<usize>,
    /// The index in `cells` of the emoji that a finger or the mouse is pressing.
    #[rust] pressed_cell: Option<usize>,
    #[rust] was_scrolling_at_press: bool,
    /// The index in `cells` of the emoji that the arrow keys moved to.
    /// With a search, `None` means the row for reacting with the search text, which Enter then picks.
    #[rust] focused_cell: Option<usize>,
    #[rust] is_text_reaction_hovered: bool,
    /// How far the user had scrolled the grid when we last checked.
    #[rust] last_scroll_travel: f64,
    /// The section highlighted in the category bar.
    #[rust] active_section: Option<Section>,
    /// The section that the user last jumped to from the category bar, and how far the grid had scrolled then.
    /// It stays highlighted until they scroll, even if the grid couldn't scroll that section to the top.
    #[rust] jumped_to_section: Option<(Section, f64)>,
    /// Whether to put the key focus in the search input once it's drawn, since it can't take it before that.
    #[rust] is_search_focus_pending: bool,
    #[rust] was_search_focused_at_press: bool,
    /// Highlights the section that the grid scrolled to, since the category bar gets drawn before the grid.
    #[rust] highlight_section_frame: NextFrame,
    #[rust] tooltip: RoomActionTooltip,
    #[rust] child_refs: Option<ChildRefs>,
}

impl ScriptHook for EmojiPicker {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| self.populate_skin_tone_buttons(cx));
    }

    fn on_after_reload(&mut self, vm: &mut ScriptVm) {
        self.child_refs = None;
        // A reload resets the buttons' text and colors, so we re-apply them.
        let active_section = self.active_section.take();
        vm.with_cx_mut(|cx| {
            self.populate_skin_tone_buttons(cx);
            self.highlight_section(cx, active_section);
        });
    }
}

impl Widget for EmojiPicker {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let search_input = self.child_refs(cx).search_input.clone();
        // This must come before the buttons handle the event, else they'd claim the hover first.
        if let Some(refs) = &self.child_refs {
            let buttons = refs.category_buttons.iter()
                .chain(&refs.skin_tone_buttons)
                .chain([&refs.skin_tone_button])
                .map(|button| (**button).clone());
            let tooltips = CATEGORY_SECTIONS.iter()
                .map(|section| section.title())
                .chain(SKIN_TONE_TOOLTIPS)
                .chain(["Choose a skin tone"]);
            self.tooltip.handle_event_with_text_buttons(cx, event, buttons.zip(tooltips), TooltipPosition::Top);
        }
        if self.highlight_section_frame.is_event(event).is_some() {
            let section = self.find_current_section(cx);
            self.highlight_section(cx, section);
            self.redraw(cx);
        }
        // The search input keeps the key focus, so we take the keys that move around the grid from it.
        if let Event::KeyDown(key) = event
            && cx.has_key_focus(search_input.area())
            && self.handle_key_down(cx, key)
        {
            return;
        }

        // The search input drops the key focus upon any click outside of it, but clicking around
        // the rest of the picker shouldn't stop the user from typing in it.
        if let Event::MouseDown(e) = event && self.view.area().rect(cx).contains(e.abs) {
            self.was_search_focused_at_press = cx.has_key_focus(search_input.area());
        }
        self.view.handle_event(cx, event, scope);
        if let Event::MouseUp(_) = event && std::mem::take(&mut self.was_search_focused_at_press) {
            search_input.take_key_focus(cx);
        }

        // Scrolling moves the emoji out from under the mouse or finger, so they're no longer hovered or pressed.
        let scroll_travel = self.child_refs(cx).grid.user_scroll_travel();
        if scroll_travel != self.last_scroll_travel || matches!(event, Event::ClearHover) {
            self.last_scroll_travel = scroll_travel;
            let was_text_reaction_hovered = std::mem::take(&mut self.is_text_reaction_hovered);
            if self.hovered_cell.take().is_some() | self.pressed_cell.take().is_some() | was_text_reaction_hovered {
                self.populate_preview(cx);
                self.redraw(cx);
            }
        }

        if let Event::Actions(actions) = event {
            self.handle_actions(cx, actions);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(widget) = self.view.draw_walk(cx, scope, walk).step() {
            let portal_list = widget.as_portal_list();
            let Some(mut list) = portal_list.borrow_mut() else { continue };
            // The grid has already started drawing, so this is its real size for this draw.
            let viewport = cx.turtle().inner_rect();
            self.grid_viewport_height = viewport.size.y;
            let columns = ((viewport.size.x / CELL_SIZE).floor() as usize).max(1);
            if self.needs_rebuild || columns != self.columns {
                // When only the number of columns changed, keep the same emoji at the top of the grid.
                let top = (!self.needs_rebuild)
                    .then(|| self.find_top_row(list.first_id(), list.first_scroll()))
                    .and_then(|(row, offset)| Some((self.find_anchor_of_row(row)?, offset)));
                self.columns = columns;
                self.needs_rebuild = false;
                self.rebuild_grid(cx);
                if let Some(row) = top.and_then(|(anchor, offset)| Some((self.find_row_of_anchor(anchor)?, offset))) {
                    list.set_first_id_and_scroll_in_place(row.0, row.1);
                }
            }
            let cell_width = viewport.size.x / columns as f64;
            let highlighted_cell = self.pressed_cell.or(self.hovered_cell);

            list.set_item_range(cx, 0, self.rows.len());
            while let Some(index) = list.next_visible_item(cx) {
                let Some(row) = self.rows.get(index) else { continue };
                let item = match row {
                    GridRow::SectionHeader(section) => {
                        let item = list.item(cx, index, id!(section_header));
                        item.label(cx, ids!(label)).set_text(cx, section.title());
                        item
                    }
                    GridRow::Emojis { start, end } => {
                        let item = list.item(cx, index, id!(emoji_row));
                        if let Some(mut row) = item.as_emoji_grid_row().borrow_mut() {
                            let column_of = |cell: Option<usize>| cell
                                .filter(|cell| (*start .. *end).contains(cell))
                                .map(|cell| cell - start);
                            row.set_cells(
                                &self.cells[*start .. *end],
                                *start,
                                cell_width,
                                column_of(highlighted_cell),
                                column_of(self.focused_cell),
                            );
                        }
                        item
                    }
                    GridRow::TextReaction => {
                        let item = list.item(cx, index, id!(text_reaction));
                        let is_own_reaction = matches!(&self.mode, EmojiPickerMode::React { own_reactions } if own_reactions.contains(self.get_text_reaction()));
                        let action = if is_own_reaction { "Remove" } else { "React with" };
                        item.label(cx, ids!(label)).set_text(cx, &format!("{action} \u{201c}{}\u{201d}", self.query));
                        let color = match (self.focused_cell.is_none(), is_own_reaction, self.is_text_reaction_hovered) {
                            (true, ..) => self.color_text_reaction_focus,
                            (false, true, true) => self.color_text_reaction_own_hover,
                            (false, true, false) => self.color_text_reaction_own,
                            (false, false, true) => self.color_text_reaction_hover,
                            (false, false, false) => self.color_text_reaction,
                        };
                        set_view_bg_color(cx, &item.as_view(), color);
                        item
                    }
                    GridRow::NoResults => list.item(cx, index, id!(no_results)),
                };
                item.draw_all(cx, scope);
            }
        }
        if std::mem::take(&mut self.is_search_focus_pending) {
            let search_input = self.child_refs(cx).search_input.clone();
            search_input.take_key_focus(cx);
        }
        if self.find_current_section(cx) != self.active_section {
            self.highlight_section_frame = cx.new_next_frame();
        }
        DrawStep::done()
    }
}

impl EmojiPicker {
    fn child_refs(&mut self, cx: &mut Cx) -> &ChildRefs {
        self.child_refs.get_or_insert_with(|| {
            let view = &self.view;
            ChildRefs {
                search_bar: view.room_filter_input_bar(cx, ids!(search_bar)),
                search_input: view.text_input(cx, ids!(search_bar.input.text_input)),
                skin_tone_button: view.button(cx, ids!(skin_tone_button)),
                skin_tone_buttons: [
                    view.button(cx, ids!(skin_tone_0)),
                    view.button(cx, ids!(skin_tone_1)),
                    view.button(cx, ids!(skin_tone_2)),
                    view.button(cx, ids!(skin_tone_3)),
                    view.button(cx, ids!(skin_tone_4)),
                    view.button(cx, ids!(skin_tone_5)),
                ],
                category_bar: view.view(cx, ids!(category_bar)),
                skin_tones: view.view(cx, ids!(skin_tones)),
                category_buttons: [
                    view.button(cx, ids!(frequently_used)),
                    view.button(cx, ids!(smileys_and_emotion)),
                    view.button(cx, ids!(people_and_body)),
                    view.button(cx, ids!(animals_and_nature)),
                    view.button(cx, ids!(food_and_drink)),
                    view.button(cx, ids!(travel_and_places)),
                    view.button(cx, ids!(activities)),
                    view.button(cx, ids!(objects)),
                    view.button(cx, ids!(symbols)),
                    view.button(cx, ids!(flags)),
                ],
                grid: view.portal_list(cx, ids!(grid)),
                preview: view.view(cx, ids!(preview)),
            }
        })
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let ChildRefs {
            search_bar,
            skin_tone_button,
            skin_tone_buttons,
            category_bar,
            skin_tones,
            category_buttons,
            grid,
            ..
        } = self.child_refs(cx).clone();

        if let Some(query) = search_bar.changed(actions) {
            self.show_search_results(cx, query);
        }

        if skin_tone_button.clicked(actions) {
            let show_skin_tones = !skin_tones.visible();
            skin_tones.set_visible(cx, show_skin_tones);
            category_bar.set_visible(cx, !show_skin_tones);
            self.redraw(cx);
        }
        if let Some(index) = skin_tone_buttons.iter().position(|button| button.clicked(actions)) {
            set_skin_tone(cx, SKIN_TONES[index]);
            self.populate_skin_tone_buttons(cx);
            skin_tones.set_visible(cx, false);
            category_bar.set_visible(cx, true);
            self.needs_rebuild = true;
            self.redraw(cx);
        }

        if let Some(index) = category_buttons.iter().position(|button| button.clicked(actions)) {
            self.jump_to_section(cx, CATEGORY_SECTIONS[index]);
        }

        let items = grid.items_with_actions(actions);
        let text_reaction_items: Vec<WidgetRef> = items.iter()
            .filter(|(index, _)| matches!(self.rows.get(*index), Some(GridRow::TextReaction)))
            .map(|(_, item)| item.clone())
            .collect();
        let mut hovered_text_reaction = self.is_text_reaction_hovered.then_some(());
        let (_, hover_changed) = handle_row_actions(text_reaction_items.iter().map(|item| ((), item.clone())), grid.was_scrolling(), actions, &mut hovered_text_reaction);
        self.is_text_reaction_hovered = hovered_text_reaction.is_some();
        if hover_changed {
            self.redraw(cx);
        }
        for item in text_reaction_items {
            let row = item.as_view();
            if row.finger_down(actions).is_some() {
                self.was_scrolling_at_press = grid.was_scrolling();
            }
            if let Some(fe) = row.finger_up(actions)
                && fe.is_over && fe.is_primary_hit() && is_tap(&fe) && !self.was_scrolling_at_press
            {
                self.emit_text_reaction(cx, fe.modifiers.shift);
            }
        }

        for (index, item) in items {
            for action in actions.filter_widget_actions(item.widget_uid()) {
                match action.cast() {
                    EmojiGridRowAction::Hovered(cell) if cell != self.hovered_cell => {
                        // Moving up into the row above hovers that row before this one hears that the mouse left it.
                        let is_stale_hover_out = cell.is_none() && !matches!(
                            self.rows.get(index),
                            Some(GridRow::Emojis { start, end }) if self.hovered_cell.is_some_and(|hovered| (*start .. *end).contains(&hovered))
                        );
                        if !is_stale_hover_out {
                            self.hovered_cell = cell;
                            self.populate_preview(cx);
                            self.redraw(cx);
                        }
                    }
                    EmojiGridRowAction::Pressed(cell) => {
                        // The grid stops scrolling once this press began, so we have to check it now.
                        self.was_scrolling_at_press = grid.was_scrolling();
                        self.pressed_cell = Some(cell);
                        self.redraw(cx);
                    }
                    EmojiGridRowAction::Released { picked_cell, keep_open } => {
                        self.pressed_cell = None;
                        self.redraw(cx);
                        if !self.was_scrolling_at_press
                            && let Some(cell) = picked_cell.and_then(|cell| self.cells.get(cell))
                        {
                            let emoji = cell.emoji.as_str();
                            self.emit_picked(cx, emoji, keep_open);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Handles a key press in the search input, returning `true` if it moved around the grid or picked something.
    fn handle_key_down(&mut self, cx: &mut Cx, key: &KeyEvent) -> bool {
        let is_enter = matches!(key.key_code, KeyCode::ReturnKey | KeyCode::NumpadEnter);
        let Some(focused_cell) = self.focused_cell else {
            if key.key_code == KeyCode::ArrowDown {
                return self.move_focus_to(cx, 0);
            }
            if is_enter && matches!(self.mode, EmojiPickerMode::React { .. }) && !self.query.is_empty() {
                self.emit_text_reaction(cx, key.modifiers.shift);
                return true;
            }
            // Submitting the search would take the key focus away from it, though a soft keyboard's Enter should hide it.
            return is_enter && !has_only_soft_keyboard(cx);
        };
        if is_enter {
            if let Some(cell) = self.cells.get(focused_cell) {
                self.emit_picked(cx, cell.emoji.as_str(), key.modifiers.shift);
            }
            return true;
        }
        match key.key_code {
            KeyCode::ArrowDown => self.move_focus_by_rows(cx, focused_cell, 1),
            KeyCode::ArrowUp => self.move_focus_by_rows(cx, focused_cell, -1),
            KeyCode::ArrowRight => self.move_focus_to(cx, focused_cell + 1),
            KeyCode::ArrowLeft => self.move_focus_to(cx, focused_cell.saturating_sub(1)),
            _ => false,
        }
    }

    fn emit_picked(&self, cx: &mut Cx, emoji: &str, keep_open: bool) {
        cx.widget_action(
            self.widget_uid(),
            EmojiPickerAction::Picked { emoji: emoji.to_owned(), keep_open },
        );
    }

    /// Returns the search text as a reaction, in its fully-qualified form if it's an emoji.
    fn get_text_reaction(&self) -> &str {
        emojis::get(&self.query).map_or(self.query.as_str(), |emoji| emoji.as_str())
    }

    fn emit_text_reaction(&self, cx: &mut Cx, keep_open: bool) {
        self.emit_picked(cx, self.get_text_reaction(), keep_open);
    }

    fn show_search_results(&mut self, cx: &mut Cx, query: String) {
        if query == self.query {
            return;
        }
        self.query = query;
        self.focused_cell = None;
        self.jumped_to_section = None;
        self.rebuild_grid(cx);
        // The best match is focused, so Enter picks it right away.
        if !self.query.is_empty() && !self.cells.is_empty() {
            self.focused_cell = Some(0);
        }
        self.populate_preview(cx);
        self.jump_grid_to(cx, 0, 0.0);
        self.redraw(cx);
    }

    fn jump_to_section(&mut self, cx: &mut Cx, section: Section) {
        let ChildRefs { search_bar, grid, .. } = self.child_refs(cx).clone();
        if !self.query.is_empty() {
            search_bar.clear(cx);
            self.query.clear();
            self.focused_cell = None;
            self.rebuild_grid(cx);
        }
        // Without any frequently used emoji, that button jumps to the top.
        let row = self.section_rows.iter()
            .find_map(|(s, row)| (*s == section).then_some(*row))
            .unwrap_or(0);
        self.jump_grid_to(cx, row, 0.0);
        self.jumped_to_section = Some((section, grid.user_scroll_travel()));
        self.redraw(cx);
    }

    /// Moves the keyboard focus up or down by the given number of rows of emoji, keeping its column.
    fn move_focus_by_rows(&mut self, cx: &mut Cx, focused_cell: usize, delta: isize) -> bool {
        let emoji_rows: Vec<(usize, usize)> = self.rows.iter()
            .filter_map(|row| match row {
                GridRow::Emojis { start, end } => Some((*start, *end)),
                _ => None,
            })
            .collect();
        let Some(current) = emoji_rows.iter().position(|(start, end)| (*start .. *end).contains(&focused_cell)) else {
            return false;
        };
        let Some((start, end)) = current.checked_add_signed(delta).and_then(|target| emoji_rows.get(target)) else {
            // Moving up from the first row of search results goes to the row for reacting with the search text.
            if delta < 0 && matches!(self.rows.first(), Some(GridRow::TextReaction)) {
                self.focused_cell = None;
                self.populate_preview(cx);
                self.jump_grid_to(cx, 0, 0.0);
                self.redraw(cx);
            }
            return true;
        };
        let column = focused_cell - emoji_rows[current].0;
        self.move_focus_to(cx, (start + column).min(end - 1))
    }

    /// Moves the keyboard focus to the given cell, scrolling the grid so that it's fully visible.
    fn move_focus_to(&mut self, cx: &mut Cx, cell: usize) -> bool {
        if cell >= self.cells.len() {
            return true;
        }
        self.focused_cell = Some(cell);
        self.populate_preview(cx);
        self.jumped_to_section = None;
        let Some(row) = self.find_row_of_cell(cell) else { return true };
        let grid = self.child_refs(cx).grid.clone();
        let is_fully_visible = grid.drawn_slot(row)
            .is_some_and(|slot| slot.start >= 0.0 && slot.start + slot.size <= self.grid_viewport_height);
        if !is_fully_visible {
            let (top_row, _) = self.find_top_row(grid.first_id(), grid.scroll_position());
            if row <= top_row {
                self.jump_grid_to(cx, row, 0.0);
            } else {
                self.jump_grid_to(cx, row, (self.grid_viewport_height - CELL_SIZE).max(0.0));
            }
        }
        self.redraw(cx);
        true
    }

    /// Jumps the grid to the given row and scroll offset, stopping any fling that would carry it away again.
    fn jump_grid_to(&mut self, cx: &mut Cx, row: usize, scroll: f64) {
        let grid = self.child_refs(cx).grid.clone();
        grid.stop_scrolling();
        grid.set_first_id_and_scroll(row, scroll);
    }

    /// Rebuilds the rows of the grid, e.g., after the query, skin tone, or number of columns changed.
    fn rebuild_grid(&mut self, cx: &mut Cx) {
        // The cells can move around, so we find the focused emoji again afterwards.
        let focused = self.focused_cell.and_then(|cell| {
            let section = self.find_row_of_cell(cell).and_then(|row| self.find_section_at_row(row));
            Some((section, self.cells.get(cell)?.emoji))
        });
        self.cells.clear();
        self.rows.clear();
        self.section_rows.clear();
        self.hovered_cell = None;
        self.pressed_cell = None;
        let catalog = get_emoji_catalog();
        let skin_tone = get_skin_tone(cx);
        let own_reactions = match &self.mode {
            EmojiPickerMode::React { own_reactions } => Some(own_reactions.clone()),
            EmojiPickerMode::Insert => None,
        };
        let to_cell = |emoji: &'static Emoji| GridCell {
            emoji,
            is_own_reaction: own_reactions.as_ref().is_some_and(|own| own.contains(emoji.as_str())),
        };

        if self.query.is_empty() {
            let frequently_used = get_frequently_used_emojis(cx, self.columns.max(1) * FREQUENTLY_USED_ROWS);
            if !frequently_used.is_empty() {
                self.push_section(Section::FrequentlyUsed, frequently_used.into_iter().map(to_cell));
            }
            for (group, range) in &catalog.groups {
                let emojis = catalog.emojis[range.clone()].iter().map(|entry| to_cell(with_skin_tone(entry.emoji, skin_tone)));
                self.push_section(Section::Group(*group), emojis);
            }
        } else {
            if matches!(self.mode, EmojiPickerMode::React { .. }) {
                self.rows.push(GridRow::TextReaction);
            }
            let results = catalog.search(&self.query, &get_pick_counts(cx));
            if results.is_empty() {
                self.rows.push(GridRow::NoResults);
            } else {
                self.push_section(Section::SearchResults, results.into_iter().map(|entry| to_cell(with_skin_tone(entry.emoji, skin_tone))));
            }
        }

        // The same emoji in another skin tone (e.g., after the user changed it) is the next best thing.
        self.focused_cell = focused.and_then(|(section, emoji)| {
            let is_in_section = |cell: &usize| self.find_row_of_cell(*cell).and_then(|row| self.find_section_at_row(row)) == section;
            let find_cell = |ignore_skin_tone: bool| {
                let is_match = |cell: &usize| {
                    let cell_emoji = self.cells[*cell].emoji;
                    cell_emoji == emoji || (ignore_skin_tone && without_skin_tone(cell_emoji) == without_skin_tone(emoji))
                };
                let mut matching_cells = (0 .. self.cells.len()).filter(is_match);
                matching_cells.clone().find(&is_in_section).or_else(|| matching_cells.next())
            };
            find_cell(false).or_else(|| find_cell(true))
        });
        // The text reaction row is gone, so it never hears that the mouse left it.
        if !matches!(self.rows.first(), Some(GridRow::TextReaction)) {
            self.is_text_reaction_hovered = false;
        }
    }

    fn push_section(&mut self, section: Section, cells: impl Iterator<Item = GridCell>) {
        self.section_rows.push((section, self.rows.len()));
        self.rows.push(GridRow::SectionHeader(section));
        let start = self.cells.len();
        self.cells.extend(cells);
        let columns = self.columns.max(1);
        for row_start in (start .. self.cells.len()).step_by(columns) {
            self.rows.push(GridRow::Emojis { start: row_start, end: (row_start + columns).min(self.cells.len()) });
        }
    }

    fn find_row_of_cell(&self, cell: usize) -> Option<usize> {
        self.rows.iter().position(|row| matches!(row, GridRow::Emojis { start, end } if (*start .. *end).contains(&cell)))
    }

    /// Returns the first row that's at least partly visible, and how far its top is above the grid's top.
    ///
    /// When scrolling down, the grid usually keeps the row just above its top as its first row.
    fn find_top_row(&self, first_row: usize, first_row_scroll: f64) -> (usize, f64) {
        let Some(last_row) = self.rows.len().checked_sub(1) else { return (0, 0.0) };
        let (mut row, mut top) = (first_row.min(last_row), first_row_scroll);
        while top > 0.0 && row > 0 {
            row -= 1;
            top -= self.rows[row].height();
        }
        while row < last_row && top + self.rows[row].height() <= 0.0 {
            top += self.rows[row].height();
            row += 1;
        }
        (row, top)
    }

    /// Returns what's shown in the given row, so we can find it again after the grid's rows are rebuilt.
    fn find_anchor_of_row(&self, row: usize) -> Option<RowAnchor> {
        let section = self.find_section_at_row(row);
        match self.rows.get(row)? {
            GridRow::SectionHeader(section) => Some(RowAnchor::SectionHeader(*section)),
            GridRow::Emojis { start, .. } => Some(RowAnchor::Emoji { section, emoji: self.cells[*start].emoji }),
            GridRow::TextReaction | GridRow::NoResults => None,
        }
    }

    fn find_row_of_anchor(&self, anchor: RowAnchor) -> Option<usize> {
        match anchor {
            RowAnchor::SectionHeader(section) => self.section_rows.iter().find_map(|(s, row)| (*s == section).then_some(*row)),
            // The same emoji can be in more than one section, like "Frequently Used".
            RowAnchor::Emoji { section, emoji } => {
                let section_start = section
                    .and_then(|section| self.section_rows.iter().find_map(|(s, row)| (*s == section).then_some(*row)))
                    .unwrap_or(0);
                self.rows.iter()
                    .enumerate()
                    .skip(section_start)
                    .find_map(|(index, row)| match row {
                        GridRow::Emojis { start, end } if self.cells[*start .. *end].iter().any(|cell| cell.emoji == emoji) => Some(index),
                        _ => None,
                    })
            }
        }
    }

    /// Returns the section that the category bar should highlight: the one at the top of the grid,
    /// or the one the user jumped to if they haven't scrolled since.
    fn find_current_section(&mut self, cx: &mut Cx) -> Option<Section> {
        let grid = self.child_refs(cx).grid.clone();
        match self.jumped_to_section {
            Some((section, scroll_travel)) if grid.user_scroll_travel() == scroll_travel => Some(section),
            _ => self.find_section_at_row(self.find_top_row(grid.first_id(), grid.scroll_position()).0),
        }
    }

    fn find_section_at_row(&self, row: usize) -> Option<Section> {
        self.section_rows.iter()
            .take_while(|(_, header_row)| *header_row <= row)
            .last()
            .map(|(section, _)| *section)
    }

    /// Shows a raised hand in each skin tone on the skin tone buttons, and in the current one on their toggle button.
    fn populate_skin_tone_buttons(&mut self, cx: &mut Cx) {
        let ChildRefs { skin_tone_button, skin_tone_buttons, .. } = self.child_refs(cx).clone();
        let Some(raised_hand) = emojis::get("\u{270b}") else { return };
        let current_skin_tone = get_skin_tone(cx);
        for (button, skin_tone) in skin_tone_buttons.iter().zip(SKIN_TONES) {
            button.set_text(cx, with_skin_tone(raised_hand, skin_tone).as_str());
        }
        skin_tone_button.set_text(cx, with_skin_tone(raised_hand, current_skin_tone).as_str());
    }

    /// Highlights the button for the given section in the category bar, and only that one,
    /// scrolling the bar to show it.
    fn highlight_section(&mut self, cx: &mut Cx, section: Option<Section>) {
        let ChildRefs { category_bar, category_buttons, .. } = self.child_refs(cx).clone();
        for (button, button_section) in category_buttons.iter().zip(CATEGORY_SECTIONS) {
            let was_active = self.active_section == Some(button_section);
            let is_active = section == Some(button_section);
            if was_active == is_active {
                continue;
            }
            let (icon_color, bg_color) = if is_active {
                (self.color_active_category_icon, self.color_active_category_bg)
            } else {
                (self.color_category_icon, self.color_category_bg)
            };
            let mut button = button.clone();
            script_apply_eval!(cx, button, {
                draw_icon +: { color: #(icon_color) }
                draw_bg +: { color: #(bg_color) }
            });
            if is_active {
                cx.send_trigger(category_bar.area(), Trigger { id: live_id!(scroll_focus_nav), from: button.area() });
            }
        }
        self.active_section = section;
    }

    /// Shows the hovered or focused emoji in the preview.
    fn populate_preview(&mut self, cx: &mut Cx) {
        let preview = self.child_refs(cx).preview.clone();
        let cell = self.hovered_cell.or(self.focused_cell).and_then(|cell| self.cells.get(cell));
        let (emoji, name, shortcode) = cell.map_or(("", "", String::new()), |cell| (
            cell.emoji.as_str(),
            cell.emoji.name(),
            get_shortcode(cell.emoji).map(|shortcode| format!(":{shortcode}:")).unwrap_or_default(),
        ));
        preview.label(cx, ids!(preview_emoji)).set_text(cx, emoji);
        preview.label(cx, ids!(preview_name)).set_text(cx, name);
        preview.label(cx, ids!(preview_shortcode)).set_text(cx, &shortcode);
    }
}

/// What a row of the grid shows, which we use to keep the same emoji at the top when the grid reflows.
#[derive(Clone, Copy, Debug)]
enum RowAnchor {
    SectionHeader(Section),
    Emoji { section: Option<Section>, emoji: &'static Emoji },
}

impl EmojiPickerRef {
    /// Resets this picker to show all emoji for the given `mode`, scrolled to the top, focusing its search if `focus_search`.
    /// `show_preview` shows the name of the emoji under the mouse, which only makes sense with a mouse.
    pub fn reset(&self, cx: &mut Cx, mode: EmojiPickerMode, focus_search: bool, show_preview: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let ChildRefs {
            search_bar,
            skin_tone_button,
            skin_tone_buttons,
            category_bar,
            skin_tones,
            category_buttons,
            preview,
            ..
        } = inner.child_refs(cx).clone();
        inner.mode = mode;
        inner.query.clear();
        inner.focused_cell = None;
        inner.hovered_cell = None;
        inner.pressed_cell = None;
        inner.is_text_reaction_hovered = false;
        inner.was_search_focused_at_press = false;
        inner.jumped_to_section = None;
        inner.needs_rebuild = true;
        search_bar.clear(cx);
        // Another account may have chosen another skin tone.
        inner.populate_skin_tone_buttons(cx);
        skin_tones.set_visible(cx, false);
        category_bar.set_visible(cx, true);
        category_bar.set_scroll_pos(cx, dvec2(0.0, 0.0));
        // The mouse may have been over a button when the picker last closed.
        for button in category_buttons.iter().chain(&skin_tone_buttons).chain([&skin_tone_button]) {
            button.reset_hover(cx);
        }
        preview.set_visible(cx, show_preview);
        inner.populate_preview(cx);
        inner.jump_grid_to(cx, 0, 0.0);
        inner.is_search_focus_pending = focus_search;
        inner.redraw(cx);
    }

    /// Toggles whether the given reaction is highlighted as one of the user's own reactions,
    /// without moving any of the emoji around.
    pub fn toggle_own_reaction(&self, cx: &mut Cx, reaction: &str) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let EmojiPickerMode::React { own_reactions } = &mut inner.mode else { return };
        let is_own_reaction = !own_reactions.remove(reaction);
        if is_own_reaction {
            own_reactions.insert(reaction.to_owned());
        }
        for cell in inner.cells.iter_mut().filter(|cell| cell.emoji.as_str() == reaction) {
            cell.is_own_reaction = is_own_reaction;
        }
        inner.redraw(cx);
    }

    /// Hides the tooltip of any of this picker's buttons, since we stop getting events once we're closed.
    pub fn hide_tooltip(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.tooltip.hide(cx);
    }

    pub fn has_search_focus(&self, cx: &mut Cx) -> bool {
        let Some(mut inner) = self.borrow_mut() else { return false };
        let search_input = inner.child_refs(cx).search_input.clone();
        cx.has_key_focus(search_input.area())
    }

}

/// Returns the emoji (or text) that the user picked in the picker with the given `widget_uid`, or that the
/// `EmojiPickerPopup` passed on to the message input with that UID, and whether they want to keep picking more.
pub fn get_picked_emoji(actions: &Actions, widget_uid: WidgetUid) -> Option<(String, bool)> {
    actions.filter_widget_actions(widget_uid).find_map(|action| match action.cast() {
        EmojiPickerAction::Picked { emoji, keep_open } => Some((emoji, keep_open)),
        EmojiPickerAction::Cancelled | EmojiPickerAction::None => None,
    })
}


/// Actions emitted by an `EmojiGridRow`, each with the index of an emoji in the picker's cells.
#[derive(Clone, Debug, Default)]
enum EmojiGridRowAction {
    /// The mouse moved over the given emoji, or off of this row if `None`.
    Hovered(Option<usize>),
    /// A finger or the mouse pressed down on the given emoji.
    Pressed(usize),
    /// The press ended, picking the given emoji if it was a tap or click on it.
    ///
    /// `keep_open` is true if Shift was held.
    Released { picked_cell: Option<usize>, keep_open: bool },
    #[default]
    None,
}

/// A row of emoji in the `EmojiPicker`'s grid, which draws all of its emoji itself.
///
/// The picker decides which emoji are highlighted, since rows get reused for other emoji as the grid scrolls.
#[derive(Script, ScriptHook, Widget)]
pub struct EmojiGridRow {
    #[uid] uid: WidgetUid,
    #[walk] walk: Walk,
    #[redraw] #[rust] area: Area,
    #[live] draw_cell: DrawColor,
    #[live] draw_emoji: DrawText,
    #[live] color_hover: Vec4f,
    #[live] color_focus: Vec4f,
    #[live] color_own_reaction: Vec4f,
    #[live] color_own_reaction_hover: Vec4f,

    #[rust] cells: Vec<GridCell>,
    /// The index of our first emoji in the picker's cells.
    #[rust] first_cell: usize,
    #[rust] cell_width: f64,
    #[rust] highlighted_column: Option<usize>,
    #[rust] focused_column: Option<usize>,
}

impl Widget for EmojiGridRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                cx.set_cursor(MouseCursor::Hand);
                let column = self.find_column_at(fe.abs.x - fe.rect.pos.x);
                if column != self.highlighted_column {
                    self.highlighted_column = column;
                    cx.widget_action(self.uid, EmojiGridRowAction::Hovered(column.map(|column| self.first_cell + column)));
                }
            }
            Hit::FingerHoverOut(_) if self.highlighted_column.take().is_some() => {
                cx.widget_action(self.uid, EmojiGridRowAction::Hovered(None));
            }
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                if let Some(column) = self.find_column_at(fe.abs.x - fe.rect.pos.x) {
                    cx.widget_action(self.uid, EmojiGridRowAction::Pressed(self.first_cell + column));
                }
            }
            Hit::FingerUp(fe) => {
                let column = self.find_column_at(fe.abs_start.x - fe.rect.pos.x)
                    .filter(|column| Some(*column) == self.find_column_at(fe.abs.x - fe.rect.pos.x));
                let picked_cell = column
                    .filter(|_| is_tap(&fe) && fe.is_over && fe.is_primary_hit())
                    .map(|column| self.first_cell + column);
                cx.widget_action(self.uid, EmojiGridRowAction::Released { picked_cell, keep_open: fe.modifiers.shift });
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle_with_area(&mut self.area, walk);
        let cell_size = dvec2(self.cell_width, rect.size.y);
        let cell_pos = |column: usize| dvec2(rect.pos.x + column as f64 * self.cell_width, rect.pos.y);

        for (column, cell) in self.cells.iter().enumerate() {
            let is_highlighted = self.highlighted_column == Some(column);
            let color = match (cell.is_own_reaction, is_highlighted) {
                (true, true) => self.color_own_reaction_hover,
                (true, false) => self.color_own_reaction,
                (false, true) => self.color_hover,
                (false, false) if self.focused_column == Some(column) => self.color_focus,
                (false, false) => continue,
            };
            self.draw_cell.color = color;
            self.draw_cell.draw_abs(cx, Rect { pos: cell_pos(column), size: cell_size });
        }

        // Every emoji is the same size, so we center them all by the size of the first one.
        let Some(first) = self.cells.first() else { return DrawStep::done() };
        let emoji_size = self.draw_emoji.layout(cx, 0.0, 0.0, None, false, Align::default(), first.emoji.as_str())
            .size_in_lpxs;
        let emoji_size = dvec2(emoji_size.width as f64, emoji_size.height as f64) * self.draw_emoji.font_scale as f64;
        let offset = (cell_size - emoji_size) * 0.5;
        self.draw_emoji.begin_many_instances(cx);
        for (column, cell) in self.cells.iter().enumerate() {
            self.draw_emoji.draw_abs(cx, cell_pos(column) + offset, cell.emoji.as_str());
        }
        self.draw_emoji.end_many_instances(cx);
        DrawStep::done()
    }
}

impl EmojiGridRow {
    /// Sets the emoji to show, which start at `first_cell` in the picker's cells,
    /// and which of them (by column) are highlighted and focused.
    fn set_cells(
        &mut self,
        cells: &[GridCell],
        first_cell: usize,
        cell_width: f64,
        highlighted_column: Option<usize>,
        focused_column: Option<usize>,
    ) {
        self.cells.clear();
        self.cells.extend_from_slice(cells);
        self.first_cell = first_cell;
        self.cell_width = cell_width;
        self.highlighted_column = highlighted_column;
        self.focused_column = focused_column;
    }

    /// Returns the column at the given x position, relative to this row's left edge.
    fn find_column_at(&self, x: f64) -> Option<usize> {
        if x < 0.0 || self.cell_width <= 0.0 {
            return None;
        }
        let column = (x / self.cell_width) as usize;
        (column < self.cells.len()).then_some(column)
    }
}

/// Returns whether the given release ends a tap or a click, since a slow mouse click is still a click
/// but a slow touch is a long press.
fn is_tap(fe: &FingerUpEvent) -> bool {
    if fe.device.has_hovers() {
        !fe.is_sweep && (fe.abs - fe.abs_start).length() < TAP_COUNT_DISTANCE
    } else {
        fe.was_tap()
    }
}
