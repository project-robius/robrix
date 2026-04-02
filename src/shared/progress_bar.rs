//! A progress bar widget with capsule-shaped design for showing upload/download progress.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ProgressBar = set_type_default() do #(ProgressBar::register_widget(vm)) {
        width: Fill,
        height: 8,
        show_bg: true,

        draw_bg +: {
            progress: instance(0.0)

            // Background color (track)
            color: (COLOR_SECONDARY)
            // Filled portion color
            progress_color: instance((COLOR_ACTIVE_PRIMARY))

            border_radius: 4.0

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

                // Draw progress fill (partial width based on progress, rounded)
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

    /// Current progress value between 0.0 and 1.0
    #[rust] progress: f32,
}

impl Widget for ProgressBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Update the progress uniform before drawing
        let progress = self.progress.clamp(0.0, 1.0);
        script_apply_eval!(cx, self.view, {
            draw_bg.progress: #(progress as f64),
        });
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ProgressBar {
    /// Sets the progress value (0.0 to 1.0).
    pub fn set_progress(&mut self, cx: &mut Cx, value: f32) {
        self.progress = value.clamp(0.0, 1.0);
        self.redraw(cx);
    }

    /// Gets the current progress value.
    pub fn progress(&self) -> f32 {
        self.progress
    }
}

impl ProgressBarRef {
    /// Sets the progress value (0.0 to 1.0).
    pub fn set_progress(&self, cx: &mut Cx, value: f32) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_progress(cx, value);
        }
    }

    /// Gets the current progress value.
    pub fn progress(&self) -> f32 {
        self.borrow().map(|inner| inner.progress()).unwrap_or(0.0)
    }
}
