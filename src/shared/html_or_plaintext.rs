//! A `HtmlOrPlaintext` view can display either plaintext or rich HTML content.

use makepad_widgets::{makepad_html::HtmlDoc, *};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    
    use crate::shared::styles::*;

    // These match the `MESSAGE_*` styles defined in `styles.rs`.
    // For some reason, they're not the same. That's TBD.
    // HTML_LINE_SPACING = 6.0
    // HTML_TEXT_HEIGHT_FACTOR = 1.1


    // This is an HTML subwidget used to handle `<font>` and `<span>` tags,
    // specifically: foreground text color, background color, and spoilers.
    pub MatrixHtmlSpan = {{MatrixHtmlSpan}}<Label> {
        width: Fit,
        height: Fit,

        draw_text: {
            wrap: Word,
            color: (MESSAGE_TEXT_COLOR),
            text_style: <MESSAGE_TEXT_STYLE> { } //height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) },
        }
        text: "MatrixHtmlSpan placeholder",

    }

    // A centralized widget where we define styles and custom elements for HTML
    // message content. This is a wrapper around Makepad's built-in `Html` widget.
    pub MessageHtml = <Html> {
        padding: 0.0,
        width: Fill, height: Fit, // see comment in `HtmlOrPlaintext`
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
        code_layout: { padding: {left: 7.0, right: 7.0, top: 8.0, bottom: 0.0}, }
        quote_layout: { padding: {top: 0.0, bottom: 0.0}, }
        inline_code_padding: { left: 5.0, right: 5.0, top: 7.0, bottom: 0.0 }

        font = <MatrixHtmlSpan> { }
        span = <MatrixHtmlSpan> { }

        a = {
            padding: {left: 1.0, right: 1.5},
        }

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


/// A custom HTML subwidget used to handle `<font>` and `<span>` tags,
/// specifically: foreground text color, background text color, and spoilers.
#[derive(Live, Widget)]
pub struct MatrixHtmlSpan {
    /// The URL of the image to display.
    #[deref] ll: Label,
    /// Background color: the `data-mx-bg-color` attribute.
    #[rust] bg_color: Option<Vec4>,
    /// Foreground (text) color: the `data-mx-color` or `color` attributes.
    #[rust] fg_color: Option<Vec4>,
    /// There are three possible spoiler variants:
    /// 1. `None`: no spoiler attribute was present at all.
    /// 2. `Some(empty)`: there was a spoiler but no reason was given.
    /// 3. `Some(reason)`: there was a spoiler with a reason given for it being hidden.
    #[rust] spoiler: Option<String>,
}

impl Widget for MatrixHtmlSpan {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.ll.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.ll.draw_walk(cx, scope, walk)
    }

    fn set_text(&mut self, v: &str) {
        self.ll.set_text(v);
    }
}

impl LiveHook for MatrixHtmlSpan {
    // After an MatrixHtmlSpan instance has been instantiated ("applied"),
    // populate its struct fields from the `<span>` tag's attributes.
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        // The attributes we care about in `<font>` and `<span>` tags are:
        // * data-mx-bg-color, data-mx-color, data-mx-spoiler, color.

        if let ApplyFrom::NewFromDoc {..} = apply.from {
            if let Some(scope) = apply.scope.as_ref() {
                if let Some(doc) = scope.props.get::<HtmlDoc>() {
                    let mut walker = doc.new_walker_with_index(scope.index + 1);
                    while let Some((lc, attr)) = walker.while_attr_lc(){
                        let attr = attr.trim_matches(['"', '\'']);
                        match lc {
                            live_id!(color)
                            | live_id!(data-mx-color) => self.fg_color = Vec4::from_hex_str(attr).ok(),
                            live_id!(data-mx-bg-color) => self.bg_color = Vec4::from_hex_str(attr).ok(),
                            live_id!(data-mx-spoiler) => self.spoiler = Some(attr.into()),
                            _ => ()
                        }
                    }

                    // Set the Label's foreground text color and background color
                    if let Some(fg_color) = self.fg_color {
                        self.ll.apply_over(cx, live!{ draw_text: { color: (fg_color) } });
                    }
                    if let Some(_bg_color) = self.bg_color {
                        log!("TODO: Html span/font background color is not yet implemented.")
                        // self.apply_over(cx, live!{ draw_bg: { color: (bg_color) } });
                    }
                    // TODO: need to handle label events to handle the spoiler, so we can toggle it upon click.
                }
            } else {
                warning!("MatrixHtmlSpan::after_apply(): scope not found, cannot set attributes.");
            }
        }
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
    pub fn show_plaintext<T: AsRef<str>>(&mut self, text: T) {
        self.view(id!(html_view)).set_visible(false);
        self.view(id!(plaintext_view)).set_visible(true);
        self.label(id!(plaintext_view.pt_label)).set_text(text.as_ref());
    }

    /// Sets the HTML content, making the HTML visible and the plaintext invisible.
    pub fn show_html<T: AsRef<str>>(&mut self, html_body: T) {
        self.html(id!(html_view.html)).set_text(html_body.as_ref());
        self.view(id!(html_view)).set_visible(true);
        self.view(id!(plaintext_view)).set_visible(false);
    }
}

impl HtmlOrPlaintextRef {
    /// See [`HtmlOrPlaintext::show_plaintext()`].
    pub fn show_plaintext<T: AsRef<str>>(&self, text: T) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_plaintext(text);
        }
    }

    /// See [`HtmlOrPlaintext::show_html()`].
    pub fn show_html<T: AsRef<str>>(&self, html_body: T) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_html(html_body);
        }
    }
}
