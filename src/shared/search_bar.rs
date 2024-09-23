use makepad_widgets::*;

use crate::shared::clickable_icon::{ClickableIconAction, ClickableIconWidgetExt};

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::clickable_icon::ClickableIcon;

    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")
    ICON_CLOSE = dep("crate://self/resources/icons/close.svg")

    SearchBar = {{SearchBar}} {
        width: Fill,
        height: Fit,

        <RoundedView> {
            width: Fill,
            height: Fit,

            padding: {top: 3, bottom: 3, left: 10, right: 20}
            spacing: 4,
            align: {x: 0.0, y: 0.5},

            draw_bg: {
                radius: 0.0,
                border_color: #d8d8d8,
                border_width: 0.6,
            }

            search_icon = <Icon> {
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

            clear_icon = <ClickableIcon> {
                height: Fill,
                width: 30,
                visible: false

                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    fn get_color(self) -> vec4 {
                        return (COLOR_TEXT_INPUT_IDLE);
                    }
                }

                icon_walk: {width: 8, height: Fit}

                padding: {
                    top: 5,
                    right: 5,
                    bottom: 5,
                    left: 5
                }
            }
        }
    }

}

#[derive(Live, LiveHook, Widget)]
pub struct SearchBar {
    #[deref] view: View,

    #[live] placeholder: String,
}

#[derive(Clone, DefaultNone, Debug)]
pub enum SearchBarAction {
    None,
    Change(String),
    // Maybe we can add more actions here, e.g., focus, blur, clear, etc.
}

impl Widget for SearchBar {

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {

        let uid = self.widget_uid();
        let text_input_ref = self.view.text_input(id!(input));

        // Now, handle any actions on this widget, e.g., a user focus or blur the input.
        for search_bar_action in cx.capture_actions(|cx| self.view.handle_event(cx, event, scope)) {

            // Now, we handle the clear icon click event, other events are left to the component caller to implement.
            // However, we also can expose the clear icon click event to the component caller, but now we handle it here.
            if let ClickableIconAction::Click = search_bar_action.as_widget_action().cast() {
                text_input_ref.set_text("");
                // hide the clear icon
                let clear_icon_ref = self.view.clickable_icon(id!(clear_icon));
                clear_icon_ref.set_visible(cx, false);
                cx.redraw_area(clear_icon_ref.area());
            }

            if let TextInputAction::Change(value) = search_bar_action.as_widget_action().cast() {
                // expose the value to the component caller
                // TODO: debounce the search input here, e.g., wait for 500ms before sending the search query.
                // But this comment is more commonly used on the front end, but I haven't tried it in makepad yet.
                cx.widget_action(
                    uid,
                    &scope.path,
                    SearchBarAction::Change(value)
                );
            }

            // if let TextInputAction::KeyFocus = search_bar_action.as_widget_action().cast() {
            //     log!("SearchBar key focus");
            // }

            // if let TextInputAction::KeyFocusLost = search_bar_action.as_widget_action().cast() {
            //     log!("SearchBar key blur");
            // }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {

        let input_ref = self.view.text_input(id!(input));
        let clear_icon_ref = self.view.clickable_icon(id!(clear_icon));

        if input_ref.text().is_empty() {

            if let Some(mut inner) = input_ref.borrow_mut() {
                // only set the empty message if placeholder is not empty, otherwise, it will be set to "Search"
                if !self.placeholder.is_empty() {
                     // set the empty message
                    inner.empty_message = self.placeholder.clone();
                } else {
                    // default empty message
                    inner.empty_message = "Search".to_string();
                }
            }

            // hide the clear icon
            clear_icon_ref.set_visible(cx, false);
            cx.redraw_area(clear_icon_ref.area());
        } else {
            // show the clear icon
            clear_icon_ref.set_visible(cx, true);
            cx.redraw_area(clear_icon_ref.area());
        }

        self.view.draw_walk(cx, scope, walk)
    }
}