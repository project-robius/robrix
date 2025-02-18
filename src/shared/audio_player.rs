use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub AudioPlayer = {{AudioPlayer}} {
        width: Fill, height: Fit,
        flow: Overlay,
        play_or_pause_button = <RobrixIconButton> {
            width: Fit,
            height: Fit,
            margin: {left: 0, top: 4, bottom: 4, right: 4},
            padding: 8,
            align: {x: 0.5, y: 0.5}
            draw_icon: {
                svg_file: (ICON_PLAY),
                fn get_color(self) -> vec4 {
                    return #x888;
                }
            }
            icon_walk: {width: 12, height: 12}
        }
    }
}


/// A view that holds an image or text content, and can switch between the two.
///
/// This is useful for displaying alternate text when an image is not (yet) available
/// or fails to load. It can also be used to display a loading message while an image
/// is being fetched.
#[derive(Live, Widget, LiveHook)]
pub struct AudioPlayer {
    #[deref] view: View,
}

impl Widget for AudioPlayer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
