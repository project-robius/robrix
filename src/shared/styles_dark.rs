//! Dark mode overrides for the colors defined in `styles.rs`.

use makepad_widgets::*;

script_mod! {
    // Always use the full `mod.widgets.X` path here, since `use mod.widgets.*`
    // would only give us a copy of the light colors.

    mod.widgets.USERNAME_TEXT_COLOR = #xE3E3E3

    mod.widgets.COLOR_ROBRIX_PURPLE = #x572DCC

    mod.widgets.COLOR_ROBRIX_CYAN = #x05CDC7

    mod.widgets.TYPING_NOTICE_TEXT_COLOR = #xA9B4F5

    mod.widgets.MESSAGE_TEXT_COLOR = #xDEDEDE
    // notices (automated messages from bots) use a dimmer color
    mod.widgets.COLOR_MESSAGE_NOTICE_TEXT = #x9C9C9C

    mod.widgets.SMALL_STATE_TEXT_COLOR = #x9C9C9C

    mod.widgets.TIMESTAMP_TEXT_COLOR = #x999999

    mod.widgets.ROOM_NAME_TEXT_COLOR = #xEBEBEB

    mod.widgets.COLOR_META = #x505050

    mod.widgets.COLOR_DIVIDER = #xFFFFFF18

    mod.widgets.COLOR_DIVIDER_DARK = #xFFFFFF56

    mod.widgets.COLOR_FG_ACCEPT_GREEN = #x4AC25B
    mod.widgets.COLOR_BG_ACCEPT_GREEN_SOLID = #x138808
    mod.widgets.COLOR_BG_ACCEPT_GREEN = #x29392C
    mod.widgets.COLOR_BG_ACCEPT_GREEN_HOVER = #x2B3F2F
    mod.widgets.COLOR_BG_ACCEPT_GREEN_DOWN = #x2E4532
    mod.widgets.COLOR_FG_DANGER_RED = #xFF7B72
    mod.widgets.COLOR_BG_DANGER_RED_SOLID = #xDC0005
    mod.widgets.COLOR_BG_DANGER_RED_SOLID_HOVER = #xB00004
    mod.widgets.COLOR_BG_DANGER_RED_SOLID_DOWN = #x930003
    mod.widgets.COLOR_BG_DANGER_RED = #x412C2B
    mod.widgets.COLOR_BG_DANGER_RED_HOVER = #x4A2E2C
    mod.widgets.COLOR_BG_DANGER_RED_DOWN = #x532F2E
    mod.widgets.COLOR_BG_NEUTRAL = #x3A3A3A
    mod.widgets.COLOR_BG_NEUTRAL_HOVER = #x464646
    mod.widgets.COLOR_BG_NEUTRAL_DOWN = #x505050
    mod.widgets.COLOR_FG_DISABLED = #x6E6E6E
    mod.widgets.COLOR_BG_DISABLED = #x333333
    mod.widgets.COLOR_INFO_BLUE = #x0675DF
    mod.widgets.COLOR_FG_INFO_BLUE = #x5AABFF
    mod.widgets.COLOR_BG_INFO_BLUE = #x1C2533
    mod.widgets.COLOR_WARNING_YELLOW = #xF2CD13
    mod.widgets.COLOR_TEXT_WARNING_NOT_FOUND = #xF0954A

    mod.widgets.COLOR_SELECT_TEXT = #x3B8EFF4D

    mod.widgets.COLOR_PRIMARY = #x262626

    mod.widgets.COLOR_PRIMARY_DARKER = #x252525
    mod.widgets.COLOR_SECONDARY = #x1C1C1C
    mod.widgets.COLOR_SECONDARY_DARKER = #x505050

    // Modals, menus, popups, and other surfaces that float above the panes.
    mod.widgets.COLOR_BG_FLOATING = #x2E2E2E
    mod.widgets.COLOR_FLOATING_ITEM_BG_HOVER = #x3A3A3A
    mod.widgets.COLOR_FLOATING_ITEM_BG_SELECTED = #x2F3B52

    // What a list item, e.g., a room, message, or menu item, lightens to on hover or press.
    mod.widgets.COLOR_LIST_ITEM_BG_HOVER = #x2E2E2E

    mod.widgets.COLOR_ACTIVE_PRIMARY = #x0675DF

    mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER = #x5AABFF

    mod.widgets.COLOR_ACTIVE_PRIMARY_HOVER = #x0367C4
    mod.widgets.COLOR_ACTIVE_PRIMARY_DOWN = #x035AAE

    mod.widgets.COLOR_BG_PREVIEW = #x2B3240
    // Looks like COLOR_BG_PREVIEW atop a pane, but it also tints a hovered or highlighted message behind it.
    mod.widgets.COLOR_BG_PREVIEW_TRANSLUCENT = #x4D8DFF1F

    // The dark lavender of the room action buttons and room pane headers.
    mod.widgets.COLOR_BG_LAVENDER = #x342F42
    mod.widgets.COLOR_BG_LAVENDER_HOVER = #x3A334C
    mod.widgets.COLOR_BG_LAVENDER_DOWN = #x3F3657

    // The brighter lavender of an "active"/selected room action button,
    // a button whose corresponding room pane is currently shown.
    mod.widgets.COLOR_BG_LAVENDER_SELECTED = #x7C5BD7
    mod.widgets.COLOR_BG_LAVENDER_SELECTED_HOVER = #x7451D5
    mod.widgets.COLOR_BG_LAVENDER_SELECTED_DOWN = #x6B46D2

    mod.widgets.COLOR_BG_PREVIEW_HOVER = #x2A4236
    mod.widgets.COLOR_BG_PREVIEW_DOWN = #x264837

    // A thread's summary beneath its root message, whose reply count color the threads list uses too.
    mod.widgets.COLOR_THREAD_SUMMARY_BG = #x403528
    mod.widgets.COLOR_THREAD_SUMMARY_BG_HOVER = #x493A29
    mod.widgets.COLOR_THREAD_SUMMARY_BORDER = #x6E5638
    mod.widgets.COLOR_THREAD_SUMMARY_REPLY_COUNT = #xF2A65A

    mod.widgets.COLOR_AVATAR_BG = #x2B827C
    mod.widgets.COLOR_UNKNOWN_ROOM_AVATAR = #xC25400

    mod.widgets.COLOR_AVATAR_BG_IDLE = #x727272

    mod.widgets.COLOR_UNREAD_BADGE_MENTIONS = #xE71336

    mod.widgets.COLOR_UNREAD_BADGE_MARKED = (mod.widgets.COLOR_ROBRIX_CYAN)
    mod.widgets.COLOR_UNREAD_BADGE_MESSAGES = #x6E6E6E

    mod.widgets.COLOR_TEXT_IDLE = #x727272

    mod.widgets.COLOR_TEXT = #xDCE1EE
    mod.widgets.COLOR_TEXT_INPUT_IDLE = #x727272

    mod.widgets.COLOR_TRANSPARENT = #x00000000

    mod.widgets.COLOR_WARNING = #xF2CD13

    mod.widgets.COLOR_LINK_HOVER = #x3CCB86
    mod.widgets.COLOR_LINK = #x8AB4F8

    mod.widgets.COLOR_LINK_PILL_BG = #x484848
    mod.widgets.COLOR_LINK_PILL_BG_MENTION = #xD91B38
    mod.widgets.COLOR_SPOILER_REASON_TEXT = #x999999

    // A message that mentions the current user (or @room).
    mod.widgets.COLOR_MENTION_HIGHLIGHT_BG = #x352D19
    mod.widgets.COLOR_MENTION_HIGHLIGHT_BG_HOVER = #x393017
    mod.widgets.COLOR_MENTION_HIGHLIGHT_BAR = #xFFD54F

    mod.widgets.COLOR_REACTION_BG = #x303030
    mod.widgets.COLOR_REACTION_BORDER = #x505050
    mod.widgets.COLOR_REACTION_BG_INCLUDES_USER = #x273A35
    mod.widgets.COLOR_REACTION_BORDER_INCLUDES_USER = #x2EB88F

    // The latest message preview in a rooms list entry.
    mod.widgets.COLOR_ROOM_PREVIEW_TEXT = #xBDBDBD
    mod.widgets.COLOR_ROOM_PREVIEW_CODE_BG = #x3A3A3A
    mod.widgets.COLOR_ROOM_PREVIEW_CODE_BG_SELECTED = #x4D4D4D

    mod.widgets.COLOR_ROOM_TAB_BUTTON_BG_HOVER = #xFFFFFF1F
    mod.widgets.COLOR_ROOM_TAB_BUTTON_BG_DOWN = #xFFFFFF33
    mod.widgets.COLOR_ROOM_TAB_BUTTON_BG_ACTIVE_HOVER = #xFFFFFF33
    mod.widgets.COLOR_ROOM_TAB_BUTTON_BG_ACTIVE_DOWN = #xFFFFFF4D

    mod.widgets.COLOR_ROOM_ACTION_BUTTON_BG_HOVER = #xFFFFFF11
    mod.widgets.COLOR_ROOM_ACTION_BUTTON_BG_DOWN = #xFFFFFF22
    mod.widgets.COLOR_HEADER_SHADOW = #x00000080

    mod.widgets.COLOR_PANE_DIVIDER = #x141414
    mod.widgets.COLOR_PANE_DIVIDER_GRIP = #x666666

    mod.widgets.COLOR_SPEECH_BUTTON_ICON = #xB5B5B5
    mod.widgets.COLOR_SPEECH_BUTTON_BG_HOVER = #x333333
    mod.widgets.COLOR_SPEECH_BUTTON_BG_DOWN = #x3D3D3D
    mod.widgets.COLOR_SPEECH_BUTTON_BG_RECORDING = #xE6E6E6
    mod.widgets.COLOR_SPEECH_BUTTON_BG_RECORDING_HOVER = #xCCCCCC

    mod.widgets.COLOR_OVERLAY_BG = #x000000BB

    mod.widgets.COLOR_TOOLTIP_BG = #x454545
    mod.widgets.COLOR_TOOLTIP_TEXT = #xFFFFFF

    // Text tiers, from lightest to darkest: TITLE and BODY, MUTED, SUBTLE, FAINT, HINT.
    mod.widgets.COLOR_TEXT_TITLE = #xEBEBEB
    mod.widgets.COLOR_TEXT_BODY = #xEBEBEB
    mod.widgets.COLOR_TEXT_MUTED = #xADADAD
    mod.widgets.COLOR_TEXT_SUBTLE = #xA2A2A2
    mod.widgets.COLOR_TEXT_FAINT = #x9C9C9C
    mod.widgets.COLOR_TEXT_HINT = #x8E8E8E
    mod.widgets.COLOR_TEXT_ON_ACCENT = #xFFFFFF
    mod.widgets.COLOR_TEXT_ON_ACCENT_MUTED = #xFFFFFFD9
    mod.widgets.COLOR_TEXT_ON_WARNING = #x000000
    mod.widgets.COLOR_TEXT_PURPLE = #xA991FF

    mod.widgets.COLOR_TEXT_INPUT_PLACEHOLDER = #x737373
    mod.widgets.COLOR_TEXT_INPUT_PLACEHOLDER_HOVER = #x8A8A8A
    mod.widgets.COLOR_TEXT_INPUT_PLACEHOLDER_FOCUS = #x8A8A8A
    mod.widgets.COLOR_PASSWORD_EYE_BUTTON_BG = #x333333
    mod.widgets.COLOR_MODAL_INPUT_TEXT = #xEBEBEB

    mod.widgets.COLOR_SHADOW_SOFT = #x00000066
    mod.widgets.COLOR_SHADOW = #x00000080
    mod.widgets.COLOR_SHADOW_STRONG = #x00000099
    mod.widgets.COLOR_LOADING_PANE_OVERLAY_BG = #x000000B2
    mod.widgets.COLOR_CONTEXT_MENU_OVERLAY_BG = #x00000066

    mod.widgets.COLOR_LABELED_DIVIDER = #x4A4A4A

    mod.widgets.COLOR_FLAT_ICON_BUTTON_BG_HOVER = #xFFFFFF15
    mod.widgets.COLOR_FLAT_ICON_BUTTON_BG_DOWN = #xFFFFFF25
    mod.widgets.COLOR_NAV_HISTORY_BUTTON_BG_HOVER = #xFFFFFF14
    mod.widgets.COLOR_NAV_HISTORY_BUTTON_BG_DOWN = #xFFFFFF26
    mod.widgets.COLOR_PREVIEW_TOGGLE_BUTTON_BORDER = #x4A4A4A

    mod.widgets.SEND_STATUS_ICON_COLOR = #x999999
    mod.widgets.COLOR_VERIFICATION_BADGE_UNKNOWN = #x888888

    mod.widgets.COLOR_LIST_ITEM_BG_FOCUS = #x314363
    mod.widgets.COLOR_LIST_ROW_TEXT_MUTED = #xB5B5B5
    mod.widgets.COLOR_CONTEXT_MENU_ITEM_BG_DOWN = #x444444
    mod.widgets.COLOR_CONTEXT_MENU_BORDER = #x555555
    mod.widgets.COLOR_ROOM_INPUT_MENU_ITEM_BG_DOWN = #x444444
    mod.widgets.COLOR_MENTION_POPUP_BORDER = #x444444

    mod.widgets.COLOR_HEADER_FG = #xFFFFFF
    mod.widgets.COLOR_HEADER_BG = (mod.widgets.COLOR_ROBRIX_PURPLE)

    mod.widgets.COLOR_WINDOW_CLEAR = #x26262600
    mod.widgets.COLOR_CAPTION_BAR_BG = #x212121
    mod.widgets.COLOR_CAPTION_BUTTON_ICON = #xFFFFFF99
    mod.widgets.COLOR_CAPTION_BUTTON_ICON_HOVER = #xFFFFFF
    mod.widgets.COLOR_CAPTION_BUTTON_BG_HOVER = #x333333
    mod.widgets.COLOR_CAPTION_BUTTON_BG_DOWN = #x454545
    mod.widgets.COLOR_SCROLL_BAR_HANDLE = #xFFFFFF40
    mod.widgets.COLOR_SCROLL_BAR_HANDLE_HOVER = #xFFFFFF60
    mod.widgets.COLOR_SCROLL_BAR_HANDLE_DRAG = #xFFFFFF80

    mod.widgets.COLOR_TIMELINE_ITEM_BG_HIGHLIGHT = #x25314B
    mod.widgets.COLOR_TIMELINE_LOADING_BG = #x213226F0
    mod.widgets.COLOR_READ_MARKER = #xFF6670
    mod.widgets.COLOR_MESSAGE_CODE_BG = #xFFFFFF12
    mod.widgets.COLOR_MESSAGE_QUOTE_BG = #xFFFFFF12
    mod.widgets.COLOR_TYPING_NOTICE_BG = #x2A3441
    mod.widgets.COLOR_FAILED_SEND_BANNER_BORDER = #xE34B4F
    mod.widgets.COLOR_IMAGE_PLACEHOLDER_BG = #x3A3A3A
    mod.widgets.COLOR_LINK_PILL_TEXT = #xFFFFFF
    mod.widgets.COLOR_LINK_PILL_AVATAR_BG = #xFFFFFF

    mod.widgets.COLOR_REACTION_HOVER_TINT = #xC9A23F

    mod.widgets.COLOR_INPUT_BAR_HEADER_TEXT = #xE3E3E3
    mod.widgets.COLOR_INPUT_BAR_ADD_BUTTON_BG_HOVER = #x323A4A
    mod.widgets.COLOR_INPUT_BAR_ADD_BUTTON_BG_DOWN = #x374153
    mod.widgets.COLOR_SPEECH_BUTTON_LEVEL_BARS = #x1C1C1C

    mod.widgets.COLOR_UNREAD_BADGE_MENTIONS_FADE = #xB64B3C
    mod.widgets.COLOR_JUMP_TO_BOTTOM_BG = #x3A3A3ACE
    mod.widgets.COLOR_JUMP_TO_BOTTOM_BG_HOVER = #x474747CE

    mod.widgets.COLOR_POPUP_TEXT = #xEBEBEB
    mod.widgets.COLOR_POPUP_BORDER = #x737373
    mod.widgets.COLOR_POPUP_PROGRESS_BAR = #x00000080
    mod.widgets.COLOR_POPUP_CLOSE_BG = #xFFFFFF14
    mod.widgets.COLOR_POPUP_CLOSE_BG_HOVER = #xFFFFFF22
    mod.widgets.COLOR_POPUP_CLOSE_BG_DOWN = #xFFFFFF2E
    mod.widgets.COLOR_POPUP_CLOSE_BORDER = #xFFFFFF20
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_HOVER = #xFFFFFF2C
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_DOWN = #xFFFFFF38
    mod.widgets.COLOR_POPUP_CLOSE_ICON = #xFFFFFFD0

    mod.widgets.COLOR_DOCK_TAB_FG = #xEBEBEB
    mod.widgets.COLOR_DOCK_TAB_BG = #x252C37
    mod.widgets.COLOR_DOCK_TAB_BG_HOVER = #x2C3646
    mod.widgets.COLOR_DOCK_TAB_BG_ACTIVE = #x0660FE
    mod.widgets.COLOR_DOCK_TAB_BG_ACTIVE_BOTTOM = #x2575EB
    mod.widgets.COLOR_DOCK_TAB_BAR_BG = #x212121F4
    mod.widgets.COLOR_DOCK_TAB_BAR_BG_DRAG = #xE3E3E3
    mod.widgets.COLOR_DOCK_DROP_PREVIEW_MIX = #xFFFFFF00
    mod.widgets.COLOR_PANE_GRAB_HANDLE_DOTS = #xFFFFFFA6

    mod.widgets.COLOR_IMAGE_VIEWER_FG = #xFFFFFF
    mod.widgets.COLOR_IMAGE_VIEWER_FG_MUTED = #xFFFFFFBF
    mod.widgets.COLOR_IMAGE_VIEWER_BUTTON_BG_HOVER = #xFFFFFF26
    mod.widgets.COLOR_IMAGE_VIEWER_BUTTON_BG_DOWN = #xFFFFFF40
    mod.widgets.COLOR_IMAGE_VIEWER_META_BORDER = #xFFFFFF29
    mod.widgets.COLOR_IMAGE_VIEWER_META_SHADOW = #x00000066

    mod.widgets.COLOR_WELCOME_BG_TOP = #x2F2B38
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_BG_RIGHT = #x0E64CD
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_SHADOW = #x12082E80
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_ICON_BG = #xFFFFFF2E
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_BUTTON_BG = #xFFFFFF
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_BUTTON_BG_HOVER = #xE8E1FA
    mod.widgets.COLOR_WELCOME_ROOM_PANEL_BUTTON_BG_DOWN = #xE1D7F7
    mod.widgets.COLOR_WELCOME_TIPS_PANEL_BORDER = #xFFFFFF10
    mod.widgets.COLOR_WELCOME_TIPS_PANEL_SHADOW = #x00000040
    mod.widgets.COLOR_WELCOME_TIP_BODY_TEXT = #xA9B0BF
    mod.widgets.COLOR_WELCOME_TIP_HINT_BG_WORKSPACE = #x2F2B38
    mod.widgets.COLOR_WELCOME_TIP_HINT_TEXT_WORKSPACE = #xB4A0FF
    mod.widgets.COLOR_WELCOME_TIP_BADGE_BG_SETTINGS = #x232D40
    mod.widgets.COLOR_WELCOME_TIP_HINT_BG_SETTINGS = #x2A303C
    mod.widgets.COLOR_WELCOME_TIP_HINT_TEXT_SETTINGS = #x79B8FF
    mod.widgets.COLOR_WELCOME_TIP_BADGE_BG_NAVIGATION = #x27403F
    mod.widgets.COLOR_WELCOME_TIP_ICON_NAVIGATION = #x2CC9C2
    mod.widgets.COLOR_WELCOME_TIP_HINT_BG_NAVIGATION = #x273636
    mod.widgets.COLOR_WELCOME_TIP_HINT_TEXT_NAVIGATION = #x5ED6CF
    mod.widgets.COLOR_WELCOME_FOOTER_TEXT = #x9FA6B7

    // A darker version of the main Robrix purple color
    mod.widgets.COLOR_LOGIN_BG_TOP = #x292536
    mod.widgets.COLOR_LOGIN_BG_BOTTOM = #x1C1C1C

    mod.widgets.COLOR_SPACE_LOBBY_NAME_TEXT = #xE6E6E6
    mod.widgets.COLOR_SPACE_LOBBY_TREE_LINES = #x737373
    mod.widgets.COLOR_SPACE_LOBBY_ENTRY_TEXT_ACTIVE = #xFFFFFF

    mod.widgets.COLOR_INVITE_SCREEN_TOPIC_TEXT = #xBDBDBD

    mod.widgets.COLOR_MODAL_TIP_TEXT = #x8E8E8E
    mod.widgets.COLOR_MODAL_PANEL_BG = #x262626
    mod.widgets.COLOR_CODE_BLOCK_BORDER = #x505050
    mod.widgets.COLOR_TSP_FIELD_CAPTION_TEXT = #x9F9F9F
    mod.widgets.COLOR_NO_PREVIEW_ICON_DISC = #x2A374D
    mod.widgets.COLOR_NO_PREVIEW_ICON_PAGE = #x45546E
    mod.widgets.COLOR_NO_PREVIEW_ICON_ACCENT = #x7AA8E8

    mod.widgets.COLOR_SETTINGS_LIST_BG = #x2C2C2C

    // Code view syntax colors, one per kind of code token.
    mod.widgets.COLOR_CODE_TEXT = #xE1E4E8
    mod.widgets.COLOR_CODE_WHITESPACE = #x6A737D
    mod.widgets.COLOR_CODE_DELIMITER = #xE1E4E8
    mod.widgets.COLOR_CODE_DELIMITER_HIGHLIGHT = #x79B8FF
    mod.widgets.COLOR_CODE_ERROR_DECORATION = #xF97583
    mod.widgets.COLOR_CODE_WARNING_DECORATION = #xE3B341
    mod.widgets.COLOR_CODE_BRANCH_KEYWORD = #xF97583
    mod.widgets.COLOR_CODE_CONSTANT = #x79B8FF
    mod.widgets.COLOR_CODE_IDENTIFIER = #xE1E4E8
    mod.widgets.COLOR_CODE_LOOP_KEYWORD = #xF97583
    mod.widgets.COLOR_CODE_NUMBER = #x79B8FF
    mod.widgets.COLOR_CODE_OTHER_KEYWORD = #xF97583
    mod.widgets.COLOR_CODE_PUNCTUATOR = #xE1E4E8
    mod.widgets.COLOR_CODE_STRING = #x85E89D
    mod.widgets.COLOR_CODE_FUNCTION = #xB392F0
    mod.widgets.COLOR_CODE_MACRO_IDENTIFIER = #xB392F0
    mod.widgets.COLOR_CODE_ATTRIBUTE = #xB392F0
    mod.widgets.COLOR_CODE_TYPENAME = #xFFAB70
    mod.widgets.COLOR_CODE_COMMENT = #x959DA5

    // The close button on an error, info, or success popup's colored background.
    mod.widgets.COLOR_POPUP_CLOSE_BG_ON_ACCENT = #xFFFFFF22
    mod.widgets.COLOR_POPUP_CLOSE_BG_ON_ACCENT_HOVER = #xFFFFFF30
    mod.widgets.COLOR_POPUP_CLOSE_BG_ON_ACCENT_DOWN = #xFFFFFF42
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_ACCENT = #xFFFFFF2C
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_ACCENT_HOVER = #xFFFFFF38
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_ACCENT_DOWN = #xFFFFFF4A
    mod.widgets.COLOR_POPUP_CLOSE_ICON_ON_ACCENT = #xFFFFFFFA
    // The close button on a warning popup's yellow background.
    mod.widgets.COLOR_POPUP_CLOSE_BG_ON_WARNING = #x00000014
    mod.widgets.COLOR_POPUP_CLOSE_BG_ON_WARNING_HOVER = #x00000022
    mod.widgets.COLOR_POPUP_CLOSE_BG_ON_WARNING_DOWN = #x0000002E
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_WARNING = #x00000020
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_WARNING_HOVER = #x0000002C
    mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_WARNING_DOWN = #x00000038
    mod.widgets.COLOR_POPUP_CLOSE_ICON_ON_WARNING = #x000000D0

    // Toggles, checkboxes, and radio buttons.
    // The mark is a toggle's knob, a checkbox's check, or a radio button's dot.
    mod.widgets.COLOR_TOGGLE_BG = #xFFFFFF19
    mod.widgets.COLOR_TOGGLE_BG_DISABLED = #xFFFFFF08
    mod.widgets.COLOR_TOGGLE_BORDER = #xFFFFFF1F
    mod.widgets.COLOR_TOGGLE_BORDER_HOVER = #xFFFFFF3F
    mod.widgets.COLOR_TOGGLE_BORDER_DISABLED = #x4A4A4A
    mod.widgets.COLOR_TOGGLE_MARK = #xFFFFFFBF
    mod.widgets.COLOR_TOGGLE_MARK_ACTIVE = #xFFFFFF99
    mod.widgets.COLOR_TOGGLE_TEXT = #xFFFFFFBF
    mod.widgets.COLOR_TOGGLE_TEXT_DISABLED = #xFFFFFF54

    mod.widgets.COLOR_LINK_PREVIEW_TITLE_DOWN = #xFFFFFFBF
    mod.widgets.COLOR_LINK_PREVIEW_TITLE_FOCUS = #xFFFFFF99
    mod.widgets.COLOR_LINK_PREVIEW_UNDERLINE = #xFFFFFF99
    mod.widgets.COLOR_LINK_PREVIEW_UNDERLINE_DOWN = #xFFFFFFBF

    mod.widgets.COLOR_MESSAGE_TABLE_HEADER_BG = #xFFFFFF0C
    mod.widgets.COLOR_MESSAGE_TABLE_BORDER = #xFFFFFF1F

    mod.widgets.COLOR_NAVIGATION_TAB_FG = #xDCE1EE
    mod.widgets.COLOR_NAVIGATION_TAB_BG = #x303030
    mod.widgets.COLOR_NAVIGATION_TAB_BG_HOVER = #x3D3D3DD8
    mod.widgets.COLOR_NAVIGATION_TAB_BG_ACTIVE = #x4D4D4D

    mod.widgets.COLOR_IMAGE_VIEWER_BACKGROUND = #x121212E6

    mod.widgets.COLOR_IMAGE_VIEWER_META_BACKGROUND = #x1E1E1ECC
}
