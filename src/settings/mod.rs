use makepad_widgets::Cx;

pub mod settings_screen;
pub mod account_settings;

/// A wrapper that shows the TSP settings screen if the `tsp` feature is enabled,
/// otherwise it shows a message indicating that TSP is not available.
pub mod tsp_settings_screen_wrapper;

pub fn live_design(cx: &mut Cx) {
    account_settings::live_design(cx);
    tsp_settings_screen_wrapper::live_design(cx);
    settings_screen::live_design(cx);
}

/// Actions that can be sent to/from the settings screen.
#[derive(Debug, Clone)]
pub enum SettingsAction {
    /// Action to open the settings screen.
    OpenSettings,
    /// Action to close the settings screen.
    CloseSettings,
    // TODO: add specific actions for settings changes,
    //       so that other widgets can be notified of any changes
    //       that they need to respond to.
    //       Examples: changed avatar, changed display name, etc.
}
