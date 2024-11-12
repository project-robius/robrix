use makepad_widgets::*;

live_design! {

    import makepad_widgets::theme_desktop_dark::*;
    ICON_BLOCK_USER  = dep("crate://self/resources/icons/forbidden.svg")
    ICON_CHECKMARK   = dep("crate://self/resources/icons/checkmark.svg")
    ICON_CLOSE       = dep("crate://self/resources/icons/close.svg")



    TITLE_TEXT = <THEME_FONT_REGULAR>{
        font_size: (13),
    }

    REGULAR_TEXT = <THEME_FONT_REGULAR>{
        font_size: (10),
    }

    TEXT_SUB = <THEME_FONT_REGULAR>{
        font_size: (10),
    }

    USERNAME_FONT_SIZE = 11
    USERNAME_TEXT_COLOR = #x2
    USERNAME_TEXT_STYLE = <THEME_FONT_BOLD>{
        font_size: (USERNAME_FONT_SIZE),
    }


    TYPING_NOTICE_TEXT_COLOR = #121570

    MESSAGE_FONT_SIZE = 11
    MESSAGE_TEXT_COLOR = #x444
    MESSAGE_TEXT_LINE_SPACING = 1.35
    MESSAGE_TEXT_HEIGHT_FACTOR = 1.55
    // This font should only be used for plaintext labels. Don't use this for Html content,
    // as the Html widget sets different fonts for different text styles (e.g., bold, italic).
    MESSAGE_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (MESSAGE_FONT_SIZE),
        height_factor: (MESSAGE_TEXT_HEIGHT_FACTOR),
        line_spacing: (MESSAGE_TEXT_LINE_SPACING),
    }

    MESSAGE_REPLY_PREVIEW_FONT_SIZE = 9.5

    SMALL_STATE_FONT_SIZE = 9.0
    SMALL_STATE_TEXT_COLOR = #x888
    SMALL_STATE_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (SMALL_STATE_FONT_SIZE),
        height_factor: 1.3,
    }

    TIMESTAMP_FONT_SIZE = 8.5
    TIMESTAMP_TEXT_COLOR = #x999
    TIMESTAMP_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (TIMESTAMP_FONT_SIZE),
    }

    COLOR_META = #xccc

    COLOR_PROFILE_CIRCLE = #xfff8ee
    COLOR_DIVIDER = #x00000018
    COLOR_DIVIDER_DARK = #x00000044

    COLOR_DANGER_RED = #xDC0005
    COLOR_ACCEPT_GREEN = #x138808

    COLOR_PRIMARY = #ffffff
    COLOR_PRIMARY_DARKER = #fefefe
    COLOR_SECONDARY = #eef2f4

    COLOR_SELECTED_PRIMARY = #0f88fe
    COLOR_SELECTED_PRIMARY_DARKER = #106fcc

    COLOR_AVATAR_BG = #52b2ac
    COLOR_AVATAR_BG_IDLE = #d8d8d8

    COLOR_UNREAD_MESSAGE_BADGE = (COLOR_AVATAR_BG)

    COLOR_TEXT_IDLE = #d8d8d8
    COLOR_TEXT = #1C274C

    COLOR_TEXT_INPUT_IDLE = #d8d8d8
}
