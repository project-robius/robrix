use makepad_widgets::*;
use gen_components::*;



live_design! {
    use link::widgets::*;
    use link::theme::*;
    use link::shaders::*;
    
    use link::gen_components::*;

    pub NotificationPage = {{NotificationPage}}{
        <ScrollYView> {
            height: Fill,
            width: Fill,
            flow: Down,
            spacing: 10.0,
            align: {
                x: 0.5,
                y: 0.5
            },
            <GVLayout>{
                spacing: 6.0,
                height: Fit,
                width: Fit,
                margin: {bottom: 50}

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
                                width: 480.0,
                                align: {
                                    x: 0.0,
                                    y: 0.5
                                },
                                <GLabel> {
                                    text: "Gobal",
                                    font_size: 20.0,
                                    color: #667085
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                align: {
                                    x: 0.5,
                                    y: 1.0
                                },
                                <GLabel> {
                                    text: "Off"
                                    font_size: 10.0,
                                    color: #667085
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                align: {
                                    x: 0.5,
                                    y: 1.0
                                },
                                <GLabel> {
                                    text: "On"
                                    font_size: 10.0
                                    color: #667085

                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                align: {
                                    x: 0.5,
                                    y: 1.0
                                },
                                <GLabel> {
                                    text: "Nolsy"
                                    font_size: 10.0
                                    color: #667085
                                }
                            }
                        }
                    }
                    body: <GTBody>{
                        height: Fit,
                        width: Fit,
                        <GTRow>{
                            height: 32.0,
                            width: Fit,
                            <GTCell>{
                                height: Fill,
                                width: 480.0,
                                align: {
                                    x: 0.0,
                                    y: 0.5
                                },
                                <GLabel>{
                                    color: #667085,
                                    text: "Messages in one-to-one chats",
                                }  
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                        }
                        <GTRow>{
                            height: 32.0,
                            width: Fit,
                            <GTCell>{
                                height: Fill,
                                width: 480.0,
                                align: {
                                    x: 0.0,
                                    y: 0.5
                                },
                                <GLabel>{
                                    color: #667085,
                                    text: "Encrypted messages in one-to-one chats",
                                }  
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                        }
                        <GTRow> {
                            height: 32.0,
                            width: Fit,
                            <GTCell>{
                                height: Fill,
                                width: 480.0,
                                align: {
                                    x: 0.0,
                                    y: 0.5
                                },
                                <GLabel>{
                                    color: #667085,
                                    text: "Messages in group chats",
                                }  
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                        }
                        <GTRow> {
                            height: 32.0,
                            width: Fit,
                            <GTCell>{
                                height: Fill,
                                width: 480.0,
                                align: {
                                    x: 0.0,
                                    y: 0.5
                                },
                                <GLabel>{
                                    color: #667085,
                                    text: "Encrypted messages in group chats",
                                }  
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                            <GTCell>{
                                height: Fill,
                                width: 60.0,
                                <GCheckbox>{
                                    theme: Dark,
                                }
                            }
                        }
                    }
                }
            }

            <View> {
                width: Fit,
                height: Fit,
                flow: Down,
                spacing: 10,
                margin: {bottom: 50}

                <View> {
                    flow: Right,
                    width: Fit,
                    height: Fit
                    spacing: 10,

                    <GLabel> {
                        width: 500.0,
                        text: "Enable notifications for this account"
                    }

                    toggle = <GToggle> {
                        theme: Primary,   
                    }
                }

                <View> {
                    flow: Right,
                    width: Fit,
                    height: Fit
                    spacing: 10,

                    <GLabel> {
                        width: 500.0,
                        text: "Enable notifications for this device"
                    }

                    toggle = <GToggle> {
                        theme: Success,   
                    }
                }

                <View> {
                    flow: Right,
                    width: Fit,
                    height: Fit
                    spacing: 10,

                    <GLabel> {
                        width: 500.0,
                        text: "Enable desktop notifications for this session"
                    }

                    toggle = <GToggle> {
                        theme: Error,   
                    }
                }

                <View> {
                    flow: Right,
                    width: Fit,
                    height: Fit
                    spacing: 10,

                    <GLabel> {
                        width: 500.0,
                        text: "Show message in desktop notification"
                    }

                    toggle = <GToggle> {
                        theme: Info,   
                    }
                }

                <View> {
                    flow: Right,
                    width: Fit,
                    height: Fit
                    spacing: 10,

                    <GLabel> {
                        width: 500.0,
                        text: "Enable audible notifications for this session"
                    }

                    toggle = <GToggle> {
                        theme: Warning,   
                    }
                }

                <View> {
                    flow: Right,
                    width: Fit,
                    height: Fit
                    spacing: 10,

                    <GLabel> {
                        width: 500.0,
                        text: "Enable email notifications for g1024536444@gmail.com"
                    }

                    toggle = <GToggle> {
                        theme: Dark,   
                    }
                }
            }
            
            <View> {
                height: Fit,
                width: 550,
                flow: Down,
                spacing: 20,

                <GLabel> {
                    text: "Lanuage"
                    font_size: 20.0
                    draw_text:{
                        color:#fff
                    }
                }

                examplaselect = <View> {
                    height: Fit,
                    width: Fit,

                    select = <GSelect> {
                        background_color: #fb505a,
                        background_visible: true,
                        // hover_color: #152e5b,
                        select_item: <GSelectItem> {
                            color: #000,
                            font_size: 16.0,

                        }
                    }
                }
            }
        
        }
    }
        
}

#[derive(Widget, Live)]
pub struct NotificationPage {
    #[deref]
    view: View,
}

impl LiveHook for NotificationPage {
    fn after_apply(&mut self, _cx: &mut Cx, _apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        self.gselect(id!(select)).borrow_mut().map(|mut x| {
            x.options = vec![
                ("English", "en").into(),
                ("Chinses", "ch").into(),
            ];
        });
    }
}

impl Widget for NotificationPage {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)  
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }
    
}

impl WidgetMatchEvent for NotificationPage {
    fn handle_actions(&mut self, _cx: &mut Cx, _e:&Actions, _scope: &mut Scope) {
        
    }
    
}
