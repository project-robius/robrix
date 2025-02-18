//! A tooltip widget that a callout pointing towards the referenced widget.
//!
//! By default, the tooltip has a black background color

use makepad_widgets::*;
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    // A tooltip that appears when hovering over certain elements in the RoomScreen,
    // such as reactions or read receipts.
    pub CalloutTooltipInner = <Tooltip> {
        content: <View> {
            flow: Overlay
            width: Fit
            height: Fit

            rounded_view = <RoundedView> {
                width: Fill,
                height: Fit,

                padding: 15,

                draw_bg: {
                    color: #fff,
                    border_width: 7.5,
                    border_color: #D0D5DD,
                    radius: 2.,
                    instance background_color: (#3b444b),
                    // Height of isoceles triangle
                    // instance callout_triangle_height: 7.5,
                    // instance callout_offset: 15.0,
                    // callout angle in clockwise direction
                    // 0.0 is pointing up,
                    // 90.0 is pointing left, pointing right is not supported
                    // 180.0 is pointing down,
                    // 270.0 is pointing left
                    //instance target_x: 33.0,
                    instance target_x: 80.0,
                    instance target_y: 40.0,
                    instance target_width: 40.0,
                    instance target_height: 40.0,
                    instance rect_top_left_x: 33.0,
                    instance rect_top_left_y: 71.0,
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        let rect_size = self.rect_size;
                        sdf.box(
                            // Minus 2.0 to overlap the triangle and rectangle
                            self.border_width,
                            self.border_width,
                            rect_size.x - (self.border_width * 2.0),
                            rect_size.y - (self.border_width * 2.0),
                            max(1.0, self.radius)
                        )
                        sdf.fill(self.background_color);
                        let diff_x = self.target_x - self.rect_top_left_x;
                        let diff_y = self.target_y - self.rect_top_left_y;
                        let mut angle = 0.0;
                        if diff_x >= 0.0 && diff_y <= 0.0 {
                            angle = 45.0;
                        } else if diff_x >= 0.0 && diff_y > 0.0 {
                            angle = 135.0;
                        }  else if diff_x <= 0.0 && diff_y <= 0.0 {
                            angle = 225.0;
                        }   else {
                            angle = 315.0;
                        }
                        let triangle_height = 7.5;
                        let mut vertex1 = vec2(0.0, 0.0);
                        let mut vertex2 = vec2(0.0, 0.0);
                        let mut vertex3 = vec2(0.0, 0.0);
                        let diff_x_from_center = self.target_x + self.target_width / 2.0 - self.rect_top_left_x - triangle_height;
                        if angle == 45.0 || angle == 225.0 {
                            vertex1 = vec2(max(self.border_width + 2.0, diff_x_from_center), self.border_width + 2.0);
                            vertex2 = vec2(vertex1.x + triangle_height, vertex1.y - triangle_height);
                            vertex3 = vec2(vertex1.x + triangle_height * 2.0, vertex1.y);
                        } else {
                            vertex1 = vec2(max(self.border_width + 2.0, diff_x_from_center) + triangle_height * 2.0 , rect_size.y - triangle_height - 2.0);
                            vertex2 = vec2(vertex1.x - triangle_height, vertex1.y + triangle_height);
                            vertex3 = vec2(vertex1.x - triangle_height * 2.0, vertex1.y );
                        }
                        
                        sdf.move_to(vertex1.x, vertex1.y);
                        sdf.line_to(vertex2.x, vertex2.y);
                        sdf.line_to(vertex3.x, vertex3.y);
                        sdf.close_path();
                        sdf.fill(self.background_color);
                        // sdf.move_to(self.border_width, self.border_width);
                        // sdf.line_to(self.diff_x, self.diff_y);
                        // sdf.line_to(self.border_width * 2.0, self.border_width * 2.0);
                        // sdf.close_path();
                        // sdf.fill(self.background_color);
                        return sdf.result;
                        // let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        // let rect_size = self.rect_size;
                        // if self.callout_angle < 0.5 {
                        //     sdf.box(
                        //         // Minus 2.0 to overlap the triangle and rectangle
                        //         self.border_width,
                        //         (self.callout_triangle_height - 2.0) + self.border_width,
                        //         rect_size.x - (self.border_width * 2.0) ,
                        //         rect_size.y - (self.border_width * 2.0) - (self.callout_triangle_height - 2.0),
                        //         max(1.0, self.radius)
                        //     )
                        //     sdf.fill(self.background_color);
                        //     sdf.translate(self.callout_offset - 2.0 * self.callout_triangle_height, 1.0);
                        //     // Draw up-pointed arrow triangle
                        //     sdf.move_to(self.callout_triangle_height * 2.0, self.callout_triangle_height * 1.0);
                        //     sdf.line_to(0.0, self.callout_triangle_height * 1.0);
                        //     sdf.line_to(self.callout_triangle_height, 0.0);
                        // } else if self.callout_angle < 90.5 || self.callout_angle > 180.5 {
                        //     sdf.box(
                        //         // Minus 2.0 to overlap the triangle and rectangle
                        //         (self.callout_triangle_height - 2.0) + self.border_width,
                        //         0.0 + self.border_width,
                        //         rect_size.x - (self.border_width * 2.0) - (self.callout_triangle_height - 2.0),
                        //         rect_size.y - (self.border_width * 2.0),
                        //         max(1.0, self.radius)
                        //     )
                        //     sdf.fill(self.background_color);
                        //     sdf.translate(0.5, self.callout_offset);
                        //     // Draw left-pointed arrow triangle
                        //     sdf.move_to(self.callout_triangle_height, 0.0);
                        //     sdf.line_to(self.callout_triangle_height, self.callout_triangle_height * 2.0);
                        //     sdf.line_to(0.5, self.callout_triangle_height);
                        // } else if self.callout_angle < 180.5 {
                        //     sdf.box(
                        //         // Minus 2.0 to overlap the triangle and rectangle
                        //         self.border_width,
                        //         self.border_width,
                        //         rect_size.x - (self.border_width * 2.0) ,
                        //         rect_size.y - (self.border_width * 2.0) - (self.callout_triangle_height - 2.0),
                        //         max(1.0, self.radius)
                        //     )
                        //     sdf.fill(self.background_color);
                        //     sdf.translate(self.callout_offset - self.callout_triangle_height, rect_size.y - 2.0);
                        //     // Draw down-pointed arrow triangle
                        //     sdf.move_to(self.callout_triangle_height * 2.0, - self.callout_triangle_height * 1.0);
                        //     sdf.line_to(self.callout_triangle_height, -0.5);
                        //     sdf.line_to(0.0, 0.0 - self.callout_triangle_height * 1.0);
                        // }

                        // sdf.close_path();

                        // sdf.fill((self.background_color));
                        // return sdf.result;
                    }

                }

                tooltip_label = <Label> {
                    width: Fill,
                    height: Fit,
                    draw_text: {
                        text_style: <THEME_FONT_REGULAR>{font_size: 9},
                        //text_wrap: Word,
                        text_wrap: Line,
                        color: (COLOR_PRIMARY)
                    }
                }
            }
        }
    }
    pub CalloutTooltip = {{CalloutTooltip}} {
        tooltip = <CalloutTooltipInner> {

        }
    }
}
pub const TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM: f64 = 80.0;

#[derive(Debug)]
/// A struct that holds the options for a callout tooltip
pub struct CalloutTooltipOptions {
    /// The rect of the widget that the tooltip is pointing to
    pub parent_rect: Rect,
    /// The rect of the widget that the tooltip is pointing to
    pub widget_rect: Rect,
    /// Tooltip width
    pub tooltip_width: f64,
    /// The background color of the tooltip
    pub color: Option<Vec4>,
}

/// A tooltip widget that a callout pointing towards the referenced widget.
#[derive(Live, LiveHook, Widget)]
pub struct CalloutTooltip {
    #[deref]
    view: View,
    #[rust] expected_dimensions: Option<DVec2>,
    #[redraw]
    #[rust] area: Area
}

impl Widget for CalloutTooltip {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        //self.widget_match_event(cx, event, scope);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl CalloutTooltip {
    /// Shows a tooltip with the given text and options.
    ///
    /// The tooltip comes with a callout pointing to it's target.
    ///
    /// By default, the tooltip will be displayed to the widget's right.
    ///
    /// If the widget is too close to right of the window, the tooltip is positioned to the
    /// bottom of the widget, pointed at the center. If it is too close to bottom, the
    /// tooltip is positioned above the widget.
    pub fn show_with_options(&mut self, cx: &mut Cx, text: &str, options: CalloutTooltipOptions) {
        let mut too_close_to_right = false;
        let mut too_close_to_bottom = false;
        let mut too_close_to_left = false;
        let window_size = cx.display_context.screen_size;
        let CalloutTooltipOptions {
            parent_rect,
            widget_rect,
            tooltip_width,
            color,
        } = options;
        if (widget_rect.pos.x + widget_rect.size.x) + tooltip_width > window_size.x {
            too_close_to_right = true;
        }
        if (widget_rect.pos.y + widget_rect.size.y) + TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM
            > window_size.y
        {
            too_close_to_bottom = true;
        }
        let mut pos = if too_close_to_right {
            DVec2 {
                x: if widget_rect.pos.x + (widget_rect.size.x - tooltip_width) < 0.0 {
                    too_close_to_left = true;
                    0.0
                } else {
                    widget_rect.pos.x + (widget_rect.size.x - tooltip_width)
                },
                y: widget_rect.pos.y + widget_rect.size.y,
            }
        } else {
            DVec2 {
                x: widget_rect.pos.x + widget_rect.size.x,
                y: widget_rect.pos.y - 5.0,
            }
        };
        if too_close_to_bottom && !too_close_to_right {
            pos.x = widget_rect.pos.x + widget_rect.size.x / 2.0 - 10.0;
            pos.y = widget_rect.pos.y - TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM + 10.0;
        }
        if too_close_to_bottom && too_close_to_right {
            pos.x = widget_rect.pos.x + (widget_rect.size.x / 2.0 - tooltip_width);
            pos.y = widget_rect.pos.y - TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM;
        }
        let callout_offset = if too_close_to_left {
            widget_rect.pos.x + widget_rect.size.x / 2.0
        } else if too_close_to_right {
            std::cmp::min((tooltip_width - (widget_rect.size.x - 10.0) / 2.0) as i64, (tooltip_width - 15.0) as i64) as f64
        } else {
            10.0
        };
        let callout_angle = match (too_close_to_right, too_close_to_bottom) {
            (true, true) => 180.0,   //point down
            (true, false) => 0.0,    // point up
            (false, true) => 180.0,  //point down
            (false, false) => 270.0, //point left
        };
        let tooltip = self.view.tooltip(id!(tooltip));
        tooltip.apply_over(
            cx,
            live!(
                content: {
                    margin: { left: (pos.x), top: (pos.y)},
                    width: (tooltip_width),
                    height: Fit,
                    rounded_view = {
                        height: Fit,
                        draw_bg: {
                            callout_offset: (callout_offset)
                            // callout angle in clockwise direction
                            callout_angle: (callout_angle)
                            background_color: (
                                if let Some(color) = color {
                                    color
                                } else {
                                    //#3b444b
                                    vec4(0.26, 0.30, 0.333, 1.0)
                                }
                            )
                        }
                        padding: {
                            left: (
                                if callout_angle == 270.0 {
                                    10.0 + 7.5 // 7.5 is the height of the isoceles triangle
                                } else {
                                    10.0
                                }
                            ), bottom: (
                                if callout_angle == 180.0 {
                                    10.0 + 7.5 // 7.5 is the height of the isoceles triangle
                                } else {
                                    10.0
                                }
                            )
                        }
                    }
                }
            ),
        );

        if let Some(mut tooltip) = tooltip.borrow_mut() {
            tooltip.set_text(cx, text);
        };

        let area: Rect = tooltip.view(id!(rounded_view)).area().rect(cx);
        let area: Rect = tooltip.view(id!(tooltip_label)).area().rect(cx);
        if too_close_to_bottom && area.size.y + 10.0 > TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM {
            tooltip.apply_over(
                cx,
                live!(
                    content: {
                        margin: { top: (widget_rect.pos.y - area.size.y )},
                    }
                ),
            );
        }
        tooltip.show(cx);
    }

    pub fn show_with_options2(&mut self, cx: &mut Cx, text: &str, options: CalloutTooltipOptions) {
        let tooltip = self.view.tooltip(id!(tooltip));
        let pos = options.widget_rect.pos;
        tooltip.apply_over(
            cx,
            live!(
                content: {
                    margin: { left: (pos.x), top: (pos.y)},
                    width: (options.tooltip_width),
                    rounded_view = {
                        height: Fit,
                        draw_bg: {
                            background_color: (vec4(0.26, 0.30, 0.333, 1.0))
                        }
                    }
                })
        );
        if let Some(mut tooltip) = tooltip.borrow_mut() {
            tooltip.set_text(cx, text);
        };
        //let area = self.area.rect(cx);
        let area: Rect = tooltip.view(id!(rounded_view)).area().rect(cx);
        let mut expected_dimensions = area.size;
        expected_dimensions.x = options.tooltip_width;
        if expected_dimensions.y == 0.0 {
            println!("expected_dimensions.y is zero");
        } else {
            println!("expected_dimensions.y {:?}", expected_dimensions.y);
        }
        // if expected_dimensions.y != 0.0 {
        //     self.expected_dimensions = Some(expected_dimensions);
        // }
        //let rect = cx.display_context.screen_size;
        let rect = options.parent_rect.size;
        // padding_y: 15
        // padding_y: border 7.5
        let mut pos_x = min(pos.x, rect.x - expected_dimensions.x);
        let mut pos_y = min(pos.y + options.widget_rect.size.y, rect.y - expected_dimensions.y);
        //if let Some(expected_dimensions) = expected_dimensions {
            if pos_y == rect.y - expected_dimensions.y {
                pos_y -=  (options.widget_rect.size.y + 15.0 * 2.0 + 7.5 * 2.0); //padding *2 + border_width * 2
            }
        //}
        
        let target = DVec2{
            x: options.widget_rect.pos.x,
            y: options.widget_rect.pos.y
        };
        let target_width = options.widget_rect.size.x;
        let target_height = options.widget_rect.size.y;
        let rect_top_left = DVec2{
            x: pos_x,
            y: pos_y
        };
        // println!("target {:?} rect_top_left {:?} pos.y + options.widget_rect.size.y {:?} rect.y - expected_dimensions.y {:?}", target, rect_top_left, pos.y + options.widget_rect.size.y, rect.y - expected_dimensions.y);
        // println!("rect {:?} expected_dimensions {:?} options {:?}",rect, self.expected_dimensions, options);
        tooltip.apply_over(
            cx,
            live!(
                content: { margin: { left: (pos_x), top: (pos_y) }
                rounded_view = {
                    height: Fit,
                    draw_bg: {
                        rect_top_left_x: (rect_top_left.x),
                        rect_top_left_y: (rect_top_left.y),
                        target_x: (target.x),
                        target_y: (target.y),
                        target_height: (target_height),
                        target_width: (target_width)
                    }
                }
            }
            ),
        );
        tooltip.show(cx);
    }
    /// Shows the tooltip.
    pub fn show(&self, cx: &mut Cx) {
        self.view.tooltip(id!(tooltip)).show(cx);
    }
    /// Hide the tooltip.
    pub fn hide(&self, cx: &mut Cx) {
        self.view.tooltip(id!(tooltip)).hide(cx);
    }
}

impl CalloutTooltipRef {
    /// See [`CalloutTooltip::show_with_options()`].
    pub fn show_with_options(&mut self, cx: &mut Cx, text: &str, options: CalloutTooltipOptions) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_with_options2(cx, text, options);
        }
    }
    /// See [`CalloutTooltip::show()`].
    pub fn show(&self, cx: &mut Cx) {
        if let Some(inner) = self.borrow_mut() {
            inner.show(cx);
        }
    }
    /// See [`CalloutTooltip::hide()`].
    pub fn hide(&self, cx: &mut Cx) {
        if let Some(inner) = self.borrow_mut() {
            inner.hide(cx);
        }
    }
}

/// An action emitted to show or hide the `tooltip`.
#[derive(Clone, Debug, DefaultNone)]
pub enum TooltipAction {
    HoverIn {
        widget_rect: Rect,
        tooltip_width: f64,
        /// Color of the background
        color: Option<Vec4>,
        /// Tooltip text
        text: String,
    },
    HoverOut,
    None,
}
