use gen_components::*;
use makepad_widgets::*;

live_design! {
    use link::widgets::*;
    use link::theme::*;
    use link::shaders::*;
    
    use link::gen_components::*;

    pub KeyboardPage = {{KeyboardPage}}{
        <ScrollYView> {
            flow: Down,
            spacing: 16.0,
            align: {
                x: 0.5,
                y: 0.5
            },
            show_bg: true,
            <GTable>{
                mode: Real,
                background_color: #667085,
                background_visible: true,
                height: Fit,
                width: Fit,
                header: <GTableHeader> {
                    height: Fit,
                    width: Fit,
                    <GTRow>{
                        height: 32.0,
                        width: Fit,
                        <GTCell>{
                            height: Fill,
                            width: 660.0,
                            align: {
                                x: 0.0,
                                y: 0.5
                            },
                            <GLabel> {
                                padding: {left: 3}
                                text: "Composer"
                                font_size: 20.0
                                color:#000
                            }
                        }
                    }
                }
                body: <GTBody>{
                    height: Fit,
                    width: Fit,
                    <GTRow>{
                        height: 50.0,
                        width: Fit,
                        <GTCell>{
                            height: Fill,
                            width: 480.0,
                            align: {
                                x: 0.0,
                                y: 0.5
                            },
                            <GLabel>{
                                padding: {left: 3}
                                color: #667085,
                                font_size: 13.0
                                text: "Send message",
                            }  
                        }
                        <GTCell>{
                            height: Fill,
                            width: 180.0,
                            <GLabel>{
                                padding: {left: 3}
                                color: #667085,
                                text: "Ctrl + Enter",
                            }  
                        }
                    }
                    <GTRow>{
                        height: 50.0,
                        width: Fit,
                        <GTCell>{
                            height: Fill,
                            width: 480.0,
                            align: {
                                x: 0.0,
                                y: 0.5
                            },
                            <GLabel>{
                                padding: {left: 3}
                                font_size: 13.0
                                color: #667085,
                                text: "New line",
                            }  
                        }
                        <GTCell>{
                            height: Fill,
                            width: 180.0,
                            <GLabel>{
                                padding: {left: 3}
                                color: #667085,
                                text: "Shift + Enter",
                            } 
                        }
                    }
                    <GTRow>{
                        height: 50.0,
                        width: Fit,
                        <GTCell>{
                            height: Fill,
                            width: 480.0,
                            align: {
                                x: 0.0,
                                y: 0.5
                            },
                            <GLabel>{
                                padding: {left: 3}
                                font_size: 13.0
                                color: #667085,
                                text: "Toggle Bold",
                            }  
                        }
                        <GTCell>{
                            height: Fill,
                            width: 180.0,
                            <GLabel>{
                                padding: {left: 3}
                                color: #667085,
                                text: "Crtl + B",
                            } 
                        }
                    }
                    <GTRow>{
                        height: 50.0,
                        width: Fit,
                        <GTCell>{
                            height: Fill,
                            width: 480.0,
                            align: {
                                x: 0.0,
                                y: 0.5
                            },
                            <GLabel>{
                                padding: {left: 3}
                                font_size: 13.0
                                color: #667085,
                                text: "Toggle Italics",
                            }  
                        }
                        <GTCell>{
                            height: Fill,
                            width: 180.0,
                            <GLabel>{
                                padding: {left: 3}
                                color: #667085,
                                text: "Ctrl + I",
                            } 
                        }
                    }
                }
            }
        }
    }
        
}

#[derive(Widget, Live, LiveHook)]
pub struct KeyboardPage {
    #[deref]
    view: View,
}

impl Widget for KeyboardPage {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)  
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }
    
}

impl WidgetMatchEvent for KeyboardPage {
    fn handle_actions(&mut self, _cx: &mut Cx, _e:&Actions, _scope: &mut Scope) {
        
    }
    
}


