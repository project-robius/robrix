use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::clickable_icon::ClickableIcon;

    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")
    ICON_CLOSE = dep("crate://self/resources/icons/close.svg")

    SearchBar = {{SearchBar}}<RoundedView> {
        width: Fill,
        height: Fit,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
        }

        padding: {top: 3, bottom: 3, left: 10, right: 20}
        spacing: 4,
        align: {x: 0.0, y: 0.5},

        draw_bg: {
            radius: 0.0,
            border_color: #d8d8d8,
            border_width: 0.6,
        }

        <Icon> {
            draw_icon: {
                svg_file: (ICON_SEARCH),
                fn get_color(self) -> vec4 {
                    return (COLOR_TEXT_INPUT_IDLE);
                }
            }
            icon_walk: {width: 14, height: Fit}
        }

        input = <TextInput> {
            width: Fill,
            height: 30.,

            empty_message: "Search"

            draw_text: {
                text_style: { font_size: 10 },
                fn get_color(self) -> vec4 {
                    return (COLOR_TEXT_INPUT_IDLE);
                }
            }

            // TODO find a way to override colors
            draw_cursor: {
                instance focus: 0.0
                uniform border_radius: 0.5
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    sdf.box(
                        0.,
                        0.,
                        self.rect_size.x,
                        self.rect_size.y,
                        self.border_radius
                    )
                    sdf.fill(mix(#fff, #bbb, self.focus));
                    return sdf.result
                }
            }

            // TODO find a way to override colors
            draw_selection: {
                instance hover: 0.0
                instance focus: 0.0
                uniform border_radius: 2.0
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    sdf.box(
                        0.,
                        0.,
                        self.rect_size.x,
                        self.rect_size.y,
                        self.border_radius
                    )
                    sdf.fill(mix(#eee, #ddd, self.focus)); // Pad color
                    return sdf.result
                }
            }

            draw_bg: {
                color: (COLOR_PRIMARY)
                instance radius: 0.0
                instance border_width: 0.0
                instance border_color: #3
                instance inset: vec4(0.0, 0.0, 0.0, 0.0)

                fn get_color(self) -> vec4 {
                    return self.color
                }

                fn get_border_color(self) -> vec4 {
                    return self.border_color
                }

                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    sdf.box(
                        self.inset.x + self.border_width,
                        self.inset.y + self.border_width,
                        self.rect_size.x - (self.inset.x + self.inset.z + self.border_width * 2.0),
                        self.rect_size.y - (self.inset.y + self.inset.w + self.border_width * 2.0),
                        max(1.0, self.radius)
                    )
                    sdf.fill_keep(self.get_color())
                    if self.border_width > 0.0 {
                        sdf.stroke(self.get_border_color(), self.border_width)
                    }
                    return sdf.result;
                }
            }
        }

        clear_button = <Button> {
            height: Fit,
            height: Fit,

            visible: false

            draw_bg: {
                instance color: #0000
                instance color_hover: #fff
                instance border_width: 1.0
                instance border_color: #0000
                instance border_color_hover: #fff
                instance radius: 2.5

                fn get_color(self) -> vec4 {
                    return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
                }

                fn get_border_color(self) -> vec4 {
                    return mix(self.border_color, mix(self.border_color, self.border_color_hover, 0.2), self.hover)
                }

                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
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
                    return sdf.result;
                }
            }

            draw_icon: {
                svg_file: (ICON_CLOSE),
                fn get_color(self) -> vec4 {
                    return (COLOR_TEXT_INPUT_IDLE);
                }
            }

            icon_walk: { width: 8, height: 8 }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct SearchBar {
    #[deref] view: View,

    /// The placeholder text for the search input.
    #[live] pub placeholder: String,

    #[rust]
    search_timer: Timer,

    #[live(0.3)]
    search_debounce_time: f64,
}

#[derive(Clone, DefaultNone, Debug)]
pub enum SearchBarAction {
    SearchValue(String),
    None,
}

impl Widget for SearchBar {

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);

        if self.search_timer.is_event(event).is_some() {
            self.search_timer = Timer::default();

            let input = self.text_input(id!(input));
            let keywords = input.text();

            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                SearchBarAction::SearchValue(keywords)
            );

        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {

        let input = self.view.text_input(id!(input));
        let clear_button = self.view.button(id!(clear_button));

        if input.text().is_empty() {

            if let Some(mut inner) = input.borrow_mut() {
                // only set the empty message if placeholder is not empty, otherwise, it will be set to "Search"
                if !self.placeholder.is_empty() {
                    inner.empty_message = self.placeholder.clone();
                } else {
                    inner.empty_message = "Search".to_string();
                }
            }

            clear_button.set_visible(false);
        } else {
            clear_button.set_visible(true);
        }

        self.view.draw_walk(cx, scope, walk)
    }

}

impl WidgetMatchEvent for SearchBar {

    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions, scope: &mut Scope) {

        let input = self.text_input(id!(input));
        let clear_button = self.button(id!(clear_button));

        if clear_button.clicked(actions) {
            input.set_text("");
            clear_button.set_visible(false);

            self.redraw(cx);
        }

        if let Some(_) = input.changed(actions) {
            cx.stop_timer(self.search_timer);
            self.search_timer = cx.start_timeout(self.search_debounce_time);
        }

    }

}