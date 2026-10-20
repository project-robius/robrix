use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // Every scroll bar uses this styling.
    mod.widgets.ScrollBar.draw_bg.color = (COLOR_SCROLL_BAR_HANDLE)
    mod.widgets.ScrollBar.draw_bg.color_hover = (COLOR_SCROLL_BAR_HANDLE_HOVER)
    mod.widgets.ScrollBar.draw_bg.color_drag = (COLOR_SCROLL_BAR_HANDLE_DRAG)

    // Every toggle, checkbox, and radio button uses this styling.
    mod.widgets.CheckBoxFlat.draw_bg.color = (COLOR_TOGGLE_BG)
    mod.widgets.CheckBoxFlat.draw_bg.color_hover = (COLOR_TOGGLE_BG)
    mod.widgets.CheckBoxFlat.draw_bg.color_down = (COLOR_TOGGLE_BG)
    mod.widgets.CheckBoxFlat.draw_bg.color_focus = (COLOR_TOGGLE_BG)
    mod.widgets.CheckBoxFlat.draw_bg.color_active = (COLOR_TOGGLE_BG)
    mod.widgets.CheckBoxFlat.draw_bg.color_disabled = (COLOR_TOGGLE_BG_DISABLED)
    mod.widgets.CheckBoxFlat.draw_bg.border_color = (COLOR_TOGGLE_BORDER)
    mod.widgets.CheckBoxFlat.draw_bg.border_color_hover = (COLOR_TOGGLE_BORDER_HOVER)
    mod.widgets.CheckBoxFlat.draw_bg.border_color_down = (COLOR_TOGGLE_BORDER_HOVER)
    mod.widgets.CheckBoxFlat.draw_bg.border_color_focus = (COLOR_TOGGLE_BORDER_HOVER)
    mod.widgets.CheckBoxFlat.draw_bg.border_color_active = (COLOR_TOGGLE_BORDER)
    mod.widgets.CheckBoxFlat.draw_bg.border_color_disabled = (COLOR_TOGGLE_BORDER_DISABLED)
    mod.widgets.CheckBoxFlat.draw_bg.mark_color_active = (COLOR_TOGGLE_MARK_ACTIVE)

    // ToggleFlat derives from CheckBoxFlat but has its own knob colors.
    mod.widgets.ToggleFlat.draw_bg.mark_color = (COLOR_TOGGLE_MARK)
    mod.widgets.ToggleFlat.draw_bg.mark_color_hover = (COLOR_TOGGLE_MARK)
    mod.widgets.ToggleFlat.draw_bg.mark_color_active = (COLOR_TOGGLE_MARK_ACTIVE)

    mod.widgets.RadioButtonFlat.draw_bg.color = (COLOR_TOGGLE_BG)
    mod.widgets.RadioButtonFlat.draw_bg.color_hover = (COLOR_TOGGLE_BG)
    mod.widgets.RadioButtonFlat.draw_bg.color_down = (COLOR_TOGGLE_BG)
    mod.widgets.RadioButtonFlat.draw_bg.color_focus = (COLOR_TOGGLE_BG)
    mod.widgets.RadioButtonFlat.draw_bg.color_active = (COLOR_TOGGLE_BG)
    mod.widgets.RadioButtonFlat.draw_bg.color_disabled = (COLOR_TOGGLE_BG_DISABLED)
    mod.widgets.RadioButtonFlat.draw_bg.border_color = (COLOR_TOGGLE_BORDER)
    mod.widgets.RadioButtonFlat.draw_bg.border_color_hover = (COLOR_TOGGLE_BORDER_HOVER)
    mod.widgets.RadioButtonFlat.draw_bg.border_color_down = (COLOR_TOGGLE_BORDER_HOVER)
    mod.widgets.RadioButtonFlat.draw_bg.border_color_focus = (COLOR_TOGGLE_BORDER_HOVER)
    mod.widgets.RadioButtonFlat.draw_bg.border_color_active = (COLOR_TOGGLE_BORDER)
    mod.widgets.RadioButtonFlat.draw_bg.border_color_disabled = (COLOR_TOGGLE_BORDER_DISABLED)
    mod.widgets.RadioButtonFlat.draw_bg.mark_color_active = (COLOR_TOGGLE_MARK_ACTIVE)

    mod.widgets.DrawCheckLabel.color = (COLOR_TOGGLE_TEXT)
    mod.widgets.DrawCheckLabel.color_hover = (COLOR_TOGGLE_TEXT)
    mod.widgets.DrawCheckLabel.color_down = (COLOR_TOGGLE_TEXT)
    mod.widgets.DrawCheckLabel.color_focus = (COLOR_TOGGLE_TEXT)
    mod.widgets.DrawCheckLabel.color_active = (COLOR_TOGGLE_TEXT)
    mod.widgets.DrawCheckLabel.color_disabled = (COLOR_TOGGLE_TEXT_DISABLED)

    // The scroll bar used by all lists (like portallists).
    mod.widgets.ListScrollBar = ScrollBar { bar_size: 9 }

    // A text input widget styled for Robrix.
    mod.widgets.RobrixTextInput = TextInput {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true},
        align: Align{y: 0.5}
        margin: 0,
        padding: 10,
        scroll_bar_inset: (mod.widgets.TEXT_INPUT_SCROLL_BAR_INSET)

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
            color_empty: (COLOR_TEXT_INPUT_PLACEHOLDER),
            color_empty_hover: (COLOR_TEXT_INPUT_PLACEHOLDER_HOVER),
            color_empty_focus: (COLOR_TEXT_INPUT_PLACEHOLDER_FOCUS),

            text_style: mod.widgets.MESSAGE_TEXT_STYLE {},
        }
    }

    // A read-only CodeView with our light-theme syntax highlighting colors.
    mod.widgets.LightCodeView = mod.widgets.CodeView {
        editor +: {
            word_wrap: true
            scroll_bars +: {
                show_scroll_x: false
                scroll_bar_y.drag_scrolling: true
            }
            draw_bg +: { color: (mod.widgets.COLOR_TRANSPARENT) }

            // Light mode syntax highlighting (inspired by GitHub Light / VS Code Light+)
            token_colors +: {
                whitespace: (COLOR_CODE_WHITESPACE),                   // Gray for whitespace markers
                delimiter: (COLOR_CODE_DELIMITER),                     // Dark gray for punctuation
                delimiter_highlight: (COLOR_CODE_DELIMITER_HIGHLIGHT), // Blue for highlighted delimiters
                error_decoration: (COLOR_CODE_ERROR_DECORATION),       // Red for errors
                warning_decoration: (COLOR_CODE_WARNING_DECORATION),   // Dark yellow/amber for warnings

                unknown: (COLOR_CODE_TEXT),                            // Default dark text
                branch_keyword: (COLOR_CODE_BRANCH_KEYWORD),           // Red/pink for keywords (if, else, match)
                constant: (COLOR_CODE_CONSTANT),                       // Blue for constants
                identifier: (COLOR_CODE_IDENTIFIER),                   // Dark gray for variables
                loop_keyword: (COLOR_CODE_LOOP_KEYWORD),               // Red/pink for loop keywords
                number: (COLOR_CODE_NUMBER),                           // Blue for numbers
                other_keyword: (COLOR_CODE_OTHER_KEYWORD),             // Red/pink for other keywords
                punctuator: (COLOR_CODE_PUNCTUATOR),                   // Dark gray for punctuation
                string: (COLOR_CODE_STRING),                           // Green for strings
                function: (COLOR_CODE_FUNCTION),                       // Purple for functions
                macro_identifier: (COLOR_CODE_MACRO_IDENTIFIER),       // Purple: a macro reads as a call
                attribute: (COLOR_CODE_ATTRIBUTE),                     // Purple for #[derive(..)] and friends
                typename: (COLOR_CODE_TYPENAME),                       // Orange for types
                comment: (COLOR_CODE_COMMENT),                         // Gray for comments
            }
        }
    }

    // A read-only CodeView with light-theme color without any syntax highlighting.
    mod.widgets.PlainCodeView = mod.widgets.LightCodeView {
        editor +: {
            token_colors +: {
                whitespace: (COLOR_CODE_TEXT),
                delimiter: (COLOR_CODE_TEXT),
                delimiter_highlight: (COLOR_CODE_TEXT),
                error_decoration: (COLOR_CODE_TEXT),
                warning_decoration: (COLOR_CODE_TEXT),
                unknown: (COLOR_CODE_TEXT),
                branch_keyword: (COLOR_CODE_TEXT),
                constant: (COLOR_CODE_TEXT),
                identifier: (COLOR_CODE_TEXT),
                loop_keyword: (COLOR_CODE_TEXT),
                number: (COLOR_CODE_TEXT),
                other_keyword: (COLOR_CODE_TEXT),
                punctuator: (COLOR_CODE_TEXT),
                string: (COLOR_CODE_TEXT),
                function: (COLOR_CODE_TEXT),
                macro_identifier: (COLOR_CODE_TEXT),
                attribute: (COLOR_CODE_TEXT),
                typename: (COLOR_CODE_TEXT),
                comment: (COLOR_CODE_TEXT),
            }
        }
    }


    mod.widgets.TitleLabel = Label {
        width: Fill, height: Fit
        margin: Inset{top: 5},
        align: Align{x: 0.0, y: 0.5}
        flow: Flow.Right{wrap: true},
        draw_text +: {
            text_style: TITLE_TEXT {font_size: 15},
            color: (COLOR_TEXT_TITLE)
        }
    }

    mod.widgets.SubsectionLabel = Label {
        width: Fill, height: Fit
        margin: Inset{top: 5},
        align: Align{x: 0.0, y: 0.5}
        flow: Flow.Right { wrap: true },
        draw_text +: {
            color: (COLOR_TEXT),
            text_style: theme.font_bold { font_size: 13 },
        }
    }

    mod.widgets.LineH = RoundedView {
        width: Fill,
        height: 2.0,
        margin: 0.0,
        padding: 0.0, spacing: 0.0
        show_bg: true
        draw_bg.color: (COLOR_DIVIDER_DARK)
    }

    mod.widgets.Filler = View { width: Fill, height: Fill }

    mod.widgets.FillerX = View { width: Fill, height: Fit }
    mod.widgets.FillerY = View { width: Fit,  height: Fill }


    // The base widget definition used for all "small" modals in Robrix.
    mod.widgets.SmallModal = RoundedView {
        width: Fill { max: 500 }
        height: Fit { max: FitBound.Rel{base: Base.Full, factor: 1.0} }
        margin: Inset { top: 30, bottom: 30, left: 15, right: 15 },
        padding: Inset{top: 30, right: 25, bottom: 20, left: 25}
        flow: Down

        // Make the modal scrollable for cases when it's too tall for the app window.
        scroll_bars: ScrollBars {
            show_scroll_x: false
            show_scroll_y: true
            scroll_bar_y.drag_scrolling: true
        }

        show_bg: true
        draw_bg +: {
            color: (COLOR_BG_FLOATING)
            border_radius: 4.0
        }
    }

    mod.widgets.ModalTitle = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        align: Align{x: 0.5}
        margin: Inset{bottom: 25}
        draw_text +: {
            text_style: TITLE_TEXT {font_size: 13},
            color: (COLOR_TEXT_TITLE)
        }
    }

    // A modal body or description label, which fills the width and wraps.
    mod.widgets.ModalBody = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 11.5},
            color: (COLOR_TEXT_BODY)
        }
    }

    // A modal buttons row that fills the width, is right-aligned,
    // and wraps to the next line.
    mod.widgets.ModalButtonsRow = View {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        align: Align{x: 1.0, y: 0.5}
        spacing: 15
        padding: Inset{top: 20, bottom: 20}
    }
}
