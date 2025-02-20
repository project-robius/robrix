//! A tooltip widget that a callout pointing towards the referenced widget.
//!
//! By default, the tooltip has a black background color

use makepad_widgets::*;
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    // A tooltip that appears when hovering over target's area
    pub CalloutTooltipInner = <Tooltip> {
        content: <View> {
            flow: Overlay,
            width: Fit,
            height: Fit,
            margin: { left: (0.0), top: (0.0) },
            rounded_view = <RoundedView> {
                width: Fit,
                height: Fit,

                padding: 15,

                draw_bg: {
                    color: #fff,
                    border_width: 7.5,
                    border_color: #D0D5DD,
                    radius: 2.,
                    instance background_color: #3b444b,
                    // Absolute position of top left corner of the tooltip
                    instance tooltip_pos: vec2(0.0, 0.0),
                    // Absolute position of the moused over widget
                    instance target_pos: vec2(0.0, 0.0),
                    // Size of the moused over widget
                    instance target_size: vec2(0.0, 0.0),
                    instance expected_dimension_x: 0.0,
                    instance triangle_height: 7.5,
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        let rect_size = self.rect_size;
                        // If there is no expected_dimension_x, it means the tooltip size is not calculated yet, do not draw anything
                        if self.expected_dimension_x == 0.0 {
                            return sdf.result;
                        }
                        // Draw rounded box
                        sdf.box(
                            self.border_width,
                            self.border_width,
                            rect_size.x - (self.border_width * 2.0),
                            rect_size.y - (self.border_width * 2.0),
                            max(1.0, self.radius)
                        )
                        sdf.fill(self.background_color);
                        let triangle_height = self.triangle_height;
                        let diff_x = self.target_pos.x + self.target_size.x / 2.0 - self.tooltip_pos.x - triangle_height;
                        let diff_y = self.target_pos.y + self.target_size.y / 2.0 - self.tooltip_pos.y - triangle_height;
                        // Quadrant angle to define the direction from target's center to the tooltip's center
                        // ___315___|___45_______
                        //    225   |   135
                        // Callout only point upwards or downwards, towards left and right are omitted.
                        let mut angle = 0.0;
                        if diff_x >= 0.0 && diff_y <= 0.0 {
                            angle = 45.0;
                        } else if diff_x >= 0.0 && diff_y > 0.0 {
                            angle = 135.0;
                        } else if diff_x < 0.0 && diff_y <= 0.0 {
                            angle = 225.0;
                        } else {
                            angle = 315.0;
                        }
                        let mut vertex1 = vec2(0.0, 0.0);
                        let mut vertex2 = vec2(0.0, 0.0);
                        let mut vertex3 = vec2(0.0, 0.0);
                        if angle == 45.0 || angle == 315.0 {
                            // Point upwards
                            vertex1 = vec2(max(self.border_width + 2.0, diff_x), self.border_width + 2.0); // + 2.0 to overlap the triangle
                            vertex2 = vec2(vertex1.x + triangle_height, vertex1.y - triangle_height);
                            vertex3 = vec2(vertex1.x + triangle_height * 2.0, vertex1.y);
                        } else {
                            // Point downwards
                            vertex1 = vec2(max(self.border_width + 2.0, diff_x) + triangle_height * 2.0 , rect_size.y - triangle_height - 2.0); // +/- 2.0 to overlap the triangle
                            vertex2 = vec2(vertex1.x - triangle_height, vertex1.y + triangle_height);
                            vertex3 = vec2(vertex1.x - triangle_height * 2.0, vertex1.y);
                        }
                        sdf.move_to(vertex1.x, vertex1.y);
                        sdf.line_to(vertex2.x, vertex2.y);
                        sdf.line_to(vertex3.x, vertex3.y);
                        sdf.close_path();
                        sdf.fill(self.background_color);
                        return sdf.result;
                    }

                }

                tooltip_label = <Label> {
                    width: Fit,
                    height: Fit,
                    draw_text: {
                        text_style: <THEME_FONT_REGULAR>{font_size: 9},
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

#[derive(Debug)]
/// A struct that holds the options for a callout tooltip
pub struct CalloutTooltipOptions {
    /// The screen_size of the widget that the tooltip is pointing to
    pub widget_rect: Rect,
    /// The background color of the tooltip
    pub color: Option<Vec4>,
}

/// A tooltip widget that a callout pointing towards the referenced widget.
#[derive(Live, LiveHook, Widget)]
pub struct CalloutTooltip {
    #[deref]
    view: View,
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
    /// The tooltip comes with a callout pointing to its target.
    ///
    /// By default, the tooltip will be displayed to the widget's right.
    ///
    /// If the widget is too close to right of the window, the tooltip is positioned to the
    /// bottom of the widget, pointed at the center. If it is too close to bottom, the
    /// tooltip is positioned above the widget.
    pub fn show_with_options(&mut self, cx: &mut Cx, text: &str, options: CalloutTooltipOptions) {
        let tooltip = self.view.tooltip(id!(tooltip));

        let pos = options.widget_rect.pos;
        if let Some(mut tooltip) = tooltip.borrow_mut() {
            // When there is line break in the text label, the label's width follows the length of the last line.
            // When the previous lines is longer than the last line, text will be cut off.
            // Hence we need to lengthen the last line to be the same length as the longest line.
            tooltip.set_text(cx, &pad_last_line(text));
        };
        // Expected_dimension size is 0.0 when mouse first moved in and the tooltip may be cut off.
        // When the mouse is hover over, the expected_dimension is not 0.0 and will be used to re-position the tooltip to avoid cut off.
        let expected_dimension = tooltip.view(id!(rounded_view)).area().rect(cx).size;

        let screen_size = tooltip.area().rect(cx).size;
        let mut tooltip_pos = DVec2 {
            x: min(pos.x, screen_size.x - expected_dimension.x),
            y: min(
                pos.y + options.widget_rect.size.y,
                screen_size.y - expected_dimension.y,
            ),
        };
        let mut fixed_width = false;
        if tooltip_pos.y == screen_size.y - expected_dimension.y {
            // If the tooltip is too close to the bottom, position it above the widget
            tooltip_pos.y =
                options.widget_rect.pos.y - max(expected_dimension.y, options.widget_rect.size.y);
        }
        // When tooltip_pos.x is less than 0.0, reposition it
        // If the expected_dimension's width is already the screen_size's width, fix the width of the tooltip.
        if tooltip_pos.x == screen_size.x - expected_dimension.x && tooltip_pos.x < 0.0
            || expected_dimension.x == screen_size.x
        {
            tooltip_pos.x = 0.0;
            fixed_width = true;
        }
        let target = vec2(
            options.widget_rect.pos.x as f32,
            options.widget_rect.pos.y as f32,
        );
        let tooltip_pos = vec2(tooltip_pos.x as f32, tooltip_pos.y as f32);
        let target_size = vec2(
            options.widget_rect.size.x as f32,
            options.widget_rect.size.y as f32,
        );
        // Default dark gray color
        let color = options
            .color
            .unwrap_or_else(|| vec4(0.26, 0.30, 0.333, 1.0));
        if fixed_width {
            tooltip.apply_over(
                cx,
                live!(
                content: {
                    margin: { left: (tooltip_pos.x), top: (tooltip_pos.y) },
                    rounded_view = {
                        height: Fit,
                        draw_bg: {
                            background_color: (color),
                            tooltip_pos: (tooltip_pos),
                            target_pos: (target),
                            target_size: (target_size),
                            expected_dimension_x: (expected_dimension.x)
                        }
                        tooltip_label = {
                            width: (screen_size.x - 15.0 * 2.0), // minus rounded_view's padding
                        }
                    }
                }),
            );
        } else {
            // If width of the tooltip is not fixed, the tooltip label's width is set back to Fit so that
            // the it will wrap the tooltip text properly
            tooltip.apply_over(
                cx,
                live!(
                content: {
                    margin: { left: (tooltip_pos.x), top: (tooltip_pos.y) },
                    rounded_view = {
                        height: Fit,
                        draw_bg: {
                            background_color: (color),
                            tooltip_pos: (tooltip_pos),
                            target_pos: (target),
                            target_size: (target_size),
                            expected_dimension_x: (expected_dimension.x)
                        }
                        tooltip_label = {
                            width: Fit,
                        }
                    }
                }),
            );
        }
        tooltip.show(cx);
    }
    /// Shows the tooltip.
    pub fn show(&mut self, cx: &mut Cx) {
        self.view.tooltip(id!(tooltip)).show(cx);
    }
    /// Hide the tooltip.
    pub fn hide(&mut self, cx: &mut Cx) {
        self.view.tooltip(id!(tooltip)).hide(cx);
    }
}

impl CalloutTooltipRef {
    /// See [`CalloutTooltip::show_with_options()`].
    pub fn show_with_options(&mut self, cx: &mut Cx, text: &str, options: CalloutTooltipOptions) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_with_options(cx, text, options);
        }
    }
    /// See [`CalloutTooltip::show()`].
    pub fn show(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx);
        }
    }
    /// See [`CalloutTooltip::hide()`].
    pub fn hide(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide(cx);
        }
    }
}

/// An action emitted to show or hide the `tooltip`.
#[derive(Clone, Debug, DefaultNone)]
pub enum TooltipAction {
    HoverIn {
        widget_rect: Rect,
        /// Color of the background
        color: Option<Vec4>,
        /// Tooltip text
        text: String,
    },
    HoverOut,
    None,
}

/// Takes a string and lengthens the last line of the string to be the same
/// length as the longest line in the string.
///
/// This is useful for creating tooltips that line up with the text above
/// them.
fn pad_last_line(text: &str) -> String {
    let lines = text.split('\n');
    let longest_line = lines.clone().map(|s| s.len()).max().unwrap_or(0);
    let lines_len = lines.clone().count();

    let mut full_text = String::with_capacity(text.len() + longest_line + 4);
    for (i, line) in lines.enumerate() {
        full_text.push_str(line);
        if i < lines_len - 1 {
            full_text.push('\n');
        } else {
            // Plus 4 is added to add more width to the last line otherwise the first line is still being cut off
            full_text.push_str(&" ".repeat(longest_line - line.len() + 4));
        }
    }
    full_text
}
