use makepad_widgets::*;

use crate::room::room_info_pane::RoomInfoPaneWidgetExt;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::*;
    import crate::shared::styles::*;
    import crate::shared::avatar::*;
    import crate::shared::icon_button::*;

    import crate::room::room_info_pane::*;
    import crate::room::room_members_pane::*;

    RobrixRadioButtonTab = <RadioButtonTab> {
        padding: 10,

        draw_radio: {
            uniform radius: 3.0
            uniform border_width: 0.0
            instance color_unselected: (THEME_COLOR_TEXT_DEFAULT)
            instance color_unselected_hover: (THEME_COLOR_TEXT_HOVER)
            instance color_selected: (THEME_COLOR_TEXT_SELECTED)
            instance border_color: (THEME_COLOR_TEXT_SELECTED)

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color_unselected,
                        self.color_unselected_hover,
                        self.hover
                    ),
                    self.color_selected,
                    self.selected
                )
            }

            fn get_border_color(self) -> vec4 {
                return self.border_color;
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                match self.radio_type {
                    RadioType::Tab => {
                        sdf.box(
                            self.border_width,
                            self.border_width,
                            self.rect_size.x - (self.border_width * 2.0),
                            self.rect_size.y - (self.border_width * 2.0),
                            max(1.0, self.radius)
                        )
                        sdf.fill_keep(self.get_color())
                        if self.border_width > 0.0 {
                            sdf.stroke(self.get_border_color(), self.border_width)
                        }
                    }
                }
                return sdf.result
            }
        }

        draw_text: {
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color_unselected,
                        self.color_unselected_hover,
                        self.hover
                    ),
                    self.color_selected,
                    self.selected
                )
            }
        }
    }

    ActionToggleButton = <RobrixRadioButtonTab> {
        width: Fill,
        align: { x: 0.5 },
        padding: { left: 20, top: 10, bottom: 10, right: 20 },
        label_walk: { margin: 0 }
        draw_text: {
            text_style: {font_size: 9},
            color_selected: #475467;
            color_unselected: #475467;
            color_unselected_hover: #173437;
        }
        draw_radio: {
            color_unselected: #D0D5DD,
            color_selected: #fff,
            color_unselected_hover: #D0D5DD,
            border_color: #D0D5DD,
            border_width: 1.0,
            radius: 3.0
        }
    }

    RoomDetailsActions = <RoundedView> {
        width: Fill, height: Fit,
        spacing: 5
        padding: 5
        draw_bg: {
            color: #D0D5DD
            radius: 3.0
        }
        info_button = <ActionToggleButton> {
            text: "Info"
            animator: {
                selected = { default: on }
            }
        }
        members_button = <ActionToggleButton> {
            text: "Members"
        }
    }

    // Copied from Moxin
    FadeView = <CachedView> {
        draw_bg: {
            instance opacity: 1.0

            fn pixel(self) -> vec4 {
                let color = sample2d_rt(self.image, self.pos * self.scale + self.shift) + vec4(self.marked, 0.0, 0.0, 0.0);
                return Pal::premul(vec4(color.xyz, color.w * self.opacity))
            }
        }
    }

    RoomDetailsSlidingPaneHeader = <View> {
        width: Fill, height: Fit,
        flow: Right
        padding: 10
        align: { y: 0.5 }
        spacing: 10

        room_details_actions = <RoomDetailsActions> {}

        // The "X" close button on the top right
        close_button = <RobrixIconButton> {
            width: Fit,
            height: Fit,
            padding: 10,
            draw_icon: {
                svg_file: (ICON_CLOSE),
                fn get_color(self) -> vec4 {
                    return #x0;
                }
            }
            icon_walk: {width: 12, height: 12}
        }
    }

    RoomDetailsSlidingPane = {{RoomDetailsSlidingPane}} {
        flow: Overlay,
        width: Fill, height: Fill,
        align: { x: 1.0, y: 0 }
        visible: false

        bg_view = <View> {
            width: Fill
            height: Fill
            visible: false,
            show_bg: true
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return vec4(0., 0., 0., 0.7)
                }
            }
        }

        main_content = <FadeView> {
            width: 360, height: Fill,
            flow: Overlay,

            <View> {
                height: Fill, width: Fill
                show_bg: true,
                draw_bg: {
                    color: #f
                }
                flow: Down,
                <RoomDetailsSlidingPaneHeader> {}
                room_info_pane = <RoomInfoPane> {}
                // room_members_pane = <RoomMembersPane> {}
            }
        }
        animator: {
            panel = {
                default: hide,
                show = {
                    redraw: true,
                    from: {all: Forward {duration: 0.4}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {main_content = { width: 360, draw_bg: {opacity: 1.0} }}
                }
                hide = {
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {main_content = { width: 0, draw_bg: {opacity: 0.0} }}
                }
            }
        }
    }

}

#[derive(Clone, Debug, Default)]
pub enum RoomDetailsSlidingPaneType {
    #[default]
    Info,
    Members,
}

#[derive(Live, LiveHook, Widget)]
pub struct RoomDetailsSlidingPane {
    #[deref]
    view: View,
    #[animator]
    animator: Animator,

}

impl Widget for RoomDetailsSlidingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        // Close the pane when the close button is clicked, the back button is pressed, or the escape key is pressed
        let close_pane = match event {
            Event::Actions(actions) => self.button(id!(close_button)).clicked(actions),
            Event::MouseUp(mouse) => mouse.button == 3, // the "back" button on the mouse
            Event::KeyUp(key) => key.key_code == KeyCode::Escape,
            Event::BackPressed => true,
            _ => false,
        };
        if close_pane {
            self.animator_play(cx, id!(panel.hide));
            self.view(id!(bg_view)).set_visible(false);
            return;
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RoomDetailsSlidingPane {
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions, scope: &mut Scope) {
        let actions_tab_buttons = self.widget(id!(room_details_actions)).radio_button_set(ids!(
            info_button,
            members_button
        ));

        if let Some(index) = actions_tab_buttons.selected(cx, actions) {
            match index {
                0 => {
                    log!("Info button clicked");
                    self.redraw(cx);
                }
                1 => {
                    log!("Members button clicked");
                    self.redraw(cx);
                }
                _ => {}
            }
        }
    }
}

impl RoomDetailsSlidingPane {

    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        self.visible && self.animator_in_state(cx, id!(panel.show))
    }

    pub fn show(&mut self, cx: &mut Cx, pane_type: RoomDetailsSlidingPaneType) {
        self.visible = true;
        self.animator_play(cx, id!(panel.show));
        self.view(id!(bg_view)).set_visible(true);

        match pane_type {
            RoomDetailsSlidingPaneType::Info => {
                self.radio_button(id!(info_button)).select(cx, &mut Scope::default());
            }
            RoomDetailsSlidingPaneType::Members => {
                self.radio_button(id!(members_button)).select(cx, &mut Scope::default());
            }
        }

        self.redraw(cx);
    }
}

impl RoomDetailsSlidingPaneRef {

    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    pub fn show(&self, cx: &mut Cx, pane_type: RoomDetailsSlidingPaneType) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, pane_type);
    }
}
