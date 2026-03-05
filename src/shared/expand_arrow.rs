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
    #[source] source: ScriptObjectRef,
    #[apply_default] animator: Animator,
    #[redraw] #[live] draw_bg: DrawQuad,
    #[walk] walk: Walk,
    /// Tracks the desired opened state set from outside.
    /// Applied to draw_bg.opened during draw_walk.
    #[rust] opened_value: f32,
}

impl ExpandArrow {
    /// Animate open/close (use in event handlers only, not during draw).
    pub fn set_is_open(&mut self, cx: &mut Cx, is_open: bool, animate: Animate) {
        self.opened_value = if is_open { 1.0 } else { 0.0 };
        self.animator_toggle(cx, is_open, animate, ids!(expand.expanded), ids!(expand.collapsed))
    }

    /// Set open/close state without animation (safe to call anytime).
    pub fn set_is_open_no_animate(&mut self, is_open: bool) {
        self.opened_value = if is_open { 1.0 } else { 0.0 };
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
