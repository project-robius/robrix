use makepad_widgets::*;

live_design! {
    TITLE_TEXT = {
        font_size: (14),
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }

    REGULAR_TEXT = {
        font_size: (12),
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }

    TEXT_SUB = {
        font_size: (10),
        height_factor: 1.5,
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }

    USERNAME_FONT_SIZE = 13.0
    USERNAME_TEXT_COLOR = #x060 // dark green
    USERNAME_TEXT_STYLE = {
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
        font_size: (USERNAME_FONT_SIZE),
        // height_factor: 1.5,
    }


    MESSAGE_FONT_SIZE = 12.0
    MESSAGE_TEXT_COLOR = #x555
    MESSAGE_TEXT_LINE_SPACING = 1.35
    MESSAGE_TEXT_HEIGHT_FACTOR = 1.55
    // This font should only be used for plaintext labels. Don't use this for Html content,
    // as the Html widget sets different fonts for different text styles (e.g., bold, italic).
    MESSAGE_TEXT_STYLE = {
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
        font_size: (MESSAGE_FONT_SIZE),
        height_factor: (MESSAGE_TEXT_HEIGHT_FACTOR),
        line_spacing: (MESSAGE_TEXT_LINE_SPACING),
    }

    SMALL_STATE_FONT_SIZE = 9.5
    SMALL_STATE_TEXT_COLOR = #x999
    SMALL_STATE_TEXT_STYLE = {
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
        font_size: (SMALL_STATE_FONT_SIZE),
        height_factor: 1.3,
    }

    TIMESTAMP_FONT_SIZE = 8.5
    TIMESTAMP_TEXT_COLOR = #x999
    TIMESTAMP_TEXT_STYLE = {
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
        font_size: (TIMESTAMP_FONT_SIZE),
    }

    COLOR_PROFILE_CIRCLE = #xfff8ee
    COLOR_DIVIDER = #x00000018
    COLOR_DIVIDER_DARK = #x00000044

    COLOR_DANGER_RED = #xDC0005
}
