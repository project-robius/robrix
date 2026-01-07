//! A `HtmlOrPlaintext` view can display either plaintext or rich HTML content.

use makepad_widgets::{makepad_html::HtmlDoc, *};
use matrix_sdk::{ruma::{matrix_uri::MatrixId, OwnedMxcUri}, OwnedServerName};

use crate::{avatar_cache::{self, AvatarCacheEntry}, profile::user_profile_cache, sliding_sync::{current_user_id, submit_async_request, MatrixRequest}, utils};

use super::avatar::AvatarWidgetExt;

/// The color of the text used to print the spoiler reason before the hidden text.
const COLOR_SPOILER_REASON: Vec4 = vec4(0.6, 0.6, 0.6, 1.0);

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::avatar::Avatar;

    BaseLinkPill = <RoundedView> {
        width: Fit, height: Fit,
        flow: Right,
        align: { y: 0.5 }
        padding: { left: 7, right: 7, bottom: 5, top: 5 }
        spacing: 5.0,

        show_bg: true,
        draw_bg: {
            color: #000,
            border_radius: 7.0,
        }

        avatar = <Avatar> {
            height: 18.0, width: 18.0,
            text_view = { text = { draw_text: {
                text_style: <TITLE_TEXT>{ font_size: 10.0 }
            }}}
        }

        title = <Label> {
            flow: Right, // do not wrap
            draw_text: {
                color: #f,
                text_style: <MESSAGE_TEXT_STYLE> { font_size: 10.0 },
            }
            text: "Unknown",
        }
    }

    // A pill-shaped widget that displays a Matrix link,
    // either a link to a user, a room, or a message in a room.
    MatrixLinkPill = {{MatrixLinkPill}}<BaseLinkPill> { }

    // A RobrixHtmlLink is either a regular Html link (default) or a Matrix link.
    // The Matrix link is a pill-shaped widget with an avatar and a title.
    pub RobrixHtmlLink = {{RobrixHtmlLink}} {
        width: Fit, height: Fit,
        flow: RightWrap, // ensure the link text can wrap
        align: { y: 0.5 },
        cursor: Hand,

        html_link_view = <View> {
            visible: true,
            width: Fit, height: Fit,
            flow: RightWrap,

            html_link = <HtmlLink> {
                hover_color: (COLOR_LINK_HOVER)
                grab_key_focus: false,
                padding: {left: 1.0, right: 1.5},
            }
        }

        matrix_link_view = <View> {
            visible: false
            width: Fit, height: Fit,

            matrix_link = <MatrixLinkPill> { }
        }
    }

    // This is an HTML subwidget used to handle `<font>` and `<span>` tags,
    // specifically: foreground text color, background color, and spoilers.
    pub MatrixHtmlSpan = {{MatrixHtmlSpan}} {
        width: Fit, height: Fit,
        align: {x: 0., y: 0.}
    }


    // A centralized widget where we define styles and custom elements for HTML
    // message content. This is a wrapper around Makepad's built-in `Html` widget.
    pub MessageHtml = <Html> {
        padding: 0.0,
        width: Fill, height: Fit, // see comment in `HtmlOrPlaintext`
        flow: RightWrap,
        align: { y: 0.5 }
        font_size: (MESSAGE_FONT_SIZE),
        font_color: (MESSAGE_TEXT_COLOR),
        draw_normal:      { color: (MESSAGE_TEXT_COLOR), text_style: { line_spacing: (MESSAGE_TEXT_LINE_SPACING) } }
        draw_italic:      { color: (MESSAGE_TEXT_COLOR), text_style: { line_spacing: (MESSAGE_TEXT_LINE_SPACING) } }
        draw_bold:        { color: (MESSAGE_TEXT_COLOR), text_style: { line_spacing: (MESSAGE_TEXT_LINE_SPACING) } }
        draw_bold_italic: { color: (MESSAGE_TEXT_COLOR), text_style: { line_spacing: (MESSAGE_TEXT_LINE_SPACING) } }
        draw_fixed:       { color: (MESSAGE_TEXT_COLOR), text_style: { line_spacing: (MESSAGE_TEXT_LINE_SPACING) } }
        draw_block: {
            line_color: (MESSAGE_TEXT_COLOR)
            sep_color: (MESSAGE_TEXT_COLOR)
            code_color: (#EDEDED)
            quote_bg_color: (#EDEDED)
            quote_fg_color: (MESSAGE_TEXT_COLOR)
        }

        quote_layout: { spacing: 0, padding: {left: 15, top: 10.0, bottom: 10.0}, }
        quote_walk: { margin: { top: 5, bottom: 5, left: 0 } }

        sep_walk: { margin: { top: 10, bottom: 10 } }

        list_item_layout: { padding: {left: 5.0, top: 1.0, bottom: 1.0}, }
        list_item_walk: { margin: { left: 0, right: 0, top: 3, bottom: 3 } }
        code_layout: { padding: {top: 15.0, bottom: 15.0, left: 15, right: 5 } }
        code_walk: { margin: { top: 10, bottom: 10, left: 0, right: 0 } }

        heading_margin: { top: 1.0, bottom: 0.1 }
        paragraph_margin: { top: 0.33, bottom: 0.33 }

        inline_code_padding: {top: 3, bottom: 3, left: 4, right: 4 }
        inline_code_margin: { left: 3, right: 3, bottom: 2, top: 2 }

        font = <MatrixHtmlSpan> { }
        span = <MatrixHtmlSpan> { }
        a = <RobrixHtmlLink> { }

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
    pub HtmlOrPlaintext = {{HtmlOrPlaintext}} {
        width: Fill, height: Fit, // see above comment
        flow: Overlay

        plaintext_view = <View> {
            visible: true,
            width: Fill, height: Fit, // see above comment
            pt_label = <Label> {
                width: Fill, height: Fit, // see above comment
                flow: RightWrap,
                padding: 0,
                draw_text: {
                    wrap: Word,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE> { font_size: (MESSAGE_FONT_SIZE) },
                }
                text: "[plaintext message placeholder]",
            }
        }

        html_view = <View> {
            visible: false,
            width: Fill, height: Fit, // see above comment
            html = <MessageHtml> {}
        }
    }
}

#[derive(Debug, Clone, DefaultNone)]
pub enum RobrixHtmlLinkAction{
    ClickedMatrixLink {
        /// The URL of the link, which is only temporarily needed here
        /// because we don't fully handle MatrixId links directly in-app yet.
        url: String,
        matrix_id: MatrixId,
        via: Vec<OwnedServerName>,
        key_modifiers: KeyModifiers,
    },
    None,
}

/// A RobrixHtmlLink is either a regular `HtmlLink` (default) or a Matrix link.
///
/// Matrix links are displayed using the [`MatrixLinkPill`] widget.
#[derive(Live, Widget)]
struct RobrixHtmlLink {
    #[deref] view: View,

    /// The displayable text of the link.
    /// This should be set automatically by the Html widget
    /// when it parses and draws an Html `<a>` tag.
    #[live] pub text: ArcStringMut,
    /// The URL of the link.
    /// This is set by the `after_apply()` logic below.
    #[live] pub url: String,
}

impl LiveHook for RobrixHtmlLink {
    fn after_apply(&mut self, _cx: &mut Cx, apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        if let ApplyFrom::NewFromDoc { .. } = apply.from {
            let scope = apply.scope.as_ref().unwrap();
            let doc = scope.props.get::<HtmlDoc>().unwrap();
            let mut walker = doc.new_walker_with_index(scope.index + 1);
            if let Some((id!(href), attr)) = walker.while_attr_lc() {
                self.url = attr.into();
            }
        }
    }
}

impl Widget for RobrixHtmlLink {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // TODO: this is currently disabled because Makepad doesn't yet support
        // partial vertical alignment of inline Html subwidgets with the surrounding text.
        // Once makepad supports that, we can re-enable this to show the Pill widgets.
        /*
        if let Ok(matrix_to_uri) = MatrixToUri::parse(&self.url) {
            self.draw_matrix_pill(cx, matrix_to_uri.id(), matrix_to_uri.via());
        } else if let Ok(matrix_uri) = MatrixUri::parse(&self.url) {
            self.draw_matrix_pill(cx, matrix_uri.id(), matrix_uri.via());
        } else {
            self.draw_html_link(cx);
        }
        */
        self.draw_html_link(cx);
        self.view.draw_walk(cx, scope, walk)
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
    #[allow(unused)]
    fn draw_matrix_pill(&mut self, cx: &mut Cx, matrix_id: &MatrixId, via: &[OwnedServerName]) {
        if let Some(mut pill) = self.matrix_link_pill(ids!(matrix_link)).borrow_mut() {
            pill.populate_pill(cx, self.url.clone(), matrix_id, via);
        }
        self.view(ids!(matrix_link_view)).set_visible(cx, true);
        self.view(ids!(html_link_view)).set_visible(cx, false);
    }

    /// Shows the inner plain HTML link and hides the Matrix link pill view.
    fn draw_html_link(&mut self, cx: &mut Cx) {
        self.view(ids!(html_link_view)).set_visible(cx, true);
        self.view(ids!(matrix_link_view)).set_visible(cx, false);
        let mut html_link = self.html_link(ids!(html_link));
        html_link.set_url(&self.url);
        html_link.set_text(cx, self.text.as_ref());
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum MatrixLinkPillState {
    Requested,
    Loaded {
        matrix_id: MatrixId,
        name: String,
        avatar_url: Option<OwnedMxcUri>,
    },
    None,
}

/// A pill-shaped widget that shows a Matrix link as an avatar and a title.
///
/// This can be a link to a user, a room, or a message in a room.
#[derive(Live, LiveHook, Widget)]
struct MatrixLinkPill {
    #[deref] view: View,

    #[rust] matrix_id: Option<MatrixId>,
    #[rust] via: Vec<OwnedServerName>,
    #[rust] state: MatrixLinkPillState,
    #[rust] url: String,
}

impl Widget for MatrixLinkPill {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
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

        // To catch updates Redraw upon a UI Signal in order to catch updates to a user profile.
        if matches!(event, Event::Signal) && matches!(self.matrix_id, Some(MatrixId::User(_))) {
            self.redraw(cx);
        }

        if let Hit::FingerUp(fe) = event.hits_with_capture_overload(cx, self.area(), true) {
            if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                if let Some(matrix_id) = self.matrix_id.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path,
                        RobrixHtmlLinkAction::ClickedMatrixLink {
                            matrix_id,
                            via: self.via.clone(),
                            key_modifiers: fe.modifiers,
                            url: self.url.clone(),
                        }
                    );
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        self.label(ids!(title)).text()
    }

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        self.label(ids!(title)).set_text(cx, v);
    }
}

impl MatrixLinkPill {
    /// Populates this pill's info based on the given Matrix ID and via servers.
    fn populate_pill(&mut self, cx: &mut Cx, url: String, matrix_id: &MatrixId, via: &[OwnedServerName]) {
        self.url = url;
        self.matrix_id = Some(matrix_id.clone());
        self.via = via.to_vec();

        // Handle a user ID link by querying the user profile cache.
        if let MatrixId::User(user_id) = matrix_id {
            // Apply red background for current user
            if current_user_id().is_some_and(|u| &u == user_id) {
                self.apply_over(cx, live! {
                    draw_bg: { color: #d91b38 }
                });
            }

            match user_profile_cache::with_user_profile(
                cx,
                user_id.clone(),
                true,
                |profile, _| { (profile.displayable_name().to_owned(), profile.avatar_state.clone()) }
            ) {
                Some((name, avatar)) => {
                    self.set_text(cx, &name);
                    self.populate_avatar(cx, avatar.uri().cloned());
                }
                None => {
                    self.set_text(cx, user_id.as_ref());
                    self.populate_avatar(cx, None);
                }
            }
            return;
        }

        // Handle room ID or alias
        match &self.state {
            MatrixLinkPillState::Loaded { name, avatar_url, .. } => {
                self.label(ids!(title)).set_text(cx, name);
                self.populate_avatar(cx, avatar_url.clone());
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
        // While waiting for the async request to complete, show the matrix room ID/alias.
        match matrix_id {
            MatrixId::Room(room_id) => self.set_text(cx, room_id.as_str()),
            MatrixId::RoomAlias(alias) => self.set_text(cx, alias.as_str()),
            MatrixId::Event(room_or_alias, _) => self.set_text(cx, &format!("Message in {}", room_or_alias.as_str())),
            _ => { }
        }
        self.populate_avatar(cx, None);
    }

    fn populate_avatar(&self, cx: &mut Cx, avatar_url: Option<OwnedMxcUri>) {
        let avatar_ref = self.avatar(ids!(avatar));
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
        // Show a text avatar if we couldn't load an image into the avatar.
        avatar_ref.show_text(cx, None, None, self.text());
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
#[derive(Live, Widget)]
struct MatrixHtmlSpan {
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

impl LiveHook for MatrixHtmlSpan {
    // After an MatrixHtmlSpan instance has been instantiated ("applied"),
    // populate its struct fields from the `<span>` or `<font>` tag's attributes.
    fn after_apply(&mut self, _cx: &mut Cx, apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        // The attributes we care about (we allow all attributes in both tags):
        // * in `<font>` tags: `color`
        // * in `<span>` tags: `data-mx-color`, `data-mx-bg-color`, `data-mx-spoiler`

        if let ApplyFrom::NewFromDoc {..} = apply.from {
            if let Some(scope) = apply.scope.as_ref() {
                if let Some(doc) = scope.props.get::<HtmlDoc>() {
                    let mut walker = doc.new_walker_with_index(scope.index + 1);
                    while let Some((lc, attr)) = walker.while_attr_lc() {
                        let attr = attr.trim_matches(['"', '\'']);
                        match lc {
                            id!(color)
                            | id!(data-mx-color) => self.fg_color = Vec4::from_hex_str(attr).ok(),
                            id!(data-mx-bg-color) => self.bg_color = Vec4::from_hex_str(attr).ok(),
                            id!(data-mx-spoiler) => self.spoiler = SpoilerDisplay::Hidden { reason: attr.into() },
                            _ => ()
                        }
                    }
                }
            } else {
                error!("BUG: MatrixHtmlSpan::after_apply(): scope not found, cannot set attributes.");
            }
        }
    }
}

impl Widget for MatrixHtmlSpan {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let mut needs_redraw = false;
        for area in self.drawn_areas.clone().into_iter() {
            match event.hits(cx, area) {
                Hit::FingerDown(..) if self.grab_key_focus => {
                    cx.set_key_focus(self.area());
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
        self.redraw(cx);
    }
}


#[derive(LiveHook, Live, Widget)]
pub struct HtmlOrPlaintext {
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
        self.view(ids!(html_view)).set_visible(cx, false);
        self.view(ids!(plaintext_view)).set_visible(cx, true);
        self.label(ids!(plaintext_view.pt_label)).set_text(cx, text.as_ref());
    }

    /// Sets the HTML content, making the HTML visible and the plaintext invisible.
    pub fn show_html<T: AsRef<str>>(&mut self, cx: &mut Cx, html_body: T) {
        self.html(ids!(html_view.html)).set_text(cx, html_body.as_ref());
        self.view(ids!(html_view)).set_visible(cx, true);
        self.view(ids!(plaintext_view)).set_visible(cx, false);
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
}
