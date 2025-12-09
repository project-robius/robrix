//! A tooltip widget with a callout arrow/triangle that points at the referenced widget.
//!
//! By default, the tooltip has a black background color.

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

            rounded_view = <RoundedView> {
                width: Fit,
                height: Fit,
                padding: 15,

                draw_bg: {
                    color: #fff,
                    border_color: #D0D5DD,
                    border_radius: 2.,
                    instance background_color: #3b444b,
                    // Absolute position of top left corner of the tooltip
                    instance tooltip_pos: vec2(0.0, 0.0),
                    // Absolute position of the moused over widget
                    instance target_pos: vec2(0.0, 0.0),
                    // Size of the moused over widget
                    instance target_size: vec2(0.0, 0.0),
                    // Expected Width of the the tooltip 
                    instance expected_dimension_x: 0.0,
                    // Determine height of the triangle in the callout pointer
                    instance triangle_height: 7.5,
                    // Determine angle of the triangle in the callout pointer in degrees
                    instance callout_position: 180.0,

                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        let rect_size = self.rect_size;
                        let triangle_height = self.triangle_height;
                        // If there is no expected_dimension_x, it means the tooltip size is not calculated yet, do not draw anything
                        if self.expected_dimension_x == 0.0 {
                            return sdf.result;
                        }
                        // Draw rounded box with border equals to triangle_height.
                        sdf.box(
                            triangle_height,
                            triangle_height,
                            rect_size.x - (triangle_height * 2.0),
                            rect_size.y - (triangle_height * 2.0),
                            max(1.0, self.border_radius)
                        )
                        sdf.fill(self.background_color);
               
                        let mut vertex1 = vec2(0.0, 0.0);
                        let mut vertex2 = vec2(0.0, 0.0);
                        let mut vertex3 = vec2(0.0, 0.0);
                        if self.callout_position == 0.0 {
                            // Point upwards
                            // + 2.0 to overlap the triangle
                            let diff_x = self.target_pos.x + self.target_size.x / 2.0 - self.tooltip_pos.x - triangle_height;
                            vertex1 = vec2(
                                min(max(triangle_height + 2.0, diff_x), rect_size.x - triangle_height * 3.0 - 2.0),
                                triangle_height + 2.0
                            );
                            vertex2 = vec2(vertex1.x + triangle_height, vertex1.y - triangle_height);
                            vertex3 = vec2(vertex1.x + triangle_height * 2.0, vertex1.y);
                        } else if self.callout_position == 90.0 {
                            // Point rightwards  
                            // Triangle points to the right from the left edge of the tooltip
                            vertex1 = vec2(rect_size.x - 2.0, rect_size.y * 0.5);
                            vertex2 = vec2(vertex1.x - triangle_height, vertex1.y + triangle_height);
                            vertex3 = vec2(vertex1.x - triangle_height, vertex1.y - triangle_height);
                        } else if self.callout_position == 180.0 {
                            // Point downwards
                            // +/- 2.0 to overlap the triangle
                            let diff_x = self.target_pos.x + self.target_size.x / 2.0 - self.tooltip_pos.x + triangle_height;
                            vertex1 = vec2(
                                min(max(triangle_height * 3.0 + 2.0, diff_x), rect_size.x - triangle_height - 2.0),
                                rect_size.y - triangle_height - 2.0
                            );
                            vertex2 = vec2(vertex1.x - triangle_height, vertex1.y + triangle_height);
                            vertex3 = vec2(vertex1.x - triangle_height * 2.0, vertex1.y);
                        } else {
                            // Point leftwards
                            // Triangle points to the left from the right edge of the tooltip
                            vertex1 = vec2(2.0, rect_size.y * 0.5);
                            vertex2 = vec2(vertex1.x + triangle_height, vertex1.y - triangle_height);
                            vertex3 = vec2(vertex1.x + triangle_height, vertex1.y + triangle_height);
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
                        color: (COLOR_PRIMARY),
                    }
                }
            }
        }
    }

    pub CalloutTooltip = {{CalloutTooltip}} {
        tooltip = <CalloutTooltipInner> { }
    }
}

/// Options that affect how a CalloutTooltip is displayed.
///
/// You don't have to specify all values, they each have a sensible default.
#[derive(Clone, Debug)]
pub struct CalloutTooltipOptions {
    /// The color of the tooltip text. Defaults to pure white: #FFFFFF.
    pub text_color: Vec4,
    /// The background color of the tooltip. Defaults to dark gray: #424C54.
    pub bg_color: Vec4,
    /// The position of the tooltip relative to the widget that it's related to.
    pub position: TooltipPosition,
    /// The height/length of the callout triangle that points to the related widget.
    pub triangle_height: f64,
}
impl Default for CalloutTooltipOptions {
    fn default() -> Self {
        Self {
            text_color: vec4(1.0, 1.0, 1.0, 1.0),
            bg_color: vec4(0.26, 0.30, 0.333, 1.0),
            position: TooltipPosition::default(),
            triangle_height: 7.5,
        }
    }
}

/// The location of the tooltip with respect to its target widget.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TooltipPosition {
    /// The tooltip will be drawn above the target widget.
    Top,
    /// The tooltip will be drawn below the target widget.
    Bottom,
    /// The tooltip will be drawn to the left of the target widget.
    Left,
    /// (Default) The tooltip will be drawn to the right of the target widget.
    #[default] Right,
}

/// A tooltip widget that a callout pointing towards the referenced widget.
#[derive(Live, LiveHook, Widget)]
pub struct CalloutTooltip {
    #[deref] view: View,
}


#[derive(Debug)]
struct PositionCalculation {
    tooltip_pos: DVec2,
    callout_position: f64,
    fixed_width: bool,
    width_to_be_fixed: f64,
}

impl Widget for CalloutTooltip {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl CalloutTooltip {
    /// Calculate tooltip position and layout parameters for a given position
    fn calculate_position(
        options: &CalloutTooltipOptions,
        widget_rect: Rect,
        expected_dimension: DVec2,
        screen_size: DVec2,
        triangle_height: f64,
    ) -> PositionCalculation {
        let pos = widget_rect.pos;
        let size = widget_rect.size;
        let mut tooltip_pos = DVec2 {
            x: min(pos.x, screen_size.x - expected_dimension.x),
            y: min(
                pos.y + widget_rect.size.y,
                screen_size.y - expected_dimension.y,
            ),
        };
        let mut fixed_width = false;
        let mut callout_position = 0.0;
        let mut width_to_be_fixed = screen_size.x;

        match options.position {
            TooltipPosition::Top => {
                tooltip_pos.y = widget_rect.pos.y - max(expected_dimension.y, size.y);
                callout_position = 180.0;
            }
            TooltipPosition::Bottom => {
                if tooltip_pos.y == screen_size.y - expected_dimension.y {
                    tooltip_pos.y = widget_rect.pos.y - max(expected_dimension.y, size.y);
                    callout_position = 180.0;
                } else {
                    tooltip_pos.y = widget_rect.pos.y + widget_rect.size.y;
                }
            }
            TooltipPosition::Left => {
                tooltip_pos.x = widget_rect.pos.x
                    - max(expected_dimension.x, widget_rect.size.x)
                    - triangle_height;
                tooltip_pos.y = widget_rect.pos.y
                    + 0.5 * (widget_rect.size.y - max(expected_dimension.y, widget_rect.size.y));
                callout_position = 90.0;
            }
            TooltipPosition::Right => {
                tooltip_pos.x = widget_rect.pos.x + widget_rect.size.x;
                tooltip_pos.y = widget_rect.pos.y + 0.5 * widget_rect.size.y - expected_dimension.y * 0.5;
                width_to_be_fixed = max(
                    screen_size.x - (pos.x + widget_rect.size.x + triangle_height * 2.0),
                    expected_dimension.x,
                );
                callout_position = 270.0;
            }
        }
        
        Self::apply_edge_case_fix(
            &options.position,
            &mut tooltip_pos,
            &mut fixed_width,
            &mut width_to_be_fixed,
            screen_size,
            expected_dimension,
            widget_rect,
            triangle_height,
        );

        PositionCalculation {
            tooltip_pos,
            callout_position,
            fixed_width,
            width_to_be_fixed,
        }
    }

    /// Check if width fixing is needed for edge cases
    fn needs_width_fix(tooltip_x: f64, screen_width: f64, expected_width: f64) -> bool {
        tooltip_x == screen_width - expected_width && tooltip_x < 0.0
    }
    
    /// Apply edge case handling for position and width fixing
    fn apply_edge_case_fix(
        position: &TooltipPosition,
        tooltip_pos: &mut DVec2,
        fixed_width: &mut bool,
        width_to_be_fixed: &mut f64,
        screen_size: DVec2,
        expected_dimension: DVec2,
        widget_rect: Rect,
        triangle_height: f64,
    ) {
        match position {
            TooltipPosition::Top | TooltipPosition::Bottom => {
                if Self::needs_width_fix(tooltip_pos.x, screen_size.x, expected_dimension.x) {
                    *fixed_width = true;
                    tooltip_pos.x = 0.0;
                }
            }
            TooltipPosition::Left => {
                if tooltip_pos.x < 0.0 {
                    *fixed_width = true;
                    *width_to_be_fixed = widget_rect.pos.x - triangle_height;
                    tooltip_pos.x = 0.0;
                }
            }
            TooltipPosition::Right => {
                if *width_to_be_fixed == expected_dimension.x
                    && *width_to_be_fixed > screen_size.x - widget_rect.pos.x - widget_rect.size.x
                {
                    *fixed_width = true;
                    *width_to_be_fixed = screen_size.x - widget_rect.pos.x - widget_rect.size.x;
                }
            }
        }
    }

    /// Apply tooltip configuration with given parameters
    fn apply_tooltip_config(
        tooltip: &mut TooltipRef,
        cx: &mut Cx,
        position_calc: &PositionCalculation,
        target: Vec2,
        target_size: Vec2,
        expected_dimension: DVec2,
        triangle_height: f64,
        text_color: Vec4,
        bg_color: Vec4,
    ) {
        let tooltip_pos = vec2(position_calc.tooltip_pos.x as f32, position_calc.tooltip_pos.y as f32);
        
        if position_calc.fixed_width {
            tooltip.apply_over(
                cx,
                live!(
                content: {
                    margin: { left: (tooltip_pos.x), top: (tooltip_pos.y) },
                    rounded_view = {
                        height: Fit,
                        draw_bg: {
                            triangle_height: (triangle_height),
                            background_color: (bg_color),
                            tooltip_pos: (tooltip_pos),
                            target_pos: (target),
                            target_size: (target_size),
                            expected_dimension_x: (expected_dimension.x),
                            callout_position: (position_calc.callout_position)
                        }
                        tooltip_label = {
                            width: (position_calc.width_to_be_fixed - 15.0 * 2.0),
                            draw_text: { color: (text_color) }
                        }
                    }
                }),
            );
        } else {
            tooltip.apply_over(cx, live!(
                content: {
                    margin: { left: (tooltip_pos.x), top: (tooltip_pos.y) },
                    rounded_view = {
                        height: Fit,
                        draw_bg: {
                            triangle_height: (triangle_height),
                            background_color: (bg_color),
                            tooltip_pos: (tooltip_pos),
                            target_pos: (target),
                            target_size: (target_size),
                            expected_dimension_x: (expected_dimension.x),
                            callout_position: (position_calc.callout_position)
                        }
                        tooltip_label = {
                            width: Fit,
                            draw_text: { color: (text_color) }
                        }
                    }
                }
            ));
        }
    }

    /// Shows a tooltip with the given text and options.
    ///
    /// The tooltip comes with a callout pointing to its target.
    ///
    /// By default, the tooltip will be displayed to the widget's right.
    ///
    /// If the widget is too close to the edge of the window, the tooltip is positioned
    /// to avoid being cut off, with automatic fallback to opposite directions.
    pub fn show_with_options(
        &mut self,
        cx: &mut Cx,
        text: &str,
        widget_rect: Rect,
        options: CalloutTooltipOptions,
    ) {
        let mut tooltip = self.view.tooltip(ids!(tooltip));
        tooltip.set_text(cx, &pad_last_line(text));

        let expected_dimension = tooltip.view(ids!(rounded_view)).area().rect(cx).size;
        let screen_size = tooltip.area().rect(cx).size;
        let position_calc = Self::calculate_position(
            &options,
            widget_rect,
            expected_dimension,
            screen_size,
            options.triangle_height,
        );

        let target = vec2(
            widget_rect.pos.x as f32,
            widget_rect.pos.y as f32,
        );
        let target_size = vec2(
            widget_rect.size.x as f32,
            widget_rect.size.y as f32,
        );

        let mut text_color = options.text_color;
        if expected_dimension.x == 0.0 {
            text_color.w = 0.0;
        }

        Self::apply_tooltip_config(
            &mut tooltip,
            cx,
            &position_calc,
            target,
            target_size,
            expected_dimension,
            options.triangle_height,
            text_color,
            options.bg_color,
        );
        tooltip.show(cx);
    }

    /// Shows the tooltip.
    pub fn show(&mut self, cx: &mut Cx) {
        self.view.tooltip(ids!(tooltip)).show(cx);
    }

    /// Hide the tooltip.
    pub fn hide(&mut self, cx: &mut Cx) {
        self.view.tooltip(ids!(tooltip)).hide(cx);
    }
}

impl CalloutTooltipRef {
    /// See [`CalloutTooltip::show_with_options()`].
    pub fn show_with_options(
        &mut self,
        cx: &mut Cx,
        text: &str,
        widget_rect: Rect,
        options: CalloutTooltipOptions,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_with_options(cx, text, widget_rect, options);
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

/// Actions that can be emitted from anywhere to show or hide the `tooltip`.
#[derive(Clone, Debug, DefaultNone)]
pub enum TooltipAction {
    /// Show the tooltip with the given text and options.
    HoverIn {
        text: String,
        /// The location of the widget that the tooltip is positioned relative to.
        widget_rect: Rect,
        options: CalloutTooltipOptions,
    },
    /// Hide the tooltip.
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
    let (lines_len, _) = lines.size_hint();
    if lines_len <= 1 {
        return text.to_string();
    }
    let longest_line = lines.clone().map(|s| s.len()).max().unwrap_or(0);
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
