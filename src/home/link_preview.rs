//! A link preview widget that provides a method to populate link preview view for setting its' children.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use makepad_widgets::*;
use crate::{LivePtr, utils, widget_ref_from_live_ptr};
use matrix_sdk::ruma::{events::room::{ImageInfo, MediaSource}, OwnedMxcUri, UInt};
use serde::{Deserialize, Deserializer};
use url::Url;

use crate::{
    home::room_screen::TimelineUpdate,
    media_cache::{MediaCache, MediaCacheEntry},
    settings::app_preferences::{AppPreferencesGlobal, LinkPreviewStyle},
    shared::{lazy_view::LazyViewWidgetExt, styles::set_view_bg_color, text_or_image::{TextOrImageRef, TextOrImageWidgetRefExt}},
    sliding_sync::{submit_async_request, MatrixRequest, UrlPreviewError},
};

/// Maximum number of cache entries before cleanup is triggered
const MAX_CACHE_ENTRIES_BEFORE_CLEANUP: usize = 100;
/// Maximum age for cache entries in seconds (1 hour)
const CACHE_ENTRY_MAX_AGE_SECS: u64 = 3600;
/// How many previews we show before hiding the rest behind a "show more" button.
const MAX_DEFAULT_VISIBLE_PREVIEWS: usize = 2;
/// The tallest a link preview card can be, including its image and text.
const MAX_CARD_HEIGHT: f64 = 300.0;

/// An entry in the Link Preview cache with timestamp for cleanup.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub struct TimestampedCacheEntry {
    pub entry: LinkPreviewCacheEntry,
    pub timestamp: Instant,
}

/// An entry in the Link Preview cache.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum LinkPreviewCacheEntry {
    Requested,
    LoadedLinkPreview(LinkPreviewData),
    Failed,
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE = theme.font_regular {
        font_size: (16),
        line_spacing: (1.2),
    }

    mod.widgets.LinkPreviewImageArea = RoundedView {
        align: Align{x: 0.5, y: 0.5}
        show_bg: true
        draw_bg +: {
            color: (COLOR_BG_LINK_PREVIEW_IMAGE)
            border_radius: 3.0
        }
    }

    // Says that a link has no image, in the space where its image would be.
    mod.widgets.LinkPreviewNoImageMessage = View {
        width: Fill, height: Fit
        flow: Down
        align: Align{x: 0.5}
        spacing: 4
        padding: Inset{left: 6, right: 6}

        Icon {
            width: Fit, height: Fit
            draw_icon +: {
                svg: (ICON_BROKEN_IMAGE)
                color: (COLOR_TEXT_MUTED)
            }
            icon_walk: Walk{width: 24, height: 24}
        }
        Label {
            width: Fill, height: Fit
            padding: 0
            flow: Flow.Right{wrap: true}
            align: Align{x: 0.5}
            draw_text +: {
                text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                    font_size: 10.0,
                    line_spacing: 1.0,
                },
                color: (COLOR_TEXT_MUTED),
            }
            text: "No preview available, click to open"
        }
    }

    // A link preview's image, which has a spinner or the no-image message while it's missing.
    mod.widgets.LinkPreviewImage = mod.widgets.LinkPreviewImageArea {
        flow: Overlay
        image := TextOrImage {
            width: Fill, height: Fill
            image_view +: {
                height: Fill
                flow: Down
                align: Align{x: 0.5, y: 0.5}
                image +: { height: Fill }
            }
        }
        placeholder := View {
            width: Fill, height: Fill
            align: Align{x: 0.5, y: 0.5}
            loading_spinner := LoadingSpinner {
                width: 32, height: 32
                draw_bg +: { color: (COLOR_ACTIVE_PRIMARY) }
            }
            no_image_message := mod.widgets.LinkPreviewNoImageMessage { visible: false }
        }
    }

    // What a card shows in place of its image when it has enough text to go without one.
    mod.widgets.LinkPreviewNoImage = mod.widgets.LinkPreviewImageArea {
        visible: false
        mod.widgets.LinkPreviewNoImageMessage {}
    }

    mod.widgets.LinkPreviewTitle = Label {
        width: Fill, height: Fit
        padding: 0
        flow: Flow.Right{wrap: true}
        max_lines: 2
        text_overflow: Ellipsis
        draw_text +: {
            text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                font_size: 12.0,
            },
            color: (COLOR_LINK),
        }
    }

    mod.widgets.FillLinesLabel = #(FillLinesLabel::register_widget(vm)) {
        ..mod.widgets.Label
    }

    // A link preview's title and description, which shows as much of the description as fits.
    // It's just tall enough for 4 lines, e.g., a 2-line title and a 2-line description.
    mod.widgets.LinkPreviewText = View {
        width: Fill, height: 88
        flow: Down
        align: Align{y: 0.5}
        // A bit more than the gap between the description's lines, which its line spacing adds.
        spacing: 6

        title_label := mod.widgets.LinkPreviewTitle {}
        description_label := mod.widgets.FillLinesLabel {
            width: Fill, height: Fit
            padding: 0
            flow: Flow.Right{wrap: true}
            text_overflow: Ellipsis
            draw_text +: {
                text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                    font_size: 11.0,
                },
                color: (COLOR_TEXT_MUTED),
            }
        }
    }

    mod.widgets.LinkPreviewCardBase = RoundedView {
        cursor: MouseCursor.Hand
        width: Fill, height: Fit
        margin: Inset{ top: 7 }
        padding: 8
        show_bg: true
        draw_bg +: {
            color: (COLOR_BG_PREVIEW_TRANSLUCENT)
            border_radius: 4.0
        }
    }

    // A link preview card that shows its image beside its text,
    // or above its text once the card is narrower than 450px.
    mod.widgets.LinkPreviewCard = mod.widgets.LinkPreviewCardBase {
        // `cqw` is the message's width here. The card's height never depends on its image,
        // so its text can take the image's space if there isn't one.
        height: "104px + max(0px, min(8px + max(88px, (100cqw - 16px) * 0.5236), (466px - 100cqw) * 10000))"
        flow: Flow.Right{wrap: true}
        wrap_spacing: 8
        container_id: @link_preview_card

        image_view := mod.widgets.LinkPreviewImage {
            width: "max(120px, min(100%, (450px - 100%) * 10000))"
            // Above the text, a 1.91:1 image (the usual shape for a link) fills this whole area.
            height: "max(88px, min(52.36cqw, (450px - 100cqw) * 10000))"
            // This margin (rather than the card's spacing) leaves no gap once the image is gone.
            margin: Inset{ right: 10 }
        }
        // Above the text, this takes the image's place. Beside the text, it takes no space at all,
        // since the text takes the image's space instead.
        no_image_view := mod.widgets.LinkPreviewNoImage {
            width: "max(0px, min(100%, (450px - 100%) * 10000))"
            height: "max(0px, min(max(88px, 52.36cqw), (450px - 100cqw) * 10000))"
        }
        content_view := mod.widgets.LinkPreviewText {
            width: "max(100% - 131px, min(100%, (450px - 100%) * 10000))"
        }
    }

    mod.widgets.CompactLinkPreviewCard = mod.widgets.LinkPreviewCard {
        height: 104
        flow: Right
        image_view +: { width: 120, height: 88 }
        no_image_view +: { width: 0, height: 0 }
        content_view +: { width: Fill }
    }

    mod.widgets.StackedLinkPreviewCard = mod.widgets.LinkPreviewCard {
        height: "112px + (100cqw - 16px) * 0.5236"
        flow: Down
        spacing: 8
        image_view +: { width: Fill, height: "52.36cqw", margin: 0 }
        no_image_view +: { width: Fill, height: "52.36cqw" }
        content_view +: { width: Fill }
    }

    // A card for a link to an image (like a GIF), which shows that image bigger.
    // It's only as wide as its image and title need.
    mod.widgets.LargeImageLinkPreviewCard = mod.widgets.LinkPreviewCardBase {
        width: Fit
        // `cqw` is the message's width here. The card's height never depends on its title,
        // so its image can take the title's space if there isn't one.
        height: "42px + (100cqw - 16px) * 0.5625"
        flow: Down
        spacing: 6

        image_view := mod.widgets.LinkPreviewImage {
            // `cqw` is the message's width here, and a 16:9 image takes up this whole height.
            // `populate_card` caps it, so the whole card fits in `MAX_CARD_HEIGHT`.
            width: Fit, height: "(100cqw - 16px) * 0.5625"
            // We don't center it across, since that'd stretch it out to the message's width.
            align: Align{x: 0.0, y: 0.5}
            // Without an image, this keeps its area as wide as a 16:9 image.
            placeholder +: { width: Fit{max: "100cqw - 16px"}, aspect: 1.7778 }
            image +: {
                width: Fit
                image_view +: {
                    width: Fit
                    align: Align{x: 0.0, y: 0.5}
                    image +: { width: Fit{max: "100cqw - 16px"} }
                }
            }
        }
        no_image_view := mod.widgets.LinkPreviewNoImage {
            width: Fit, height: "(100cqw - 16px) * 0.5625"
            aspect: 1.7778
        }
        content_view := View {
            width: Fit, height: 20
            title_label := mod.widgets.LinkPreviewTitle {
                width: Fit{max: "100cqw - 16px"}
                max_lines: 1
            }
        }
    }

    mod.widgets.LinkPreview = #(LinkPreview::register_widget(vm)) {
        width: Fill, height: Fit,
        flow: Down,
        card_bg_color: (COLOR_BG_PREVIEW_TRANSLUCENT)
        card_bg_color_hover: (COLOR_BG_PREVIEW_HOVER)

        collapsible_buttons := LazyView {
            template: View {
                width: Fill, height: Fit,
                flow: Right,
                align: Align{x: 0.5, y: 0.5},
                padding: Inset{top: 4},

                expand_button := RobrixIconButton {
                    width: Fit, height: Fit,
                    spacing: 4,
                    padding: Inset{top: 4, bottom: 4, left: 8, right: 8},
                    draw_icon +: {
                        svg: (ICON_TRIANGLE_DOWN)
                        color: (COLOR_TEXT_MUTED)
                    }
                    icon_walk: Walk{width: 10, height: 10}
                    draw_text +: {
                        text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                            font_size: 10.0,
                        },
                        color: (COLOR_TEXT_MUTED),
                        color_hover: (COLOR_TEXT_MUTED),
                        color_down: (COLOR_TEXT_MUTED),
                    }
                    draw_bg +: {
                        color: (COLOR_BG_PREVIEW)
                        color_hover: (COLOR_BG_PREVIEW_HOVER)
                        color_down: (COLOR_BG_PREVIEW_DOWN)
                        border_size: 1.0
                        border_color: (COLOR_PREVIEW_TOGGLE_BUTTON_BORDER)
                        border_color_hover: (COLOR_PREVIEW_TOGGLE_BUTTON_BORDER)
                        border_color_down: (COLOR_PREVIEW_TOGGLE_BUTTON_BORDER)
                        border_radius: 4.0
                    }
                    text: "Show more links"
                }

                collapse_button := RobrixIconButton {
                    visible: false,
                    width: Fit, height: Fit,
                    spacing: 4,
                    padding: Inset{top: 4, bottom: 4, left: 8, right: 8},
                    draw_icon +: {
                        svg: (ICON_TRIANGLE_UP)
                        color: (COLOR_TEXT_MUTED)
                    }
                    icon_walk: Walk{width: 10, height: 10}
                    draw_text +: {
                        text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                            font_size: 10.0,
                        },
                        color: (COLOR_TEXT_MUTED),
                        color_hover: (COLOR_TEXT_MUTED),
                        color_down: (COLOR_TEXT_MUTED),
                    }
                    draw_bg +: {
                        color: (COLOR_BG_PREVIEW)
                        color_hover: (COLOR_BG_PREVIEW_HOVER)
                        color_down: (COLOR_BG_PREVIEW_DOWN)
                        border_size: 1.0
                        border_color: (COLOR_PREVIEW_TOGGLE_BUTTON_BORDER)
                        border_color_hover: (COLOR_PREVIEW_TOGGLE_BUTTON_BORDER)
                        border_color_down: (COLOR_PREVIEW_TOGGLE_BUTTON_BORDER)
                        border_radius: 4.0
                    }
                    text: "Show fewer links"
                }
            }
        }

        card_template: mod.widgets.LinkPreviewCard {}
        compact_card_template: mod.widgets.CompactLinkPreviewCard {}
        stacked_card_template: mod.widgets.StackedLinkPreviewCard {}
        large_image_card_template: mod.widgets.LargeImageLinkPreviewCard {}
    }
}

#[derive(Clone, Debug, Default)]
enum LinkPreviewAction {
    /// The user clicked the button that shows or hides the previews past the first few.
    Toggled { is_expanded: bool },
    #[default]
    None,
}

/// Which template a link preview card was built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CardKind {
    /// Shows its image beside its text, or above it if the card is narrow.
    Responsive,
    Compact,
    Stacked,
    LargeImage,
}

/// A single link preview card, and the link that it previews.
struct LinkPreviewCard {
    link: Url,
    kind: CardKind,
    view: ViewRef,
    /// The walk of the card's text when it leaves room for an image.
    text_walk: Walk,
    /// The height of the card's image when it leaves room for text.
    image_height: Size,
}

#[derive(Script, WidgetRef, WidgetSet, WidgetRegister)]
pub struct LinkPreview {
    #[deref]
    view: View,
    #[live]
    card_template: Option<LivePtr>,
    #[live]
    compact_card_template: Option<LivePtr>,
    #[live]
    stacked_card_template: Option<LivePtr>,
    #[live]
    large_image_card_template: Option<LivePtr>,
    #[live]
    card_bg_color: Vec4f,
    #[live]
    card_bg_color_hover: Vec4f,
    /// The list of all previews, one preview card per link.
    ///
    /// Each card is a widget instantiated from one of the above templates.
    #[rust]
    cards: Vec<LinkPreviewCard>,
    #[layout]
    layout: Layout,
    #[rust]
    is_expanded: bool,
    #[rust]
    num_hidden_links: usize,
}

impl ScriptHook for LinkPreview {
    fn on_after_apply(&mut self, vm: &mut ScriptVm, apply: &Apply, scope: &mut Scope, _value: ScriptValue) {
        // A reload never reaches our cards, so we re-apply their templates to them ourselves.
        if apply.is_reload() {
            let mut cards = std::mem::take(&mut self.cards);
            for card in cards.iter_mut() {
                if let Some(template) = self.get_card_template(card.kind) {
                    card.view.script_apply(vm, apply, scope, template);
                }
            }
            self.cards = cards;
        }
    }
}

impl WidgetNode for LinkPreview {
    fn widget_uid(&self) -> WidgetUid { self.view.widget_uid() }
    fn walk(&mut self, cx: &mut Cx) -> Walk { self.view.walk(cx) }
    fn area(&self) -> Area { self.view.area() }
    fn redraw(&mut self, cx: &mut Cx) { self.view.redraw(cx) }
    fn visible(&self) -> bool { self.view.visible() }
    fn set_visible(&mut self, cx: &mut Cx, visible: bool) { self.view.set_visible(cx, visible) }
    fn set_scroll_pos(&mut self, cx: &mut Cx, v: Vec2d) { self.view.set_scroll_pos(cx, v) }

    /// Visits the deref view's children, plus each preview card that we manage ourselves.
    ///
    /// The cards come from a template, so the deref view doesn't know about them.
    /// Listing them here lets the widget tree track them and drop them along with
    /// this LinkPreview widget (e.g., when its parent `Message` gets dropped).
    ///
    /// Lookups must go through `self`, not `self.view`: a lookup re-lists this widget's children,
    /// and `self.view` doesn't list the cards, so the widget tree would lose track of them.
    fn children(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) {
        self.view.children(visit);
        for (i, card) in self.cards.iter().enumerate() {
            visit(live_id_num!(preview, i as u64), WidgetRef::clone(&card.view));
        }
    }
    /// Returns true, so searches from outside this widget don't find what's in it, like a card's title.
    fn skip_widget_tree_search(&self) -> bool { true }

    fn cancel_children_impl(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) -> bool {
        self.visible() && self.view.visit_cancel(visit)
    }
    fn find_widgets_from_point(&self, cx: &Cx, point: DVec2, found: &mut dyn FnMut(&WidgetRef)) {
        self.view.find_widgets_from_point(cx, point, found)
    }
    fn selection_text_len(&self) -> usize { self.view.selection_text_len() }
    fn selection_point_to_char_index(&self, cx: &Cx, abs: DVec2) -> Option<usize> { self.view.selection_point_to_char_index(cx, abs) }
    fn selection_set(&mut self, anchor: usize, cursor: usize) { self.view.selection_set(anchor, cursor) }
    fn selection_clear(&mut self) { self.view.selection_clear() }
    fn selection_select_all(&mut self) { self.view.selection_select_all() }
    fn selection_get_text_for_range(&self, start: usize, end: usize) -> String { self.view.selection_get_text_for_range(start, end) }
    fn selection_get_full_text(&self) -> String { self.view.selection_get_full_text() }
}

impl Widget for LinkPreview {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // clear hovers on every link preview card and the show more/fewer buttons
        if let Event::ClearHover = event {
            for card in self.cards.iter() {
                set_view_bg_color(cx, &card.view, self.card_bg_color);
            }
            self.button(cx, ids!(collapsible_buttons.expand_button)).reset_hover(cx);
            self.button(cx, ids!(collapsible_buttons.collapse_button)).reset_hover(cx);
        }

        // Handle collapsible button clicks
        if let Event::Actions(actions) = event {
            let expand_btn = self.button(cx, ids!(collapsible_buttons.expand_button));
            let collapse_btn = self.button(cx, ids!(collapsible_buttons.collapse_button));
            if expand_btn.clicked(actions) || collapse_btn.clicked(actions) {
                self.is_expanded = !self.is_expanded;
                self.update_button_and_visibility(cx);
                cx.widget_action(self.widget_uid(), LinkPreviewAction::Toggled { is_expanded: self.is_expanded });
                cx.redraw_all();
            }
        }

        for card in self.cards.iter() {
            match event.hits(cx, card.view.area()) {
                Hit::FingerHoverIn(_) | Hit::FingerDown(_) => {
                    set_view_bg_color(cx, &card.view, self.card_bg_color_hover);
                }
                Hit::FingerHoverOut(_) => {
                    set_view_bg_color(cx, &card.view, self.card_bg_color);
                }
                Hit::FingerUp(fe) => {
                    // return to normal bg color
                    set_view_bg_color(cx, &card.view, self.card_bg_color);
                    if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                        // Always use the original link upon click (not the `og:url`).
                        cx.widget_action(
                            self.widget_uid(),
                            HtmlLinkAction::Clicked {
                                url: card.link.to_string(),
                                key_modifiers: fe.modifiers,
                            },
                        );
                    }
                }
                _ => {}
            }
            card.view.handle_event(cx, event, scope);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // First, draw as many cards as should be visible.
        let num_visible = if self.is_expanded { self.cards.len() } else { MAX_DEFAULT_VISIBLE_PREVIEWS };
        for card in self.cards.iter_mut().take(num_visible) {
            let _ = card.view.draw(cx, scope);
        }
        // Then, draw the rest of the main view, e.g., the collapsible button.
        let _ = self.view.draw_walk(cx, scope, walk);
        DrawStep::done()
    }
}

impl LinkPreview {
    fn update_button_and_visibility(&mut self, cx: &mut Cx) {
        if self.num_hidden_links > 0 {
            self.lazy_view(cx, ids!(collapsible_buttons)).set_visible(cx, true);
            let expand_btn = self.button(cx, ids!(collapsible_buttons.expand_button));
            let collapse_btn = self.button(cx, ids!(collapsible_buttons.collapse_button));
            if self.is_expanded {
                expand_btn.set_visible(cx, false);
                collapse_btn.set_visible(cx, true);
            } else {
                expand_btn.set_text(cx, &format!("Show {} more links", self.num_hidden_links));
                expand_btn.set_visible(cx, true);
                collapse_btn.set_visible(cx, false);
            }
            expand_btn.reset_hover(cx);
            collapse_btn.reset_hover(cx);
        } else {
            self.lazy_view(cx, ids!(collapsible_buttons)).set_visible(cx, false);
        }
    }

    fn get_card_template(&self, kind: CardKind) -> Option<LivePtr> {
        match kind {
            CardKind::Responsive => self.card_template,
            CardKind::Compact => self.compact_card_template,
            CardKind::Stacked => self.stacked_card_template,
            CardKind::LargeImage => self.large_image_card_template,
        }
    }
}

impl LinkPreviewRef {
    /// If the user just expanded or collapsed these link previews, returns whether they're now expanded.
    pub fn toggled(&self, actions: &Actions) -> Option<bool> {
        actions.filter_widget_actions(self.widget_uid()).find_map(|action| match action.cast() {
            LinkPreviewAction::Toggled { is_expanded } => Some(is_expanded),
            LinkPreviewAction::None => None,
        })
    }

    /// Sets whether all of these link previews are shown, or just the first few.
    pub fn set_expanded(&self, cx: &mut Cx, is_expanded: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if inner.is_expanded != is_expanded {
            inner.is_expanded = is_expanded;
            inner.update_button_and_visibility(cx);
        }
    }

    /// Clears any displayed link preview(s), resetting this widget to its empty state.
    ///
    /// Needed for messages that never show link previews (e.g. redacted messages).
    pub fn clear(&mut self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.cards.clear();
            cx.widget_tree_mark_dirty(inner.widget_uid());
            inner.is_expanded = false;
            inner.num_hidden_links = 0;
            inner.update_button_and_visibility(cx);
            inner.redraw(cx);
        }
    }

    /// Populates the link previews below a message.
    ///
    /// Returns `true` if every preview was fully drawn.
    pub fn populate_below_message<F>(
        &mut self,
        cx: &mut Cx,
        links: &[url::Url],
        media_cache: &mut MediaCache,
        link_preview_cache: &mut LinkPreviewCache,
        populate_image_fn: &F,
    ) -> bool
    where
        F: Fn(&mut Cx, &TextOrImageRef, Option<&ImageInfo>, MediaSource, &str, &mut MediaCache) -> bool,
    {
        const SKIPPED_DOMAINS: &[&str] = &["matrix.to", "matrix.io"];

        let style = cx.global::<AppPreferencesGlobal>().0.link_preview_style;
        let links: &[Url] = if style == LinkPreviewStyle::Off { &[] } else { links };

        // Deduplicate links and filter out matrix or invalid links.
        let mut accepted_links: Vec<url::Url> = Vec::new();
        for link in links {
            // The homeserver can only preview web links, and we show the media of an `mxc://` link ourselves.
            if !matches!(link.scheme(), "http" | "https" | "mxc") {
                continue;
            }
            if link.host_str().is_some_and(|host|
                SKIPPED_DOMAINS.iter().any(|skip| host.ends_with(skip))
            ) {
                continue;
            }
            if !accepted_links.contains(link) {
                accepted_links.push(link.clone());
            }
        }

        let did_cards_change = match self.borrow() {
            Some(inner) => inner.cards.len() != accepted_links.len()
                || inner.cards.iter().zip(&accepted_links).any(|(card, link)|
                    card.link != *link || card.kind != get_card_kind(style, link)
                ),
            None => return true,
        };
        if did_cards_change && let Some(mut inner) = self.borrow_mut() {
            // Reuse as many old link preview cards as we can.
            let mut old_cards = std::mem::take(&mut inner.cards).into_iter();
            for link in accepted_links {
                let kind = get_card_kind(style, &link);
                let card = match old_cards.next() {
                    Some(old_card) if old_card.kind == kind => LinkPreviewCard { link, ..old_card },
                    _ => {
                        let view = widget_ref_from_live_ptr(cx, inner.get_card_template(kind)).as_view();
                        let text_walk = view.view(cx, ids!(content_view)).borrow()
                            .map_or(Walk::default(), |content_view| content_view.walk);
                        let image_height = view.view(cx, ids!(image_view)).borrow()
                            .map_or(Size::default(), |image_view| image_view.walk.height);
                        LinkPreviewCard { link, kind, view, text_walk, image_height }
                    }
                };
                // Reset each preview's per-link visual state: a reused one keeps its old
                // hover color and image, and a new one shows TextOrImage's default view.
                set_view_bg_color(cx, &card.view, inner.card_bg_color);
                card.view.text_or_image(cx, ids!(image_view.image)).clear(cx);
                inner.cards.push(card);
            }
            // Tell the widget tree to pick up the list of new cards
            cx.widget_tree_mark_dirty(inner.widget_uid());
            inner.is_expanded = false;
            inner.num_hidden_links = inner.cards.len().saturating_sub(MAX_DEFAULT_VISIBLE_PREVIEWS);
            inner.update_button_and_visibility(cx);
        }

        let Some(inner) = self.borrow() else { return true };
        let mut all_drawn = true;
        for card in inner.cards.iter() {
            let entry = (card.link.scheme() != "mxc")
                .then(|| link_preview_cache.get_or_fetch_link_preview(card.link.as_str()));
            all_drawn &= populate_card(cx, card, entry, media_cache, populate_image_fn);
        }
        all_drawn
    }
}

/// Returns which kind of card to show the given link in.
fn get_card_kind(style: LinkPreviewStyle, link: &Url) -> CardKind {
    const IMAGE_EXTENSIONS: &[&str] = &["gif", "png", "jpg", "jpeg", "webp", "apng", "avif", "bmp"];
    const IMAGE_SITES: &[&str] = &["giphy.com", "tenor.com"];

    // We only look at the link itself, so its card won't change size once its preview arrives.
    let is_image_link = || {
        link.scheme() == "mxc"
        || link.host_str().is_some_and(|host| IMAGE_SITES.iter().any(|site|
            host.strip_suffix(site).is_some_and(|subdomain| subdomain.is_empty() || subdomain.ends_with('.'))
        ))
        || link.path().rsplit_once('.').is_some_and(|(_, extension)|
            IMAGE_EXTENSIONS.iter().any(|ext| extension.eq_ignore_ascii_case(ext))
        )
    };
    match style {
        LinkPreviewStyle::Compact => CardKind::Compact,
        _ if is_image_link() => CardKind::LargeImage,
        LinkPreviewStyle::Large => CardKind::Stacked,
        LinkPreviewStyle::Automatic | LinkPreviewStyle::Off => CardKind::Responsive,
    }
}

/// Populates a single link preview card with whatever metadata has been fetched so far.
///
/// The `entry` is `None` for an `mxc://` link, which is media rather than a web page.
///
/// Returns `true` if the link preview was fully drawn.
fn populate_card<F>(
    cx: &mut Cx,
    card: &LinkPreviewCard,
    entry: Option<LinkPreviewCacheEntry>,
    media_cache: &mut MediaCache,
    populate_image_fn: &F,
) -> bool
where
    F: Fn(&mut Cx, &TextOrImageRef, Option<&ImageInfo>, MediaSource, &str, &mut MediaCache) -> bool,
{
    let title_label = card.view.label(cx, ids!(title_label));
    let description_label = card.view.widget(cx, ids!(description_label));
    let text_or_image = card.view.text_or_image(cx, ids!(image_view.image));

    let (fully_drawn, has_text_besides_link) = match entry {
        Some(LinkPreviewCacheEntry::LoadedLinkPreview(data)) => {
            let title = data.title.as_deref().filter(|title| !title.trim().is_empty());
            title_label.set_text(cx, title.unwrap_or(card.link.as_str()));
            // The description label is as many lines as fit with ellipsis wrap, so we
            // ensure that hard link breaks are ignored by converting them to spaces.
            let description = data.description.as_deref()
                .map(|description| utils::replace_linebreaks_separators(description, false));
            description_label.set_text(cx, description.as_deref().unwrap_or(""));
            // An empty description would still take up a line, which pushes the title off-center.
            let shows_description = !description_label.is_empty()
                && description.is_some_and(|description| !description.trim().is_empty());
            description_label.set_visible(cx, shows_description);

            let fully_drawn = match &data.image {
                Some(image) => {
                    let mut image_info = ImageInfo::default();
                    image_info.mimetype = data.image_type.clone();
                    image_info.size = data.image_size;
                    let source = MediaSource::Plain(OwnedMxcUri::from(image.clone()));
                    populate_image_fn(cx, &text_or_image, Some(&image_info), source, "", media_cache)
                }
                None => true,
            };
            (fully_drawn, title.is_some() || shows_description)
        }
        Some(LinkPreviewCacheEntry::Requested | LinkPreviewCacheEntry::Failed) => {
            title_label.set_text(cx, card.link.as_str());
            description_label.set_visible(cx, false);
            // We treat "Failed" as permanent, i.e., it's fully drawn.
            let is_failed = matches!(entry, Some(LinkPreviewCacheEntry::Failed));
            // Image links rarely have titles, so their cards assume there isn't one until we know.
            (is_failed, !is_failed && card.kind != CardKind::LargeImage)
        }
        // We can't tell what kind of media an `mxc://` link is, so we only show it once
        // the server makes a thumbnail of it, which means it's an image.
        None => {
            title_label.set_text(cx, card.link.as_str());
            let source = MediaSource::Plain(card.link.as_str().into());
            let fully_drawn = match media_cache.try_get_media_or_fetch(&source, utils::MEDIA_THUMBNAIL_FORMAT.into()).0 {
                MediaCacheEntry::Loaded(_) => populate_image_fn(cx, &text_or_image, Some(&ImageInfo::default()), source, "", media_cache),
                MediaCacheEntry::Requested => false,
                MediaCacheEntry::Failed(_) => true,
            };
            (fully_drawn, false)
        }
    };
    let is_showing_image = text_or_image.status().is_image();
    text_or_image.set_visible(cx, is_showing_image);
    let placeholder = card.view.view(cx, ids!(image_view.placeholder));
    placeholder.set_visible(cx, !is_showing_image);
    placeholder.view(cx, ids!(loading_spinner)).set_visible(cx, !fully_drawn);
    placeholder.view(cx, ids!(no_image_message)).set_visible(cx, fully_drawn);
    // Once we know there won't be an image, a card with text swaps to its no-image view, and text
    // beside that gets the whole width. A card with just a link says so in its image's space.
    let shows_no_image_view = has_text_besides_link && fully_drawn && !is_showing_image;
    card.view.view(cx, ids!(image_view)).set_visible(cx, !shows_no_image_view);
    card.view.view(cx, ids!(no_image_view)).set_visible(cx, shows_no_image_view);
    let full_size = Size::Rel { base: Base::Parent, factor: 1.0 };
    // The message already shows the link, so we only repeat it beside an image, not below one.
    // A narrow Responsive card's image takes its whole height, which pushes the link off the card.
    let content_view = card.view.view(cx, ids!(content_view));
    content_view.set_visible(cx, has_text_besides_link || matches!(card.kind, CardKind::Responsive | CardKind::Compact));
    if let Some(mut content_view) = content_view.borrow_mut() {
        content_view.walk.width = if shows_no_image_view { full_size } else { card.text_walk.width };
    }
    if let Some(mut image_view) = card.view.view(cx, ids!(image_view)).borrow_mut() {
        image_view.walk.height = if has_text_besides_link { card.image_height } else { full_size };
    }
    // A card's height doesn't depend on its image, so we cap it, and an image above its text gets
    // the rest of that height. That image is also no taller than an image in a message.
    let text_height = card.text_walk.height.to_fixed().unwrap_or(0.0);
    let height_besides_image = card.view.borrow().map_or(0.0, |card_view| {
        let layout = &card_view.layout;
        let gap = if card.kind == CardKind::Responsive { layout.wrap_spacing } else { layout.spacing };
        layout.padding.top + gap + text_height + layout.padding.bottom
    });
    let max_image_height = (cx.global::<AppPreferencesGlobal>().0.thumbnail_max_height.to_pixels() as f64)
        .min(MAX_CARD_HEIGHT - height_besides_image);
    for image_area in [ids!(image_view), ids!(no_image_view)] {
        if let Some(mut view) = card.view.view(cx, image_area).borrow_mut() {
            view.walk.max_height = has_text_besides_link.then_some(FitBound::Abs(max_image_height));
        }
    }
    if let Some(mut card_view) = card.view.borrow_mut() {
        card_view.walk.max_height = Some(FitBound::Abs(height_besides_image + max_image_height));
    }
    fully_drawn
}

/// A label that shows as many lines of its text as fit in the rest of its parent's height.
#[derive(Script, ScriptHook, Widget)]
struct FillLinesLabel {
    #[source] source: ScriptObjectRef,
    #[deref] label: Label,
    /// The size that `label.max_lines` was last set for.
    #[rust] fit_to_size: Option<Vec2d>,
}

impl Widget for FillLinesLabel {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // It's only as tall as the lines it shows, so its parent can center them.
        let size = cx.peek_walk_turtle(Walk { height: Size::fill(), ..walk }).size;
        if self.fit_to_size != Some(size) {
            self.fit_to_size = Some(size);
            // We lay out all of the text to count how many of its lines fit.
            self.label.draw_text.max_lines = 0;
            let scale = (self.label.draw_text.font_scale as f64).max(0.0001);
            let text = self.label.text();
            let laidout = self.label.draw_text.layout(cx, 0.0, 0.0, Some((size.x / scale) as f32), true, self.label.align, &text);
            self.label.max_lines = laidout.rows.iter()
                .take_while(|row| (row.origin_in_lpxs.y - row.descender_in_lpxs) as f64 * scale <= size.y)
                .count()
                .max(1);
        }
        self.label.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        self.label.text()
    }

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        self.fit_to_size = None;
        self.label.set_text(cx, v);
    }
}

/// The data structure from the link preview API, "/_matrix/client/v1/media/preview_url"
#[derive(Clone, Debug, Deserialize, Default)]
pub struct LinkPreviewData {
    #[serde(rename = "og:description")]
    pub description: Option<String>,
    /// The size of the image in bytes, if available
    #[serde(rename = "matrix:image:size", default, deserialize_with = "deserialize_lenient_uint")]
    pub image_size: Option<UInt>,
    /// The URL of the image
    #[serde(rename = "og:image")]
    pub image: Option<String>,
    /// The type of the image
    #[serde(rename = "og:image:type")]
    pub image_type: Option<String>,
    /// The name of the site
    #[serde(rename = "og:site_name")]
    pub site_name: Option<String>,
    /// The URL of the site
    #[serde(rename = "og:url")]
    pub url: Option<String>,
    /// The title of the site
    #[serde(rename = "og:title")]
    pub title: Option<String>,
}

/// Deserializes an optional [`UInt`] that is either a JSON number or a JSON string.
///
/// Some homeservers encode the numeric preview fields as strings, so we handle both.
fn deserialize_lenient_uint<'de, D>(deserializer: D) -> Result<Option<UInt>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumberOrString {
        Number(UInt),
        String(String),
    }
    Ok(match Option::<NumberOrString>::deserialize(deserializer)? {
        Some(NumberOrString::Number(n)) => Some(n),
        Some(NumberOrString::String(s)) => s.parse::<u64>().ok().and_then(|n| UInt::try_from(n).ok()),
        None => None,
    })
}

/// The cache for link previews, specific to a given timeline.
pub struct LinkPreviewCache {
    /// The actual cached data.
    cache: BTreeMap<String, Arc<Mutex<TimestampedCacheEntry>>>,
    /// A channel to send updates to a particular timeline when a link preview request has completed.
    timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
}

impl LinkPreviewCache {
    /// Creates a new link preview cache that will optionally send updates
    /// when a link preview request has completed.
    pub const fn new(
        timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    ) -> Self {
        Self {
            cache: BTreeMap::new(),
            timeline_update_sender,
        }
    }

    /// Sets a new timeline update sender, e.g., for when the backend re-created this room's timeline.
    pub fn set_timeline_update_sender(
        &mut self,
        sender: crossbeam_channel::Sender<TimelineUpdate>,
    ) {
        self.timeline_update_sender = Some(sender);
    }

    /// Fetches the link preview for the specified URL.
    pub fn get_or_fetch_link_preview(&mut self, url: &str) -> LinkPreviewCacheEntry {
        // Clean up old entries periodically
        if self.cache.len() > MAX_CACHE_ENTRIES_BEFORE_CLEANUP {
            self.cleanup_old_entries(Duration::from_secs(CACHE_ENTRY_MAX_AGE_SECS));
        }

        if let Some(entry) = self.cache.get(url) {
            return entry.lock().unwrap().entry.clone();
        }
        let entry_ref = Arc::new(Mutex::new(TimestampedCacheEntry {
            entry: LinkPreviewCacheEntry::Requested,
            timestamp: Instant::now(),
        }));
        self.cache.insert(url.to_owned(), entry_ref.clone());
        submit_async_request(MatrixRequest::GetUrlPreview {
            url: url.to_owned(),
            on_fetched: insert_into_cache,
            destination: entry_ref,
            update_sender: self.timeline_update_sender.clone(),
        });
        LinkPreviewCacheEntry::Requested
    }

    /// Removes all `Requested` and `Failed` entries from the link preview cache,
    /// allowing them to be re-fetched.
    ///
    /// This should be called when the app transitions from offline back to online,
    /// because any in-flight requests that were submitted while offline have likely
    /// failed, leaving stale entries that permanently block re-fetching.
    pub fn clear_all_pending_and_failed_requests(&mut self) {
        self.cache.retain(|_, entry| {
            if let Ok(guard) = entry.lock() {
                matches!(guard.entry, LinkPreviewCacheEntry::LoadedLinkPreview(_))
            } else {
                true // Keep entries we can't lock
            }
        });
    }

    /// Removes cache entries older than the specified duration
    pub fn cleanup_old_entries(&mut self, max_age: Duration) {
        let now = Instant::now();
        self.cache.retain(|_url, entry| {
            if let Ok(timestamped_entry) = entry.lock() {
                now.duration_since(timestamped_entry.timestamp) < max_age
            } else {
                true // Keep entries we can't lock
            }
        });
    }
}

/// Insert data into a previously-requested media cache entry.
fn insert_into_cache(
    value_ref: Arc<Mutex<TimestampedCacheEntry>>,
    data: Result<LinkPreviewData, UrlPreviewError>,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_entry = match data {
        Ok(data) => LinkPreviewCacheEntry::LoadedLinkPreview(data),
        Err(_e) => LinkPreviewCacheEntry::Failed,
    };

    if let Ok(mut timestamped_entry) = value_ref.lock() {
        timestamped_entry.entry = new_entry;
        timestamped_entry.timestamp = Instant::now();
    }

    if let Some(sender) = update_sender {
        // Reuse TimelineUpdate MediaFetched to trigger redraw in the timeline.
        let _ = sender.send(TimelineUpdate::LinkPreviewFetched);
    }
    SignalToUI::set_ui_signal();
}
