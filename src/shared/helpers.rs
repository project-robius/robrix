use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.TitleLabel = Label {


        width: Fill, height: Fit
        margin: Inset{top: 5},
        flow: Flow.Right{wrap: true},
        draw_text +: {
            text_style: TITLE_TEXT {font_size: 15},
            color: #000
            flow: Flow.Right{wrap: true}
        }
    }

    mod.widgets.SubsectionLabel = Label {

        width: Fill, height: Fit
        margin: Inset{top: 5},
        flow: Right,
        draw_text +: {
            color: (COLOR_TEXT),
            text_style: theme.font_bold { font_size: 13 },
        }
    }

    // Keep FadeView as a simple View to avoid custom cached-shader compile failures.
    // This preserves layout/behavior while we stay compatible with current script runtime.
    mod.widgets.FadeView = View {}

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
}
