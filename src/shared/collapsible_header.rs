//! This module defines a collapsible header wrapper with a triangle icon
//! that indicates whether the header is expanded or collapsed.
//!
//! This widget can be clicked to toggle between expanded and collapsed.
//!
//! The collapsible header is *just* the header, it doesn't actually contain any content.

use makepad_widgets::*;
use makepad_widgets::animator::Animate;


use super::expand_arrow::ExpandArrow;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.CollapsibleHeader = set_type_default() do #(CollapsibleHeader::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill,
        height: 35,
        align: Align{ x: 0.0, y: 0.5 },
        margin: Inset{top: 3, bottom: 3, left: 0, right: 0},
        padding: 5
        flow: Right,

        cursor: MouseCursor.Hand,
        draw_bg +: {
            border_radius: 4.0
            color: mod.widgets.COLOR_HEADER_BG
        }

        collapse_icon := mod.widgets.ExpandArrow {
            width: 20, height: 20,
            margin: Inset{left: 5, right: 6, top: 0, bottom: 0},
            draw_bg.color: mod.widgets.COLOR_HEADER_FG
        }

        label := Label {
            padding: 0,
            width: Fill,
            height: Fit,
            flow: Flow.Right { wrap: false }
            text: "",
            text_overflow: TextOverflow.Ellipsis,
            max_lines: 1
            draw_text +: {
                text_style: mod.widgets.REGULAR_TEXT {font_size: 11},
                color: (mod.widgets.COLOR_HEADER_FG),
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum CollapsibleHeaderAction {
    /// The header was clicked to toggled its expanded/collapsed state.
    Toggled,
    #[default]
    None,
}

#[derive(Script, Widget)]
pub struct CollapsibleHeader {
    #[deref] view: View,
    /// A reload (like a theme switch) resets `#[live]` fields,
    /// so this only seeds `is_expanded` once, when the header is created.
    #[live(true)] starts_expanded: bool,
    #[rust] is_expanded: bool,
}

impl ScriptHook for CollapsibleHeader {
    fn on_after_new(&mut self, _vm: &mut ScriptVm) {
        self.is_expanded = self.starts_expanded;
    }
}

impl Widget for CollapsibleHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Handle hits on this view as a whole before passing the event to the inner view.
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(fe) if !fe.device.is_touch() => {
                cx.set_key_focus(self.view.area());
            }
            Hit::FingerUp(fe)
                if fe.is_over && fe.is_primary_hit() && fe.was_tap() =>
            {
                self.toggle_collapse(cx, scope);
            }
            _ => { }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Set arrow state during draw to ensure child widgets are available.
        if let Some(mut arrow) = self.view.child_by_path(ids!(collapse_icon)).borrow_mut::<ExpandArrow>() {
            arrow.set_is_open_no_animate(self.is_expanded);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl CollapsibleHeader {
    fn toggle_collapse(&mut self, cx: &mut Cx, _scope: &mut Scope) {
        self.is_expanded = !self.is_expanded;
        if let Some(mut arrow) = self.view.child_by_path(ids!(collapse_icon)).borrow_mut::<ExpandArrow>() {
            arrow.set_is_open(cx, self.is_expanded, Animate::Yes);
        }
        self.redraw(cx);
        cx.widget_action(self.widget_uid(), CollapsibleHeaderAction::Toggled);
    }
}

impl CollapsibleHeaderRef {
    /// Expands or collapses the header without animating its arrow.
    pub fn set_expanded(&self, cx: &mut Cx, is_expanded: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.is_expanded = is_expanded;
        inner.redraw(cx);
    }

    pub fn is_expanded(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_expanded)
    }
}
