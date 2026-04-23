//! `NavigationBarButton`: a unified base button widget for the
//! `NavigationTabBar` and `SpacesBar`.
//!
//! This widget exists to consolidate the three previously-disjoint button styles
//! used in the navigation bar (Makepad radio buttons, `SpacesBarEntry`s, and the
//! `ProfileIcon`) into a single base type.
//!
//! ## Behavior
//! * **Arbitrary inner content.** A `NavigationBarButton` is a [`View`] under the
//!   hood, so any DSL children (icons, avatars, badges, etc.) nest naturally.
//! * **Hover animation.** On `FingerHoverIn`, the background fades to
//!   `COLOR_NAVIGATION_TAB_BG_HOVER` (a lighter gray).
//! * **"Selected" state.** Programmatically toggled via
//!   [`NavigationBarButton::set_selected`]; renders the background as
//!   `COLOR_NAVIGATION_TAB_BG_ACTIVE` (a darker gray).
//! * **Optional built-in tooltip.** When `tooltip_text` is non-empty, the widget
//!   emits `TooltipAction::HoverIn` / `HoverOut` with that text on hover.
//!   When empty (e.g. for `ProfileIcon`, which provides its own dynamic tooltip
//!   with verification badge info), the widget emits no tooltip actions.
//! * **Click actions.** Emits [`NavigationBarButtonAction::Clicked`] on primary
//!   tap and [`NavigationBarButtonAction::SecondaryClicked`] on right-click /
//!   long-press.
//!
//! ## Selection model
//! Selection is *not* enforced by this widget. The parent (e.g. `NavigationTabBar`)
//! is responsible for ensuring radio-button-like mutual exclusion: when one
//! button is clicked, the parent should call [`NavigationBarButtonRef::set_selected`]
//! on each of the buttons in the group as appropriate. This makes the widget
//! flexible enough to also be used as a non-exclusive toggle (e.g. for the
//! "show/hide spaces bar" button), where the parent simply never marks it as
//! selected.

use makepad_widgets::*;

use crate::settings::app_settings_data::effective_is_desktop;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.NavigationBarButton = #(NavigationBarButton::register_widget(vm)) {
        width: Fit,
        height: Fit,
        align: Align{x: 0.5, y: 0.5}
        cursor: MouseCursor.Hand

        // Empty by default. Parent DSL definitions can set this to enable
        // a built-in tooltip on hover. Leave empty if the parent provides
        // its own tooltip handling (e.g. ProfileIcon).
        tooltip_text: ""

        show_bg: true
        draw_bg +: {
            hover: instance(0.0)
            active: instance(0.0)

            color: instance(#0000)
            color_hover: instance((COLOR_NAVIGATION_TAB_BG_HOVER))
            color_active: instance((COLOR_NAVIGATION_TAB_BG_ACTIVE))

            border_color: instance(#0000)
            border_size: uniform(0.0)
            border_radius: uniform(4.0)
            border_inset: uniform(vec4(0.0))

            get_color: fn() -> vec4 {
                return mix(
                    mix(
                        self.color,
                        self.color_hover,
                        self.hover
                    ),
                    self.color_active,
                    self.active
                )
            }

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(
                    self.border_inset.x + self.border_size,
                    self.border_inset.y + self.border_size,
                    self.rect_size.x - (self.border_inset.x + self.border_inset.z + self.border_size * 2.0),
                    self.rect_size.y - (self.border_inset.y + self.border_inset.w + self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.border_color, self.border_size)
                }
                return sdf.result;
            }
        }

        animator: Animator {
            hover: {
                default: @off
                off: AnimatorState{
                    from: {all: Forward {duration: 0.15}}
                    apply: { draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 0.0} }
                }
                on: AnimatorState{
                    from: {all: Snap}
                    apply: { draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 1.0} }
                }
                down: AnimatorState{
                    from: {all: Forward {duration: 0.2}}
                    apply: { draw_bg: {down: [{time: 0.0, value: 1.0}], hover: 1.0} }
                }
            }
            active: {
                default: @off
                off: AnimatorState{
                    from: {all: Snap}
                    apply: { draw_bg: {active: 0.0} }
                }
                on: AnimatorState{
                    from: {all: Snap}
                    apply: { draw_bg: {active: 1.0} }
                }
            }
        }
    }
}

/// A unified base button widget used by the `NavigationTabBar` and `SpacesBar`.
///
/// See the [module docs](self) for behavior and the selection model.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct NavigationBarButton {
    #[source] source: ScriptObjectRef,
    /// The inner View. Public so that wrapper widgets (e.g. `ProfileIcon`,
    /// `SpacesBarEntry`) which embed a `NavigationBarButton` via `#[deref]` can
    /// reach its child widgets via `child_by_path` for configuration during draw.
    #[deref] pub view: View,
    #[apply_default] animator: Animator,

    /// The text shown in a built-in tooltip when this button is hovered.
    /// If empty, no tooltip action is emitted (the parent may provide its own).
    #[live] tooltip_text: String,
}

impl Widget for NavigationBarButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        // IMPORTANT: run our own `event.hits()` BEFORE forwarding to children.
        // Otherwise child widgets (e.g. an Avatar's View with `show_bg`) will
        // mark the event as handled on their own area, causing our hit test to
        // see `handled_area != self.area` and return `FingerHoverOut` (or
        // consume the click). After our hit test fires, children still receive
        // Event::Actions / Signal / etc. via the forward below — they just
        // won't intercept the hover/click on our button area.
        let area = self.view.area();
        let widget_uid = self.widget_uid();

        let emit_hover_in = |this: &Self, cx: &mut Cx| {
            let widget_rect = area.rect(cx);
            // Always emit a HoverIn action so that wrapping widgets (e.g. `ProfileIcon`)
            // can react with their own custom tooltip. This avoids requiring those
            // wrappers to call `event.hits()` again on the same area.
            cx.widget_action(
                widget_uid,
                NavigationBarButtonAction::HoverIn { widget_rect },
            );
            // If a built-in tooltip text is set, additionally emit a TooltipAction.
            if this.tooltip_text.is_empty() { return; }
            let is_desktop = effective_is_desktop(cx);
            cx.widget_action(
                widget_uid,
                TooltipAction::HoverIn {
                    widget_rect,
                    text: this.tooltip_text.clone(),
                    options: CalloutTooltipOptions {
                        position: if is_desktop {
                            TooltipPosition::Right
                        } else {
                            TooltipPosition::Top
                        },
                        ..Default::default()
                    },
                },
            );
        };

        let emit_hover_out = |this: &Self, cx: &mut Cx| {
            cx.widget_action(widget_uid, NavigationBarButtonAction::HoverOut);
            if !this.tooltip_text.is_empty() {
                cx.widget_action(widget_uid, TooltipAction::HoverOut);
            }
        };

        match event.hits(cx, area) {
            Hit::FingerHoverIn(_) => {
                self.animator_play(cx, ids!(hover.on));
                emit_hover_in(self, cx);
            }
            Hit::FingerHoverOut(_) => {
                self.animator_play(cx, ids!(hover.off));
                emit_hover_out(self, cx);
            }
            Hit::FingerDown(fe) => {
                self.animator_play(cx, ids!(hover.down));
                if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                    cx.widget_action(widget_uid, NavigationBarButtonAction::SecondaryClicked);
                }
            }
            Hit::FingerLongPress(_) => {
                self.animator_play(cx, ids!(hover.down));
                emit_hover_in(self, cx);
                cx.widget_action(widget_uid, NavigationBarButtonAction::SecondaryClicked);
            }
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                self.animator_play(cx, ids!(hover.on));
                cx.widget_action(widget_uid, NavigationBarButtonAction::Clicked);
            }
            Hit::FingerUp(fe) if !fe.is_over => {
                self.animator_play(cx, ids!(hover.off));
            }
            _ => {}
        }

        // Forward to children so they still receive non-hit events
        // (Event::Actions, Event::Signal, etc.). Their own hit tests on this
        // same area will short-circuit because we've already handled it above.
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl NavigationBarButton {
    /// Sets the text shown in the built-in tooltip when this button is hovered.
    /// Pass an empty string to disable the built-in tooltip.
    pub fn set_tooltip_text(&mut self, text: impl Into<String>) {
        self.tooltip_text = text.into();
    }

    /// Sets whether this button is visually marked as selected.
    ///
    /// The "selected" state is drawn with a darker background
    /// (`COLOR_NAVIGATION_TAB_BG_ACTIVE`).
    ///
    /// Selection is *not* exclusive — to achieve radio-button-like behavior,
    /// the caller is responsible for clearing the selection on the other
    /// `NavigationBarButton`s in the group.
    pub fn set_selected(&mut self, cx: &mut Cx, is_selected: bool) {
        self.animator_toggle(cx, is_selected, Animate::No, ids!(active.on), ids!(active.off));
    }

    /// Returns `true` if this button was clicked (primary tap) in the given actions.
    pub fn clicked(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            return matches!(item.cast(), NavigationBarButtonAction::Clicked);
        }
        false
    }

    /// Returns `true` if this button was secondary-clicked or long-pressed
    /// in the given actions.
    pub fn secondary_clicked(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            return matches!(item.cast(), NavigationBarButtonAction::SecondaryClicked);
        }
        false
    }
}

impl NavigationBarButtonRef {
    /// See [`NavigationBarButton::set_selected`].
    pub fn set_selected(&self, cx: &mut Cx, is_selected: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_selected(cx, is_selected);
    }

    /// See [`NavigationBarButton::clicked`].
    pub fn clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.clicked(actions))
    }

    /// See [`NavigationBarButton::secondary_clicked`].
    pub fn secondary_clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.secondary_clicked(actions))
    }
}

/// Actions emitted by [`NavigationBarButton`] in response to user input.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum NavigationBarButtonAction {
    /// The user primary-clicked or tapped this button.
    Clicked,
    /// The user secondary-clicked or long-pressed this button.
    SecondaryClicked,
    /// The button has been hovered over (mouse-enter or long-press start).
    /// Includes the on-screen rect of the button so wrappers can position
    /// their own custom tooltip.
    HoverIn { widget_rect: Rect },
    /// The pointer has left the button.
    HoverOut,
    #[default]
    None,
}
