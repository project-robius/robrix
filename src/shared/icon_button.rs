use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // The base Robrix button widget.
    // Uses COLOR_ACTIVE_PRIMARY (blue) background with white text by default.
    // See also the preset variants below:
    //   RobrixPositiveIconButton, RobrixNegativeIconButton, RobrixNeutralIconButton.
    mod.widgets.RobrixIconButton = Button {
        width: Fit,
        height: Fit,
        spacing: 10,
        padding: 10,
        align: Align{x: 0, y: 0.5}

        // Disable focus visual styling entirely so that clicking a button
        // and hovering away doesn't leave it stuck on the theme's focus color.
        // This works by keeping the `focus` uniform at 0.0 in both on/off states,
        // so the shader's `mix(color, color_focus, focus)` always evaluates to just `color`.
        animator +: {
            focus: {
                default: @off
                off: AnimatorState {
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        draw_bg: {focus: 0.0}
                        draw_text: {focus: 0.0}
                    }
                }
                on: AnimatorState {
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        draw_bg: {focus: 0.0}
                        draw_text: {focus: 0.0}
                    }
                }
            }
        }

        draw_bg +: {
            border_size: 0.0
            border_radius: 4.0

            color: (COLOR_ACTIVE_PRIMARY)
            color_hover: (COLOR_ACTIVE_PRIMARY_DARKER)
            color_down: #0C5DAA
            color_disabled: (COLOR_BG_DISABLED)

            border_color: #0000
            border_color_hover: #0000
            border_color_down: #0000
            border_color_focus: #0000
            border_color_disabled: #0000

            // Disable gradient (color_2) by default
            color_2: vec4(-1.0, -1.0, -1.0, -1.0)
            border_color_2: vec4(-1.0, -1.0, -1.0, -1.0)
        }

        draw_icon.color: (COLOR_PRIMARY)
        icon_walk: Walk{width: 16, height: 16}

        draw_text +: {
            color: (COLOR_PRIMARY)
            color_hover: (COLOR_PRIMARY)
            color_down: (COLOR_PRIMARY)
            color_disabled: (COLOR_FG_DISABLED)
            text_style: mod.widgets.REGULAR_TEXT {font_size: 10},
        }
        text: ""
    }

    // Green button for positive/accept actions: joining a room, confirming, accepting an invite.
    mod.widgets.RobrixPositiveIconButton = mod.widgets.RobrixIconButton {
        draw_bg +: {
            border_color: (COLOR_FG_ACCEPT_GREEN)
            border_color_hover: (COLOR_FG_ACCEPT_GREEN)
            border_color_down: (COLOR_FG_ACCEPT_GREEN)
            color: (COLOR_BG_ACCEPT_GREEN)
            color_hover: #D4EED4
            color_down: #B8E0B8
        }
        draw_icon.color: (COLOR_FG_ACCEPT_GREEN)
        draw_text +: {
            color: (COLOR_FG_ACCEPT_GREEN)
            color_hover: (COLOR_FG_ACCEPT_GREEN)
            color_down: (COLOR_FG_ACCEPT_GREEN)
        }
    }

    // Red button for negative/dangerous actions: rejecting, leaving, deleting, blocking.
    mod.widgets.RobrixNegativeIconButton = mod.widgets.RobrixIconButton {
        draw_bg +: {
            border_color: (COLOR_FG_DANGER_RED)
            border_color_hover: (COLOR_FG_DANGER_RED)
            border_color_down: (COLOR_FG_DANGER_RED)
            color: (COLOR_BG_DANGER_RED)
            color_hover: #F0D4D4
            color_down: #E0B8B8
        }
        draw_icon.color: (COLOR_FG_DANGER_RED)
        draw_text +: {
            color: (COLOR_FG_DANGER_RED)
            color_hover: (COLOR_FG_DANGER_RED)
            color_down: (COLOR_FG_DANGER_RED)
        }
    }

    // Gray button for cancel/dismiss actions: canceling, closing, going back.
    mod.widgets.RobrixNeutralIconButton = mod.widgets.RobrixIconButton {
        draw_bg +: {
            border_color: (COLOR_BG_DISABLED)
            border_color_hover: (COLOR_BG_DISABLED)
            border_color_down: (COLOR_BG_DISABLED)
            color: (COLOR_SECONDARY)
            color_hover: #D0D0D0
            color_down: #C0C0C0
        }
        draw_icon.color: (COLOR_TEXT)
        draw_text +: {
            color: (COLOR_TEXT)
            color_hover: (COLOR_TEXT)
            color_down: (COLOR_TEXT)
        }
    }
}
