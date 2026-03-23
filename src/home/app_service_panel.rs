use makepad_widgets::*;

use crate::home::room_screen::RoomScreenProps;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.AppServicePanel = #(AppServicePanel::register_widget(vm)) {
        visible: false
        width: Fill
        height: Fit
        margin: Inset{left: 12, right: 12, top: 6, bottom: 8}
        flow: Down
        align: Align{x: 0.0, y: 0.0}

        card := RoundedView {
            width: Fill
            height: Fit
            flow: Down
            spacing: 10
            padding: Inset{top: 12, right: 12, bottom: 12, left: 12}

            show_bg: true
            draw_bg +: {
                color: #xEEF4FB
                border_radius: 14.0
                border_size: 1.0
                border_color: #xD6E2F0
            }

            header := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 10
                align: Align{y: 0.5}

                title_group := View {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 4

                    title := Label {
                        width: Fill
                        height: Fit
                        draw_text +: {
                            text_style: TITLE_TEXT {font_size: 11.5}
                            color: #111
                        }
                        text: "App Service Actions"
                    }

                    subtitle := Label {
                        width: Fill
                        height: Fit
                        flow: Flow.Right{wrap: true}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 10.0}
                            color: #556070
                        }
                        text: "Commands are sent into this room after BotFather is bound, similar to an inline bot tools card."
                    }
                }

                dismiss_button := RobrixIconButton {
                    width: Fit
                    height: Fit
                    padding: 8
                    spacing: 0
                    align: Align{x: 0.5, y: 0.0}
                    draw_icon.svg: (ICON_CLOSE)
                    draw_icon.color: #66768A
                    icon_walk: Walk{width: 14, height: 14, margin: 0}
                    draw_bg +: {
                        border_size: 0
                        border_radius: 999.0
                        color: #0000
                        color_hover: #00000012
                        color_down: #x0000001e
                    }
                    text: ""
                }
            }

            first_row := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 8

                create_button := RobrixPositiveIconButton {
                    width: Fill
                    padding: Inset{top: 11, bottom: 11, left: 12, right: 12}
                    draw_icon.svg: (ICON_CHECKMARK)
                    icon_walk: Walk{width: 16, height: 16}
                    text: "Create Bot"
                }

                list_button := RobrixNeutralIconButton {
                    width: Fill
                    padding: Inset{top: 11, bottom: 11, left: 12, right: 12}
                    draw_icon.svg: (ICON_SEARCH)
                    icon_walk: Walk{width: 15, height: 15}
                    text: "List Bots"
                }
            }

            second_row := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 8

                delete_button := RobrixNegativeIconButton {
                    width: Fill
                    padding: Inset{top: 11, bottom: 11, left: 12, right: 12}
                    draw_icon.svg: (ICON_TRASH)
                    icon_walk: Walk{width: 16, height: 16}
                    text: "Delete Bot"
                }

                help_button := RobrixNeutralIconButton {
                    width: Fill
                    padding: Inset{top: 11, bottom: 11, left: 12, right: 12}
                    draw_icon.svg: (ICON_INFO)
                    icon_walk: Walk{width: 15, height: 15}
                    text: "Bot Help"
                }
            }

            third_row := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 8

                unbind_button := RobrixNeutralIconButton {
                    width: Fill
                    padding: Inset{top: 11, bottom: 11, left: 12, right: 12}
                    draw_icon.svg: (ICON_HIERARCHY)
                    icon_walk: Walk{width: 16, height: 16}
                    text: "Unbind BotFather"
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum AppServicePanelAction {
    Dismiss,
    OpenCreateBotModal,
    OpenDeleteBotModal,
    SendListBots,
    SendBotHelp,
    Unbind,
    #[default]
    None,
}

impl ActionDefaultRef for AppServicePanelAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: AppServicePanelAction = AppServicePanelAction::None;
        &DEFAULT
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct AppServicePanel {
    #[deref]
    view: View,
}

impl Widget for AppServicePanel {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let room_screen_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("BUG: RoomScreenProps should be available in Scope::props for AppServicePanel");

        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(card.header.dismiss_button)).clicked(actions) {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::Dismiss,
                );
            }

            if self.view.button(cx, ids!(card.first_row.create_button)).clicked(actions) {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::OpenCreateBotModal,
                );
            }

            if self.view.button(cx, ids!(card.first_row.list_button)).clicked(actions) {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::SendListBots,
                );
            }

            if self.view.button(cx, ids!(card.second_row.delete_button)).clicked(actions) {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::OpenDeleteBotModal,
                );
            }

            if self.view.button(cx, ids!(card.second_row.help_button)).clicked(actions) {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::SendBotHelp,
                );
            }

            if self.view.button(cx, ids!(card.third_row.unbind_button)).clicked(actions) {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::Unbind,
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
