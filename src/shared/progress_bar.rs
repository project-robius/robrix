//! A progress bar widget with capsule-shaped design for showing upload/download progress.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ProgressBar = set_type_default() do #(ProgressBar::register_widget(vm)) {
        width: Fill,
        height: 8,
        show_bg: true,
        fill_color: (COLOR_ACTIVE_PRIMARY)
        fill_color_failed: (COLOR_FG_DANGER_RED)

        draw_bg +: {
            progress: instance(0.0)

            // Background color (track)
            color: (COLOR_SECONDARY)
            progress_color: instance((COLOR_ACTIVE_PRIMARY))

            border_radius: 2.0

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size);

                // Draw background track (full width, rounded)
                sdf.box(
                    0.0,
                    0.0,
                    self.rect_size.x,
                    self.rect_size.y,
                    self.border_radius
                );
                sdf.fill(self.color);

                // Draw progress bar (partial width based on progress, rounded)
                let progress_width = self.rect_size.x * self.progress;
                if progress_width > 0.0 {
                    sdf.box(
                        0.0,
                        0.0,
                        progress_width,
                        self.rect_size.y,
                        self.border_radius
                    );
                    sdf.fill(self.progress_color);
                }

                return sdf.result;
            }
        }
    }
}

/// A capsule-shaped progress bar widget.
#[derive(Script, ScriptHook, Widget)]
pub struct ProgressBar {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    #[live] fill_color: Vec4f,
    #[live] fill_color_failed: Vec4f,

    /// Current progress value between 0.0 and 1.0
    #[rust] progress: f32,
    #[rust] is_failed: bool,
}

impl Widget for ProgressBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Update the progress and its color before drawing
        let progress = self.progress.clamp(0.0, 1.0);
        let color = if self.is_failed { self.fill_color_failed } else { self.fill_color };
        self.view.draw_bg.set_dyn_instance(cx, id!(progress), &[progress]);
        self.view.draw_bg.set_dyn_instance(cx, id!(progress_color), &[color.x, color.y, color.z, color.w]);
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ProgressBar {
    /// Sets the progress value (0.0 to 1.0).
    pub fn set_progress(&mut self, cx: &mut Cx, value: f32) {
        self.progress = value.clamp(0.0, 1.0);
        self.redraw(cx);
    }

    /// Marks this progress bar as failed or not, which also sets its color.
    pub fn set_failed(&mut self, cx: &mut Cx, failed: bool) {
        if self.is_failed != failed {
            self.is_failed = failed;
            self.redraw(cx);
        }
    }
}

impl ProgressBarRef {
    /// Sets the progress value (0.0 to 1.0).
    pub fn set_progress(&self, cx: &mut Cx, value: f32) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_progress(cx, value);
        }
    }

    /// See [`ProgressBar::set_failed()`].
    pub fn set_failed(&self, cx: &mut Cx, failed: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_failed(cx, failed);
        }
    }
}
