//! A `HtmlOrPlaintext` view can display either plaintext or rich HTML content.

use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::view::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::styles::*;

    HTML_LINE_SPACING = 8.0
    HTML_TEXT_HEIGHT_FACTOR = 1.3
    // A centralized widget where we define styles and custom elements for HTML
    // message content. This is a wrapper around Makepad's built-in `Html` widget.
    RobrixHtml = <Html> {
        padding: 0.0,
        line_spacing: (HTML_LINE_SPACING),
        width: Fill, height: Fit, // see comment in `HtmlOrPlaintext`
        font_size: (MESSAGE_FONT_SIZE),
        draw_normal:      { color: (MESSAGE_TEXT_COLOR), text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }
        draw_italic:      { color: (MESSAGE_TEXT_COLOR), text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }
        draw_bold:        { color: (MESSAGE_TEXT_COLOR), text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }
        draw_bold_italic: { color: (MESSAGE_TEXT_COLOR), text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }
        draw_fixed:       {                              text_style: { height_factor: (HTML_TEXT_HEIGHT_FACTOR), line_spacing: (HTML_LINE_SPACING) } }

        list_item_layout: { line_spacing: 5.0, padding: {top: 1.0, bottom: 1.0}, }
        body: "[<i> HTML message placeholder</i>]",
    }

    // A view container that displays either plaintext s(a simple `Label`)
    // or rich HTML content (an instance of `RobrixHtml`).
    //
    // Key Usage Notes:
    // * Labels need their width to be Fill *and* all of their parent views
    //   also need to have their width set to Fill. Otherwise, the label
    //   won't wrap text properly.
    // * They also need their height to be Fit along with all of their parent views,
    //   otherwise their total height will be zero (when a Fit is inside of a Fill),
    //   resulting in nothing being displayed.
    HtmlOrPlaintext = {{HtmlOrPlaintext}} {
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
                    text_style: <MESSAGE_TEXT_STYLE> { },
                }
                text: "[plaintext message placeholder]",
            }
        }
        
        html_view = <View> {
            visible: false,
            width: Fill, height: Fit, // see above comment
            html = <RobrixHtml> {}
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
