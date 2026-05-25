//! App-wide preferences and related types.

use makepad_widgets::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppPreferences {
    #[serde(default)]
    pub view_mode: ViewModeOverride,
    #[serde(default)]
    pub send_on_enter: bool,
    #[serde(default)]
    pub thumbnail_max_height: ThumbnailMaxHeight,
    #[serde(default)]
    pub ui_zoom: UiZoom,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            view_mode: ViewModeOverride::default(),
            send_on_enter: true,
            thumbnail_max_height: ThumbnailMaxHeight::default(),
            ui_zoom: UiZoom::default(),
        }
    }
}

impl AppPreferences {
    pub fn on_view_mode_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.view_mode = self.view_mode;
        cx.action(AppPreferencesAction::ViewModeChanged(self.view_mode));
    }

    pub fn on_send_on_enter_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.send_on_enter = self.send_on_enter;
        cx.action(AppPreferencesAction::SendOnEnterChanged(self.send_on_enter));
    }

    pub fn on_thumbnail_max_height_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.thumbnail_max_height = self.thumbnail_max_height;
        match self.thumbnail_max_height.to_pixels() {
            Some(px) => {
                let px = px as f64;
                script_eval!(cx, {
                    use mod.prelude.widgets.*
                    mod.widgets.IMG_MSG_FIT.max = FitBound.Abs(#(px))
                });
            }
            None => {
                script_eval!(cx, {
                    mod.widgets.IMG_MSG_FIT.max = nil
                });
            }
        }
        cx.request_script_reapply();
    }

    pub fn on_ui_zoom_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.ui_zoom = self.ui_zoom;
        let window_id = CxWindowPool::id_zero();
        let baseline = {
            let window = &cx.windows[window_id];
            window.os_dpi_factor.unwrap_or(window.window_geom.dpi_factor)
        };
        let dpi_override = if self.ui_zoom.is_default() {
            None
        } else {
            Some(baseline * self.ui_zoom.multiplier())
        };
        let new_dpi = dpi_override.unwrap_or(baseline);
        let (old_dpi, main_pass_id) = {
            let window = &cx.windows[window_id];
            (window.window_geom.dpi_factor, window.main_pass_id)
        };
        {
            let window = &mut cx.windows[window_id];
            window.dpi_override = dpi_override;
            if (new_dpi - old_dpi).abs() > f64::EPSILON && new_dpi > 0.0 {
                window.window_geom.inner_size *= old_dpi / new_dpi;
                window.window_geom.dpi_factor = new_dpi;
            }
        }
        if let Some(main_pass_id) = main_pass_id {
            cx.redraw_pass_and_child_passes(main_pass_id);
        }
        cx.redraw_all();
        cx.action(AppPreferencesAction::UiZoomChanged(self.ui_zoom));
    }

    pub fn broadcast_all(&self, cx: &mut Cx) {
        self.on_view_mode_changed(cx);
        self.on_send_on_enter_changed(cx);
        self.on_thumbnail_max_height_changed(cx);
        self.on_ui_zoom_changed(cx);
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewModeOverride {
    #[default]
    Automatic,
    ForceWide,
    ForceNarrow,
}

impl ViewModeOverride {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::ForceWide,
            2 => Self::ForceNarrow,
            _ => Self::Automatic,
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            Self::Automatic => 0,
            Self::ForceWide => 1,
            Self::ForceNarrow => 2,
        }
    }

    pub fn variant_selector(self) -> impl FnMut(&mut Cx, &Vec2d) -> LiveId + 'static {
        move |cx: &mut Cx, _parent_size: &Vec2d| match self {
            Self::Automatic => {
                if cx.display_context.is_desktop() || !cx.display_context.is_screen_size_known() {
                    live_id!(Desktop)
                } else {
                    live_id!(Mobile)
                }
            }
            Self::ForceWide => live_id!(Desktop),
            Self::ForceNarrow => live_id!(Mobile),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThumbnailMaxHeight {
    #[default]
    Small,
    Medium,
    Unlimited,
    Custom(u32),
}

impl ThumbnailMaxHeight {
    pub fn to_pixels(&self) -> Option<u32> {
        match self {
            Self::Small => Some(200),
            Self::Medium => Some(400),
            Self::Unlimited => None,
            Self::Custom(v) => Some(*v),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiZoom(pub f32);

impl UiZoom {
    pub const MIN: f32 = 0.25;
    pub const MAX: f32 = 3.00;
    pub const DEFAULT: f32 = 1.00;

    pub const STEP: f32 = 0.02;
    pub const BUTTON_STEP: f32 = 0.05;

    pub fn new(value: f32) -> Self {
        let v = if value.is_finite() { value } else { Self::DEFAULT };
        Self(v.clamp(Self::MIN, Self::MAX))
    }

    pub fn multiplier(self) -> f64 {
        self.0 as f64
    }

    pub fn is_default(self) -> bool {
        (self.0 - Self::DEFAULT).abs() < 0.01
    }

    pub fn zoom_in_by(self, delta: f32) -> Self {
        Self::new(self.0 + delta)
    }

    pub fn zoom_out_by(self, delta: f32) -> Self {
        Self::new(self.0 - delta)
    }

    pub fn reset() -> Self {
        Self(Self::DEFAULT)
    }

    pub fn format_percent(self) -> String {
        let pct = self.0 * 100.0;
        let rounded = pct.round();
        if (pct - rounded).abs() < 0.05 {
            format!("{}%", rounded as i32)
        } else {
            format!("{:.1}%", pct)
        }
    }
}

impl Default for UiZoom {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

#[derive(Debug, Clone)]
pub enum AppPreferencesAction {
    ViewModeChanged(ViewModeOverride),
    SendOnEnterChanged(bool),
    UiZoomChanged(UiZoom),
}

#[derive(Default, Clone)]
pub struct AppPreferencesGlobal(pub AppPreferences);

pub fn effective_is_desktop(cx: &mut Cx) -> bool {
    match cx.global::<AppPreferencesGlobal>().0.view_mode {
        ViewModeOverride::ForceWide => true,
        ViewModeOverride::ForceNarrow => false,
        ViewModeOverride::Automatic => {
            cx.display_context.is_desktop() || !cx.display_context.is_screen_size_known()
        }
    }
}
