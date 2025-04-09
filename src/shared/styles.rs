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
    pub ICON_PLAY         = dep("crate://self/resources/icons/play.svg")
    pub ICON_PAUSE        = dep("crate://self/resources/icons/pause.svg")

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
    pub MESSAGE_TEXT_LINE_SPACING = 1.35
    pub MESSAGE_TEXT_HEIGHT_FACTOR = 1.55
    // This font should only be used for plaintext labels. Don't use this for Html content,
    // as the Html widget sets different fonts for different text styles (e.g., bold, italic).
    pub MESSAGE_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (MESSAGE_FONT_SIZE),
        height_factor: (MESSAGE_TEXT_HEIGHT_FACTOR),
        line_spacing: (MESSAGE_TEXT_LINE_SPACING),
    }

    pub MESSAGE_REPLY_PREVIEW_FONT_SIZE = 9.5


    pub SMALL_STATE_FONT_SIZE = 9.0
    pub SMALL_STATE_TEXT_COLOR = #x888
    pub SMALL_STATE_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (SMALL_STATE_FONT_SIZE),
        height_factor: 1.3,
    }

    pub TIMESTAMP_FONT_SIZE = 8.5
    pub TIMESTAMP_TEXT_COLOR = #x999
    pub TIMESTAMP_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (TIMESTAMP_FONT_SIZE),
    }

    pub ROOM_NAME_TEXT_COLOR = #x0

    pub COLOR_META = #xccc

    pub COLOR_PROFILE_CIRCLE = #xfff8ee
    pub COLOR_DIVIDER = #x00000018
    pub COLOR_DIVIDER_DARK = #x00000044

    pub COLOR_DANGER_RED = #xDC0005
    pub COLOR_ACCEPT_GREEN = #x138808

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

    // A text input widget styled for Robrix.
    pub RobrixTextInput = <TextInput> {
        width: Fill, height: Fit,
        margin: 0,
        align: {y: 0.5}
        empty_message: "Enter text..."
        draw_bg: {
            color: (COLOR_PRIMARY)
            instance border_radius: 2.0
            instance border_size: 0.0
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
                    self.inset.x + self.border_size,
                    self.inset.y + self.border_size,
                    self.rect_size.x - (self.inset.x + self.inset.z + self.border_size * 2.0),
                    self.rect_size.y - (self.inset.y + self.inset.w + self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.get_border_color(), self.border_size)
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
        draw_highlight: {
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
}
