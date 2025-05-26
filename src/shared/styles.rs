use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    pub ICON_ADD_REACTION = dep("crate://self/resources/icons/add_reaction.svg")
    pub ICON_BLOCK_USER   = dep("crate://self/resources/icons/forbidden.svg")
    pub ICON_CHECKMARK    = dep("crate://self/resources/icons/checkmark.svg")
    pub ICON_CLOSE        = dep("crate://self/resources/icons/close.svg")
    pub ICON_COPY         = dep("crate://self/resources/icons/copy.svg")
    pub ICON_EDIT         = dep("crate://self/resources/icons/edit.svg")
    pub ICON_HTML_FILE    = dep("crate://self/resources/icons/html_file.svg")
    pub ICON_JUMP         = dep("crate://self/resources/icons/go_back.svg")
    pub ICON_LINK         = dep("crate://self/resources/icons/link.svg")
    pub ICON_PIN          = dep("crate://self/resources/icons/pin.svg")
    pub ICON_REPLY        = dep("crate://self/resources/icons/reply.svg")
    pub ICON_SEND         = dep("crate://self/resources/icon_send.svg")
    pub ICON_TRASH        = dep("crate://self/resources/icons/trash.svg")
    pub ICON_VIEW_SOURCE  = dep("crate://self/resources/icons/view_source.svg")

    pub TITLE_TEXT = <THEME_FONT_REGULAR>{
        font_size: (13),
    }

    pub REGULAR_TEXT = <THEME_FONT_REGULAR>{
        font_size: (10),
    }

    pub TEXT_SUB = <THEME_FONT_REGULAR>{
        font_size: (10),
    }

    pub USERNAME_FONT_SIZE = 11
    pub USERNAME_TEXT_COLOR = #x2
    pub USERNAME_TEXT_STYLE = <THEME_FONT_BOLD>{
        font_size: (USERNAME_FONT_SIZE),
    }


    pub TYPING_NOTICE_TEXT_COLOR = #121570

    pub MESSAGE_FONT_SIZE = 11
    pub MESSAGE_TEXT_COLOR = #x333
    // notices (automated messages from bots) use a lighter color
    pub MESSAGE_NOTICE_TEXT_COLOR = #x888
    pub MESSAGE_TEXT_LINE_SPACING = 1.3
    // This font should only be used for plaintext labels. Don't use this for Html content,
    // as the Html widget sets different fonts for different text styles (e.g., bold, italic).
    pub MESSAGE_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (MESSAGE_FONT_SIZE),
        line_spacing: (MESSAGE_TEXT_LINE_SPACING),
    }

    pub MESSAGE_REPLY_PREVIEW_FONT_SIZE = 9.5


    pub SMALL_STATE_FONT_SIZE = 9.0
    pub SMALL_STATE_TEXT_COLOR = #x888
    pub SMALL_STATE_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (SMALL_STATE_FONT_SIZE),
    }

    pub TIMESTAMP_FONT_SIZE = 8.5
    pub TIMESTAMP_TEXT_COLOR = #x999
    pub TIMESTAMP_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (TIMESTAMP_FONT_SIZE),
    }

    pub ROOM_NAME_TEXT_COLOR = #x0

    pub COLOR_ROBRIX_PURPLE = #572DCC; // the purple color from the Robrix logo
    pub COLOR_META = #xccc

    pub COLOR_PROFILE_CIRCLE = #xfff8ee
    pub COLOR_DIVIDER = #x00000018
    pub COLOR_DIVIDER_DARK = #x00000044

    pub COLOR_ACCEPT_GREEN = #138808
    pub COLOR_DANGER_RED = #DC0005
    pub COLOR_DISABLE_GRAY = #B3B3B3

    pub COLOR_SELECT_TEXT = #A6CDFE


    pub COLOR_PRIMARY = #ffffff
    pub COLOR_PRIMARY_DARKER = #fefefe
    pub COLOR_SECONDARY = #eef2f4

    pub COLOR_ACTIVE_PRIMARY = #0f88fe
    pub COLOR_ACTIVE_PRIMARY_DARKER = #106fcc

    pub COLOR_AVATAR_BG = #52b2ac
    pub COLOR_AVATAR_BG_IDLE = #d8d8d8

    pub COLOR_UNREAD_MESSAGE_BADGE = (COLOR_AVATAR_BG)
    pub COLOR_TOOLTIP_BG = (COLOR_SECONDARY)

    pub COLOR_TEXT_IDLE = #d8d8d8
    pub COLOR_TEXT = #1C274C
    pub COLOR_TEXT_INPUT_IDLE = #d8d8d8

    // An icon that can be rotated at a custom angle.
    pub IconRotated = <Icon> {
        draw_icon: {
            instance rotation_angle: 0.0,

            // Support rotation of the icon
            fn clip_and_transform_vertex(self, rect_pos: vec2, rect_size: vec2) -> vec4 {
                let clipped: vec2 = clamp(
                    self.geom_pos * rect_size + rect_pos,
                    self.draw_clip.xy,
                    self.draw_clip.zw
                )
                self.pos = (clipped - rect_pos) / rect_size

                // Calculate the texture coordinates based on the rotation angle
                let angle_rad = self.rotation_angle * 3.14159265359 / 180.0;
                let cos_angle = cos(angle_rad);
                let sin_angle = sin(angle_rad);
                let rot_matrix = mat2(
                    cos_angle, -sin_angle,
                    sin_angle, cos_angle
                );
                self.tex_coord1 = mix(
                    self.icon_t1.xy,
                    self.icon_t2.xy,
                    (rot_matrix * (self.pos.xy - vec2(0.5))) + vec2(0.5)
                );

                return self.camera_projection * (self.camera_view * (self.view_transform * vec4(
                    clipped.x,
                    clipped.y,
                    self.draw_depth + self.draw_zbias,
                    1.
                )))
            }
        }
    }

    // A text input widget styled for Robrix.
    pub RobrixTextInput = <TextInput> {
        width: Fill, height: Fit,
        margin: 0,
        align: {y: 0.5}
        empty_text: "Enter text..."

        draw_bg: {
            color: (COLOR_PRIMARY)
            border_radius: 2.0
            border_size: 0.0

            // TODO: determine these other colors below
            color_hover: (COLOR_PRIMARY)
            color_focus: (COLOR_PRIMARY)
            color_down: (COLOR_PRIMARY)
            color_empty: (COLOR_PRIMARY)
            color_disabled: (COLOR_PRIMARY)

            border_color_1: (COLOR_PRIMARY)
            border_color_1_hover: (COLOR_PRIMARY)
            border_color_1_focus: (COLOR_PRIMARY)
            border_color_1_down: (COLOR_PRIMARY)
            border_color_1_empty: (COLOR_PRIMARY)
            border_color_1_disabled: (COLOR_PRIMARY)

            border_color_2: (COLOR_PRIMARY)
            border_color_2_hover: (COLOR_PRIMARY)
            border_color_2_focus: (COLOR_PRIMARY)
            border_color_2_down: (COLOR_PRIMARY)
            border_color_2_empty: (COLOR_PRIMARY)
            border_color_2_disabled: (COLOR_PRIMARY)
        }

        draw_selection: {
            color: (COLOR_SELECT_TEXT)
            // TODO: determine these other colors below
            color_hover:  (COLOR_SELECT_TEXT)
            color_focus:  (COLOR_SELECT_TEXT)
            color_down:  (COLOR_SELECT_TEXT)
            color_empty:  (COLOR_SELECT_TEXT)
            color_disabled: (COLOR_SELECT_TEXT)
        }

        draw_cursor: {
            color: (MESSAGE_TEXT_COLOR)
        }

        draw_text: {
            text_style: <MESSAGE_TEXT_STYLE>{},
            color: (MESSAGE_TEXT_COLOR),
            // TODO: determine these colors
            uniform color_hover: (MESSAGE_TEXT_COLOR),
            uniform color_focus: (MESSAGE_TEXT_COLOR),
            uniform color_down: (MESSAGE_TEXT_COLOR),
            uniform color_disabled: (MESSAGE_TEXT_COLOR),
            uniform color_empty: #B,
            uniform color_empty_hover: #B,
            uniform color_empty_focus: #B,

            fn get_color(self) -> vec4 {
                return
                    mix( 
                        mix(
                            mix(
                                mix(
                                    self.color,
                                    mix(
                                        self.color_hover,
                                        self.color_down,
                                        self.down
                                    ),
                                    self.hover
                                ),
                                self.color_focus,
                                self.focus
                            ),
                            self.color_empty,
                            self.empty
                        ),
                        self.color_disabled,
                        self.disabled
                    )
            }
        }
    }
}

pub const COLOR_ACCEPT_GREEN: Vec3 = vec3(0.074, 0.533, 0.031); // #138808
pub const COLOR_DISABLE_GRAY: Vec3 = vec3(0.7, 0.7, 0.7); // #B3B3B3
pub const COLOR_DANGER_RED: Vec3 = vec3(0.863, 0.0, 0.02); // #DC0005
