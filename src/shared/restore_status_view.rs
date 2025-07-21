use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;

    pub RestoreStatusView = <View>{
        width: Fill, height: Fill,
        flow: Down,
        align: {x: 0.5, y: 0.5},                
        restore_status_spinner = <LoadingSpinner> {
            width: 50,
            height: 50,
            visible: true,
            draw_bg: {
                color: (COLOR_SELECT_TEXT)
                border_size: 3.0,
            }
        }
        restore_status_label = <Label> {
            width: Fill, height: Fit,
            align: {x: 0.5, y: 0.0},
            padding: {left: 5.0, right: 0.0}
            margin: {top: 10.0},
            flow: RightWrap,
            draw_text: {
                color: (TYPING_NOTICE_TEXT_COLOR),
            }
        }
    }
}
