use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::icon_button::*;

    IMG_APP_LOGO = dep("crate://self/packaging/robrix_logo_alpha.png")

    LoginTextInput = <TextInput> {
        width: Fill, height: Fit, margin: 0
        align: {y: 0.5}
        draw_bg: {
            color: (COLOR_PRIMARY)
            instance radius: 2.0
            instance border_width: 0.8
            instance border_color: #D0D5DD
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

        draw_text: {
            color: (MESSAGE_TEXT_COLOR),
            text_style: <MESSAGE_TEXT_STYLE>{},

            fn get_color(self) -> vec4 {
                return mix(
                    self.color,
                    #B,
                    self.is_empty
                )
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
    }

    LoginScreen = {{LoginScreen}} {
        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
        }
        align: {x: 0.5, y: 0.5}

        <RoundedView> {
            width: Fit, height: Fit
            flow: Down
            align: {x: 0.5, y: 0.5}
            padding: 50
            spacing: 20.0

            show_bg: true,
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            logo_image = <Image> {
                fit: Smallest,
                width: 80
                source: (IMG_APP_LOGO),
            }

            title = <Label> {
                width: Fit, height: Fit
                draw_text: {
                    color: (COLOR_TEXT)
                        text_style: <TITLE_TEXT>{font_size: 16.0}
                }
                text: "Login to Robrix"
            }

            username_input = <LoginTextInput> {
                width: 300, height: 40
                empty_message: "Username"
            }

            password_input = <LoginTextInput> {
                width: 300, height: 40
                empty_message: "Password"
                // password: true
            }

            login_button = <RobrixIconButton > {
                width: 300, height: 40
                draw_text: {
                    color: (COLOR_PRIMARY)
                    text_style: <REGULAR_TEXT> {}
                }

                draw_bg: {
                    color: (COLOR_SELECTED_PRIMARY)
                }
                    text: "Login"
            }

            error_message = <Label> {
                width: 300, height: Fit
                draw_text: {
                    color: (COLOR_DANGER_RED)
                    text_style: <REGULAR_TEXT> {}
                }
                text: ""
            }

            success_message = <Label> {
                width: 300, height: Fit
                draw_text: {
                    color: (COLOR_ACCEPT_GREEN)
                    text_style: <REGULAR_TEXT> {}
                }
                text: ""
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct LoginScreen {
    #[deref]
    view: View,
}

impl Widget for LoginScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let widget_uid = self.widget_uid();

        self.view.handle_event(cx, event, scope);

        let password_input = self.view.text_input(id!(password_input));
        let error_message_label = self.view.label(id!(error_message));
        let success_message_label = self.view.label(id!(success_message));
        let login_button = self.view.button(id!(login_button));

        let username = self.view.text_input(id!(username_input)).text();
        let password = self.view.text_input(id!(password_input)).text();

        if let Event::Actions(actions) = event {
            if login_button.clicked(actions) {
                if username.is_empty() || password.is_empty() {
                    error_message_label.set_text("Please enter both username and password.");
                } else {
                    // TODO: Implement actual login logic
                    let mut login_successful = false;

                    if password == "aa" {login_successful = true}

                    if login_successful {
                        cx.widget_action(
                            widget_uid,
                            &scope.path,
                            LoginAction::LoginSuccess,
                        );
                        error_message_label.set_text("");
                        success_message_label.set_text("Login successful!");
                    } else {
                        password_input.set_text("");
                        error_message_label.set_text("Incorrect username or password.");
                    }


                }
                self.redraw(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

#[derive(Clone, DefaultNone, Debug)]
pub enum LoginAction {
    None,
    LoginSuccess,
}
