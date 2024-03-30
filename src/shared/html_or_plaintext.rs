//! A `HtmlOrPlaintext` view can display either plaintext or rich HTML content.

use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::view::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::styles::*;

    FONT_SIZE_P = 12.5
    COLOR_P = #x999

    TEXT_P = {
        font_size: (FONT_SIZE_P),
        height_factor: 1.65,
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }

    // A centralized widget where we define styles and custom elements for HTML
    // message content. This is a wrapper around Makepad's built-in `Html` widget.
    RobrixHtml = <Html> {
        font_size: (FONT_SIZE_P),
        draw_normal: { color: (COLOR_P) }
        draw_italic: { color: (COLOR_P) }
        draw_bold:   { color: (COLOR_P) }
        draw_bold_italic: { color: (COLOR_P) }
        draw_fixed:  { color: (COLOR_P) }
        body: "Sample <b>bold</b> <i> italic</i> message",
    }

    // A view container that displays either plaintext s(a simple `Label`)
    // or rich HTML content (an instance of `RobrixHtml`).
    HtmlOrPlaintext = {{HtmlOrPlaintext}} {
        width: Fill, height: Fit,
        flow: Overlay
        
        plaintext_view = <View> {
            visible: true,
            pt_label = <Label> {
                width: Fit,
                height: Fit
                draw_text: {
                    wrap: Word,
                    color: (COLOR_P),
                    text_style: <REGULAR_TEXT> { },
                }
                text: "<PLACEHOLDER>",
            }
        }
        
        html_view = <View> {
            visible: false,
            width: Fill, height: Fit,
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
        log!("HtmlOrPlaintext::show_plaintext(): {:?}", text.as_ref());
        self.view(id!(html_view)).set_visible(false);
        self.view(id!(plaintext_view)).set_visible(true);
        self.label(id!(plaintext_view.pt_label)).set_text(text.as_ref());
    }

    /// Sets the HTML content, making the HTML visible and the plaintext invisible.
    pub fn show_html<T: AsRef<str>>(&mut self, html_body: T) {
        log!("HtmlOrPlaintext::show_html(): {:?}", html_body.as_ref());
        self.html(id!(html_view.html)).set_text(html_body.as_ref());
        self.view(id!(html_view)).set_visible(true);
        self.view(id!(plaintext_view)).set_visible(false);
    }
}

impl HtmlOrPlaintextRef {
    /// See [`HtmlOrPlaintext::show_plaintext()`].
    pub fn show_plaintext<T: AsRef<str>>(&self, text: T) {
        log!("HtmlOrPlaintextRef::show_plaintext(): {:?}", text.as_ref());
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_plaintext(text);
        }
    }

    /// See [`HtmlOrPlaintext::show_html()`].
    pub fn show_html<T: AsRef<str>>(&self, html_body: T) {
        log!("HtmlOrPlaintextRef::show_html(): {:?}", html_body.as_ref());
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_html(html_body);
        }
    }
}
