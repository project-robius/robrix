use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ExpandArrowBase = #(ExpandArrow::register_widget(vm))

    mod.widgets.ExpandArrow = set_type_default() do mod.widgets.ExpandArrowBase {
        width: 18, height: 18,

        draw_bg +: {
            opened: instance(0.0)
            color: instance(#888)
            border_radius: uniform(2.25)

            pixel: fn() {
                let corner_round = self.border_radius
                let sz = self.rect_size.x * 0.3 - corner_round * 0.5
                let c = vec2(self.rect_size.x * 0.5, self.rect_size.y * 0.5)
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.clear(vec4(0.0))

                // Triangle pointing up; rotation maps opened to:
                //   0.0 -> 90deg (right-pointing, collapsed)
                //   1.0 -> 180deg (down-pointing, expanded)
                sdf.rotate(self.opened * 0.5 * PI + 0.5 * PI, c.x, c.y)
                sdf.move_to(c.x - sz, c.y + sz)
                sdf.line_to(c.x, c.y - sz)
                sdf.line_to(c.x + sz, c.y + sz)
                sdf.close_path()

                // Keep the filled triangle, then slightly expand it with a crisp stroke
                // to geometrically round sharp corners (no blur).
                sdf.fill_keep(self.color)
                return sdf.stroke(self.color, corner_round)
            }
        }

        animator: Animator{
            expand: {
                default: @collapsed
                collapsed: AnimatorState{
                    from: {all: Forward {duration: 0.15}}
                    ease: ExpDecay {d1: 0.96, d2: 0.97}
                    redraw: true
                    apply: { draw_bg: {opened: 0.0} }
                }
                expanded: AnimatorState{
                    from: {all: Forward {duration: 0.15}}
                    ease: ExpDecay {d1: 0.98, d2: 0.95}
                    redraw: true
                    apply: { draw_bg: {opened: 1.0} }
                }
            }
        }
    }
}

/// Animated expand/collapse triangle arrow.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct ExpandArrow {
    #[uid] uid: WidgetUid,
    #[source] source: ScriptObjectRef,
    #[apply_default] animator: Animator,
    #[redraw] #[live] draw_bg: DrawQuad,
    #[walk] walk: Walk,
    /// Tracks the desired opened state set from outside.
    /// Applied to draw_bg.opened during draw_walk.
    #[rust] opened_value: f32,
}

impl ExpandArrow {
    /// Sets the arrow to open/close with or without animation.
    ///
    /// This should only be used in event handlers, not during a draw function.
    pub fn set_is_open(&mut self, cx: &mut Cx, is_open: bool, animate: Animate) {
        if matches!(animate, Animate::Yes) && !self.animator.is_track_animating(id!(expand)) {
            let drawn_open = self.opened_value > 0.5;
            self.animator_cut(cx, if drawn_open { ids!(expand.expanded) } else { ids!(expand.collapsed) });
        }
        self.opened_value = if is_open { 1.0 } else { 0.0 };
        self.animator_toggle(cx, is_open, animate, ids!(expand.expanded), ids!(expand.collapsed))
    }

    /// Sets the open/close state without animating.
    ///
    /// This is okay to call at any point in any context.
    pub fn set_is_open_no_animate(&mut self, is_open: bool) {
        self.opened_value = if is_open { 1.0 } else { 0.0 };
    }

    pub fn set_color(&mut self, cx: &mut Cx, color: Vec4) {
        self.draw_bg.set_dyn_instance(cx, id!(color), &[color.x, color.y, color.z, color.w]);
        self.draw_bg.redraw(cx);
    }

    /// Sets the arrow to point up without animating it.
    pub fn set_pointing_up_no_animate(&mut self) {
        // The shader draws the triangle pointing up at zero rotation, which is one step before "closed".
        self.opened_value = -1.0;
    }
}

impl Widget for ExpandArrow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.draw_bg.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.animator.is_track_animating(id!(expand)) {
            self.draw_bg.set_dyn_instance(cx, id!(opened), &[self.opened_value]);
        }
        self.draw_bg.draw_walk(cx, walk);
        DrawStep::done()
    }
}
