//! Follows the system's light or dark theme, which the "Auto" appearance setting uses.

use makepad_widgets::*;
use robius_theme::{Subscription, Theme};

use super::app_preferences::{Appearance, AppearanceGlobal};

/// Posted when the system theme changes, from whichever thread the system reports it on.
#[derive(Debug)]
pub struct SystemThemeAction(pub Option<Theme>);

/// The system theme, and the subscription that keeps it up to date.
#[derive(Default)]
struct SystemTheme {
    theme: Option<Theme>,
    _subscription: Option<Subscription>,
}

/// Starts following the system theme. This must run before the app's styles are first loaded.
pub fn init(cx: &mut Cx) {
    let subscription = robius_theme::subscribe(|theme| Cx::post_action(SystemThemeAction(theme)))
        .inspect_err(|e| error!("Failed to subscribe to system theme changes: {e}"))
        .ok();
    cx.set_global(SystemTheme { theme: query(), _subscription: subscription });
}

fn query() -> Option<Theme> {
    robius_theme::query()
        .inspect_err(|e| error!("Failed to get the system theme: {e}"))
        .ok()
        .flatten()
}

/// The theme that the app is drawn in. With no system preference, "Auto" is light.
pub fn effective_theme(cx: &mut Cx) -> Theme {
    let system_theme = cx.global::<SystemTheme>().theme;
    cx.global::<AppearanceGlobal>().0.resolve(system_theme)
}

/// How the parts that the system draws for us should look, like title bars and keyboards.
pub fn native_appearance(cx: &mut Cx) -> NativeAppearance {
    if cx.global::<AppearanceGlobal>().0 == Appearance::Auto && is_system_drawn_ui_auto() {
        return NativeAppearance::System;
    }
    match effective_theme(cx) {
        Theme::Light => NativeAppearance::Light,
        Theme::Dark => NativeAppearance::Dark,
    }
}

/// Whether the parts that the system draws follow its theme correctly if we leave them alone.
///
/// Forcing a theme on Android or iOS would also hide the system's own theme from us.
/// Windows and most Linux window managers draw a light title bar unless told otherwise.
fn is_system_drawn_ui_auto() -> bool {
    if cfg!(target_os = "linux") {
        // KWin uses the user's own color scheme, which is closer than our dark or light one.
        return std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| desktop.to_ascii_uppercase().contains("KDE"));
    }
    cfg!(any(target_os = "macos", target_os = "ios", target_os = "android"))
}

/// Makes a change to the appearance setting or the system theme,
/// and then restyles the app only if that changed its theme.
pub fn apply(cx: &mut Cx, change: impl FnOnce(&mut Cx)) {
    let old_theme = effective_theme(cx);
    change(cx);
    let native_appearance = native_appearance(cx);
    cx.set_native_appearance(native_appearance);
    if effective_theme(cx) != old_theme {
        cx.request_style_reload();
    }
}

/// Handles a [`SystemThemeAction`].
pub fn set_system_theme(cx: &mut Cx, theme: Option<Theme>) {
    apply(cx, |cx| cx.global::<SystemTheme>().theme = theme);
}

/// Asks the system for its theme again, in case a theme that we forced hid a change from us.
pub fn refresh(cx: &mut Cx) {
    cx.global::<SystemTheme>().theme = query();
}
