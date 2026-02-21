use std::borrow::Cow;
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use crate::ApplyOverCompat;
use crate::{LivePtr, view_from_live_ptr};

use crate::shared::styles::*;

static POPUP_NOTIFICATION: SegQueue<PopupItem> = SegQueue::new();
const POPUP_KINDS: [(PopupKind, Vec4); 4] = [
    (PopupKind::Error, COLOR_FG_DANGER_RED),
    (PopupKind::Info, COLOR_INFO_BLUE),
    (PopupKind::Success, COLOR_FG_ACCEPT_GREEN),
    (PopupKind::Warning, COLOR_WARNING_YELLOW),
];
const ICON_SET: &[&[LiveId]] = ids_array!(error_icon, info_icon, success_icon, warning_icon,);

/// Displays a new popup notification with a popup item.
///
/// This function can be used when there is no Makepad widget context in its arguments.
/// Popup notifications will be shown in the order they were enqueued,
/// and can be removed when manually closed by the user or automatically.
/// Maximum auto dismissal duration is 3 minutes.
pub fn enqueue_popup_notification(
    message: impl Into<Cow<'static, str>>,
    kind: PopupKind,
    auto_dismissal_duration: Option<f64>,
) {
    let mut popup_item = PopupItem {
        message: message.into(),
        kind,
        auto_dismissal_duration,
    };
    // Limit auto dismiss duration to 180 seconds
    popup_item.auto_dismissal_duration = popup_item
        .auto_dismissal_duration
        .map(|duration| duration.min(3. * 60.));
    POPUP_NOTIFICATION.push(popup_item);
    SignalToUI::set_ui_signal();
}

/// Retrieves a mutable reference to the global `RobrixPopupNotificationRef`.
///
/// This function accesses the global context to obtain a reference to the
/// `RobrixPopupNotificationRef`, which is used for managing and displaying
/// popup notifications within the application. It enables interaction with
/// the popup notification system from various parts of the application.
pub fn get_global_popup_list(cx: &mut Cx) -> &mut RobrixPopupNotificationRef {
    cx.get_global::<RobrixPopupNotificationRef>()
}

/// Sets the global popup list notification widget reference.
///
/// This function sets the global context to point to the provided
/// `WidgetRef`, which is expected to be a `RobrixPopupNotificationRef`.
/// It is used to display popup notifications anywhere in the application.
pub fn set_global_popup_list(cx: &mut Cx, parent_ref: &WidgetRef) {
    Cx::set_global(
        cx,
        parent_ref.robrix_popup_notification(cx, ids!(popup_notification)),
    );
}

/// Kind of a popup notification.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PopupKind {
    /// Shows no icon at all.
    #[default]
    Blank,
    /// Shows a red background and a error icon.
    Error,
    /// Shows a white background and a blue info icon.
    Info,
    /// Shows a green background and a checkmark icon.
    Success,
    /// Shows a yellow background and a warning icon.
    Warning,
}

/// Popup notification item.
#[derive(Default, Debug, Clone)]
pub struct PopupItem {
    /// Text to be displayed in the popup.
    pub message: Cow<'static, str>,
    /// Duration in seconds after which the popup will be automatically closed.
    /// Maximum duration is 3 minutes.
    /// If none, the popup will not automatically close.
    pub auto_dismissal_duration: Option<f64>,
    /// Kind of the popup defined by [`PopupKind`].
    pub kind: PopupKind,
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.CheckIcon = View {

        width: 28,
        height: 28,
        visible: false,
        Icon {
            draw_icon +: {
                svg_file: (ICON_CHECKMARK),
                color: #ffffff,
            }
            icon_walk: Walk{ width: 22, height: 22 }
        }
    }
    mod.widgets.ForbiddenIcon = mod.widgets.CheckIcon {
        Icon {
            draw_icon +: {
                svg_file: (ICON_FORBIDDEN),
                color: #ffffff,
            }
            icon_walk: Walk{ width: 22, height: 22 }
        }
    }
    mod.widgets.InfoIcon = mod.widgets.CheckIcon {
        Icon {
            draw_icon +: {
                svg_file: (ICON_INFO),
                color: #ffffff,
            }
            icon_walk: Walk{ width: 22, height: 22 }
        }
    }
    mod.widgets.WarningIcon = mod.widgets.CheckIcon {
        Icon {
            draw_icon +: {
                svg_file: (ICON_WARNING),
                color: #ffffff,
            }
            icon_walk: Walk{ width: 22, height: 22 }
        }
    }
    mod.widgets.ProgressBar = View {
        width: Fill,
        height: 10,
        show_bg: true,
        margin: Inset{ bottom: 0 },
        padding: 0,
        draw_bg +: {
            direction: uniform(0.0), // Direction of the progress bar: 0.0 is right to left, 1.0 is top to bottom.
            border_radius: uniform(4.),
            border_size: uniform(1.0),
            progress_bar_color: uniform(#00000080), //Black with 50% opacity.
            // Display progress bar when there is auto_dismissal_duration.
            display_progress_bar: uniform(1.0)
            // Display progress bar even when mode.slide is off.
            // 0.0 animate according to anim_time and anim_duration, 1.0 displays oscillating progress bar.
            debug_progress_bar: uniform(0.0),
            anim_time: uniform(0.0),
            anim_duration: uniform(2.0),
            pixel: fn() -> vec4 {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                let rect_size = self.rect_size;
                let time = self.anim_time / self.anim_duration;
                if self.debug_progress_bar > 0.5 {
                    time = sin(self.time * PI)
                }
                if self.display_progress_bar > 0.5 {
                    if self.direction > 0.5 {
                        // Top to bottom
                        sdf.box(
                            self.border_size * 2.0,
                            self.border_size * 2.0,
                            rect_size.x - self.border_size * 2.0,
                            rect_size.y * min(1.0, time) - self.border_size * 2.0,
                            max(1.0, self.border_radius)
                        )
                    } else {
                        // Right to left
                        sdf.box(
                            self.border_size * 2.0,
                            self.border_size * 2.0,
                            rect_size.x * max(0.0, 1.0 - time) - self.border_size * 2.0,
                            rect_size.y - self.border_size * 2.0,
                            max(1.0, self.border_radius)
                        )
                    }
                    sdf.fill(self.progress_bar_color);
                }
                return sdf.result
            }
        }
    }
    mod.widgets.MainContent = View {
        width: Fill,
        height: Fit,
        align: Align{ x: 0.0, y: 0.5 }
        padding: Inset{left: 0, top: 0, bottom: 10, right: 5}
        popup_label := Label {
            width: Fill,
            height: Fit,
            draw_text +: {
                color: (#000000),
                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 10 },
            }
        }
    }
    mod.widgets.LeftSideView = View {
        width: Fit,
        height: Fit,
        success_icon := mod.widgets.CheckIcon {}
        error_icon := mod.widgets.ForbiddenIcon {}
        info_icon := mod.widgets.InfoIcon {}
        warning_icon := mod.widgets.WarningIcon {}
    }
    mod.widgets.CloseButtonView = View {
        width: Fill,
        height: Fit,
        flow: Down,
        padding: Inset{ top: 3 }
        align: Align{ x: 0.98 }
        
        RoundedView {
            width: Fit, height: Fit
            show_bg: true,
            draw_bg +: {
                color: (COLOR_BG_DISABLED)
            }
            align: Align{ x: 0.5, y: 0.5 }
            // The "X" close button on the top right
            close_button := RobrixIconButton {
                width: Fit, height: Fit,
                padding: Inset{ top: 5, bottom: 5, left: 8, right: 8 },
                spacing: 0,
                align: Align{ x: 0.5, y: 0.5 }
                draw_bg +: {
                    color: (COLOR_BG_DISABLED)
                }
                draw_icon +: {
                    svg_file: (ICON_CLOSE),
                    color: (COLOR_DIVIDER_DARK),
                }
                icon_walk: Walk{width: 15, height: 15}
            }
        }
    }
    // Other possible color themes that is not too glaring.
    // COLOR_POPUP_GREEN = #43bb9e;
    // COLOR_POPUP_RED = #e74c3c;
    mod.widgets.PopupDialogRightToLeftProgress = RoundedView {
        width: 275
        height: Fit
        padding: 0,
        flow: Overlay
        show_bg: true,
        draw_bg +: {
            border_radius: uniform(4.0)
            border_color: uniform(#000000)
            border_size: uniform(2.0)
            background_color: instance(#ffffff)
            pixel: fn() -> vec4 {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                sdf.box(
                    1.,
                    1.,
                    self.rect_size.x - 2.0,
                    self.rect_size.y - 2.0,
                    self.border_radius
                )
                sdf.fill_keep(self.background_color)

                // Only draw black border for white background (blank popups)
                if length(self.background_color.rgb - vec3(1.0, 1.0, 1.0)) < 0.1 {
                    sdf.stroke(
                        self.border_color,
                        self.border_size
                    )
                }
                return sdf.result
            }
        }

        popup_content := View {
            width: Fill, height: Fit,
            flow: Down
            //Right side view with close button
            close_button_view := mod.widgets.CloseButtonView {}
            padding: Inset{ right: 2, top: 2}
            inner := View {
                width: Fill, height: Fit,
                padding: Inset{ top: 0, right: 5, bottom: 0, left: 10 }
                flow: Right,
                align: Align{
                    y: 0,
                }
                // Left side with icon for popup kind.
                mod.widgets.LeftSideView {}
                // Main content area
                main_content := mod.widgets.MainContent {}
            }
            progress_bar := mod.widgets.ProgressBar {}
            // Add a small gap between the progress bar and the end of the popup 
            // to ensure the progress bar is within the popup.
            View {
                height: 0.2
            }
        }

        animator: Animator{
            mode: {
                default: @close_slider
                close_slider: AnimatorState{
                    redraw: true
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        popup_content: {
                            progress_bar: {
                                draw_bg: {anim_time: 0.0}
                            }
                        }
                    }
                }
                slide: AnimatorState{
                    redraw: true
                    // Maximum auto dismissal duration is 3 minutes.
                    from: {all: Forward {duration: 180.0}}
                    apply: {
                        popup_content: {
                            progress_bar: {
                                draw_bg: {anim_time: 180.0}
                            }
                        }
                    }
                }
            }
            hover: {
                default: @off
                off: AnimatorState{ apply: { } }
                on: AnimatorState{ apply: { } }
            }
            down: {
                default: @off
                off: AnimatorState{ apply: { } }
                on: AnimatorState{ apply: { } }
            }
        }
    }
    mod.widgets.PopupDialogTopToBottomProgress = mod.widgets.PopupDialogRightToLeftProgress {
        popup_content := View {
            width: Fill,
            height: Fit,
            flow: Right,
            spacing: 0,
            align: Align{ x: 0.0, y: 0.5 }
            // Left side with for popup kind.
            mod.widgets.LeftSideView {
                height: Fit,
                margin: Inset{left: 10 }
                spacing: 0,
            }
            inner := View {
                width: 230,
                height: Fit,
                padding: 0,
                flow: Down,
                close_button_view := mod.widgets.CloseButtonView {}
                // Main content area
                main_content := mod.widgets.MainContent {
                    padding: Inset{left: 0}
                }
            }
            progress_bar := mod.widgets.ProgressBar {
                width: 10,
                height: Fill,
                draw_bg +: {
                    direction: uniform(1.0),
                    anim_time: uniform(1.0),
                    border_radius: uniform(2.),
                }
            }
        }
    }
    mod.widgets.RobrixPopupNotificationRightToLeftProgress = #(RobrixPopupNotification::register_widget(vm)) {
        width: 275
        height: Fit
        flow: Down
        draw_bg +: {
            pixel: fn() -> vec4 {
                return vec4(0., 0., 0., 0.0)
            }
        }
        content: mod.widgets.PopupDialogRightToLeftProgress {}
    }
    mod.widgets.RobrixPopupNotificationTopToBottomProgress = mod.widgets.RobrixPopupNotificationRightToLeftProgress {
        content: mod.widgets.PopupDialogTopToBottomProgress {}
    }
    // A widget that displays a vertical list of popups at the top right corner of the screen.
    // The progress bar slides from right to left.
    mod.widgets.PopupList = View {
        width: Fill,
        height: Fill,
        margin: Inset{ top: 10 }
        align: Align{x: 0.99, }
        popup_notification := mod.widgets.RobrixPopupNotificationRightToLeftProgress {}
    }
    // A widget that displays a vertical list of popups at the top right corner of the screen.
    // The progress bar slides from top to bottom.
    mod.widgets.PopupListTopToBottomProgress = mod.widgets.PopupList {
        popup_notification := mod.widgets.RobrixPopupNotificationTopToBottomProgress {}
    }
}

/// A widget that displays a vertical list of popups.
#[derive(Script, Widget)]
pub struct RobrixPopupNotification {
    #[live]
    pub content: Option<LivePtr>,

    #[rust]
    draw_list: Option<DrawList2d>,

    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    // A list of tuples containing individual widgets, its content and the close timer in the order they were added.
    #[rust]
    pub popups: Vec<(View, PopupItem, Timer)>,
}

impl ScriptHook for RobrixPopupNotification {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        self.draw_list = Some(DrawList2d::script_new(vm));
    }

    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        _apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        vm.with_cx_mut(|cx| {
            if let Some(draw_list) = &self.draw_list {
                draw_list.redraw(cx);
            }
        });
    }
}

impl Widget for RobrixPopupNotification {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if matches!(event, Event::Signal) {
            while let Some(popup_item) = POPUP_NOTIFICATION.pop() {
                self.push(cx, popup_item);
            }
        }
        if self.popups.is_empty() {
            return;
        }
        for (index, (view, _popup_item, close_popup_timer)) in self.popups.iter_mut().enumerate() {
            view.handle_event(cx, event, scope);
            if close_popup_timer.is_event(event).is_some() {
                self.popups.remove(index);
                // Without this redraw, the last popup will not be removed from the screen automatically.
                self.draw_bg.redraw(cx);
                break;
            }
        }
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let draw_list = self.draw_list.as_mut().unwrap();
        draw_list.begin_overlay_reuse(cx);
        self.draw_bg.begin(cx, walk, self.layout);
        if !self.popups.is_empty() {
            cx.begin_turtle(walk, self.layout);
            for (view, _popup_item, _) in self.popups.iter_mut() {
                let walk = walk.with_margin_bottom(5.0);
                let _ = view.draw_walk(cx, scope, walk);
            }
            cx.end_turtle();
        }
        self.draw_bg.end(cx);
        draw_list.end(cx);
        DrawStep::done()
    }
}

impl RobrixPopupNotification {
    /// Adds a new popup with a close button to the right side of the screen.
    ///
    /// The popup's content is a string given by the `PopupItem` parameter.
    /// New popup will be displayed below the previous ones.
    pub fn push(&mut self, cx: &mut Cx, popup_item: PopupItem) {
        let mut view = view_from_live_ptr(cx, self.content);
        let mut background_color = None;
        view.label(cx, ids!(popup_label))
            .set_text(cx, &popup_item.message);
        for (view, (popup_kind, color)) in view.view_set(cx, ICON_SET).iter().zip(POPUP_KINDS) {
            if popup_item.kind == popup_kind {
                view.set_visible(cx, true);
                background_color = Some(color);
            } else {
                view.set_visible(cx, false);
            }
        }
        // Apply popup item kind-specific styling
        if let Some(_background_color) = background_color {
            let _text_color = if popup_item.kind == PopupKind::Warning {
                vec4(0.0, 0.0, 0.0, 1.0) // Black text for Warning
            } else {
                COLOR_WHITE // White text for all other kinds
            };
            
            (&mut view).apply_over(
                cx,
                live! {
                    popup_content: {
                        inner: {
                            main_content: {
                                popup_label: {
                                    draw_text: {
                                        color: (text_color),
                                    }
                                }
                            }
                            // For top to bottom progress bar.
                            close_button_view: {
                                close_button: {
                                    draw_icon: {
                                        color: (text_color),
                                    }
                                }
                            }
                        }
                        // For Right to left progress bar.
                        close_button_view: {
                            close_button: {
                                draw_icon: {
                                    color: (text_color),
                                }
                            }
                        }
                    }
                    draw_bg: {
                        background_color: ( background_color )
                    }
                },
            );
        }
        let close_timer = if let Some(duration) = popup_item.auto_dismissal_duration {
            (&mut view).apply_over(
                cx,
                live! {
                    popup_content: {
                        progress_bar: {
                            draw_bg: { anim_duration: (duration) }
                        }
                    }
                },
            );
            view.animator_play(cx, ids!(mode.slide));
            cx.start_timeout(duration)
        } else {
            (&mut view).apply_over(
                cx,
                live! {
                    popup_content: {
                        progress_bar: {
                            draw_bg: { display_progress_bar: 0.0 }
                        }
                    }
                },
            );
            Timer::empty()
        };
        self.popups.push((view, popup_item, close_timer));
        self.redraw(cx);
    }

    /// Adds a new popup with a custom view to the right side of the screen.
    ///
    /// This allows arbitrary content to be displayed via RobrixPopupNotification's content live dsl.
    ///
    /// The `view` parameter should be constructed using `RobrixPopupNotification::content()`.
    /// The view should have a view with the id `popup_content` which should contain
    /// a `progress_bar` view with the id `progress_bar` and a `popup_label` view with the id `popup_label`.
    /// The `progress_bar` view should have a `draw_bg` field with a `anim_duration` field that will be used to animate the progress bar.
    /// The `popup_label` view should have a `draw_text` field that will be used to display the popup's message.
    /// The custom view should also have a close button with the id `close_button` which should have a `draw_icon` field that will be used to display the close button's icon.
    ///
    /// The `auto_dismissal_duration` field of the `PopupItem` parameter will be used to automatically dismiss the popup after the given duration.
    /// If `auto_dismissal_duration` is `None`, the popup will not be automatically dismissed and the user will have to manually close it.
    /// The maximum auto dismissal duration is 3 minutes.
    ///
    /// # Examples
    ///
    /// ```
    /// crate::shared::popup_list::set_global_popup_list(cx, &self.ui);
    /// let content = crate::shared::popup_list::get_global_popup_list(cx).content();
    /// let view = View::new_from_ptr(cx, content);
    /// let popup_item = PopupItem {
    ///     kind: PopupKind::Info,
    ///     message: Cow::Borrowed("Welcome!"),
    ///     auto_dismissal_duration: Some(4.0),
    /// };
    ///  view.label(cx, ids!(popup_label))
    ///     .set_text(cx, &popup_item.message);
    ///  let close_timer = if let Some(duration) = popup_item.auto_dismissal_duration {
    ///     cx.start_timeout(duration)
    /// } else {
    ///     Timer::empty()
    /// };
    /// crate::shared::popup_list::get_global_popup_list(cx).push_with_custom_view(popup_item, view, close_timer);
    /// ```
    pub fn push_with_custom_view(
        &mut self,
        mut popup_item: PopupItem,
        view: View,
        close_timer: Timer,
    ) {
        popup_item.auto_dismissal_duration = popup_item
            .auto_dismissal_duration
            .map(|duration| duration.min(3. * 60.));
        self.popups.push((view, popup_item, close_timer));
    }

    /// Returns a clone of the template for each  popup in the list.
    ///
    /// This is used to construct the View for the popup notification.
    pub fn content(&self) -> Option<LivePtr> {
        self.content
    }
}

impl WidgetMatchEvent for RobrixPopupNotification {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        for (i, (view, _popup_item, close_timer)) in self.popups.iter_mut().enumerate() {
            if view.button(cx, ids!(close_button)).clicked(actions) {
                cx.stop_timer(*close_timer);
                view.animator_cut(cx, ids!(mode.close_slider));
                self.popups.remove(i);
                self.draw_bg.redraw(cx);
                break;
            }
        }
    }
}
impl RobrixPopupNotificationRef {
    /// See [`RobrixPopupNotification::push()`].
    pub fn push(&self, cx: &mut Cx, popup_item: PopupItem) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.push(cx, popup_item);
        } else {
            log!("RobrixPopupNotificationRef is not initialized.");
        }
    }

    /// See [`RobrixPopupNotification::content()`].
    pub fn content(&self) -> Option<LivePtr> {
        if let Some(inner) = self.borrow() {
            inner.content()
        } else {
            None
        }
    }

    /// See [`RobrixPopupNotification::push_with_custom_view()`].
    pub fn push_with_custom_view(&self, popup_item: PopupItem, view: View, close_timer: Timer) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.push_with_custom_view(popup_item, view, close_timer);
        } else {
            log!("RobrixPopupNotificationRef is not initialized.");
        }
    }
}
