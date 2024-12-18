use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;

    pub ICON_BLOCK_USER  = dep("crate://self/resources/icons/forbidden.svg")
    pub ICON_CHECKMARK   = dep("crate://self/resources/icons/checkmark.svg")
    pub ICON_CLOSE       = dep("crate://self/resources/icons/close.svg")

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

    pub COLOR_SELECTED_PRIMARY = #0f88fe
    pub COLOR_SELECTED_PRIMARY_DARKER = #106fcc

    pub COLOR_AVATAR_BG = #52b2ac
    pub COLOR_AVATAR_BG_IDLE = #d8d8d8

    pub COLOR_UNREAD_MESSAGE_BADGE = (COLOR_AVATAR_BG)
    pub COLOR_TOOLTIP_BG = (COLOR_SECONDARY)

    pub COLOR_TEXT_IDLE = #d8d8d8
    pub COLOR_TEXT = #1C274C

    pub COLOR_TEXT_INPUT_IDLE = #d8d8d8
}
