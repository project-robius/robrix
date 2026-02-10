use makepad_widgets::*;
use crate::shared::callout_tooltip::{TooltipAction, CalloutTooltipOptions, TooltipPosition};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    COLOR_BRAND = #x5
    COLOR_BRAND_HOVER = #x3
    COLOR_META_TEXT = #xaaa

    pub IconButton = <Button> {
        draw_text: {
            instance hover: 0.0
            instance down: 0.0
            text_style: {
                font_size: 11.0
            }
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        (COLOR_META_TEXT),
                        (COLOR_BRAND),
                        self.hover
                    ),
                    (COLOR_BRAND_HOVER),
                    self.down
                )
            }
        }
        draw_icon: {
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        (COLOR_META),
                        (COLOR_BRAND),
                        self.hover
                    ),
                    (COLOR_BRAND_HOVER),
                    self.down
                )
            }
        }
        icon_walk: {width: 7.5, height: Fit, margin: {left: 5.0}}
        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }
        padding: 9.0
        text: ""
    }


    // Customized button widget, based on the RoundedView shaders with some modifications
    // which is a better fit with our application UI design
    pub RobrixIconButton = {{RobrixIconButton}} {
        width: Fit,
        height: Fit,
        spacing: 10,
        padding: 10,
        align: {x: 0, y: 0.5}

        draw_bg: {
            instance color: (COLOR_PRIMARY)
            // We set a mid-gray hover color, which gets mixed with the bg color itself
            // in order to create a "lightening" effect upon hover.
            instance color_hover: #A
            instance border_size: 0.0
            instance border_color: #D0D5DD
            instance border_radius: 4.0

            fn get_color(self) -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.box(
                    self.border_size,
                    self.border_size,
                    self.rect_size.x - (self.border_size * 2.0),
                    self.rect_size.y - (self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.border_color, self.border_size)
                }
                return sdf.result;
            }
        }

        draw_icon: {
            instance color: #000
            instance color_hover: #000
            fn get_color(self) -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }
        }
        icon_walk: {width: 16, height: 16}

        draw_text: {
            text_style: <REGULAR_TEXT>{font_size: 10},
            color: #000
            fn get_color(self) -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }
        }
        text: ""
        tooltip: ""
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RobrixIconButton {
    #[deref]
    button: Button,

    #[live]
    tooltip: String,
}

impl Widget for RobrixIconButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        match event.hits(cx, self.button.area()) {
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
                if !self.tooltip.is_empty() {
                    let widget_rect = self.button.area().rect(cx);
                    let position = if widget_rect.pos.y < 50.0 {
                        TooltipPosition::Bottom
                    } else {
                        TooltipPosition::Top
                    };

                    cx.widget_action(
                        self.button.widget_uid(),
                        &scope.path,
                        TooltipAction::HoverIn {
                            text: self.tooltip.clone(),
                            widget_rect,
                            options: CalloutTooltipOptions {
                                position,
                                ..Default::default()
                            },
                        },
                    );
                }
            }
            Hit::FingerHoverOut(_) => {
                cx.set_cursor(MouseCursor::Arrow);
                 if !self.tooltip.is_empty() {
                    cx.widget_action(
                        self.button.widget_uid(),
                        &scope.path,
                        TooltipAction::HoverOut,
                    );
                }
            }
            _ => ()
        }
        self.button.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.button.draw_walk(cx, scope, walk)
    }
}
