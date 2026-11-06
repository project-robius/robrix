//! A `CollapsibleHtml` shows only the first few lines of a long text (e.g., a room's topic),
//! with its URLs shown as links, and "Show more" and "Show less" buttons to expand and collapse it.
//!
//! While collapsed, line breaks become spaces, so those few lines hold as much text as they can.
//! Once expanded, it shows the full text as written.

use std::borrow::Cow;

use makepad_widgets::*;

use crate::{shared::html_or_plaintext::RobrixHtmlLinkAction, utils};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // The show more button (to expand) and show less button (to collapse) tall content.
    mod.widgets.ShowMoreLessButton = RobrixIconButton {
        visible: false
        width: Fit, height: Fit
        spacing: 4
        padding: Inset{ top: 4, bottom: 4, left: 8, right: 8 }
        draw_icon +: { color: (COLOR_TEXT_MUTED) }
        icon_walk: Walk{ width: 10, height: 10 }
        draw_text +: {
            text_style: theme.font_regular { font_size: 10.0, line_spacing: 1.2 }
            color: (COLOR_TEXT_MUTED)
            color_hover: (COLOR_TEXT_MUTED)
            color_down: (COLOR_TEXT_MUTED)
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
    }

    let ShowMoreButton = mod.widgets.ShowMoreLessButton {
        margin: Inset{top: 6}
        draw_icon +: { svg: (ICON_TRIANGLE_DOWN) }
        text: "Show more…"
    }

    let ShowLessButton = mod.widgets.ShowMoreLessButton {
        margin: Inset{top: 6}
        draw_icon +: { svg: (ICON_TRIANGLE_UP) }
        text: "Show less"
    }

    mod.widgets.CollapsibleHtml = #(CollapsibleHtml::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down
        collapsed_lines: 2

        html := mod.widgets.MessageHtml {
            margin: 0
            text_overflow: Ellipsis
            body: ""
            // Like a label's, its lines are closer together than a message's.
            text_style_normal      +: { line_spacing: 1.2 }
            text_style_italic      +: { line_spacing: 1.2 }
            text_style_bold        +: { line_spacing: 1.2 }
            text_style_bold_italic +: { line_spacing: 1.2 }
            text_style_fixed       +: { line_spacing: 1.2 }
        }

        expand_button := ShowMoreButton {}
        collapse_button := ShowLessButton {}
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct CollapsibleHtml {
    #[deref] view: View,
    #[walk] walk: Walk,
    #[layout] layout: Layout,
    #[redraw] #[area] #[rust] area: Area,
    /// How many lines of text to show while collapsed.
    #[live] collapsed_lines: usize,
    #[rust] text: String,
    #[rust] is_expanded: bool,
}

impl Widget for CollapsibleHtml {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if let Event::Actions(actions) = event {
            let expand_button = self.view.button(cx, ids!(expand_button));
            let collapse_button = self.view.button(cx, ids!(collapse_button));
            if expand_button.clicked(actions) || collapse_button.clicked(actions) {
                self.is_expanded = !self.is_expanded;
                expand_button.reset_hover(cx);
                collapse_button.reset_hover(cx);
                self.show_text_as_html(cx);
                self.redraw(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let html = self.view.html(cx, ids!(html));
        if let Some(mut html) = html.borrow_mut() {
            html.max_lines = if self.is_expanded { 0 } else { self.collapsed_lines };
        }
        cx.begin_turtle(walk, self.layout);
        html.draw_all(cx, scope);

        // We only know if the collapsed text overflows right after drawing it,
        // so once it's expanded, we rely on it having overflowed to get there.
        let is_collapsible = self.is_expanded || html.borrow().is_some_and(|html| html.is_content_truncated());
        let expand_button = self.view.button(cx, ids!(expand_button));
        let collapse_button = self.view.button(cx, ids!(collapse_button));
        let (shown_button, hidden_button) = if self.is_expanded {
            (collapse_button, expand_button)
        } else {
            (expand_button, collapse_button)
        };
        hidden_button.set_visible(cx, false);
        shown_button.set_visible(cx, is_collapsible);
        if is_collapsible {
            shown_button.draw_all(cx, scope);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }

    fn text(&self) -> String {
        self.text.clone()
    }

    /// Sets the plaintext to show, collapsing it again if it changed.
    fn set_text(&mut self, cx: &mut Cx, text: &str) {
        if self.text == text {
            return;
        }
        self.text.clear();
        self.text.push_str(text);
        self.is_expanded = false;
        self.show_text_as_html(cx);
        self.redraw(cx);
    }
}

impl CollapsibleHtml {
    /// Shows our text as HTML with links for its URLs, which keeps its line breaks only while expanded.
    fn show_text_as_html(&mut self, cx: &mut Cx) {
        let text = if self.is_expanded {
            Cow::Borrowed(self.text.as_str())
        } else {
            utils::replace_linebreaks_separators(&self.text, false)
        };
        let html = match utils::linkify(&text, false) {
            Cow::Borrowed(plaintext) => utils::escape_text_and_line_breaks(plaintext).into_owned(),
            Cow::Owned(linkified) => linkified.replace('\n', "<br>"),
        };
        self.view.html(cx, ids!(html)).set_text(cx, &html);
    }
}

/// Handles a link click in a topic by opening that link, and returns `true` if the given action was a link click.
///
/// A `RoomScreen` handles the links in its own topics instead, since it can show them in-app.
pub fn handle_link_clicked(action: &Action) -> bool {
    let url = match action.as_widget_action().cast() {
        HtmlLinkAction::Clicked { url, .. } => url,
        // A matrix.to link is drawn as a pill, which emits its own action instead.
        // TODO: navigate to the room/user in-app, like `RoomScreen` does.
        _ => match action.as_widget_action().cast() {
            RobrixHtmlLinkAction::ClickedMatrixLink { url, .. } => url,
            _ => return false,
        }
    };
    if !url.is_empty() {
        utils::open_url(&url);
    }
    true
}
