use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use link::shaders::*;

    use crate::shared::styles::*;

    pub TitleLabel = <Label> {
        width: Fill, height: Fit
        margin: {top: 5},
        flow: RightWrap,
        draw_text: {
            text_style: <TITLE_TEXT>{font_size: 15},
            color: #000
            wrap: Word
        }
    }

    pub SubsectionLabel = <Label> {
        width: Fill, height: Fit
        margin: {top: 5},
        flow: Right,
        draw_text: {
            color: (COLOR_TEXT),
            text_style: <THEME_FONT_BOLD>{ font_size: 13 },
        }
    }

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

    pub Filler  = <View> { width: Fill, height: Fill }
    pub FillerX = <View> { width: Fill, height: Fit }
    pub FillerY = <View> { width: Fit,  height: Fill }
}
