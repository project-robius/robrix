//! A `HtmlOrPlaintext` view can display either plaintext or rich HTML content.

use makepad_widgets::*;
use matrix_sdk::{ruma::{matrix_uri::MatrixId, MatrixToUri, MatrixUri, OwnedMxcUri}, OwnedServerName};

use crate::{avatar_cache::{self, AvatarCacheEntry}, profile::user_profile_cache, sliding_sync::{current_user_id, submit_async_request, MatrixRequest}, utils};

use super::avatar::AvatarWidgetExt;

/// The color of the text used to print the spoiler reason before the hidden text.
const COLOR_SPOILER_REASON: Vec4 = vec4(0.6, 0.6, 0.6, 1.0);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // A pill-shaped widget that displays a Matrix link,
    // either a link to a user, a room, or a message in a room.
    //
    // The outer widget derefs to View (not RoundedView), so we nest a
    // RoundedView child to get the rounded background + border_radius shader.
    mod.widgets.MatrixLinkPill = #(MatrixLinkPill::register_widget(vm)) {
        width: Fit, height: Fit,
        cursor: MouseCursor.Hand,

        pill_bg := RoundedView {
            width: Fit, height: Fit,
            flow: Right,
            align: Align{ y: 0.5 }
            padding: Inset{ left: 6, right: 4, bottom: -3, top: -3 }
            margin: Inset{ right: 1 }
            spacing: 1,

            show_bg: true,
            draw_bg +: {
                color: #000
                border_radius: 6.0
            }

            avatar := Avatar {
                height: (MESSAGE_FONT_SIZE + 5), width: (MESSAGE_FONT_SIZE + 5),
                // White bg so transparent avatar images are visible on the
                // pill's black background.
                img_view +: {
                    show_bg: true,
                    draw_bg +: { color: #fff }
                }
                text_view +: {
                    text +: {
                        draw_text +: {
                            text_style: TITLE_TEXT { font_size: (MESSAGE_FONT_SIZE - 2) }
                        }
                    }
                }
            }

            title := Label {
                flow: Right, // do not wrap
                draw_text +: {
                    color: #f,
                    // line_spacing 1.0 prevents the label's row height from
                    // inflating the pill vertically (pill is single-line).
                    text_style: MESSAGE_TEXT_STYLE { font_size: (MESSAGE_FONT_SIZE), line_spacing: 1.0 },
                }
                text: "Unknown",
            }
        }
    }

    // A RobrixHtmlLink is either a regular Html link (default) or a Matrix link.
    // The Matrix link is a pill-shaped widget with an avatar and a title.
    //
    // Drawing notes (see `RobrixHtmlLink::draw_walk` for the full story):
    // * `html_link` is a direct child, NOT wrapped in an intermediate View.
    //   `HtmlLink::draw_walk` draws inline text into the parent `TextFlow`'s
    //   turtle; any surrounding View would open its own turtle and break the
    //   inline wrap math. So `draw_walk` calls `html_link.draw_walk` directly
    //   (bypassing `self.view.draw_walk`) for the inline-text path.
    // * `matrix_link_view` IS a View (a pill is an atomic inline block with its
    //   own turtle). `draw_walk` calls `matrix_link_view.draw_walk` directly
    //   (also bypassing `self.view.draw_walk`) for the pill path.
    // * Neither path goes through `self.view.draw_walk`, which would iterate
    //   ALL children — and since `HtmlLink` has no `visible` field (so its
    //   `visible()` is always `true`), it would double-draw alongside the pill.
    mod.widgets.RobrixHtmlLink = #(RobrixHtmlLink::register_widget(vm)) {
        width: Fit, height: Fit,
        align: Align{ y: 0.5 },

        html_link := HtmlLink {
            hover_color: (COLOR_LINK_HOVER)
            grab_key_focus: false,
            padding: Inset{left: 1.0, right: 1.5},
        }

        matrix_link_view := View {
            visible: false
            width: Fit, height: Fit,

            matrix_link := mod.widgets.MatrixLinkPill { }
        }
    }

    // This is an HTML subwidget used to handle `<font>` and `<span>` tags,
    // specifically: foreground text color, background color, and spoilers.
    mod.widgets.MatrixHtmlSpan = #(MatrixHtmlSpan::register_widget(vm)) {
        width: Fit, height: Fit,
        align: Align{x: 0., y: 0.}
    }


    // A centralized widget where we define styles and custom elements for HTML
    // message content. This is a wrapper around Makepad's built-in `Html` widget.
    mod.widgets.MessageHtml = Html {
        padding: 0.0,
        width: Fill, height: Fit, // see comment in `HtmlOrPlaintext`
        // `RowAlign.Center` vertically centers each walk on its row. With the
        // per-row FinishedWalk support in draw_walk_resumable_with, each visual
        // row of wrapped text gets its own walk entry, so centering applies
        // correctly even when a pill and multi-row text share the first line.
        flow: Flow.Right{wrap: true, row_align: RowAlign.Center},
        align: Align{ y: 0.5 }
        font_size: (MESSAGE_FONT_SIZE),
        font_color: (MESSAGE_TEXT_COLOR),
        draw_text +: { color: (MESSAGE_TEXT_COLOR) }
        text_style_normal: mod.widgets.MESSAGE_TEXT_STYLE {
            font_size: (MESSAGE_FONT_SIZE)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
        }
        text_style_italic: theme.font_italic {
            font_size: (MESSAGE_FONT_SIZE)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
        }
        text_style_bold: theme.font_bold {
            font_size: (MESSAGE_FONT_SIZE)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
        }
        text_style_bold_italic: theme.font_bold_italic {
            font_size: (MESSAGE_FONT_SIZE)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
        }
        text_style_fixed: theme.font_code {
            font_size: (MESSAGE_FONT_SIZE)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
            top_drop: 0.11
        }
        draw_block +: {
            line_color: (MESSAGE_TEXT_COLOR)
            sep_color: (MESSAGE_TEXT_COLOR)
            code_color: (#EDEDED)
            quote_bg_color: (#EDEDED)
            quote_fg_color: (MESSAGE_TEXT_COLOR)
        }

        quote_layout: Layout{ flow: Flow.Right{wrap: true, row_align: RowAlign.Center}, spacing: 0, padding: Inset{left: 15, top: 10.0, bottom: 10.0}, }
        quote_walk: Walk{ margin: Inset{ top: 5, bottom: 5, left: 0 } }

        sep_walk: Walk{ margin: Inset{ top: 10, bottom: 10 } }

        list_item_layout: Layout{ flow: Flow.Right{wrap: true, row_align: RowAlign.Center}, padding: Inset{left: 5.0, top: 1.0, bottom: 1.0}, }
        list_item_marker_pad: 8.0
        list_item_walk: Walk{ margin: Inset{ left: 0, right: 0, top: 1, bottom: 3 } }
        code_layout: Layout{ padding: Inset{top: 15.0, bottom: 15.0, left: 15, right: 5 } }
        code_walk: Walk{ margin: Inset{ top: 10, bottom: 10, left: 0, right: 0 } }

        heading_margin: Inset{ top: 1.0, bottom: 0.1 }
        paragraph_margin: Inset{ top: 0.33, bottom: 0.33 }

        inline_code_padding: Inset{top: 3, bottom: 3, left: 4, right: 4 }
        inline_code_margin: Inset{ left: 3, right: 3, bottom: 2, top: 2 }

        font := mod.widgets.MatrixHtmlSpan { }
        span := mod.widgets.MatrixHtmlSpan { }
        a := mod.widgets.RobrixHtmlLink { }

        body: "[<i>HTML message placeholder</i>]",
    }

    // A view container that displays either plaintext (a simple `Label`)
    // or rich HTML content (an instance of `MessageHtml`).
    //
    // Key Usage Notes:
    // * Labels need their width to be Fill *and* all of their parent views
    //   also need to have their width set to Fill. Otherwise, the label
    //   won't wrap text properly.
    // * They also need their height to be Fit along with all of their parent views,
    //   otherwise their total height will be zero (when a Fit is inside of a Fill),
    //   resulting in nothing being displayed.
    mod.widgets.HtmlOrPlaintext = #(HtmlOrPlaintext::register_widget(vm)) {
        width: Fill, height: Fit, // see above comment
        flow: Overlay

        plaintext_view := View {
            visible: true,
            width: Fill, height: Fit, // see above comment
            pt_label := Label {
                width: Fill, height: Fit, // see above comment
                flow: Flow.Right{wrap: true},
                padding: 0,
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: (MESSAGE_FONT_SIZE) },
                }
            }
        }

        html_view := View {
            visible: false,
            width: Fill, height: Fit, // see above comment
            html := mod.widgets.MessageHtml {}
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum RobrixHtmlLinkAction{
    ClickedMatrixLink {
        /// The URL of the link, which is only temporarily needed here
        /// because we don't fully handle MatrixId links directly in-app yet.
        url: String,
        matrix_id: MatrixId,
        via: Vec<OwnedServerName>,
        key_modifiers: KeyModifiers,
    },
    #[default]
    None,
}

/// A RobrixHtmlLink is either a regular `HtmlLink` (default) or a Matrix link.
///
/// Matrix links are displayed using the [`MatrixLinkPill`] widget.
#[derive(Script, Widget)]
struct RobrixHtmlLink {
    #[deref] view: View,

    /// The displayable text of the link.
    /// This should be set automatically by the Html widget
    /// when it parses and draws an Html `<a>` tag.
    #[live] pub text: ArcStringMut,
    /// The URL of the link.
    /// This is set by the `on_after_new_scoped()` hook below.
    #[live] pub url: String,
}

impl ScriptHook for RobrixHtmlLink {
    fn on_after_new_scoped(&mut self, _vm: &mut ScriptVm, scope: &mut Scope) {
        if let Some(doc) = scope.props.get::<makepad_html::HtmlDoc>() {
            let mut walker = doc.new_walker_with_index(scope.index + 1);
            while let Some((lc, attr)) = walker.while_attr_lc() {
                match lc {
                    live_id!(href) => {
                        self.url = attr.into();
                        break;
                    }
                    _ => { }
                }
            }
        }
    }
}

impl Widget for RobrixHtmlLink {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // If the URL is a Matrix user/room/event link, render it as a pill.
        // Otherwise fall back to an inline-wrapping HTML link.
        if let Ok(matrix_to_uri) = MatrixToUri::parse(&self.url) {
            return self.draw_matrix_pill(cx, scope, walk, matrix_to_uri.id(), matrix_to_uri.via());
        } else if let Ok(matrix_uri) = MatrixUri::parse(&self.url) {
            return self.draw_matrix_pill(cx, scope, walk, matrix_uri.id(), matrix_uri.via());
        }
        self.draw_html_link(cx, scope, walk)
    }

    fn text(&self) -> String {
        self.text.as_ref().to_string()
    }

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        self.text.as_mut_empty().push_str(v);
        self.redraw(cx);
    }
}

impl RobrixHtmlLink {
    /// Draws the Matrix link pill as an atomic inline block.
    ///
    /// The pill is drawn via `matrix_link_view.draw_walk` directly (not
    /// `self.view.draw_walk`). Its turtle allocates the pill's natural height
    /// in the parent TextFlow. `RowAlign::Center` on the parent `MessageHtml`
    /// handles vertical centering: at each visual-row boundary, `finish_row`
    /// shifts shorter items (text) down so their vertical centers align with
    /// the tallest item (the pill) on that row.
    fn draw_matrix_pill(
        &mut self,
        cx: &mut Cx2d,
        scope: &mut Scope,
        walk: Walk,
        matrix_id: &MatrixId,
        via: &[OwnedServerName],
    ) -> DrawStep {
        if let Some(mut pill) = self.matrix_link_pill(cx, ids!(matrix_link)).borrow_mut() {
            pill.populate_pill(cx, self.url.clone(), matrix_id, via, self.text.as_ref());
        }
        let mlv_ref = self.view(cx, ids!(matrix_link_view));
        mlv_ref.set_visible(cx, true);
        let Some(mut mlv) = mlv_ref.borrow_mut() else {
            return DrawStep::done();
        };
        mlv.draw_walk(cx, scope, walk)
    }

    /// Draws the inner `HtmlLink` as inline wrapping text.
    ///
    /// We invoke `HtmlLink::draw_walk` directly rather than going through
    /// `self.view.draw_walk`. Going through the outer View would open a new
    /// turtle, which in turn would become the turtle that `tf.draw_text()`
    /// reads inside `HtmlLink::draw_walk` — that turtle has `width: Fit` and
    /// therefore no wrap bound, so the link text would refuse to break across
    /// lines. By calling `draw_walk` directly on the `HtmlLink`, the current
    /// turtle remains the parent Html widget's `TextFlow` turtle, which has
    /// the full message width and wraps correctly.
    fn draw_html_link(
        &mut self,
        cx: &mut Cx2d,
        scope: &mut Scope,
        walk: Walk,
    ) -> DrawStep {
        // Hide the pill view in case we're switching away from pill mode.
        // (No-op if it was already hidden.) `HtmlLink` has no `visible` field
        // so there's nothing analogous to set on it.
        self.view(cx, ids!(matrix_link_view)).set_visible(cx, false);

        let mut html_link_ref = self.html_link(cx, ids!(html_link));
        html_link_ref.set_url(&self.url);
        html_link_ref.set_text(cx, self.text.as_ref());

        let Some(mut html_link) = html_link_ref.borrow_mut() else {
            return DrawStep::done();
        };
        html_link.draw_walk(cx, scope, walk)
    }
}

#[derive(Clone, Debug, Default)]
pub enum MatrixLinkPillState {
    Requested,
    Loaded {
        matrix_id: MatrixId,
        name: String,
        avatar_url: Option<OwnedMxcUri>,
    },
    #[default]
    None,
}

/// A pill-shaped widget that shows a Matrix link as an avatar and a title.
///
/// This can be a link to a user, a room, or a message in a room.
#[derive(Script, ScriptHook, Widget)]
struct MatrixLinkPill {
    #[deref] view: View,

    #[rust] matrix_id: Option<MatrixId>,
    #[rust] via: Vec<OwnedServerName>,
    #[rust] state: MatrixLinkPillState,
    #[rust] url: String,
}

impl Widget for MatrixLinkPill {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(loaded @ MatrixLinkPillState::Loaded { matrix_id, .. }) = action.downcast_ref() {
                    if self.matrix_id.as_ref() == Some(matrix_id) {
                        self.state = loaded.clone();
                        self.redraw(cx);
                    }
                }
            }
        }

        // Redraw upon a UI Signal to catch updates to a user profile.
        if matches!(event, Event::Signal) && matches!(self.matrix_id, Some(MatrixId::User(_))) {
            self.redraw(cx);
        }

        // Handle hover (to set the cursor) and click in a single hit-test,
        // avoiding the overhead of event-propagation to all child widgets.
        match event.hits(cx, self.area()) {
            Hit::FingerHoverIn(_) | Hit::FingerHoverOver(_) => {
                if let Some(cursor) = self.cursor {
                    cx.set_cursor(cursor);
                }
            }
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                if let Some(matrix_id) = self.matrix_id.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        RobrixHtmlLinkAction::ClickedMatrixLink {
                            matrix_id,
                            via: self.via.clone(),
                            key_modifiers: fe.modifiers,
                            url: self.url.clone(),
                        }
                    );
                }
            }
            _ => (),
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        match &self.state {
            MatrixLinkPillState::Loaded { name, .. } => name.clone(),
            _ => String::new(),
        }
    }

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        self.label(cx, ids!(title)).set_text(cx, v);
    }
}

impl MatrixLinkPill {
    /// Populates this pill's info based on the given Matrix ID and via servers.
    fn populate_pill(&mut self, cx: &mut Cx, url: String, matrix_id: &MatrixId, via: &[OwnedServerName], link_text: &str) {
        self.url = url;
        self.matrix_id = Some(matrix_id.clone());
        self.via = via.to_vec();

        let is_room_mention = link_text == "@room";
        let is_self_mention = matches!(matrix_id, MatrixId::User(uid) if current_user_id().is_some_and(|u| &u == uid));

        // Reset pill bg to default black, then apply red for mentions.
        // This prevents stale red from persisting if a cached widget is
        // reused for a different (non-mention) link after a message edit.
        {
            let mut pill_bg = self.view(cx, ids!(pill_bg));
            if is_room_mention || is_self_mention {
                script_apply_eval!(cx, pill_bg, { draw_bg +: { color: #d91b38 } });
            } else {
                script_apply_eval!(cx, pill_bg, { draw_bg +: { color: #000 } });
            }
        }

        // Handle a user ID link by querying the user profile cache.
        if let MatrixId::User(user_id) = matrix_id {

            let (name, avatar_uri) = match user_profile_cache::with_user_profile(
                cx,
                user_id.clone(),
                None,
                true,
                |profile, _| { (profile.displayable_name().to_owned(), profile.avatar_state.clone()) }
            ) {
                Some((name, avatar)) => (name, avatar.uri().cloned()),
                None => (user_id.to_string(), None),
            };
            self.set_text(cx, &name);
            self.populate_avatar(cx, avatar_uri.as_ref(), &name);
            return;
        }

        // Handle room ID or alias
        match &self.state {
            MatrixLinkPillState::Loaded { name, avatar_url, .. } => {
                // For @room mentions, show "@room" as the title, not the room name.
                let display_name = if is_room_mention { "@room" } else { name.as_str() };
                self.label(cx, ids!(title)).set_text(cx, display_name);
                self.populate_avatar(cx, avatar_url.as_ref(), display_name);
                return;
            }
            MatrixLinkPillState::None => {
                submit_async_request(MatrixRequest::GetMatrixRoomLinkPillInfo {
                    matrix_id: matrix_id.clone(),
                    via: via.to_vec(),
                });
                self.state = MatrixLinkPillState::Requested;
            }
            MatrixLinkPillState::Requested => { }
        }
        // While waiting for the async request to complete, show "@room" or the room ID/alias.
        let fallback_name = if is_room_mention {
            "@room".to_owned()
        } else {
            match matrix_id {
                MatrixId::Room(room_id) => room_id.as_str().to_owned(),
                MatrixId::RoomAlias(alias) => alias.as_str().to_owned(),
                MatrixId::Event(room_or_alias, _) => format!("Message in {}", room_or_alias.as_str()),
                _ => String::new(),
            }
        };
        self.set_text(cx, &fallback_name);
        self.populate_avatar(cx, None, &fallback_name);
    }

    fn populate_avatar(&self, cx: &mut Cx, avatar_url: Option<&OwnedMxcUri>, display_name: &str) {
        let avatar_ref = self.avatar(cx, ids!(avatar));
        if let Some(avatar_url) = avatar_url {
            if let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(cx, avatar_url) {
                let res = avatar_ref.show_image(
                    cx,
                    None, // Don't make this avatar clickable
                    |cx, img_ref| utils::load_png_or_jpg(&img_ref, cx, &data),
                );
                if res.is_ok() {
                    return;
                }
            }
        }
        avatar_ref.show_text(cx, None, None, display_name);
    }
}

impl MatrixLinkPillRef {
    pub fn get_matrix_id(&self) -> Option<MatrixId> {
        self.borrow().and_then(|inner| inner.matrix_id.clone())
    }

    pub fn get_via(&self) -> Vec<OwnedServerName> {
        self.borrow().map(|inner| inner.via.clone()).unwrap_or_default()
    }
}

/// A widget used to display a single HTML `<span>` tag or a `<font>` tag.
#[derive(Script, Widget)]
struct MatrixHtmlSpan {
    #[uid] uid: WidgetUid,
    // TODO: this is unused; just here to invalidly satisfy the area provider.
    //       I'm not sure how to implement `fn area()` given that it has multiple area rects.
    #[redraw] #[area] area: Area,

    // TODO: remove these if they're unneeded
    #[walk] walk: Walk,
    #[layout] layout: Layout,

    #[rust] drawn_areas: SmallVec<[Area; 2]>,

    /// Whether to grab key focus when pressed.
    #[live(true)] grab_key_focus: bool,

    /// The text content within the `<span>` tag.
    #[live] text: ArcStringMut,
    /// The current display state of the spoiler.
    #[rust] spoiler: SpoilerDisplay,
    /// Foreground (text) color: the `data-mx-color` or `color` attributes.
    #[rust] fg_color: Option<Vec4>,
    /// Background color: the `data-mx-bg-color` attribute.
    #[rust] bg_color: Option<Vec4>,
}

impl ScriptHook for MatrixHtmlSpan {
    // After an MatrixHtmlSpan instance has been instantiated, we must
    // populate its struct fields from the `<span>` or `<font>` tag's attributes.
    fn on_after_new_scoped(&mut self, _vm: &mut ScriptVm, scope: &mut Scope) {
        // The attributes we care about (we allow all attributes in both tags):
        // * in `<font>` tags: `color`
        // * in `<span>` tags: `data-mx-color`, `data-mx-bg-color`, `data-mx-spoiler`
        if let Some(doc) = scope.props.get::<makepad_html::HtmlDoc>() {
            let mut walker = doc.new_walker_with_index(scope.index + 1);
            while let Some((lc, attr)) = walker.while_attr_lc() {
                let attr = attr.trim_matches(['"', '\'']);
                match lc {
                    id!(color)
                    | id!(data-mx-color) => self.fg_color = utils::vec4_from_hex_str(attr),
                    id!(data-mx-bg-color) => self.bg_color = utils::vec4_from_hex_str(attr),
                    id!(data-mx-spoiler) => self.spoiler = SpoilerDisplay::Hidden { reason: attr.into() },
                    _ => ()
                }
            }
        }
    }
}




/// The possible states that a spoiler can be in: hidden or revealed.
///
/// The enclosed `reason` string is an optional reason given for why
/// the text is hidden; if empty, then no reason was given.
#[derive(Default, Debug)]
enum SpoilerDisplay {
    /// There is no spoiler at all.
    #[default]
    None,
    /// The spoiler text is hidden, with an optional reason given.
    Hidden { reason: String },
    /// The spoiler text is revealed, with an optional reason given.
    Revealed { reason: String },
}
impl SpoilerDisplay {
    /// Toggles the spoiler's display state.
    fn toggle(&mut self) {
        match self {
            SpoilerDisplay::Hidden { reason } => {
                let s = std::mem::take(reason);
                *self = SpoilerDisplay::Revealed { reason: s };
            }
            SpoilerDisplay::Revealed { reason } => {
                let s = std::mem::take(reason);
                *self = SpoilerDisplay::Hidden { reason: s };
            }
            SpoilerDisplay::None => { }
        }
    }

    /// Returns `true` if this spoiler is not `None`, i.e., if it's `Hidden` or `Revealed`.
    fn is_some(&self) -> bool {
        !matches!(self, SpoilerDisplay::None)
    }
}

impl Widget for MatrixHtmlSpan {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let mut needs_redraw = false;
        for area in &self.drawn_areas {
            match event.hits(cx, *area) {
                Hit::FingerDown(..) if self.grab_key_focus => {
                    cx.set_key_focus(self.area);
                }
                Hit::FingerHoverIn(..) if self.spoiler.is_some() => {
                    cx.set_cursor(MouseCursor::Hand);
                }
                Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                    self.spoiler.toggle();
                    needs_redraw = true;
                }
                _ => (),
            }
        }
        if needs_redraw {
            for area in &self.drawn_areas {
                cx.redraw_area(*area);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, _walk: Walk) -> DrawStep {
        let Some(tf) = scope.data.get_mut::<TextFlow>() else {
            return DrawStep::done();
        };

        // Here: the text flow has already began drawing,
        // so we just need to tweak the formatting and draw the text.
        tf.areas_tracker.push_tracker();
        let mut pushed_color = false;
        let mut pushed_inline_code = false;
        let mut old_code_color = None;

        if let Some(fg_color) = self.fg_color {
            tf.font_colors.push(fg_color);
            pushed_color = true;
        }

        if let Some(bg_color) = self.bg_color {
            // Reuse the inline code drawblock to set the background color.
            tf.inline_code.push();
            pushed_inline_code = true;
            old_code_color = Some(tf.draw_block.code_color);
            tf.draw_block.code_color = bg_color;
        }

        match &self.spoiler {
            SpoilerDisplay::Hidden { reason }
            | SpoilerDisplay::Revealed { reason } => {
                // Draw the spoiler reason text in an italic gray font.
                tf.font_colors.push(COLOR_SPOILER_REASON);
                tf.italic.push();
                // tf.push_size_rel_scale(0.8);
                if reason.is_empty() {
                    tf.draw_text(cx, " [Spoiler]  ");
                } else {
                    tf.draw_text(cx, &format!(" [Spoiler: {}]  ", reason));
                }
                // tf.font_sizes.pop();
                tf.italic.pop();
                tf.font_colors.pop();

                // Now, draw the spoiler context text itself, either hidden or revealed.
                if matches!(self.spoiler, SpoilerDisplay::Hidden {..}) {
                    // Use a background color that is the same as the foreground color,
                    // which is a hacky way to make the spoiled text non-readable.
                    // In the future, we should use a proper blur effect.
                    let spoiler_bg_color = self.fg_color
                        .or_else(|| tf.font_colors.last().copied())
                        .unwrap_or(tf.font_color);

                    tf.inline_code.push();
                    let old_bg_color = tf.draw_block.code_color;
                    tf.draw_block.code_color = spoiler_bg_color;

                    tf.draw_text(cx, self.text.as_ref());

                    tf.draw_block.code_color = old_bg_color;
                    tf.inline_code.pop();

                } else {
                    tf.draw_text(cx, self.text.as_ref());
                }
            }
            SpoilerDisplay::None => {
                tf.draw_text(cx, self.text.as_ref());
            }
        }

        if pushed_color {
            tf.font_colors.pop();
        }
        if pushed_inline_code {
            tf.inline_code.pop();
        }
        if let Some(old_code_color) = old_code_color {
            tf.draw_block.code_color = old_code_color;
        }

        let (start, end) = tf.areas_tracker.pop_tracker();
        self.drawn_areas = SmallVec::from(
            &tf.areas_tracker.areas[start..end]
        );

        DrawStep::done()
    }

    fn text(&self) -> String {
        self.text.as_ref().to_string()
    }

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        self.text.as_mut_empty().push_str(v);
        self.area.redraw(cx);
    }
}


#[derive(ScriptHook, Script, Widget)]
pub struct HtmlOrPlaintext {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
}

impl Widget for HtmlOrPlaintext {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl HtmlOrPlaintext {
    /// Sets the plaintext content and makes it visible, hiding the rich HTML content.
    pub fn show_plaintext<T: AsRef<str>>(&mut self, cx: &mut Cx, text: T) {
        self.view(cx, ids!(html_view)).set_visible(cx, false);
        self.view(cx, ids!(plaintext_view)).set_visible(cx, true);
        self.label(cx, ids!(plaintext_view.pt_label)).set_text(cx, text.as_ref());
    }

    /// Sets the HTML content, making the HTML visible and the plaintext invisible.
    pub fn show_html<T: AsRef<str>>(&mut self, cx: &mut Cx, html_body: T) {
        self.html(cx, ids!(html_view.html)).set_text(cx, html_body.as_ref());
        self.view(cx, ids!(html_view)).set_visible(cx, true);
        self.view(cx, ids!(plaintext_view)).set_visible(cx, false);
    }
}

impl HtmlOrPlaintextRef {
    /// See [`HtmlOrPlaintext::show_plaintext()`].
    pub fn show_plaintext<T: AsRef<str>>(&self, cx: &mut Cx, text: T) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_plaintext(cx, text);
        }
    }

    /// See [`HtmlOrPlaintext::show_html()`].
    pub fn show_html<T: AsRef<str>>(&self, cx: &mut Cx, html_body: T) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_html(cx, html_body);
        }
    }

    /// Sets the color of links in the HTML content.
    ///
    /// This modifies the cached `HtmlLink` widget instances inside the inner
    /// `Html` widget's `TextFlow` items. On the very first draw (before items
    /// are created), this is a no-op, but subsequent frames will have the
    /// correct color.
    pub fn set_link_color(&self, cx: &mut Cx, color: Option<Vec4>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_link_color(cx, color);
        }
    }
}

impl HtmlOrPlaintext {
    /// See [`HtmlOrPlaintextRef::set_link_color()`].
    pub fn set_link_color(&mut self, cx: &mut Cx, color: Option<Vec4>) {
        let html_ref = self.html(cx, ids!(html_view.html));
        let Some(mut html) = html_ref.borrow_mut() else { return };
        // Iterate over cached TextFlow items (auto-generated IDs start at 1)
        // until we hit a non-existent item.
        let mut i = 1u64;
        loop {
            let item = html.existing_item(LiveId(i));
            if item.is_empty() { break; }
            // Check if this item is a RobrixHtmlLink and modify its inner HtmlLink.
            if let Some(link) = item.borrow_mut::<RobrixHtmlLink>() {
                let mut html_link = link.html_link(cx, ids!(html_link));
                match color {
                    Some(c) => {
                        script_apply_eval!(cx, html_link, {
                            color: #(c)
                        });
                    }
                    None => {
                        script_apply_eval!(cx, html_link, {
                            color: nil
                        });
                    }
                }
            }
            i += 1;
        }
    }
}
