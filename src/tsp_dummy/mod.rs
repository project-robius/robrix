//! This module provides dummy TSP-related widgets that do nothing.
//!
//! We only need to define dummy widgets for TSP-specific widgets that are used
//! from non-TSP DSL code, i.e., any widgets that exist on the boundary between
//! TSP and non-TSP code.
//! Those real TSP widgets are all defined in the `link tsp_enabled` namespace.
//!
//! They are enabled via the `cx.link()` call in `App::live_register()`,
//! which connects the `tsp` DSL namespace to the `tsp_disabled` namespace
//! defined in this module, only when the `tsp` feature is not enabled.
//!
//! This allows the rest of the application's DSL to *appear* to directly use TSP widgets,
//! but they will be replaced with these dummy widgets when the `tsp` feature is not enabled.

use makepad_widgets::*;

live_design! {
    link tsp_disabled

    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;

    pub TspSettingsScreen = <View> {
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

    pub CreateWalletModal = <View> {
        visible: false,
    }
}
