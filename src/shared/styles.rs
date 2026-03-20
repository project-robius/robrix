use makepad_widgets::*;

script_mod! {

    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ICON_ADD              = crate_resource("self://resources/icons/add.svg")
    mod.widgets.ICON_ADD_REACTION     = crate_resource("self://resources/icons/add_reaction.svg")
    mod.widgets.ICON_ADD_USER         = crate_resource("self://resources/icons/add_user.svg") // TODO: FIX
    mod.widgets.ICON_ADD_WALLET       = crate_resource("self://resources/icons/add_wallet.svg")
    mod.widgets.ICON_FORBIDDEN        = crate_resource("self://resources/icons/forbidden.svg")
    mod.widgets.ICON_CHECKMARK        = crate_resource("self://resources/icons/checkmark.svg")
    mod.widgets.ICON_CLOSE            = crate_resource("self://resources/icons/close.svg")
    mod.widgets.ICON_CLOUD_CHECKMARK  = crate_resource("self://resources/icons/cloud_checkmark.svg")
    mod.widgets.ICON_CLOUD_OFFLINE    = crate_resource("self://resources/icons/cloud_offline.svg")
    mod.widgets.ICON_ROTATE_CW        = crate_resource("self://resources/icons/rotate_right_fa.svg")
    mod.widgets.ICON_ROTATE_CCW       = crate_resource("self://resources/icons/rotate_left_fa.svg")
    mod.widgets.ICON_COPY             = crate_resource("self://resources/icons/copy.svg")
    mod.widgets.ICON_EDIT             = crate_resource("self://resources/icons/edit.svg")
    mod.widgets.ICON_EXTERNAL_LINK    = crate_resource("self://resources/icons/external_link.svg")
    mod.widgets.ICON_IMPORT           = crate_resource("self://resources/icons/import.svg") // TODO: FIX
    mod.widgets.ICON_HIERARCHY        = crate_resource("self://resources/icons/hierarchy.svg")
    mod.widgets.ICON_HOME             = crate_resource("self://resources/icons/home.svg")
    mod.widgets.ICON_HTML_FILE        = crate_resource("self://resources/icons/html_file.svg")
    mod.widgets.ICON_INFO             = crate_resource("self://resources/icons/info.svg")
    mod.widgets.ICON_INVITE           = crate_resource("self://resources/icons/invite.svg")
    mod.widgets.ICON_JOIN_ROOM        = crate_resource("self://resources/icons/join_room.svg")
    mod.widgets.ICON_JUMP             = crate_resource("self://resources/icons/go_back.svg")
    mod.widgets.ICON_LOGOUT           = crate_resource("self://resources/icons/logout.svg")
    mod.widgets.ICON_LINK             = crate_resource("self://resources/icons/link.svg")
    mod.widgets.ICON_PIN              = crate_resource("self://resources/icons/pin.svg")
    mod.widgets.ICON_REPLY            = crate_resource("self://resources/icons/reply.svg")
    mod.widgets.ICON_SEARCH           = crate_resource("self://resources/icons/search.svg")
    mod.widgets.ICON_SEND             = crate_resource("self://resources/icon_send.svg")
    mod.widgets.ICON_SETTINGS         = crate_resource("self://resources/icons/settings.svg")
    mod.widgets.ICON_SQUARES          = crate_resource("self://resources/icons/squares_filled.svg")
    mod.widgets.ICON_TOMBSTONE        = crate_resource("self://resources/icons/tombstone.svg")
    mod.widgets.ICON_TRASH            = crate_resource("self://resources/icons/trash.svg")
    mod.widgets.ICON_TRIANGLE_DOWN    = crate_resource("self://resources/icons/triangle_down_fill.svg")
    mod.widgets.ICON_TRIANGLE_UP      = crate_resource("self://resources/icons/triangle_up_fill.svg")
    mod.widgets.ICON_UPLOAD           = crate_resource("self://resources/icons/upload.svg")
    mod.widgets.ICON_VIEW_SOURCE      = crate_resource("self://resources/icons/view_source.svg")
    mod.widgets.ICON_WARNING          = crate_resource("self://resources/icons/warning.svg")
    mod.widgets.ICON_ZOOM_IN          = crate_resource("self://resources/icons/zoom_in.svg")
    mod.widgets.ICON_ZOOM_OUT         = crate_resource("self://resources/icons/zoom_out.svg")

    mod.widgets.TITLE_TEXT = theme.font_regular {
        font_size: (13),
    }

    mod.widgets.REGULAR_TEXT = theme.font_regular {
        font_size: (10),
    }

    mod.widgets.TEXT_SUB = theme.font_regular {
        font_size: (10),
    }

    mod.widgets.USERNAME_FONT_SIZE = 11

    mod.widgets.USERNAME_TEXT_COLOR = #x2
    mod.widgets.USERNAME_TEXT_STYLE = theme.font_bold {
        font_size: (mod.widgets.USERNAME_FONT_SIZE),
    }

    mod.widgets.COLOR_ROBRIX_PURPLE = #572DCC; // the purple color from the Robrix logo

    mod.widgets.COLOR_ROBRIX_CYAN = #05CDC7; // the cyan color from the Robrix logo

    mod.widgets.TYPING_NOTICE_TEXT_COLOR = #121570


    mod.widgets.MESSAGE_FONT_SIZE = 11

    mod.widgets.MESSAGE_TEXT_COLOR = #x333
    // notices (automated messages from bots) use a lighter color
    mod.widgets.COLOR_MESSAGE_NOTICE_TEXT = #x888
    mod.widgets.MESSAGE_TEXT_LINE_SPACING = 1.3
    // This font should only be used for plaintext labels. Don't use this for Html content,
    // as the Html widget sets different fonts for different text styles (e.g., bold, italic).
    mod.widgets.MESSAGE_TEXT_STYLE = theme.font_regular {
        font_size: (mod.widgets.MESSAGE_FONT_SIZE),
        line_spacing: (mod.widgets.MESSAGE_TEXT_LINE_SPACING),
    }

    mod.widgets.MESSAGE_REPLY_PREVIEW_FONT_SIZE = 9.5



    mod.widgets.SMALL_STATE_FONT_SIZE = 9.0


    mod.widgets.SMALL_STATE_TEXT_COLOR = #x888
    mod.widgets.SMALL_STATE_TEXT_STYLE = theme.font_regular {
        font_size: (mod.widgets.SMALL_STATE_FONT_SIZE),
    }

    mod.widgets.TIMESTAMP_FONT_SIZE = 8.5

    mod.widgets.TIMESTAMP_TEXT_COLOR = #x999
    mod.widgets.TIMESTAMP_TEXT_STYLE = theme.font_regular {
        font_size: (mod.widgets.TIMESTAMP_FONT_SIZE),
    }

    mod.widgets.ROOM_NAME_TEXT_COLOR = #x0

    mod.widgets.COLOR_META = #xccc

    mod.widgets.COLOR_DIVIDER = #00000018

    mod.widgets.COLOR_DIVIDER_DARK = #00000044

    mod.widgets.COLOR_FG_ACCEPT_GREEN = #138808
    mod.widgets.COLOR_BG_ACCEPT_GREEN = #F0FFF0
    mod.widgets.COLOR_FG_DANGER_RED = #DC0005
    mod.widgets.COLOR_BG_DANGER_RED = #FFF0F0
    mod.widgets.COLOR_FG_DISABLED = #B3B3B3
    mod.widgets.COLOR_BG_DISABLED = #E0E0E0
    mod.widgets.COLOR_INFO_BLUE = #0f88fe
    mod.widgets.COLOR_WARNING_YELLOW = #fcdb03
    mod.widgets.COLOR_TEXT_WARNING_NOT_FOUND = #953800

    // mod.widgets.COLOR_SELECT_TEXT = #A6CDFE
    // mod.widgets.COLOR_SELECT_TEXT = #B5D8FE
    // mod.widgets.COLOR_SELECT_TEXT = #6BB1FD88 // results in #B5D8FE when mixed halfway with white
    // mod.widgets.COLOR_SELECT_TEXT = #57A3FB44
    // 0x4C is ~30% opacity , which results in #B5D8FE when atop pure white
    // But i like the look of 0x33 20% opacity a little better.
    mod.widgets.COLOR_SELECT_TEXT = #087DFC33
    // mod.widgets.COLOR_SELECT_TEXT = #4D9BFD88 // results in #A6CDFE when mixed halfway with white

    mod.widgets.COLOR_PRIMARY = #ffffff

    mod.widgets.COLOR_PRIMARY_DARKER = #fefefe
    mod.widgets.COLOR_SECONDARY = #E3E3E3
    mod.widgets.COLOR_SECONDARY_DARKER = #C8C8C8

    mod.widgets.COLOR_ACTIVE_PRIMARY = #0f88fe

    mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER = #106fcc

    mod.widgets.COLOR_BG_PREVIEW = #F0F5FF

    mod.widgets.COLOR_BG_PREVIEW_HOVER = #CDEDDF

    mod.widgets.COLOR_AVATAR_BG = #52b2ac

    mod.widgets.COLOR_AVATAR_BG_IDLE = #d8d8d8


    mod.widgets.COLOR_UNREAD_BADGE_MENTIONS = #FF0000;


    mod.widgets.COLOR_UNREAD_BADGE_MARKED = (mod.widgets.COLOR_ROBRIX_CYAN);
    mod.widgets.COLOR_UNREAD_BADGE_MESSAGES = #AAAAAA


    mod.widgets.COLOR_TEXT_IDLE = #d8d8d8


    mod.widgets.COLOR_TEXT = #1C274C
    mod.widgets.COLOR_TEXT_INPUT_IDLE = #d8d8d8

    mod.widgets.COLOR_TRANSPARENT = #00000000

    mod.widgets.COLOR_WARNING = #fcdb03

    mod.widgets.COLOR_LINK_HOVER = #21B070


    // This is chosen to nicely fit the 3 window chrome buttons on macOS
    mod.widgets.NAVIGATION_TAB_BAR_SIZE = 76
    mod.widgets.NAVIGATION_TAB_BAR_AVATAR_SIZE = (mod.widgets.NAVIGATION_TAB_BAR_SIZE * 0.65)
    mod.widgets.NAVIGATION_TAB_BAR_AVATAR_FONT_SIZE = (mod.widgets.NAVIGATION_TAB_BAR_AVATAR_SIZE * 0.4)


    mod.widgets.COLOR_NAVIGATION_TAB_FG = (mod.widgets.COLOR_TEXT)
    mod.widgets.COLOR_NAVIGATION_TAB_FG_HOVER = (mod.widgets.COLOR_TEXT)
    mod.widgets.COLOR_NAVIGATION_TAB_FG_ACTIVE = (mod.widgets.COLOR_TEXT)
    mod.widgets.COLOR_NAVIGATION_TAB_BG = (mod.widgets.COLOR_SECONDARY)
    mod.widgets.COLOR_NAVIGATION_TAB_BG_HOVER = (mod.widgets.COLOR_SECONDARY * 0.85)
    mod.widgets.COLOR_NAVIGATION_TAB_BG_ACTIVE = #9

    mod.widgets.COLOR_IMAGE_VIEWER_BACKGROUND = #333333CC // 80% Opacity

    mod.widgets.COLOR_IMAGE_VIEWER_META_BACKGROUND = #E8E8E8

    // A text input widget styled for Robrix.
    mod.widgets.RobrixTextInput = TextInput {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true},
        align: Align{y: 0.5}
        margin: 0,
        padding: 10,

        draw_bg +: {
            border_radius: 4.0 // was previously 2.0
            border_size: 1.0

            color: (mod.widgets.COLOR_PRIMARY)
            color_hover: (mod.widgets.COLOR_PRIMARY)
            color_focus: (mod.widgets.COLOR_PRIMARY)
            color_down: (mod.widgets.COLOR_PRIMARY)
            color_empty: (mod.widgets.COLOR_PRIMARY)
            color_disabled: (mod.widgets.COLOR_BG_DISABLED)

            border_color: (mod.widgets.COLOR_SECONDARY_DARKER)
            border_color_hover: (mod.widgets.COLOR_ACTIVE_PRIMARY)
            border_color_focus: (mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER)
            border_color_down: (mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER)
            border_color_empty: (mod.widgets.COLOR_SECONDARY_DARKER)
            border_color_disabled: (mod.widgets.COLOR_FG_DISABLED)

            color_2: vec4(-1.0, -1.0, -1.0, -1.0) // don't use color_2*
            border_color_2: vec4(-1.0, -1.0, -1.0, -1.0) // don't use border_color_2*
        }

        draw_selection +: {
            color: mod.widgets.COLOR_SELECT_TEXT
            // color: mix(mod.widgets.COLOR_BG_DISABLED, mod.widgets.COLOR_SELECT_TEXT, 0.5)
            color_hover:  (mod.widgets.COLOR_SELECT_TEXT)
            color_focus:  (mod.widgets.COLOR_SELECT_TEXT)
            color_down:  (mod.widgets.COLOR_SELECT_TEXT)
            color_empty:  (mod.widgets.COLOR_SELECT_TEXT)
            color_disabled: (mod.widgets.COLOR_SELECT_TEXT)
        }

        draw_cursor +: {
            color: (mod.widgets.MESSAGE_TEXT_COLOR)
        }

        draw_text +: {
            color: (mod.widgets.MESSAGE_TEXT_COLOR),
            color_hover: (mod.widgets.MESSAGE_TEXT_COLOR),
            color_focus: (mod.widgets.MESSAGE_TEXT_COLOR),
            color_down: (mod.widgets.MESSAGE_TEXT_COLOR),
            color_disabled: (mod.widgets.COLOR_FG_DISABLED),
            color_empty: #B,
            color_empty_hover: #9,
            color_empty_focus: #9,

            text_style: mod.widgets.MESSAGE_TEXT_STYLE {},
        }
    }
}


pub const NAVIGATION_TAB_BAR_SIZE: f64 = 68.0;
pub const REDACTED_MESSAGE_FONT_SIZE: f32 = 10.0;

/// #FFFFFF
pub const COLOR_PRIMARY:               Vec4 = vec4(1.0, 1.0, 1.0, 1.0);
/// #0F88FE
pub const COLOR_ACTIVE_PRIMARY:        Vec4 = vec4(0.059, 0.533, 0.996, 1.0);
/// #106FCC
pub const COLOR_ACTIVE_PRIMARY_DARKER: Vec4 = vec4(0.063, 0.435, 0.682, 1.0);
/// #138808
pub const COLOR_FG_ACCEPT_GREEN:       Vec4 = vec4(0.074, 0.533, 0.031, 1.0);
/// #F0FFF0
pub const COLOR_BG_ACCEPT_GREEN:       Vec4 = vec4(0.941, 1.0, 0.941, 1.0);
/// #B3B3B3
pub const COLOR_FG_DISABLED:           Vec4 = vec4(0.7, 0.7, 0.7, 1.0);
/// #E0E0E0
pub const COLOR_BG_DISABLED:           Vec4 = vec4(0.878, 0.878, 0.878, 1.0);
/// #DC0005
pub const COLOR_FG_DANGER_RED:         Vec4 = vec4(0.863, 0.0, 0.02, 1.0);
/// #FFF0F0
pub const COLOR_BG_DANGER_RED:         Vec4 = vec4(1.0, 0.941, 0.941, 1.0);
/// #572DCC
pub const COLOR_ROBRIX_PURPLE:         Vec4 = vec4(0.341, 0.176, 0.8, 1.0);
/// #05CDC7
pub const COLOR_ROBRIX_CYAN:           Vec4 = vec4(0.031, 0.804, 0.78, 1.0);
/// #FF0000
pub const COLOR_UNREAD_BADGE_MENTIONS: Vec4 = vec4(1.0, 0.0, 0.0, 1.0);
/// #572DCC
pub const COLOR_UNREAD_BADGE_MARKED:   Vec4 = COLOR_ROBRIX_CYAN;
/// #AAAAAA
pub const COLOR_UNREAD_BADGE_MESSAGES: Vec4 = vec4(0.667, 0.667, 0.667, 1.0);
/// #FF6e00
pub const COLOR_UNKNOWN_ROOM_AVATAR:   Vec4 = vec4(1.0, 0.431, 0.0, 1.0);
/// #888888
pub const COLOR_MESSAGE_NOTICE_TEXT:   Vec4 = vec4(0.5, 0.5, 0.5, 1.0);
/// #953800
pub const COLOR_TEXT_WARNING_NOT_FOUND: Vec4 = vec4(0.584, 0.219, 0.0, 1.0);
/// #F0F5FF
pub const COLOR_BG_PREVIEW:            Vec4 = vec4(0.941, 0.961, 1.0, 1.0);
/// #CDEDDF
pub const COLOR_BG_PREVIEW_HOVER:      Vec4 = vec4(0.804, 0.929, 0.875, 1.0);

/// Applies positive (green) button styling to the given button.
pub fn apply_positive_button_style(cx: &mut Cx, button: &mut ButtonRef) {
    script_apply_eval!(cx, button, {
        draw_bg +: {
            border_color: mod.widgets.COLOR_FG_ACCEPT_GREEN,
            color: mod.widgets.COLOR_BG_ACCEPT_GREEN,
            color_hover: #D4EED4,
            color_down: #B8E0B8,
        }
        draw_text +: {
            color: mod.widgets.COLOR_FG_ACCEPT_GREEN,
            color_hover: mod.widgets.COLOR_FG_ACCEPT_GREEN,
            color_down: mod.widgets.COLOR_FG_ACCEPT_GREEN,
        }
        draw_icon +: {
            color: mod.widgets.COLOR_FG_ACCEPT_GREEN,
        }
    });
}

/// Applies negative (red) button styling to the given button.
pub fn apply_negative_button_style(cx: &mut Cx, button: &mut ButtonRef) {
    script_apply_eval!(cx, button, {
        draw_bg +: {
            border_color: mod.widgets.COLOR_FG_DANGER_RED,
            color: mod.widgets.COLOR_BG_DANGER_RED,
            color_hover: #F0D4D4,
            color_down: #E0B8B8,
        }
        draw_text +: {
            color: mod.widgets.COLOR_FG_DANGER_RED,
            color_hover: mod.widgets.COLOR_FG_DANGER_RED,
            color_down: mod.widgets.COLOR_FG_DANGER_RED,
        }
        draw_icon +: {
            color: mod.widgets.COLOR_FG_DANGER_RED,
        }
    });
}

/// Applies neutral (gray) button styling to the given button.
pub fn apply_neutral_button_style(cx: &mut Cx, button: &mut ButtonRef) {
    script_apply_eval!(cx, button, {
        draw_bg +: {
            border_color: mod.widgets.COLOR_BG_DISABLED,
            color: mod.widgets.COLOR_SECONDARY,
            color_hover: #D0D0D0,
            color_down: #C0C0C0,
        }
        draw_text +: {
            color: mod.widgets.COLOR_TEXT,
            color_hover: mod.widgets.COLOR_TEXT,
            color_down: mod.widgets.COLOR_TEXT,
        }
        draw_icon +: {
            color: mod.widgets.COLOR_TEXT,
        }
    });
}

/// Applies the primary (blue) button styling to the given button.
pub fn apply_primary_button_style(cx: &mut Cx, button: &mut ButtonRef) {
    script_apply_eval!(cx, button, {
        draw_bg +: {
            color: mod.widgets.COLOR_ACTIVE_PRIMARY,
            color_hover: mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER,
            color_down: #0C5DAA,
            border_color: #0000,
            border_color_hover: #0000,
            border_color_down: #0000,
        }
        draw_text +: {
            color: mod.widgets.COLOR_PRIMARY,
            color_hover: mod.widgets.COLOR_PRIMARY,
            color_down: mod.widgets.COLOR_PRIMARY,
        }
        draw_icon +: {
            color: mod.widgets.COLOR_PRIMARY,
        }
    });
}
