//! `LazyView`, a widget that only gets built from its template once it's first shown.
//!
//! Even a hidden widget is costly to create, so a template with rarely shown parts
//! (like a message's reply preview) wraps them in this to only pay for the ones it shows.

use makepad_widgets::*;

use crate::{LivePtr, widget_ref_from_live_ptr};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LazyView = #(LazyView::register_widget(vm)) {}
}

/// Until it's first shown, this takes up no space and gets no events.
/// Afterwards, it lays out, draws and handles events just like the widget built from `template`.
#[derive(Script, WidgetRef, WidgetSet, WidgetRegister)]
pub struct LazyView {
    #[uid] uid: WidgetUid,
    /// We only build this to show it, so it should start out visible.
    #[live] template: Option<LivePtr>,
    #[rust] content: WidgetRef,
}

impl ScriptHook for LazyView {
    fn on_after_apply(&mut self, vm: &mut ScriptVm, apply: &Apply, scope: &mut Scope, _value: ScriptValue) {
        // A reload never reaches our content, so we re-apply the template to it ourselves.
        if apply.is_reload()
            && !self.content.is_empty()
            && let Some(template) = self.template
        {
            self.content.script_apply(vm, apply, scope, template);
        }
    }
}

impl WidgetNode for LazyView {
    fn widget_uid(&self) -> WidgetUid { self.uid }
    fn walk(&mut self, cx: &mut Cx) -> Walk { self.content.walk(cx) }
    fn area(&self) -> Area { self.content.area() }
    fn redraw(&mut self, cx: &mut Cx) { self.content.redraw(cx) }
    fn visible(&self) -> bool { !self.content.is_empty() && self.content.visible() }

    fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        if visible && self.content.is_empty() {
            self.content = widget_ref_from_live_ptr(cx, self.template);
            cx.widget_tree_mark_dirty(self.uid);
            // We have no area to redraw until our content draws, so outside of a draw walk we redraw everything.
            if !cx.is_in_draw_walk {
                cx.redraw_all();
            }
        }
        self.content.set_visible(cx, visible);
    }

    fn children(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) {
        if !self.content.is_empty() {
            visit(LiveId(0), self.content.clone());
        }
    }
    fn find_widgets_from_point(&self, cx: &Cx, point: DVec2, found: &mut dyn FnMut(&WidgetRef)) {
        self.content.find_widgets_from_point(cx, point, found)
    }
    fn selection_text_len(&self) -> usize { self.content.selection_text_len() }
    fn selection_point_to_char_index(&self, cx: &Cx, abs: DVec2) -> Option<usize> { self.content.selection_point_to_char_index(cx, abs) }
    fn selection_set(&mut self, anchor: usize, cursor: usize) { self.content.selection_set(anchor, cursor) }
    fn selection_clear(&mut self) { self.content.selection_clear() }
    fn selection_select_all(&mut self) { self.content.selection_select_all() }
    fn selection_get_text_for_range(&self, start: usize, end: usize) -> String { self.content.selection_get_text_for_range(start, end) }
    fn selection_get_full_text(&self) -> String { self.content.selection_get_full_text() }
}

impl Widget for LazyView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.content.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.content.draw_walk(cx, scope, walk)
    }

    // Only our content is interactive, and it gets checked on its own.
    fn is_interactive(&self) -> bool { false }
}

impl LazyViewRef {
    /// Returns the widget built from this view's template, or an empty ref if it hasn't been shown yet.
    pub fn content(&self) -> WidgetRef {
        self.borrow().map_or_else(WidgetRef::empty, |inner| inner.content.clone())
    }
}
