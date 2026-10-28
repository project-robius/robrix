use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.BouncingDots = View {
        width: 24,
        height: 12,
        flow: Down,

        show_bg: true,
        draw_bg +: {
            color: uniform((COLOR_TEXT_BODY)),
            freq: uniform(0.9),  // Animation frequency
            phase_offset: uniform(5.0), // Phase difference
            dot_radius: uniform(1.5), // Dot radius
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                let amplitude = self.rect_size.y * 0.21;
                let center_y = self.rect_size.y * 0.5;
                // Animating from the pass's clock repaints only while we're drawn, without any NextFrames.
                let phase = self.draw_pass.time * 2.0 * PI * self.freq;
                // Create three circle SDFs
                sdf.circle(
                    self.rect_size.x * 0.25,
                    amplitude * sin(phase) + center_y,
                    self.dot_radius
                );
                sdf.fill(self.color);
                sdf.circle(
                    self.rect_size.x * 0.5,
                    amplitude * sin(phase + self.phase_offset) + center_y,
                    self.dot_radius
                );
                sdf.fill(self.color);
                sdf.circle(
                    self.rect_size.x * 0.75,
                    amplitude * sin(phase + self.phase_offset * 2) + center_y,
                    self.dot_radius
                );
                sdf.fill(self.color);
                return sdf.result;
            }
        }
    }
}
