use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;

    pub ColorTooltip = {{ColorTooltip}} {
        width: Fill,
        height: Fill,
        flow: Overlay
        align: {x: 0.0, y: 0.0}

        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.0)
            }
        }

        content: <View> {
            width: 250
            height: Fit
            visible: false,
            padding: 2.0

            tooltip_bg = <RoundedView> {
                width: Fill,
                height: Fit,
                padding: 7,

                draw_bg: {
                    color: #fff,
                    border_width: 1.5,
                    border_color: #fff,
                    radius: 3.0
                }

                tooltip_label = <Label> {
                    width: Fill,
                    height: Fit,
                    draw_text: {
                        text_style: <REGULAR_TEXT> {}
                        color: #fff
                        text_wrap: Word
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ColorTooltip {
    #[rust]
    opened: bool,

    #[live]
    #[find]
    content: View,

    #[rust(DrawList2d::new(cx))]
    draw_list: DrawList2d,

    #[redraw]
    #[area]
    #[live]
    draw_bg: DrawQuad,

    #[layout]
    layout: Layout,

    #[walk]
    walk: Walk,
}

impl Widget for ColorTooltip {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.opened {
            self.content.handle_event(cx, event, scope);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, _walk: Walk) -> DrawStep {
        // Start a new overlay,
        // which allows us to break through the boundaries of the parent component.
        self.draw_list.begin_overlay_reuse(cx);
        // Create an independent rendering pass to ensure correct size calculations.
        cx.begin_pass_sized_turtle(self.layout);

        self.draw_bg.begin(cx, self.walk, self.layout);
        if self.opened {
            self.content.draw_all(cx, scope);
        }
        self.draw_bg.end(cx);
        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);
        DrawStep::done()
    }

    fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.label(id!(tooltip_label)).set_text(cx, text);
    }
}

impl ColorTooltip {
    pub fn set_pos(&mut self, cx: &mut Cx, pos: DVec2) {
        self.apply_over(
            cx,
            live! {
                content: { margin: { left: (pos.x), top: (pos.y) } }
            },
        );
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.opened = true;
        self.content.visible = true;
        self.redraw(cx);
    }

    pub fn hide(&mut self, cx: &mut Cx) {
        self.opened = false;
        self.content.visible = false;
        self.redraw(cx);
    }

    pub fn show_with_options(&mut self, cx: &mut Cx, pos: DVec2, text: &str, color: Vec4) {
        self.set_text(cx, text);
        self.set_pos(cx, pos);
        self.set_bg_color(cx, color);
        self.show(cx);
    }

    fn set_bg_color(&mut self, cx: &mut Cx, color: Vec4) {
        self.apply_over(
            cx,
            live! {
                content: {
                    tooltip_bg = {
                        draw_bg: {
                            color: (color)
                        }
                    }
                }
            },
        );
    }
}

impl ColorTooltipRef {
    pub fn set_text(&self, cx: &mut Cx, text: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, text);
        }
    }

    pub fn set_pos(&self, cx: &mut Cx, pos: DVec2) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_pos(cx, pos);
        }
    }

    pub fn show(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx);
        }
    }

    pub fn hide(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide(cx);
        }
    }

    pub fn show_with_options(&self, cx: &mut Cx, pos: DVec2, text: &str, color: Vec4) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_with_options(cx, pos, text, color);
        }
    }
}
