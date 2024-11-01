use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::icon_button::RobrixIconButton;

    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")

    SearchBar = {{SearchBar}}<RoundedView> {
        width: Fill,
        height: Fit,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
        }

        padding: {top: 3, bottom: 3, left: 10, right: 10}
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

        clear_button = <RobrixIconButton> {
            visible: false,
            padding: {left: 10, right: 10}
            draw_icon: {
                svg_file: (ICON_CLOSE),
                color: (COLOR_TEXT_INPUT_IDLE)
            }
            icon_walk: {width: 10, height: Fit}
        }
    }
}
#[derive(Live, LiveHook, Widget)]
pub struct SearchBar {
    #[deref]
    view: View,
}
#[derive(Clone, Debug, DefaultNone)]
pub enum SearchBarAction {
    /// The user has entered a search query.
    Search(String),
    /// The user has cleared the search query or the search has been reset.
    ResetSearch,
    None
}

impl Widget for SearchBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for SearchBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let input = self.text_input(id!(input));
        let clear_button = self.button(id!(clear_button));

        // Handle user changing the input text
        if let Some(keywords) = input.changed(actions) {
            clear_button.set_visible(!keywords.is_empty());
            if keywords.is_empty() {
                let widget_uid = self.widget_uid();
                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    SearchBarAction::ResetSearch
                );
            } else {
                let widget_uid = self.widget_uid();
                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    SearchBarAction::Search(keywords)
                );
            }
        }

        // Handle user clicked the clear button
        if clear_button.clicked(actions) {
            input.set_text_and_redraw(cx, "");
            clear_button.set_visible(false);
            input.set_key_focus(cx);

            let widget_uid = self.widget_uid();
            cx.widget_action(widget_uid, &scope.path, SearchBarAction::ResetSearch);
        }

    }
}