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


    // The base widget definition used for all "small" modals in Robrix.
    mod.widgets.SmallModal = RoundedView {
        width: Fill { max: 400 }
        height: Fit { max: FitBound.Rel{base: Base.Full, factor: 1.0} }
        margin: 20
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
            color: (COLOR_PRIMARY)
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
            color: #000
        }
    }

    // A modal body or description label, which fills the width and wraps.
    mod.widgets.ModalBody = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 11.5},
            color: #000
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
