//! A simple checkbox displayed by the message text input box
//! that allows the user to sign a message using TSP `sign_anycast()`.

use makepad_widgets::*;

live_design! {
    link tsp_enabled

    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub TspSignAnycastCheckbox = <CheckBoxFlat> {
        text: "TSP",
        active: false,
    }
}
