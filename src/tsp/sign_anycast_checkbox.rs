//! A simple checkbox displayed by the message text input box
//! that allows the user to sign a message using TSP `sign_anycast()`.

use makepad_widgets::*;

script_mod! {
    link tsp_enabled

    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.TspSignAnycastCheckbox = CheckBoxFlat {
        text: "TSP",
        active: false,
    }
}
