//! This module provides dummy TSP-related widgets that do nothing.
//!
//! We only need to define dummy widgets for TSP-specific widgets that are used
//! from non-TSP DSL code, i.e., any widgets that exist on the boundary between
//! TSP and non-TSP code.
//!
//! The real TSP widgets are all defined in the `tsp_enabled` namespace,
//! and their live_design DSL blocks all start with `link tsp_enabled`,
//! which declares the namespace that they exist within.
//!
//! The "active" namespace is selected via the `cx.link()` call in `App::live_register()`,
//! which connects the `tsp_link` DSL namespace to the `tsp_disabled` namespace
//! defined in this module, only when the `tsp` feature is not enabled.
//!
//! This allows the rest of the application's DSL to directly use TSP widgets,
//! but the widgets that actually get imported under the `tsp_link` namespace
//! will be replaced with these dummy widgets when the `tsp` feature is not enabled.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.TspSettingsScreen = View {
        width: Fill, height: 0
        flow: Down
    }

    mod.widgets.CreateWalletModal = View {
        visible: false,
    }

    mod.widgets.CreateDidModal = View {
        visible: false,
    }

    mod.widgets.TspVerifyUser = View {
        height: 50
        width: Fill,
    }

    mod.widgets.TspVerificationModal = View {
        visible: false
    }

    mod.widgets.TspSignAnycastCheckbox = View {
        visible: false
    }

    mod.widgets.TspSignIndicator = View {
        visible: false
    }
}
