use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.TitleLabel = Label {
        width: Fill, height: Fit
        margin: Inset{top: 5},
        align: Align{x: 0.0, y: 0.5}
        flow: Flow.Right{wrap: true},
        draw_text +: {
            text_style: TITLE_TEXT {font_size: 15},
            color: #000
        }
    }

    mod.widgets.SubsectionLabel = Label {
        width: Fill, height: Fit
        margin: Inset{top: 5},
        align: Align{x: 0.0, y: 0.5}
        flow: Right,
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

    // Shared base for all small (non-full-screen) modal dialogs: fills width up to 400
    // with a 20px left/right gutter so it stays off the edges on narrow screens, caps its
    // height at 90% of the viewport, and scrolls vertically when content is taller.
    // Defines the standard padding all modals share; modals add only their content.
    mod.widgets.SmallModal = RoundedView {
        width: Fill { max: 400 }
        height: Fit { max: FitBound.Rel{base: Base.Full, factor: 0.9} }
        // Only a horizontal gutter; the vertical gap comes from the Modal centering
        // the (capped) box, so a tall/scrollable modal can't overflow off the bottom.
        margin: Inset{left: 20, right: 20}
        padding: Inset{top: 30, right: 25, bottom: 20, left: 25}
        flow: Down

        // Scroll the content when it's taller than the max height, so the
        // buttons stay reachable on short windows.
        scroll_bars: ScrollBars {
            show_scroll_x: false
            show_scroll_y: true
            scroll_bar_y.drag_scrolling: true
        }

        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 4.0
        }
    }

    // Shared dialog pieces for SmallModal-based modals. Declared inline in each modal
    // (Makepad can't inherit child widgets from a plain base), but they centralize the
    // styling and the wrapping behavior so modals don't repeat it.

    // A modal title: fills the width and wraps, centered.
    mod.widgets.ModalTitle = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        align: Align{x: 0.5}
        margin: Inset{bottom: 25}
        draw_text +: {
            text_style: TITLE_TEXT {font_size: 13},
            color: #000
        }
    }

    // A modal body/description label: fills the width and wraps.
    mod.widgets.ModalBody = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 11.5},
            color: #000
        }
    }

    // A modal buttons row: fills the width, right-aligned, and wraps to the next line
    // on narrow screens. Each modal adds its own buttons inside.
    mod.widgets.ModalButtonsRow = View {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        align: Align{x: 1.0, y: 0.5}
        spacing: 20
        padding: Inset{top: 20, bottom: 20}
    }
}
