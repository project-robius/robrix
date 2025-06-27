use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use link::shaders::*;

    use crate::shared::styles::*;

    // Copied from Moly
    pub FadeView = <CachedView> {
        draw_bg: {
            instance opacity: 1.0

            fn pixel(self) -> vec4 {
                let color = sample2d_rt(self.image, self.pos * self.scale + self.shift);
                return Pal::premul(vec4(color.xyz, color.w * self.opacity))
            }
        }
    }

    pub LineH = <RoundedView> {
        width: Fill,
        height: 2.0,
        margin: 0.0,
        padding: 0.0, spacing: 0.0
        show_bg: true
        draw_bg: {color: (COLOR_DIVIDER_DARK)}
    }

    pub FillerX = <View> { width: Fill, height: Fit }
    pub FillerY = <View> { width: Fit, height: Fill }
}
