//! A wrapper view that displays TSP settings if the `tsp` feature is enabled
//! or a placeholder view if the feature is not enabled.

use makepad_widgets::*;

#[cfg(not(feature = "tsp"))]
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;

    pub TspSettingsWrapper = <View> {
        width: Fill, height: Fit
        flow: Down
        align: {x: 0}

        <TitleLabel> {
            text: "TSP Wallet Settings"
        }

        <Label> {
            width: Fill, height: Fit
            flow: RightWrap,
            align: {x: 0}
            margin: {top: 10, bottom: 10}
            draw_text: {
                wrap: Word,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
            }
            text: "TSP features are not included in this build.\nTo use TSP, build Robrix with the 'tsp' feature enabled."
        }
    }
}


#[cfg(feature = "tsp")]
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::tsp::tsp_settings::TspSettings;

    pub TspSettingsWrapper = <View> {
        <TspSettings> { }
    }
}
