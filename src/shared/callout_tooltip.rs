use makepad_widgets::*;
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    // A tooltip that appears when hovering over certain elements in the RoomScreen,
    // such as reactions or read receipts.
    pub CalloutTooltip = <Tooltip> {
        content: <View> {
            flow: Overlay
            width: Fit
            height: Fit

            rounded_view = <RoundedView> {
                width: Fill,
                height: Fit,

                padding: 10,

                draw_bg: {
                    color: #fff,
                    border_width: 1.0,
                    border_color: #D0D5DD,
                    radius: 2.,
                    instance background_color: (#3b444b),
                    // Height of isoceles triangle
                    instance callout_triangle_height: 7.5,
                    instance callout_offset: 15.0,
                    // callout angle in clockwise direction
                    // 0.0 is pointing up,
                    // 90.0 is pointing left, pointing right is not supported
                    // 180.0 is pointing down,
                    // 270.0 is pointing left
                    instance callout_angle: 0.0,
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        let rect_size = self.rect_size;
                        if self.callout_angle < 0.5 {
                            sdf.box(
                                // Minus 2.0 to overlap the triangle and rectangle
                                self.border_width,
                                (self.callout_triangle_height - 2.0) + self.border_width,
                                rect_size.x - (self.border_width * 2.0) ,
                                rect_size.y - (self.border_width * 2.0) - (self.callout_triangle_height - 2.0),
                                max(1.0, self.radius)
                            )
                            sdf.fill(self.background_color);
                            sdf.translate(self.callout_offset - 2.0 * self.callout_triangle_height, 1.0);
                            // Draw up-pointed arrow triangle
                            sdf.move_to(self.callout_triangle_height * 2.0, self.callout_triangle_height * 1.0);
                            sdf.line_to(0.0, self.callout_triangle_height * 1.0);
                            sdf.line_to(self.callout_triangle_height, 0.0);
                        } else if self.callout_angle < 90.5 || self.callout_angle > 180.5{ // By right, it should 
                            sdf.box(
                                // Minus 2.0 to overlap the triangle and rectangle
                                (self.callout_triangle_height - 2.0) + self.border_width,
                                0.0 + self.border_width,
                                rect_size.x - (self.border_width * 2.0) - (self.callout_triangle_height - 2.0),
                                rect_size.y - (self.border_width * 2.0),
                                max(1.0, self.radius)
                            )
                            sdf.fill(self.background_color);
                            sdf.translate(0.5, self.callout_offset);
                            // Draw left-pointed arrow triangle
                            sdf.move_to(self.callout_triangle_height, 0.0);
                            sdf.line_to(self.callout_triangle_height, self.callout_triangle_height * 2.0);
                            sdf.line_to(0.5, self.callout_triangle_height);
                        } else if self.callout_angle < 180.5 {
                            sdf.box(
                                // Minus 2.0 to overlap the triangle and rectangle
                                self.border_width,
                                self.border_width,
                                rect_size.x - (self.border_width * 2.0) ,
                                rect_size.y - (self.border_width * 2.0) - (self.callout_triangle_height - 2.0),
                                max(1.0, self.radius)
                            )
                            sdf.fill(self.background_color);
                            sdf.translate(self.callout_offset - self.callout_triangle_height, rect_size.y - 2.0);
                            // Draw down-pointed arrow triangle
                            sdf.move_to(self.callout_triangle_height * 2.0, - self.callout_triangle_height * 1.0);
                            sdf.line_to(self.callout_triangle_height, -0.5);
                            sdf.line_to(0.0, 0.0 - self.callout_triangle_height * 1.0);
                        }

                        sdf.close_path();

                        sdf.fill((self.background_color));
                        return sdf.result;
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

}

pub const TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM: f64 = 80.0;

/// Calculates the position and styling attributes for a tooltip relative to a widget, 
/// ensuring the tooltip stays within the visible window area.
///
/// This function determines the optimal position for the tooltip by checking if it's 
/// too close to the right or bottom edge of the window, and adjusts its placement 
/// accordingly. It also sets the offset and angle for the callout triangle based on 
/// these conditions.
///
/// # Arguments
///
/// * `widget_rect` - The rectangle representing the widget's position and size.
/// * `window_size` - The dimensions of the window in which the widget resides.
/// * `tooltip_width` - The desired width of the tooltip.
///
/// # Returns
///
/// A vector of `LiveNode` vectors representing the tooltip's position, size, and 
/// styling attributes to be applied.
pub fn position_helper(widget_rect: Rect, window_size: DVec2, tooltip_width: f64) -> Vec<Vec<LiveNode>> {    
    let mut too_close_to_right = false;
    let mut too_close_to_bottom = false;
    
    if (widget_rect.pos.x + widget_rect.size.x) + tooltip_width > window_size.x {
        too_close_to_right = true;
    }
    if (widget_rect.pos.y + widget_rect.size.y) + TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM > window_size.y {
        too_close_to_bottom = true;
    }
    let mut pos =  if too_close_to_right {
        DVec2 {
            x: widget_rect.pos.x + (widget_rect.size.x - tooltip_width),
            y: widget_rect.pos.y + widget_rect.size.y
        }
    } else {
        DVec2 {
            x: widget_rect.pos.x + widget_rect.size.x,
            y: widget_rect.pos.y - 5.0
        }
    };
    if too_close_to_bottom && !too_close_to_right {
        pos.x = widget_rect.pos.x + (widget_rect.size.x - 10.0) / 2.0;
        pos.y = widget_rect.pos.y - TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM + 10.0;
    }
    let callout_offset = if too_close_to_right {
        tooltip_width - (widget_rect.size.x - 10.0) / 2.0
    } else {
        10.0
    };
    let callout_angle = match (too_close_to_right, too_close_to_bottom) {
        (true, true) => 0.0, //point up
        (true, false) => 90.0, // it is still pointing left, as point right is not supported
        (false, true) => 180.0, //point down
        (false, false) => 270.0 //point left
    };
    let mut to_apply = vec![live!(
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
                }
            }
        }
    ).to_vec()];
    if too_close_to_bottom {
        to_apply.push(live!(
            content: {
                height: (TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM),
                //width: (tooltip_width + 50.0), // Make too close to bottom tooltip wider
                //width: Fit
                width: Fill
                rounded_view = {
                    height: (TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM - 10.0),
                }
            }
        ).to_vec());
    }
    to_apply
}
pub fn position_helper2(widget_rect: Rect, window_size: DVec2, tooltip_width: f64) -> (DVec2, f64, f64, bool) {
    let mut too_close_to_right = false;
    let mut too_close_to_bottom = false;
    const TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM: f64 = 80.0;
    if (widget_rect.pos.x + widget_rect.size.x) + tooltip_width > window_size.x {
        too_close_to_right = true;
    }
    if (widget_rect.pos.y + widget_rect.size.y) + TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM > window_size.y {
        too_close_to_bottom = true;
    }
    let mut pos =  if too_close_to_right {
        DVec2 {
            x: widget_rect.pos.x + (widget_rect.size.x - tooltip_width),
            y: widget_rect.pos.y + widget_rect.size.y
        }
    } else {
        DVec2 {
            x: widget_rect.pos.x + widget_rect.size.x,
            y: widget_rect.pos.y - 5.0
        }
    };
    if too_close_to_bottom && !too_close_to_right {
        pos.x = widget_rect.pos.x + (widget_rect.size.x - 10.0) / 2.0;
        pos.y = widget_rect.pos.y - TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM + 10.0;
    }
    let callout_offset = if too_close_to_right {
        tooltip_width - (widget_rect.size.x - 10.0) / 2.0
    } else {
        10.0
    };
    let callout_angle = match (too_close_to_right, too_close_to_bottom) {
        (true, true) => 0.0, //point up
        (true, false) => 0.0, // point up
        (false, true) => 180.0, //point down
        (false, false) => 270.0 //point left
    };
    (pos, callout_offset, callout_angle, too_close_to_bottom)
}


/// Applies styles to the given tooltip reference for it to be drawn.
///
/// # Arguments
///
/// * `tooltip_ref` - The reference to the tooltip widget.
/// * `cx` - The drawing context.
/// * `tooltip_pos` - The position at which to draw the tooltip.
/// * `tooltip_width` - The width of the tooltip.
/// * `callout_offset` - The offset of the callout triangle relative to the right corner of the tooltip.
/// * `callout_angle` - The angle of the callout triangle in clockwise direction. 0.0 is pointing up, 90.0 is pointing left, 180.0 is pointing down, 270.0 is pointing left.
/// * `too_close_to_bottom` - Whether the too_close_to_bottom flag is set.
pub fn draw_helper(tooltip_ref: &mut TooltipRef, cx: &mut Cx, tooltip_pos: DVec2, tooltip_width: f64, callout_offset: f64, callout_angle: f64, too_close_to_bottom: bool) {
    tooltip_ref.apply_over(cx, live!(
        content: {
            margin: { left: (tooltip_pos.x), top: (tooltip_pos.y)},
            width: (tooltip_width),
            height: Fit,
            rounded_view = {
                height: Fit,
                draw_bg: {
                    callout_offset: (callout_offset)
                    // callout angle in clockwise direction
                    callout_angle: (callout_angle)
                }
            }
        }
    ));
    if too_close_to_bottom {
        tooltip_ref.apply_over(cx, live!(
            content: {
                height: (TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM),
                width: Fill
                rounded_view = {
                    height: (TOOLTIP_HEIGHT_FOR_TOO_CLOSE_BOTTOM - 10.0),
                }
            }
        ));
    }
}