use gen_components::*;
use makepad_widgets::*;

live_design! {
    use link::widgets::*;
    use link::theme::*;
    use link::shaders::*;
    
    use link::gen_components::*;
    use crate::shared::avatar::Avatar;

    pub AccountPage = {{AccountPage}}{
        <ScrollYView> {
            flow: Down,
            spacing: 30.0,
            align: {
                x: 0.5,
                y: 0.0
            },
            padding: {top: 30, right: 30, bottom: 30, left: 30}

            <GView> {
                height: Fit,
                width: Fill,
                flow: Down,
                spacing: 8,
                <GLabel> {
                    text: "Profile",
                    font_size: 20,
                    color: #fff
                }

                <GLabel> {
                    text: "This is how you appear to others on the app.",
                    font_size: 12,
                    color: #fff
                }
            }
            
            <View> {
                width: Fill,
                height: Fit,
                flow: Right,
                spacing: 40,
                <View> {
                    height: Fit,
                    width: Fit,
                    flow: Right,
                    spacing: 10,
                    <Avatar> {
                        width: 150,
                        height: 150,
                    }
                }

                <View> {
                    width: 350,
                    height: Fit,
                    flow: Down,
                    spacing: 10,
                    align: {
                        x: 0.0,
                        y: 1.0
                    }
                    <GLabel> {
                        text: "Display name"
                        font_size: 20.0
                        color:#fff
                    }

                    <GInput>{
                        font_size: 20
                        cursor_width: 3.0,
                        border_width: 1.0,
                        height: 50.0,
                        width: Fill,
                        placeholder: "Place Input"
                    }
                }
            }

            <View> {
                width: 650,
                height: Fit,
                flow: Down,
                spacing: 10,
                <GLabel> {
                    text: "Username"
                    font_size: 20.0
                    color:#fff
                }

                <GInput>{
                    spread_radius: 2.0,
                    shadow_offset: vec2(2.0, 2.0),
                    cursor_width: 3.0,
                    border_width: 1.0,
                    theme: Error,
                    height: 32.0,
                    width: Fill,
                    placeholder: "Place Input"
                }
            }

            <View> {
                height: Fit,
                width: 500,
                flow: Right,
                align: {
                    x: 0.0,
                    y: 0.5
                },

                <GButton> {
                    width: 120.0,
                    height: 50.
                    theme: Error,
                    border_width: 2.0,
                    hover_color: #52e64c,
                    border_color: #fff,
                    border_radius: 4.0,
                    slot: <View> {
                        flow: Right,
                        spacing: 6,
                        align: {
                            x: 0.5,
                            y: 0.5
                        },
                        <GIcon>{
                            width: 18.0,
                            height: 18.0,
                            theme: Info,
                            icon_type: OpenBottom,
                            stroke_width: 1.2
                        }

                        <GLabel> {
                            text: "Sign out",
                        }
                    }
                    
                    padding: {left: 14.0, right: 14.0, top: 8.0, bottom: 8.0},
                }
            }
        }
    }
        
}

#[derive(Widget, Live, LiveHook)]
pub struct AccountPage {
    #[deref]
    view: View,
}

impl Widget for AccountPage {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)  
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }
    
}

impl WidgetMatchEvent for AccountPage {
    fn handle_actions(&mut self, _cx: &mut Cx, _e:&Actions, _scope: &mut Scope) {
        
    }
    
}


