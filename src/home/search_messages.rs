//! In-room message search (server-side, sliding pane).
//!
//! This module replaces the original client-side MVP search with a
//! right-sliding overlay pane (see `ThreadsSlidingPane` for the template
//! pattern) that calls the Matrix `POST /_matrix/client/v3/search`
//! endpoint via [`crate::sliding_sync::MatrixRequest::SearchMessages`] and
//! displays the returned events with infinite-scroll pagination.
//!
//! Three widgets are exported:
//!
//!   * [`SearchMessagesButton`] — floating circular button at the top-right
//!     of the timeline. Click → opens the pane.
//!   * [`SearchMessagesSlidingPane`] — the sliding pane itself. Owns the
//!     debounced search input, the close button, the status label, the
//!     loading indicator, the empty-state label, and the results list.
//!   * [`SearchResultItem`] — one row in the results list (sender,
//!     timestamp, body preview). Click → jumps to that event.
//!
//! Action flow:
//!   - The button emits [`SearchMessagesAction::OpenRequested`].
//!   - Each input change is debounced ~600 ms and emits
//!     [`SearchMessagesAction::QueryChanged`].
//!   - Scrolling to the bottom of the results list emits
//!     [`SearchMessagesAction::LoadMoreRequested`].
//!   - The close button emits [`SearchMessagesAction::CloseRequested`].
//!   - A result tap emits [`SearchMessagesAction::JumpToEvent`].
//!
//! The room screen owns the actual request submission and feeds the
//! resulting [`MessageSearchHit`]s back via
//! [`SearchMessagesSlidingPane::set_results`] / `append_results`.

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedEventId;
use crate::{app::AppState, i18n::{AppLanguage, tr_key}};

/// Debounce delay applied to the search input. The pane waits this long
/// after the last keystroke before emitting a `QueryChanged` action.
const SEARCH_INPUT_DEBOUNCE_SECS: f64 = 0.6;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // ---------- Floating button (top-right of the timeline) ----------
    //
    // Mirrors `JumpToBottomButton`'s layout: a Fill/Fill overlay-flow View
    // aligned to the top-right corner, containing a circular icon button.
    mod.widgets.SearchMessagesButton = #(SearchMessagesButton::register_widget(vm)) {
        width: Fill,
        height: Fill,
        flow: Overlay,
        align: Align{x: 1.0, y: 0.0},
        visible: true,

        View {
            width: 65, height: 65,
            align: Align{x: 0.5, y: 0.0},
            flow: Overlay,

            inner_button := RobrixIconButton {
                spacing: 0,
                width: 40, height: 40,
                align: Align{x: 0.5, y: 0.5},
                margin: Inset{top: 8},

                draw_icon +: {
                    svg: (ICON_SEARCH),
                    color: #555,
                }
                icon_walk: Walk{width: 18, height: 18}

                draw_bg +: {
                    background_color: #edededce,
                    background_color_hover: #d0d0d0ce,
                    pixel: fn() {
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                        let c = self.rect_size * 0.5;
                        sdf.circle(c.x, c.x, c.x);
                        sdf.fill_keep(mix(self.background_color, self.background_color_hover, self.hover));
                        return sdf.result
                    }
                }
            }
        }
    }


    // ---------- One row in the results list ----------
    //
    // A View that captures finger-up taps and emits a JumpToEvent action.
    // The event_id is stored on the Rust side via `set_hit` before each draw.
    // We deliberately skip an animated hover state — `cursor: MouseCursor.Hand`
    // is enough affordance and avoids the frozen-vec pitfall of declaring an
    // instance variable inside a parent's `draw_bg +:` block.
    mod.widgets.SearchResultItem = #(SearchResultItem::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Down,
        padding: Inset{top: 8, bottom: 8, left: 15, right: 15},
        spacing: 3,
        cursor: MouseCursor.Hand,

        show_bg: true,
        draw_bg +: {
            color: #00000000,
        }

        sender_row := View {
            width: Fill, height: Fit,
            flow: Right,
            align: Align{y: 0.5},
            spacing: 6,

            sender_label := Label {
                width: Fit, height: Fit,
                draw_text +: {
                    color: (COLOR_TEXT)
                    text_style: USERNAME_TEXT_STYLE { font_size: 10.5 }
                }
                text: ""
            }
            ts_label := Label {
                width: Fit, height: Fit,
                draw_text +: {
                    color: #888
                    text_style: theme.font_regular { font_size: 9.0 }
                }
                text: ""
            }
        }

        body_label := Label {
            width: Fill, height: Fit,
            flow: Flow.Right{wrap: true},
            draw_text +: {
                color: (COLOR_TEXT)
                text_style: MESSAGE_TEXT_STYLE { font_size: 10.0 }
            }
            text: ""
        }
    }


    // ---------- Right sliding pane (mirrors ThreadsSlidingPane) ----------
    mod.widgets.SearchMessagesSlidingPane = #(SearchMessagesSlidingPane::register_widget(vm)) {
        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        align: Align{x: 1.0, y: 0}

        bg_view := SolidView {
            width: Fill
            height: Fill
            visible: false,
            show_bg: true
            draw_bg.color: #000000BB
        }

        main_content := SolidView {
            width: 360,
            height: Fill
            flow: Down,
            align: Align{x: 1.0}

            show_bg: true,
            draw_bg.color: (COLOR_PRIMARY)

            header := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                padding: Inset{top: 12, right: 10, bottom: 12, left: 15}

                title := Label {
                    width: Fit
                    height: Fit
                    draw_text +: {
                        text_style: USERNAME_TEXT_STYLE { font_size: 12.5 }
                        color: #000
                    }
                    text: "Search Messages"
                }

                spacer := View {
                    width: Fill
                    height: Fit
                }

                close_button := RobrixNeutralIconButton {
                    width: Fit,
                    height: Fit,
                    spacing: 0,
                    padding: 15,
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 14, height: 14}
                    text: ""
                }
            }

            // Search input bar.
            input_bar := RoundedView {
                width: Fill, height: 35,
                show_bg: true,
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    border_radius: 4.0
                    border_color: (COLOR_SECONDARY)
                    border_size: 1.0
                }
                padding: Inset{top: 3, bottom: 3, left: 10, right: 4.5}
                margin: Inset{left: 15, right: 15, bottom: 8}
                spacing: 4,
                align: Align{x: 0.0, y: 0.5},

                Icon {
                    draw_icon +: {
                        svg: (ICON_SEARCH),
                        color: (COLOR_TEXT_INPUT_IDLE),
                    }
                    icon_walk: Walk{width: 14, height: 14}
                }

                input := RobrixTextInput {
                    width: Fill,
                    height: Fit,
                    flow: Right,
                    padding: 5,
                    empty_text: "Search messages..."
                    draw_bg.border_size: 0.0
                    draw_text +: {
                        text_style: theme.font_regular { font_size: 10 },
                    }
                }

                clear_button := RobrixNeutralIconButton {
                    visible: false,
                    margin: 0,
                    padding: Inset{top: 5, bottom: 5, left: 9, right: 9},
                    spacing: 0,
                    align: Align{x: 0.5, y: 0.5},
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: Fit, height: 10, margin: 0}
                }
            }

            // Status label: count summary or transient hints.
            status_label := Label {
                width: Fill, height: Fit,
                visible: false,
                padding: Inset{left: 15, right: 15, top: 0, bottom: 6},
                draw_text +: {
                    color: #6E6E6E,
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10.0 }
                }
                text: "",
            }

            // Loading indicator (shown while the server query is in flight).
            loading_indicator := View {
                visible: false
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                spacing: 8
                padding: Inset{left: 15, right: 15, top: 6, bottom: 10}

                spinner := LoadingSpinner {
                    width: 18
                    height: 18
                }
                loading_label := Label {
                    width: Fit, height: Fit,
                    draw_text +: {
                        text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
                        color: #7B7B7B
                    }
                    text: "Searching..."
                }
            }

            // Empty state (no query / no results / encrypted room).
            empty_state := Label {
                visible: true,
                width: Fill, height: Fit,
                flow: Flow.Right{wrap: true},
                padding: Inset{left: 15, right: 15, top: 20, bottom: 20},
                draw_text +: {
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
                    color: #7B7B7B
                }
                text: "Type to search messages in this room.",
            }

            // The results list. Scrolling near the bottom triggers
            // pagination via SearchMessagesAction::LoadMoreRequested.
            // Visibility is toggled at runtime via `refresh_status_display`;
            // declaring `visible: false` here errors out because PortalList
            // doesn't expose the `visible` property in the DSL.
            results_list := PortalList {
                width: Fill,
                height: Fill,
                flow: Down,
                max_pull_down: 0.0,

                result_item := mod.widgets.SearchResultItem {}
            }
        }

        slide: 1.0,

        animator: Animator {
            panel: {
                default: @hide
                show: AnimatorState {
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: Ease.ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {
                        slide: 0.0
                    }
                }
                hide: AnimatorState {
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: Ease.ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {
                        slide: 1.0
                    }
                }
            }
        }
    }
}

const PANE_DESKTOP_WIDTH: f32 = 360.0;
const PANE_MOBILE_BREAKPOINT: f32 = 700.0;

// ============================== Actions ==============================

#[derive(Clone, Debug, Default)]
pub enum SearchMessagesAction {
    /// The floating button was clicked — the room should show the pane.
    OpenRequested,
    /// The pane's close button was clicked — the room should hide the pane.
    CloseRequested,
    /// The query in the search input changed (already trimmed). Fired only
    /// after the debounce timer has elapsed.
    QueryChanged(String),
    /// The user has scrolled to the bottom of the results list and a
    /// `next_batch` token exists — the room should submit a paginated
    /// follow-up request.
    LoadMoreRequested,
    /// The user clicked a result row; jump to this event in the timeline.
    JumpToEvent(OwnedEventId),
    /// Default sentinel emitted by `WidgetAction::cast_ref` when the
    /// underlying action is *not* a [`SearchMessagesAction`]. The room
    /// screen ignores this variant.
    #[default]
    None,
}

impl ActionDefaultRef for SearchMessagesAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: SearchMessagesAction = SearchMessagesAction::None;
        &DEFAULT
    }
}

// ============================== Hit ==============================

/// A single match presented in the results list. Built by the room screen
/// from a [`crate::sliding_sync::SearchedMessage`] (which carries server
/// data) and pushed into the pane via
/// [`SearchMessagesSlidingPane::set_results`] /
/// [`SearchMessagesSlidingPane::append_results`].
#[derive(Clone, Debug)]
pub struct MessageSearchHit {
    pub event_id: OwnedEventId,
    /// Sender display name (falls back to user ID when unavailable).
    pub sender_display: String,
    /// Human-formatted timestamp, e.g. `"2024-03-12 10:14"`.
    pub timestamp_display: String,
    /// Plaintext body, already truncated to a reasonable preview length.
    pub body_preview: String,
}

// ============================== Button ==============================

#[derive(Script, ScriptHook, Widget)]
pub struct SearchMessagesButton {
    #[deref] view: View,
    #[rust] app_language: AppLanguage,
}

impl Widget for SearchMessagesButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.app_language = app_language;
        }

        // Show a tooltip on hover, like JumpToBottomButton does.
        let button_area = self.button(cx, ids!(inner_button)).area();
        match event.hits(cx, button_area) {
            Hit::FingerHoverIn(_) | Hit::FingerLongPress(_) => {
                cx.widget_action(
                    self.widget_uid(),
                    TooltipAction::HoverIn {
                        text: tr_key(self.app_language, "search_messages.button.tooltip").to_string(),
                        widget_rect: button_area.rect(cx),
                        options: CalloutTooltipOptions {
                            position: TooltipPosition::Left,
                            ..Default::default()
                        },
                    },
                );
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(), TooltipAction::HoverOut);
            }
            _ => {}
        }

        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            if self.button(cx, ids!(inner_button)).clicked(actions) {
                cx.widget_action(self.widget_uid(), SearchMessagesAction::OpenRequested);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

// ============================== Result item ==============================

#[derive(Script, ScriptHook, Widget)]
pub struct SearchResultItem {
    #[deref] view: View,
    /// The matrix event this row represents. `None` until the row is bound
    /// to a [`MessageSearchHit`] via [`SearchResultItem::set_hit`].
    #[rust] event_id: Option<OwnedEventId>,
}

impl Widget for SearchResultItem {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let Some(event_id) = self.event_id.as_ref() else { return };
        let area = self.view.area();
        if let Hit::FingerUp(fe) = event.hits(cx, area) {
            if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                cx.widget_action(
                    self.widget_uid(),
                    SearchMessagesAction::JumpToEvent(event_id.clone()),
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SearchResultItem {
    pub fn set_hit(&mut self, cx: &mut Cx, hit: &MessageSearchHit) {
        self.event_id = Some(hit.event_id.clone());
        self.label(cx, ids!(sender_label)).set_text(cx, &hit.sender_display);
        self.label(cx, ids!(ts_label)).set_text(cx, &hit.timestamp_display);
        self.label(cx, ids!(body_label)).set_text(cx, &hit.body_preview);
    }
}

// ============================== Pane ==============================

/// Status displayed under the search input bar. The pane keeps the
/// outermost piece of state (loading / results / empty / error / encrypted)
/// in this enum so the draw step can pick the right visible widget.
#[derive(Clone, Debug)]
enum PaneStatus {
    /// No query has been entered yet.
    Idle,
    /// A request is in flight for `query`.
    Loading { query: String },
    /// Server returned 0 matches for the current query.
    NoResults,
    /// Server returned at least one match. `total` is the server-reported
    /// total; `shown` may be less while pagination is still in progress.
    Results { total: u64, shown: usize, has_more: bool },
    /// Search is unsupported for this room (encrypted).
    Encrypted,
    /// Last request failed with this error.
    Failed { error: String },
}

#[derive(Script, ScriptHook, Widget, Animator)]
pub struct SearchMessagesSlidingPane {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    #[live] slide: f32,

    #[rust] app_language: AppLanguage,
    #[rust] hits: Vec<MessageSearchHit>,
    #[rust(PaneStatus::Idle)] status: PaneStatus,
    /// The query the user is currently typing. Differs from the query
    /// stamped on `status` when the debounce timer is still pending.
    #[rust] pending_query: String,
    /// Debounce timer; fires `QueryChanged` after the user stops typing.
    #[rust] debounce_timer: Timer,
    #[rust] is_animating_out: bool,
}

impl Widget for SearchMessagesSlidingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }

        // Fire the debounced QueryChanged action when the timer elapses.
        if let Event::Timer(te) = event {
            if self.debounce_timer.is_timer(te).is_some() {
                cx.widget_action(
                    self.widget_uid(),
                    SearchMessagesAction::QueryChanged(self.pending_query.clone()),
                );
            }
        }

        self.view.handle_event(cx, event, scope);

        if !self.visible { return; }

        let animator_action = self.animator_handle_event(cx, event);
        if animator_action.must_redraw() {
            self.redraw(cx);
        }
        if self.is_animating_out && !self.animator.is_track_animating(id!(panel)) {
            self.visible = false;
            self.is_animating_out = false;
            cx.revert_key_focus();
            self.view(cx, ids!(bg_view)).set_visible(cx, false);
            self.redraw(cx);
            return;
        }

        // Esc / back / outside-click → request close.
        let area = self.view.area();
        let request_close = {
            event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerDown(_fde) => {
                    cx.set_key_focus(area);
                    false
                }
                Hit::FingerUp(fue) if fue.is_over => {
                    fue.mouse_button().is_some_and(|b| b.is_back())
                    || !self.view(cx, ids!(main_content)).area().rect(cx).contains(fue.abs)
                }
                _ => false,
            }
        };
        if request_close {
            cx.widget_action(self.widget_uid(), SearchMessagesAction::CloseRequested);
        }

        if let Event::Actions(actions) = event {
            if self.button(cx, ids!(close_button)).clicked(actions) {
                cx.widget_action(self.widget_uid(), SearchMessagesAction::CloseRequested);
            }

            let input = self.text_input(cx, ids!(input));
            let clear_button = self.button(cx, ids!(clear_button));

            // Track input changes — debounce before emitting QueryChanged.
            if let Some(keywords) = input.changed(actions) {
                let trimmed = keywords.trim();
                let trimmed = if trimmed.len() < keywords.len() {
                    trimmed.to_string()
                } else {
                    keywords
                };
                clear_button.set_visible(cx, !trimmed.is_empty());
                clear_button.reset_hover(cx);
                self.pending_query = trimmed;
                if self.pending_query.is_empty() {
                    // Cancel pending debounce and fire immediately so the
                    // room screen can abort any in-flight request and reset
                    // the pane to its idle state.
                    self.debounce_timer = Timer::empty();
                    cx.widget_action(
                        self.widget_uid(),
                        SearchMessagesAction::QueryChanged(String::new()),
                    );
                } else {
                    self.debounce_timer = cx.start_timeout(SEARCH_INPUT_DEBOUNCE_SECS);
                }
            }

            if clear_button.clicked(actions) {
                input.set_text(cx, "");
                clear_button.set_visible(cx, false);
                input.set_key_focus(cx);
                self.pending_query.clear();
                self.debounce_timer = Timer::empty();
                cx.widget_action(
                    self.widget_uid(),
                    SearchMessagesAction::QueryChanged(String::new()),
                );
            }

            // Pagination: when scrolled to the very bottom of the list and
            // more results are available, ask for the next page.
            let results_list = self.portal_list(cx, ids!(results_list));
            if results_list.scrolled(actions) {
                let has_more = matches!(self.status, PaneStatus::Results { has_more: true, .. });
                if has_more
                    && !self.hits.is_empty()
                    && results_list.first_id() + results_list.visible_items() >= self.hits.len().saturating_sub(1)
                {
                    cx.widget_action(
                        self.widget_uid(),
                        SearchMessagesAction::LoadMoreRequested,
                    );
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Apply slide animation: panel slides in from the right.
        let container_width = self.view.area().rect(cx).size.x as f32;
        let panel_width = if container_width > 1.0 && container_width < PANE_MOBILE_BREAKPOINT {
            container_width
        } else {
            PANE_DESKTOP_WIDTH
        };
        let right_margin = -(self.slide * panel_width);
        let mut main_content = self.view(cx, ids!(main_content));
        script_apply_eval!(cx, main_content, {
            width: #(panel_width)
            margin.right: #(right_margin)
        });
        let bg_alpha = (1.0 - self.slide) * 0.733;
        let bg_color = vec4(0.0, 0.0, 0.0, bg_alpha);
        let mut bg_view = self.view(cx, ids!(bg_view));
        script_apply_eval!(cx, bg_view, {
            draw_bg +: { color: #(bg_color) }
        });

        self.refresh_status_display(cx);

        while let Some(widget) = self.view.draw_walk(cx, scope, walk).step() {
            let portal_list_ref = widget.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            list.set_item_range(cx, 0, self.hits.len());
            while let Some(idx) = list.next_visible_item(cx) {
                let Some(hit) = self.hits.get(idx) else { continue };
                let item = list.item(cx, idx, id!(result_item));
                if let Some(mut row) = item.as_search_result_item().borrow_mut() {
                    row.set_hit(cx, hit);
                }
                item.draw_all(cx, &mut Scope::empty());
            }
        }
        DrawStep::done()
    }
}

impl SearchMessagesSlidingPane {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.view
            .text_input(cx, ids!(input))
            .set_empty_text(
                cx,
                tr_key(self.app_language, "search_messages.input.placeholder").to_string(),
            );
    }

    /// Pushes the current `status` into the visible widgets (status label,
    /// loading indicator, empty state, results list). Called from draw_walk.
    fn refresh_status_display(&mut self, cx: &mut Cx) {
        let lang = self.app_language;
        let (status_text, status_visible, loading_visible, empty_visible, empty_text, results_visible) =
            match &self.status {
                PaneStatus::Idle => (
                    String::new(), false, false,
                    true, tr_key(lang, "search_messages.empty.idle").to_string(),
                    false,
                ),
                PaneStatus::Loading { query } => (
                    format!("Searching for \u{201C}{}\u{201D}…", truncate_for_display(query, 40)),
                    true, true, false, String::new(), !self.hits.is_empty(),
                ),
                PaneStatus::NoResults => (
                    String::new(), false, false,
                    true, tr_key(lang, "search_messages.status.no_results").to_string(),
                    false,
                ),
                PaneStatus::Results { total, shown, has_more } => {
                    let body = if *has_more {
                        format!("Showing {} of {} matches", shown, total)
                    } else if *total == 1 {
                        "1 match".to_string()
                    } else {
                        format!("{total} matches")
                    };
                    (body, true, false, false, String::new(), true)
                }
                PaneStatus::Encrypted => (
                    String::new(), false, false,
                    true, tr_key(lang, "search_messages.status.encrypted").to_string(),
                    false,
                ),
                PaneStatus::Failed { error } => (
                    format!("Search failed: {error}"),
                    true, false,
                    self.hits.is_empty(), tr_key(lang, "search_messages.status.failed").to_string(),
                    !self.hits.is_empty(),
                ),
            };

        let status_label = self.label(cx, ids!(status_label));
        status_label.set_visible(cx, status_visible);
        if status_visible {
            status_label.set_text(cx, &status_text);
        }
        self.view(cx, ids!(loading_indicator)).set_visible(cx, loading_visible);
        let empty_label = self.label(cx, ids!(empty_state));
        empty_label.set_visible(cx, empty_visible);
        if empty_visible {
            empty_label.set_text(cx, &empty_text);
        }
        self.view(cx, ids!(results_list)).set_visible(cx, results_visible);
    }

    /// Replace the result list with the first page of a new search.
    pub fn set_results(
        &mut self,
        cx: &mut Cx,
        _query: String,
        hits: Vec<MessageSearchHit>,
        total_count: u64,
        has_more: bool,
    ) {
        self.hits = hits;
        if self.hits.is_empty() {
            self.status = PaneStatus::NoResults;
        } else {
            self.status = PaneStatus::Results {
                total: total_count.max(self.hits.len() as u64),
                shown: self.hits.len(),
                has_more,
            };
        }
        let results_list = self.portal_list(cx, ids!(results_list));
        results_list.set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
    }

    /// Append a follow-up page to the existing result list (pagination).
    pub fn append_results(
        &mut self,
        cx: &mut Cx,
        mut hits: Vec<MessageSearchHit>,
        total_count: u64,
        has_more: bool,
    ) {
        self.hits.append(&mut hits);
        self.status = PaneStatus::Results {
            total: total_count.max(self.hits.len() as u64),
            shown: self.hits.len(),
            has_more,
        };
        self.redraw(cx);
    }

    pub fn set_loading(&mut self, cx: &mut Cx, query: String) {
        self.status = PaneStatus::Loading { query };
        self.redraw(cx);
    }

    pub fn set_idle(&mut self, cx: &mut Cx) {
        self.hits.clear();
        self.status = PaneStatus::Idle;
        self.redraw(cx);
    }

    pub fn set_encrypted(&mut self, cx: &mut Cx) {
        self.hits.clear();
        self.status = PaneStatus::Encrypted;
        self.redraw(cx);
    }

    pub fn set_error(&mut self, cx: &mut Cx, error: String) {
        self.status = PaneStatus::Failed { error };
        self.redraw(cx);
    }

    /// Current pending query (post-debounce) from the input.
    pub fn pending_query(&self) -> &str {
        &self.pending_query
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.visible = true;
        self.is_animating_out = false;
        self.animator_play(cx, ids!(panel.show));
        self.view(cx, ids!(bg_view)).set_visible(cx, true);
        self.text_input(cx, ids!(input)).set_key_focus(cx);
        self.view.button(cx, ids!(close_button)).reset_hover(cx);
        self.redraw(cx);
    }

    pub fn hide(&mut self, cx: &mut Cx) {
        if !self.visible {
            return;
        }
        self.is_animating_out = true;
        self.animator_play(cx, ids!(panel.hide));
        self.redraw(cx);
    }

    pub fn is_currently_shown(&self) -> bool {
        self.visible
    }

    /// Reset the pane to its just-opened state (clears input, results,
    /// debounce timer). Call this when switching rooms or whenever the
    /// pane is hidden and should not retain state.
    pub fn reset(&mut self, cx: &mut Cx) {
        self.hits.clear();
        self.status = PaneStatus::Idle;
        self.pending_query.clear();
        self.debounce_timer = Timer::empty();
        self.text_input(cx, ids!(input)).set_text(cx, "");
        self.button(cx, ids!(clear_button)).set_visible(cx, false);
        self.redraw(cx);
    }
}

impl SearchMessagesSlidingPaneRef {
    pub fn show(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() { inner.show(cx); }
    }
    pub fn hide(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() { inner.hide(cx); }
    }
    pub fn is_currently_shown(&self) -> bool {
        self.borrow().map(|p| p.is_currently_shown()).unwrap_or(false)
    }
    pub fn reset(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() { inner.reset(cx); }
    }
    pub fn set_results(
        &self,
        cx: &mut Cx,
        query: String,
        hits: Vec<MessageSearchHit>,
        total_count: u64,
        has_more: bool,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_results(cx, query, hits, total_count, has_more);
        }
    }
    pub fn append_results(
        &self,
        cx: &mut Cx,
        hits: Vec<MessageSearchHit>,
        total_count: u64,
        has_more: bool,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.append_results(cx, hits, total_count, has_more);
        }
    }
    pub fn set_loading(&self, cx: &mut Cx, query: String) {
        if let Some(mut inner) = self.borrow_mut() { inner.set_loading(cx, query); }
    }
    pub fn set_idle(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() { inner.set_idle(cx); }
    }
    pub fn set_encrypted(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() { inner.set_encrypted(cx); }
    }
    pub fn set_error(&self, cx: &mut Cx, error: String) {
        if let Some(mut inner) = self.borrow_mut() { inner.set_error(cx, error); }
    }
    pub fn pending_query(&self) -> String {
        self.borrow().map(|p| p.pending_query().to_string()).unwrap_or_default()
    }
}

fn truncate_for_display(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push('…');
    out
}
