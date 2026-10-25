//! A `CollapsibleLabel` shows only the first few lines of a long text,
//! with "Show more" and "Show less" buttons to expand and collapse it.
//!
//! While collapsed, line breaks become spaces, so those few lines hold as much text as they can.
//! Once expanded, it shows the full text as written.

use makepad_widgets::*;

use crate::utils;

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

    mod.widgets.CollapsibleLabel = #(CollapsibleLabel::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down
        align: Align{x: 0.5, y: 0}
        collapsed_lines: 2

        label := Label {
            width: Fill, height: Fit
            padding: 0
            margin: 0
            flow: Flow.Right{wrap: true}
            text_overflow: Ellipsis
            text: ""
        }

        expand_button := mod.widgets.ShowMoreLessButton {
            margin: Inset{top: 6}
            draw_icon +: { svg: (ICON_TRIANGLE_DOWN) }
            text: "Show more…"
        }

        collapse_button := mod.widgets.ShowMoreLessButton {
            margin: Inset{top: 6}
            draw_icon +: { svg: (ICON_TRIANGLE_UP) }
            text: "Show less"
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct CollapsibleLabel {
    #[deref] view: View,
    #[walk] walk: Walk,
    #[layout] layout: Layout,
    #[redraw] #[area] #[rust] area: Area,
    /// How many lines of text to show while collapsed.
    #[live] collapsed_lines: usize,
    #[rust] text: String,
    /// The text with its line breaks replaced by spaces.
    #[rust] collapsed_text: String,
    #[rust] is_expanded: bool,
}

impl Widget for CollapsibleLabel {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if let Event::Actions(actions) = event {
            let expand_button = self.view.button(cx, ids!(expand_button));
            let collapse_button = self.view.button(cx, ids!(collapse_button));
            if expand_button.clicked(actions) || collapse_button.clicked(actions) {
                self.is_expanded = !self.is_expanded;
                expand_button.reset_hover(cx);
                collapse_button.reset_hover(cx);
                self.redraw(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let label = self.view.label(cx, ids!(label));
        label.set_text(cx, if self.is_expanded { &self.text } else { &self.collapsed_text });
        if let Some(mut label) = label.borrow_mut() {
            label.max_lines = if self.is_expanded { 0 } else { self.collapsed_lines };
        }
        cx.begin_turtle(walk, self.layout);
        let label_walk = label.walk(cx);
        let text_width = cx.turtle().max_width(label_walk);
        label.draw_all(cx, scope);

        // We can only tell if the collapsed text overflows once it's laid out,
        // so we check that right after drawing the label.
        let is_collapsible = !self.collapsed_text.is_empty() && label.borrow().is_some_and(|label| {
            let laidout = label.draw_text.layout(
                cx,
                0.0,
                0.0,
                text_width.map(|w| (w.max(0.0) as f32) / label.draw_text.font_scale.max(0.0001)),
                true,
                label.align,
                &self.collapsed_text,
            );
            if self.is_expanded {
                laidout.rows.len() > self.collapsed_lines
            } else {
                laidout.is_truncated
            }
        });
        let expand_button = self.view.button(cx, ids!(expand_button));
        let collapse_button = self.view.button(cx, ids!(collapse_button));
        let (shown, hidden) = if self.is_expanded {
            (collapse_button, expand_button)
        } else {
            (expand_button, collapse_button)
        };
        hidden.set_visible(cx, false);
        shown.set_visible(cx, is_collapsible);
        if is_collapsible {
            shown.draw_all(cx, scope);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }

    fn text(&self) -> String {
        self.text.clone()
    }

    /// Sets the text, collapsing it again if it changed.
    fn set_text(&mut self, cx: &mut Cx, text: &str) {
        if self.text == text {
            return;
        }
        self.text.clear();
        self.text.push_str(text);
        self.collapsed_text = utils::replace_linebreaks_separators(text, false).into_owned();
        self.is_expanded = false;
        self.redraw(cx);
    }
}
