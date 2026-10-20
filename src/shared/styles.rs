use makepad_widgets::*;

script_mod! {

    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ICON_ADD              = crate_resource("self://resources/icons/add.svg")
    mod.widgets.ICON_ADD_REACTION     = crate_resource("self://resources/icons/add_reaction.svg")
    mod.widgets.ICON_ADD_PHOTO        = crate_resource("self://resources/icons/add_photo.svg")
    mod.widgets.ICON_ADD_USER         = crate_resource("self://resources/icons/add_user.svg")
    mod.widgets.ICON_ADD_WALLET       = crate_resource("self://resources/icons/add_wallet.svg")
    mod.widgets.ICON_ARROW_LEFT       = crate_resource("self://resources/icons/arrow_left.svg")
    mod.widgets.ICON_ARROW_RIGHT      = crate_resource("self://resources/icons/arrow_right.svg")
    mod.widgets.ICON_FORBIDDEN        = crate_resource("self://resources/icons/forbidden.svg")
    mod.widgets.ICON_CHECKMARK        = crate_resource("self://resources/icons/checkmark.svg")
    mod.widgets.ICON_CHEVRON_DOWN     = crate_resource("self://resources/icons/chevron_down.svg")
    mod.widgets.ICON_CHEVRON_UP       = crate_resource("self://resources/icons/chevron_up.svg")
    mod.widgets.ICON_CLOSE            = crate_resource("self://resources/icons/close.svg")
    mod.widgets.ICON_CLOSE_THIN       = crate_resource("self://resources/icons/close_thin.svg")
    mod.widgets.ICON_CLOUD_CHECKMARK  = crate_resource("self://resources/icons/cloud_checkmark.svg")
    mod.widgets.ICON_CLOUD_OFFLINE    = crate_resource("self://resources/icons/cloud_offline.svg")
    mod.widgets.ICON_ROTATE_CW        = crate_resource("self://resources/icons/rotate_right.svg")
    mod.widgets.ICON_ROTATE_CW_THIN   = crate_resource("self://resources/icons/rotate_right_thin.svg")
    mod.widgets.ICON_COPY             = crate_resource("self://resources/icons/copy.svg")
    mod.widgets.ICON_DOWNLOAD         = crate_resource("self://resources/icons/download.svg")
    mod.widgets.ICON_DOWNLOAD_THIN    = crate_resource("self://resources/icons/download_thin.svg")
    mod.widgets.ICON_EDIT             = crate_resource("self://resources/icons/edit.svg")
    mod.widgets.ICON_EXTERNAL_LINK    = crate_resource("self://resources/icons/external_link.svg")
    mod.widgets.ICON_IMPORT           = crate_resource("self://resources/icons/import.svg")
    mod.widgets.ICON_HIERARCHY        = crate_resource("self://resources/icons/hierarchy.svg")
    mod.widgets.ICON_HOME             = crate_resource("self://resources/icons/home.svg")
    mod.widgets.ICON_HTML_FILE        = crate_resource("self://resources/icons/html_file.svg")
    mod.widgets.ICON_INFO             = crate_resource("self://resources/icons/info.svg")
    mod.widgets.ICON_INVITE           = crate_resource("self://resources/icons/invite.svg")
    mod.widgets.ICON_JOIN_ROOM        = crate_resource("self://resources/icons/join_room.svg")
    mod.widgets.ICON_JUMP             = crate_resource("self://resources/icons/go_back.svg")
    mod.widgets.ICON_LOCATION_PIN     = crate_resource("self://resources/icons/location-pin.svg")
    mod.widgets.ICON_LOGOUT           = crate_resource("self://resources/icons/logout.svg")
    mod.widgets.ICON_MEMBERS          = crate_resource("self://resources/icons/members.svg")
    mod.widgets.ICON_LINK             = crate_resource("self://resources/icons/link.svg")
    mod.widgets.ICON_PIN              = crate_resource("self://resources/icons/pin.svg")
    mod.widgets.ICON_UNPIN            = crate_resource("self://resources/icons/unpin.svg")
    mod.widgets.ICON_REPLY            = crate_resource("self://resources/icons/reply.svg")
    mod.widgets.ICON_REPLY_IN_THREAD  = crate_resource("self://resources/icons/double_chat.svg")
    mod.widgets.ICON_SEARCH           = crate_resource("self://resources/icons/search.svg")
    mod.widgets.ICON_SEND             = crate_resource("self://resources/icons/send.svg")
    mod.widgets.ICON_SEND_ENCRYPTED   = crate_resource("self://resources/icons/send_encrypted.svg")
    mod.widgets.ICON_SEND_UNENCRYPTED = crate_resource("self://resources/icons/send_unencrypted.svg")
    mod.widgets.ICON_SETTINGS         = crate_resource("self://resources/icons/settings.svg")
    mod.widgets.ICON_SHARE            = crate_resource("self://resources/icons/share.svg")
    mod.widgets.ICON_SHARE_THIN       = crate_resource("self://resources/icons/share_thin.svg")
    mod.widgets.ICON_SQUARES          = crate_resource("self://resources/icons/squares_filled.svg")
    mod.widgets.ICON_TOMBSTONE        = crate_resource("self://resources/icons/tombstone.svg")
    mod.widgets.ICON_TRASH            = crate_resource("self://resources/icons/trash.svg")
    mod.widgets.ICON_TRIANGLE_DOWN    = crate_resource("self://resources/icons/triangle_down_fill.svg")
    mod.widgets.ICON_TRIANGLE_UP      = crate_resource("self://resources/icons/triangle_up_fill.svg")
    mod.widgets.ICON_CARET_LEFT       = crate_resource("self://resources/icons/caret_left.svg")
    mod.widgets.ICON_CARET_RIGHT      = crate_resource("self://resources/icons/caret_right.svg")
    mod.widgets.ICON_CARET_UP         = crate_resource("self://resources/icons/caret_up.svg")
    mod.widgets.ICON_CARET_DOWN       = crate_resource("self://resources/icons/caret_down.svg")
    mod.widgets.ICON_UPLOAD           = crate_resource("self://resources/icons/upload.svg")
    mod.widgets.ICON_VIEW_SOURCE      = crate_resource("self://resources/icons/view_source.svg")
    mod.widgets.ICON_WARNING          = crate_resource("self://resources/icons/warning.svg")
    mod.widgets.ICON_ZOOM_IN          = crate_resource("self://resources/icons/zoom_in.svg")
    mod.widgets.ICON_ZOOM_OUT         = crate_resource("self://resources/icons/zoom_out.svg")
    mod.widgets.ICON_ZOOM_TO_FIT      = crate_resource("self://resources/icons/zoom_to_fit.svg")
    mod.widgets.ICON_ADD_ATTACHMENT   = crate_resource("self://resources/icons/add_attachment.svg")
    mod.widgets.ICON_FILE             = crate_resource("self://resources/icons/file.svg")

    mod.widgets.IMG_APP_LOGO = crate_resource("self://resources/robrix_logo_alpha.png")

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
    mod.widgets.REDACTED_MESSAGE_FONT_SIZE = 10

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
    mod.widgets.COLOR_BG_ACCEPT_GREEN_HOVER = #xD4EED4
    mod.widgets.COLOR_BG_ACCEPT_GREEN_DOWN = #xB8E0B8
    mod.widgets.COLOR_FG_DANGER_RED = #DC0005
    mod.widgets.COLOR_FG_DANGER_RED_HOVER = #xB00004
    mod.widgets.COLOR_FG_DANGER_RED_DOWN = #x930003
    mod.widgets.COLOR_BG_DANGER_RED = #FFF0F0
    mod.widgets.COLOR_BG_DANGER_RED_HOVER = #xF0D4D4
    mod.widgets.COLOR_BG_DANGER_RED_DOWN = #xE0B8B8
    mod.widgets.COLOR_FG_DISABLED = #B3B3B3
    mod.widgets.COLOR_BG_DISABLED = #E0E0E0
    mod.widgets.COLOR_INFO_BLUE = #0f88fe
    mod.widgets.COLOR_BG_INFO_BLUE = #xE8F4FD
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
    mod.widgets.COLOR_SECONDARY_HOVER = #xD0D0D0
    mod.widgets.COLOR_SECONDARY_DOWN = #xC0C0C0
    mod.widgets.COLOR_SECONDARY_DARKER = #C8C8C8

    // What a list item, e.g., a room, message, or menu item, darkens to on hover or press.
    mod.widgets.COLOR_LIST_ITEM_BG_HOVER = #xEFF3F8

    mod.widgets.COLOR_ACTIVE_PRIMARY = #0f88fe

    mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER = #106fcc

    mod.widgets.COLOR_ACTIVE_PRIMARY_DOWN = #x0C5DAA

    mod.widgets.COLOR_BG_PREVIEW = #F0F5FF
    // Looks like COLOR_BG_PREVIEW atop white, but it also darkens a hovered or highlighted message behind it.
    mod.widgets.COLOR_BG_PREVIEW_TRANSLUCENT = #x1060FF10

    // The light lavender of the room action buttons and room pane headers.
    mod.widgets.COLOR_BG_LAVENDER = #xEDE8FD
    mod.widgets.COLOR_BG_LAVENDER_HOVER = #xE8E1FA
    mod.widgets.COLOR_BG_LAVENDER_DOWN = #xE1D7F7

    // The darker lavender of an "active"/selected room action button,
    // a button whose corresponding room pane is currently shown.
    mod.widgets.COLOR_BG_LAVENDER_SELECTED = #x7C5BD7
    mod.widgets.COLOR_BG_LAVENDER_SELECTED_HOVER = #x7451D5
    mod.widgets.COLOR_BG_LAVENDER_SELECTED_DOWN = #x6B46D2

    mod.widgets.COLOR_BG_PREVIEW_HOVER = #CDEDDF
    mod.widgets.COLOR_BG_PREVIEW_DOWN = #xA8DBBF

    // A thread's summary beneath its root message, whose reply count color the threads list uses too.
    mod.widgets.COLOR_THREAD_SUMMARY_BG = #FFF4E5
    mod.widgets.COLOR_THREAD_SUMMARY_BG_HOVER = #FFEACC
    mod.widgets.COLOR_THREAD_SUMMARY_BORDER = #E8C99A
    mod.widgets.COLOR_THREAD_SUMMARY_REPLY_COUNT = #A35A00

    mod.widgets.COLOR_AVATAR_BG = #52b2ac
    mod.widgets.COLOR_UNKNOWN_ROOM_AVATAR = #xFF6E00

    mod.widgets.COLOR_AVATAR_BG_IDLE = #d8d8d8


    mod.widgets.COLOR_UNREAD_BADGE_MENTIONS = #xFF1133;


    mod.widgets.COLOR_UNREAD_BADGE_MARKED = (mod.widgets.COLOR_ROBRIX_CYAN);
    mod.widgets.COLOR_UNREAD_BADGE_MESSAGES = #AAAAAA


    mod.widgets.COLOR_TEXT_IDLE = #d8d8d8


    mod.widgets.COLOR_TEXT = #1C274C
    mod.widgets.COLOR_TEXT_INPUT_IDLE = #d8d8d8

    mod.widgets.COLOR_TRANSPARENT = #00000000

    mod.widgets.COLOR_WARNING = #fcdb03

    mod.widgets.COLOR_LINK_HOVER = #21B070
    mod.widgets.COLOR_LINK = #x0000EE

    mod.widgets.COLOR_LINK_PILL_BG = #x000000
    mod.widgets.COLOR_LINK_PILL_BG_MENTION = #xD91B38
    mod.widgets.COLOR_SPOILER_REASON_TEXT = #x999999

    // A message that mentions the current user (or @room).
    mod.widgets.COLOR_MENTION_HIGHLIGHT_BG = #xFFFFD1
    mod.widgets.COLOR_MENTION_HIGHLIGHT_BG_HOVER = #xFFF9C2
    mod.widgets.COLOR_MENTION_HIGHLIGHT_BAR = #xFFD54F

    mod.widgets.COLOR_REACTION_BG = #xF7F9FA
    mod.widgets.COLOR_REACTION_BORDER = #xB6BABF
    mod.widgets.COLOR_REACTION_BG_INCLUDES_USER = #xE3F7ED
    mod.widgets.COLOR_REACTION_BORDER_INCLUDES_USER = #x009978

    // The latest message preview in a rooms list entry.
    mod.widgets.COLOR_ROOM_PREVIEW_TEXT = #x444444
    mod.widgets.COLOR_ROOM_PREVIEW_CODE_BG = #xEDEDED
    mod.widgets.COLOR_ROOM_PREVIEW_CODE_BG_SELECTED = #x4D4D4D

    mod.widgets.COLOR_ROOM_TAB_BUTTON_BG_HOVER = #x0000001F
    mod.widgets.COLOR_ROOM_TAB_BUTTON_BG_DOWN = #x00000033
    mod.widgets.COLOR_ROOM_TAB_BUTTON_BG_ACTIVE_HOVER = #xFFFFFF33
    mod.widgets.COLOR_ROOM_TAB_BUTTON_BG_ACTIVE_DOWN = #xFFFFFF4D

    mod.widgets.COLOR_ROOM_ACTION_BUTTON_BG_HOVER = #x00000011
    mod.widgets.COLOR_ROOM_ACTION_BUTTON_BG_DOWN = #x00000022
    mod.widgets.COLOR_HEADER_SHADOW = #x00000055

    mod.widgets.COLOR_PANE_DIVIDER = #x4D4D4D

    mod.widgets.COLOR_SPEECH_BUTTON_ICON = #x555555
    mod.widgets.COLOR_SPEECH_BUTTON_BG_HOVER = #xE0E8F0
    mod.widgets.COLOR_SPEECH_BUTTON_BG_DOWN = #xD0D8E8
    mod.widgets.COLOR_SPEECH_BUTTON_BG_RECORDING = #x141414
    mod.widgets.COLOR_SPEECH_BUTTON_BG_RECORDING_HOVER = #x404040

    mod.widgets.COLOR_OVERLAY_BG = #x000000BB

    mod.widgets.COLOR_TOOLTIP_BG = vec4(0.26, 0.30, 0.333, 1.0)
    mod.widgets.COLOR_TOOLTIP_TEXT = #xFFFFFF

    // Text tiers, from darkest to lightest: TITLE and BODY, MUTED, SUBTLE, FAINT, HINT.
    mod.widgets.COLOR_TEXT_TITLE = #x000000
    mod.widgets.COLOR_TEXT_BODY = #x000000
    mod.widgets.COLOR_TEXT_MUTED = #x666666
    mod.widgets.COLOR_TEXT_SUBTLE = #x737373
    mod.widgets.COLOR_TEXT_FAINT = #x888888
    mod.widgets.COLOR_TEXT_HINT = #x8C8C8C
    mod.widgets.COLOR_TEXT_ON_ACCENT = #xFFFFFF
    mod.widgets.COLOR_TEXT_ON_ACCENT_MUTED = #xFFFFFFD9

    mod.widgets.COLOR_TEXT_INPUT_PLACEHOLDER = #xBBBBBB
    mod.widgets.COLOR_TEXT_INPUT_PLACEHOLDER_HOVER = #x999999
    mod.widgets.COLOR_TEXT_INPUT_PLACEHOLDER_FOCUS = #x999999
    mod.widgets.COLOR_MODAL_INPUT_TEXT = #x000000

    mod.widgets.COLOR_SHADOW_SOFT = #x00000033
    mod.widgets.COLOR_SHADOW = #x00000055
    mod.widgets.COLOR_SHADOW_STRONG = #x00000066
    mod.widgets.COLOR_LOADING_PANE_OVERLAY_BG = #x000000B2
    mod.widgets.COLOR_CONTEXT_MENU_OVERLAY_BG = #x0000004D

    mod.widgets.COLOR_LABELED_DIVIDER = #xC8C8C8

    mod.widgets.COLOR_FLAT_ICON_BUTTON_BG_HOVER = #x00000015
    mod.widgets.COLOR_FLAT_ICON_BUTTON_BG_DOWN = #x00000025
    mod.widgets.COLOR_NAV_HISTORY_BUTTON_BG_HOVER = #x00000014
    mod.widgets.COLOR_NAV_HISTORY_BUTTON_BG_DOWN = #x00000026
    mod.widgets.COLOR_PREVIEW_TOGGLE_BUTTON_BORDER = #xCCCCCC

    mod.widgets.SEND_STATUS_ICON_COLOR = #8C8C8C
    mod.widgets.COLOR_VERIFICATION_BADGE_UNKNOWN = #x888888

    mod.widgets.COLOR_LIST_ITEM_BG_FOCUS = #xB6D3F2
    mod.widgets.COLOR_LIST_ROW_TEXT_MUTED = #x555555
    mod.widgets.COLOR_CONTEXT_MENU_ITEM_BG_DOWN = #xDCDCDC
    mod.widgets.COLOR_CONTEXT_MENU_BORDER = #x888888
    mod.widgets.COLOR_ROOM_INPUT_MENU_ITEM_BG_DOWN = #xD0D8E8

    mod.widgets.COLOR_HEADER_FG = #F;
    mod.widgets.COLOR_HEADER_BG = (mod.widgets.COLOR_ROBRIX_PURPLE); // the purple color from the Robrix logo

    mod.widgets.COLOR_WINDOW_CLEAR = #xFFFFFF00
    mod.widgets.COLOR_CAPTION_BAR_BG = #xF3F3F3
    mod.widgets.COLOR_SCROLL_BAR_HANDLE = #x00000040
    mod.widgets.COLOR_SCROLL_BAR_HANDLE_HOVER = #x00000060
    mod.widgets.COLOR_SCROLL_BAR_HANDLE_DRAG = #x00000080

    mod.widgets.COLOR_TIMELINE_ITEM_BG_HIGHLIGHT = #xC5D6FA
    mod.widgets.COLOR_TIMELINE_LOADING_BG = #xDAF5E5F0
    mod.widgets.COLOR_READ_MARKER = #xeb2733
    mod.widgets.COLOR_MESSAGE_CODE_BG = #x00000012
    mod.widgets.COLOR_MESSAGE_QUOTE_BG = #x00000012
    mod.widgets.COLOR_TYPING_NOTICE_BG = #xE8F4FF
    mod.widgets.COLOR_FAILED_SEND_BANNER_BORDER = #xE34B4F
    mod.widgets.COLOR_IMAGE_PLACEHOLDER_BG = #xDDDDDD
    mod.widgets.COLOR_LINK_PILL_TEXT = #xFFFFFF
    mod.widgets.COLOR_LINK_PILL_AVATAR_BG = #xFFFFFF

    mod.widgets.COLOR_REACTION_HOVER_TINT = #xFEF65B

    mod.widgets.COLOR_INPUT_BAR_HEADER_TEXT = #x222222
    mod.widgets.COLOR_INPUT_BAR_ADD_BUTTON_BG_HOVER = #xE0E8F0
    mod.widgets.COLOR_INPUT_BAR_ADD_BUTTON_BG_DOWN = #xD0D8E8
    mod.widgets.COLOR_SPEECH_BUTTON_LEVEL_BARS = #xFFFFFF

    mod.widgets.COLOR_UNREAD_BADGE_MENTIONS_FADE = #xFFC8B0
    mod.widgets.COLOR_JUMP_TO_BOTTOM_BG = #xEDEDEDCE
    mod.widgets.COLOR_JUMP_TO_BOTTOM_BG_HOVER = #xD0D0D0CE

    mod.widgets.COLOR_POPUP_TEXT = #x000000
    mod.widgets.COLOR_POPUP_BORDER = #x000000
    mod.widgets.COLOR_POPUP_PROGRESS_BAR = #x00000080
    mod.widgets.COLOR_POPUP_CLOSE_BG = #x00000014
    mod.widgets.COLOR_POPUP_CLOSE_BG_HOVER = #x00000022
    mod.widgets.COLOR_POPUP_CLOSE_BG_DOWN = #x0000002E
    mod.widgets.COLOR_POPUP_CLOSE_BORDER = #x00000020
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_HOVER = #x0000002C
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_DOWN = #x00000038
    mod.widgets.COLOR_POPUP_CLOSE_ICON = #x000000D0

    mod.widgets.COLOR_DOCK_TAB_FG = #x000000
    mod.widgets.COLOR_DOCK_TAB_BG = #xE1EEFA
    mod.widgets.COLOR_DOCK_TAB_BG_HOVER = #xC8DDEF
    mod.widgets.COLOR_DOCK_TAB_BG_ACTIVE = #x0660FE
    mod.widgets.COLOR_DOCK_TAB_BG_ACTIVE_BOTTOM = #x398CFE
    mod.widgets.COLOR_DOCK_TAB_BAR_BG = (mod.widgets.COLOR_PRIMARY * 0.96)
    mod.widgets.COLOR_DOCK_TAB_BAR_BG_DRAG = #x000000
    mod.widgets.COLOR_DOCK_DROP_PREVIEW_MIX = #xFFFFFF00
    mod.widgets.COLOR_PANE_GRAB_HANDLE_DOTS = vec4(1.0, 1.0, 1.0, 0.92)

    mod.widgets.COLOR_IMAGE_VIEWER_FG = #xFFFFFF
    mod.widgets.COLOR_IMAGE_VIEWER_FG_MUTED = #xFFFFFFBF
    mod.widgets.COLOR_IMAGE_VIEWER_BUTTON_BG_HOVER = #xFFFFFF26
    mod.widgets.COLOR_IMAGE_VIEWER_BUTTON_BG_DOWN = #xFFFFFF40
    mod.widgets.COLOR_IMAGE_VIEWER_META_BORDER = #xFFFFFF29
    mod.widgets.COLOR_IMAGE_VIEWER_META_SHADOW = #x00000047

    mod.widgets.COLOR_WELCOME_BG_TOP = #xF3EFFD
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_BG_RIGHT = #x1F7BF2
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_SHADOW = #x572DCC40
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_ICON_BG = #xFFFFFF2E
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_BUTTON_BG = #xFFFFFF
    mod.widgets.COLOR_WELCOME_TIPS_PANEL_BORDER = #x00000010
    mod.widgets.COLOR_WELCOME_TIPS_PANEL_SHADOW = #x1C274C14
    mod.widgets.COLOR_WELCOME_TIP_BODY_TEXT = #x5C6478
    mod.widgets.COLOR_WELCOME_TIP_HINT_BG_WORKSPACE = #xF4F1FE
    mod.widgets.COLOR_WELCOME_TIP_HINT_TEXT_WORKSPACE = #x4A2BA8
    mod.widgets.COLOR_WELCOME_TIP_BADGE_BG_SETTINGS = #xE3F0FF
    mod.widgets.COLOR_WELCOME_TIP_HINT_BG_SETTINGS = #xEEF6FF
    mod.widgets.COLOR_WELCOME_TIP_HINT_TEXT_SETTINGS = #x0B5CAB
    mod.widgets.COLOR_WELCOME_TIP_BADGE_BG_NAVIGATION = #xDDF6F5
    mod.widgets.COLOR_WELCOME_TIP_ICON_NAVIGATION = #x039E99
    mod.widgets.COLOR_WELCOME_TIP_HINT_BG_NAVIGATION = #xEAF9F8
    mod.widgets.COLOR_WELCOME_TIP_HINT_TEXT_NAVIGATION = #x02706C
    mod.widgets.COLOR_WELCOME_FOOTER_TEXT = #x6B7287

    // A lighter version of the main Robrix purple color
    mod.widgets.COLOR_LOGIN_BG_TOP = #EEEAFA
    mod.widgets.COLOR_LOGIN_BG_BOTTOM = #FFFFFF

    mod.widgets.COLOR_SPACE_LOBBY_NAME_TEXT = #x1A1A1A
    mod.widgets.COLOR_SPACE_LOBBY_TREE_LINES = #x888888

    mod.widgets.COLOR_INVITE_SCREEN_TOPIC_TEXT = #x444444

    mod.widgets.COLOR_MODAL_TIP_TEXT = #xAAAAAA
    mod.widgets.COLOR_TSP_FIELD_CAPTION_TEXT = #x777777
    mod.widgets.COLOR_NO_PREVIEW_ICON_DISC = #xD6E6FF
    mod.widgets.COLOR_NO_PREVIEW_ICON_PAGE = #xFFFFFF
    mod.widgets.COLOR_NO_PREVIEW_ICON_ACCENT = #x7AA8E8

    mod.widgets.COLOR_SETTINGS_LIST_BG = #xF6F8F9

    // Code view syntax colors, one per kind of code token.
    mod.widgets.COLOR_CODE_TEXT = #x24292E
    mod.widgets.COLOR_CODE_WHITESPACE = #x6A737D
    mod.widgets.COLOR_CODE_DELIMITER = #x24292E
    mod.widgets.COLOR_CODE_DELIMITER_HIGHLIGHT = #x005CC5
    mod.widgets.COLOR_CODE_ERROR_DECORATION = #xCB2431
    mod.widgets.COLOR_CODE_WARNING_DECORATION = #xB08800
    mod.widgets.COLOR_CODE_BRANCH_KEYWORD = #xD73A49
    mod.widgets.COLOR_CODE_CONSTANT = #x005CC5
    mod.widgets.COLOR_CODE_IDENTIFIER = #x24292E
    mod.widgets.COLOR_CODE_LOOP_KEYWORD = #xD73A49
    mod.widgets.COLOR_CODE_NUMBER = #x005CC5
    mod.widgets.COLOR_CODE_OTHER_KEYWORD = #xD73A49
    mod.widgets.COLOR_CODE_PUNCTUATOR = #x24292E
    mod.widgets.COLOR_CODE_STRING = #x22863A
    mod.widgets.COLOR_CODE_FUNCTION = #x6F42C1
    mod.widgets.COLOR_CODE_MACRO_IDENTIFIER = #x6F42C1
    mod.widgets.COLOR_CODE_ATTRIBUTE = #x6F42C1
    mod.widgets.COLOR_CODE_TYPENAME = #xE36209
    mod.widgets.COLOR_CODE_COMMENT = #x6A737D

    // The close button on an error, info, or success popup's colored background.
    mod.widgets.COLOR_POPUP_CLOSE_BG_ON_COLOR = #xFFFFFF22
    mod.widgets.COLOR_POPUP_CLOSE_BG_ON_COLOR_HOVER = #xFFFFFF30
    mod.widgets.COLOR_POPUP_CLOSE_BG_ON_COLOR_DOWN = #xFFFFFF42
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_COLOR = #xFFFFFF2C
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_COLOR_HOVER = #xFFFFFF38
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_COLOR_DOWN = #xFFFFFF4A
    mod.widgets.COLOR_POPUP_CLOSE_ICON_ON_COLOR = #xFFFFFFFA
    mod.widgets.COLOR_POPUP_WARNING_ICON = #x000000


    // Use an even value for this, not odd, such that it can be divided in half,
    // which is needed when calculating the value of other widgets that scale with this.
    mod.widgets.NAVIGATION_TAB_BAR_SIZE = 54
    mod.widgets.NAVIGATION_TAB_BAR_AVATAR_SIZE = 40
    mod.widgets.NAVIGATION_TAB_BAR_AVATAR_FONT_SIZE = (mod.widgets.NAVIGATION_TAB_BAR_AVATAR_SIZE * 0.4)


    mod.widgets.COLOR_NAVIGATION_TAB_FG = (mod.widgets.COLOR_TEXT)
    mod.widgets.COLOR_NAVIGATION_TAB_FG_HOVER = (mod.widgets.COLOR_TEXT)
    mod.widgets.COLOR_NAVIGATION_TAB_FG_ACTIVE = (mod.widgets.COLOR_TEXT)
    mod.widgets.COLOR_NAVIGATION_TAB_BG = (mod.widgets.COLOR_SECONDARY)
    mod.widgets.COLOR_NAVIGATION_TAB_BG_HOVER = (mod.widgets.COLOR_SECONDARY * 0.85)
    mod.widgets.COLOR_NAVIGATION_TAB_BG_ACTIVE = #9

    mod.widgets.COLOR_IMAGE_VIEWER_BACKGROUND = #333333CC // 80% Opacity

    mod.widgets.COLOR_IMAGE_VIEWER_META_BACKGROUND = #18181BCC

    // Ensure all settings buttons have a consistent height
    mod.widgets.SETTINGS_BUTTON_HEIGHT = 40

    // The font size used for regular (non-title, non-subsection) text
    // within any settings screen (e.g., dropdown labels, radio/toggle
    // labels, inline helper text inside a control).
    mod.widgets.SETTINGS_REGULAR_FONT_SIZE = 11
    mod.widgets.SETTINGS_REGULAR_TEXT_STYLE = theme.font_regular {
        font_size: (mod.widgets.SETTINGS_REGULAR_FONT_SIZE),
    }


    // Set the position for scroll bars in the multiline text input
    mod.widgets.TEXT_INPUT_SCROLL_BAR_INSET = Inset{top: 3, right: 3, bottom: 3}
}


/// Applies positive (green) button styling to the given button.
pub fn apply_positive_button_style(cx: &mut Cx, button: &mut ButtonRef) {
    script_apply_eval!(cx, button, {
        draw_bg +: {
            border_color: mod.widgets.COLOR_FG_ACCEPT_GREEN,
            color: mod.widgets.COLOR_BG_ACCEPT_GREEN,
            color_hover: mod.widgets.COLOR_BG_ACCEPT_GREEN_HOVER,
            color_down: mod.widgets.COLOR_BG_ACCEPT_GREEN_DOWN,
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
            color_hover: mod.widgets.COLOR_BG_DANGER_RED_HOVER,
            color_down: mod.widgets.COLOR_BG_DANGER_RED_DOWN,
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
            color_hover: mod.widgets.COLOR_SECONDARY_HOVER,
            color_down: mod.widgets.COLOR_SECONDARY_DOWN,
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
            color_down: mod.widgets.COLOR_ACTIVE_PRIMARY_DOWN,
            border_color: mod.widgets.COLOR_TRANSPARENT,
            border_color_hover: mod.widgets.COLOR_TRANSPARENT,
            border_color_down: mod.widgets.COLOR_TRANSPARENT,
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

/// Applies danger (solid red) button styling to the given button.
pub fn apply_danger_button_style(cx: &mut Cx, button: &mut ButtonRef) {
    script_apply_eval!(cx, button, {
        draw_bg +: {
            border_color: mod.widgets.COLOR_FG_DANGER_RED,
            color: mod.widgets.COLOR_FG_DANGER_RED,
            color_hover: mod.widgets.COLOR_FG_DANGER_RED_HOVER,
            color_down: mod.widgets.COLOR_FG_DANGER_RED_DOWN,
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

/// Sets the background color of the given view, updating it in place if it's already drawn.
pub fn set_view_bg_color(cx: &mut Cx, view: &ViewRef, color: Vec4f) {
    let Some(mut view) = view.borrow_mut() else { return };
    view.draw_bg.set_dyn_instance(cx, id!(color), &[color.x, color.y, color.z, color.w]);
    view.draw_bg.update_instance_area_value(cx, ids!(color));
}
