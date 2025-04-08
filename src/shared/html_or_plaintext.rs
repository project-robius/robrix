//! A `HtmlOrPlaintext` view can display either plaintext or rich HTML content.

use makepad_widgets::{makepad_html::HtmlDoc, *};
use matrix_sdk::{ruma::{matrix_uri::MatrixId, MatrixToUri, MatrixUri, OwnedMxcUri}, OwnedServerName};

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
        flow: Overlay,
        align: { y: 0.5 },
        cursor: Hand,

        html_link_view = <View> {
            visible: true,
            width: Fit, height: Fit,

            html_link = <HtmlLink> {
                hover_color: #21b070
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
        align: { y: 0.5 }
        font_size: (MESSAGE_FONT_SIZE),
        font_color: (MESSAGE_TEXT_COLOR),
        draw_normal:      { color: (MESSAGE_TEXT_COLOR), } // text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }
        draw_italic:      { color: (MESSAGE_TEXT_COLOR), } // text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }
        draw_bold:        { color: (MESSAGE_TEXT_COLOR), } // text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }
        draw_bold_italic: { color: (MESSAGE_TEXT_COLOR), } // text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }
        draw_fixed:       { color: (MESSAGE_TEXT_COLOR), } // text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }
        draw_block: {
            line_color: (MESSAGE_TEXT_COLOR)
            sep_color: (MESSAGE_TEXT_COLOR)
            code_color: (#EDEDED)
            quote_bg_color: (#EDEDED)
            quote_fg_color: (MESSAGE_TEXT_COLOR)
        }
        list_item_layout: { padding: {left: 5.0, top: 1.0, bottom: 1.0}, }
        list_item_walk: { margin: { left: 0, right: 0, top: 2, bottom: 4 } }
        code_layout: { padding: 10.0 }
        code_walk: { margin: { top: 10, bottom: 10, left: 0, right: 0 } }
        quote_layout: { spacing: 0, padding: {top: 0.0, bottom: 0.0}, }
        quote_walk: { margin: { top: 5, bottom: 5 } }
        inline_code_padding: {top: 3, bottom: 3, left: 4, right: 4 }
        inline_code_margin: { left: 3, right: 3, bottom: 2, top: 2 }


        font = <MatrixHtmlSpan> { }
        span = <MatrixHtmlSpan> { }
        a = <RobrixHtmlLink> { }

        body: "[<i> HTML message placeholder</i>]",
    }

    // A view container that displays either plaintext s(a simple `Label`)
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
        matrix_id: MatrixId,
        via: Vec<OwnedServerName>,
        key_modifiers: KeyModifiers
    },
    None,
}

#[derive(Debug, Clone, Default)]
pub enum RobrixHtmlLinkType {
    Matrix,
    #[default]
    Normal,
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
    #[live] pub url: String,
}

impl LiveHook for RobrixHtmlLink {
    fn after_apply(&mut self, _cx: &mut Cx, apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        if let ApplyFrom::NewFromDoc { .. } = apply.from {
            let scope = apply.scope.as_ref().unwrap();
            let doc = scope.props.get::<HtmlDoc>().unwrap();
            let mut walker = doc.new_walker_with_index(scope.index + 1);
            if let Some((live_id!(href), attr)) = walker.while_attr_lc() {
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
        if let Ok(matrix_to_uri) = MatrixToUri::parse(&self.url) {
            self.show_matrix_link(cx, matrix_to_uri.id(), matrix_to_uri.via());
        } else if let Ok(matrix_uri) = MatrixUri::parse(&self.url) {
            self.show_matrix_link(cx, matrix_uri.id(), matrix_uri.via());
        } else {
            self.show_html_link(cx, self.url.clone());
        }
        // self.show_html_link(cx, self.url.clone());
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
    fn show_matrix_link(&mut self, cx: &mut Cx, matrix_id: &MatrixId, via: &[OwnedServerName]) {
        self.matrix_link_pill(id!(matrix_link))
            .set_info(cx, matrix_id, via);
        self.view(id!(matrix_link_view)).set_visible(cx, true);
        self.view(id!(html_link_view)).set_visible(cx, false);
        self.redraw(cx);
    }

    fn show_html_link(&mut self, cx: &mut Cx, url: String) {
        self.view(id!(html_link_view)).set_visible(cx, true);
        self.view(id!(matrix_link_view)).set_visible(cx, false);
        let mut html_link = self.html_link(id!(html_link));
        html_link.set_url(&url);
        html_link.set_text(cx, self.text.as_ref());
        self.redraw(cx);

    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum MatrixLinkPillInfo {
    #[default]
    None,
    Requested,
    Loaded {
        matrix_id: MatrixId,
        name: String,
        avatar_url: Option<OwnedMxcUri>,
    },
    Failed,
}

/// A pill-shaped widget that shows a Matrix link as an avatar and a title.
///
/// This can be a link to a user, a room, or a message in a room.
#[derive(Live, LiveHook, Widget)]
struct MatrixLinkPill {
    #[deref] view: View,

    #[rust] matrix_id: Option<MatrixId>,
    #[rust] via: Vec<OwnedServerName>,

    #[rust] status: MatrixLinkPillInfo,
}

impl Widget for MatrixLinkPill {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(MatrixLinkPillInfo::Loaded { matrix_id, name, avatar_url } ) = action.downcast_ref() {
                    if let Some(raw_matrix_id) = &self.matrix_id {
                        if raw_matrix_id == matrix_id {
                            self.status = MatrixLinkPillInfo::Loaded {
                                matrix_id: matrix_id.clone(),
                                name: name.clone(),
                                avatar_url: avatar_url.clone(),
                            };
                            self.redraw(cx);
                        }
                    }
                }
            }
        }

        let hit = event.hits_with_capture_overload(cx, self.area(), true);
        if let Hit::FingerUp(fe) = hit {
            if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                if let Some(matrix_id) = self.matrix_id.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path,
                        RobrixHtmlLinkAction::ClickedMatrixLink {
                            matrix_id,
                            via: self.via.clone(),
                            key_modifiers: fe.modifiers
                        }
                    );
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatrixLinkPill {
    /// Sets this pill's internal info based on the given Matrix ID and via servers.
    pub fn set_info(&mut self, cx: &mut Cx, matrix_id: &MatrixId, via: &[OwnedServerName]) {
        self.matrix_id = Some(matrix_id.clone());
        self.via = via.to_vec();

        // Handle user ID early
        if let MatrixId::User(user_id) = matrix_id {
            // Apply red background for current user
            if current_user_id().is_some_and(|u| &u == user_id) {
                self.apply_over(cx, live! {
                    draw_bg: { color: #d91b38 }
                });
            }

            // Fetch profile
            match user_profile_cache::with_user_profile(cx, user_id.clone(), true, |profile, _| {
                (profile.displayable_name().to_owned(), profile.avatar_state.clone())
            }) {
                Some((name, avatar)) => {
                    self.set_name(cx, &name);
                    self.set_avatar(cx, avatar.uri().cloned());
                }
                None => {
                    self.set_name(cx, &user_id.as_ref());
                    self.set_avatar(cx, None);
                }
            }

            self.redraw(cx);
            return;
        }

        // Handle room ID or alias
        match &self.status {
            MatrixLinkPillInfo::None => {
                submit_async_request(MatrixRequest::GetMatrixRoomLinkPillInfo {
                    matrix_id: matrix_id.clone(),
                    via: via.to_vec(),
                });
                self.status = MatrixLinkPillInfo::Requested;
                self.set_name(cx, "Loading...");
            }
            MatrixLinkPillInfo::Requested => {
                self.set_name(cx, "Loading...");
            }
            MatrixLinkPillInfo::Loaded { name, avatar_url, .. } => {
                self.set_name(cx, name);
                self.set_avatar(cx, avatar_url.clone());
                self.redraw(cx);
                return;
            }
            MatrixLinkPillInfo::Failed => {
                self.set_name(cx, "Failed to load");
            }
        }
        self.set_avatar(cx, None);
        self.redraw(cx);
    }

    fn name(&self) -> String {
        self.label(id!(title)).text()
    }

    fn set_name(&self, cx: &mut Cx, name: &str) {
        self.label(id!(title)).set_text(cx, name);
    }

    fn set_avatar(&self, cx: &mut Cx, avatar_url: Option<OwnedMxcUri>) {
        let avatar_ref = self.avatar(id!(avatar));
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
        avatar_ref.show_text(cx, None, self.name());
    }

}

impl MatrixLinkPillRef {
    /// See [`MatrixLinkPill::set_info()`].
    pub fn set_info(&self, cx: &mut Cx, matrix_id: &MatrixId, via: &[OwnedServerName]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_info(cx, matrix_id, via);
        }
    }

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
                            live_id!(color)
                            | live_id!(data-mx-color) => self.fg_color = Vec4::from_hex_str(attr).ok(),
                            live_id!(data-mx-bg-color) => self.bg_color = Vec4::from_hex_str(attr).ok(),
                            live_id!(data-mx-spoiler) => self.spoiler = SpoilerDisplay::Hidden { reason: attr.into() },
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
        self.view(id!(html_view)).set_visible(cx, false);
        self.view(id!(plaintext_view)).set_visible(cx, true);
        self.label(id!(plaintext_view.pt_label)).set_text(cx, text.as_ref());
    }

    /// Sets the HTML content, making the HTML visible and the plaintext invisible.
    pub fn show_html<T: AsRef<str>>(&mut self, cx: &mut Cx, html_body: T) {
        self.html(id!(html_view.html)).set_text(cx, html_body.as_ref());
        self.view(id!(html_view)).set_visible(cx, true);
        self.view(id!(plaintext_view)).set_visible(cx, false);
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
