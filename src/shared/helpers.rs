use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub Divider = <View> {
        width: Fill, height: Fit
        flow: Down
        <RoundedView> {
            width: Fill,
            height: 1.,
            draw_bg: {color: (#ddd)}
        }
    }

    pub LineH = <RoundedView> {
        width: Fill,
        height: 2.0,
        margin: 0.0,
        padding: 0.0, spacing: 0.0
        show_bg: true
        draw_bg: {color: (COLOR_DIVIDER)}
    }

    pub FillerX = <View> { width: Fill, height: Fit }
    pub FillerY = <View> { width: Fit, height: Fill }
}
