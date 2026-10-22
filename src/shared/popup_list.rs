use std::borrow::Cow;
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use crate::{LivePtr, view_from_live_ptr};

static PENDING_POPUP_NOTIFICATIONS: SegQueue<PopupItem> = SegQueue::new();

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
    PENDING_POPUP_NOTIFICATIONS.push(popup_item);
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

    mod.widgets.PopupProgressBar = View {
        width: Fill,
        height: 10,
        show_bg: true,
        margin: Inset{ bottom: 0 },
        padding: 0,
        draw_bg +: {
            direction: uniform(0.0), // Direction of the progress bar: 0.0 is right to left, 1.0 is top to bottom.
            border_radius:uniform(4.),
            border_size:uniform(1.0),
            progress_bar_color: uniform((COLOR_POPUP_PROGRESS_BAR)), //Black with 50% opacity.
            // Display progress bar when there is auto_dismissal_duration.
            display_progress_bar: instance(1.0)
            // Display progress bar even when progress.on is off.
            // 0.0 animate according to anim_time, 1.0 displays an oscillating progress bar.
            debug_progress_bar: uniform(0.0),
            anim_time: instance(0.0),
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                let rect_size = self.rect_size;
                let is_debug = step(0.5, self.debug_progress_bar);
                let time = (1.0 - is_debug) * self.anim_time + is_debug * (0.5 + 0.5 * sin(self.draw_pass.time * PI));
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
        animator: Animator{
            progress: {
                default: @off
                off: AnimatorState{
                    redraw: true
                    from: {all: Snap}
                    apply: {
                        draw_bg: {anim_time: 0.0}
                    }
                }
                on: AnimatorState{
                    redraw: true
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        draw_bg: {anim_time: 1.0}
                    }
                }
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
                color: (COLOR_POPUP_TEXT)
                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 10.5 },
            }
        }
    }
    mod.widgets.LeftSideView = View {
        width: Fit,
        height: Fit,
        visible: false,
        popup_icon := Icon {
            width: 28,
            height: 28,
            draw_icon +: {
                svg: (ICON_CHECKMARK),
                color: (COLOR_TEXT_ON_ACCENT),
            }
            icon_walk: Walk{ width: 22, height: 22 }
        }
    }

    mod.widgets.PopupCloseButton = ButtonFlat {
        width: 32,
        height: 32,
        text: "",
        spacing: 0,
        margin: 0,
        padding: 0,
        align: Align{ x: 0.5, y: 0.5 }
        label_walk: Walk{ width: 0, height: 0 }
        icon_walk: Walk{ width: 14, height: 14, margin: 0 }
        draw_bg +: {
            border_radius: 4.0
            border_size: 1.0
            color: (COLOR_POPUP_CLOSE_BG)
            color_hover: (COLOR_POPUP_CLOSE_BG_HOVER)
            color_down: (COLOR_POPUP_CLOSE_BG_DOWN)
            border_color: (COLOR_POPUP_CLOSE_BORDER)
            border_color_hover: (COLOR_POPUP_CLOSE_BORDER_HOVER)
            border_color_down: (COLOR_POPUP_CLOSE_BORDER_DOWN)
        }
        draw_icon +: {
            svg: (ICON_CLOSE),
            color: (COLOR_POPUP_CLOSE_ICON),
        }
    }

    mod.widgets.CloseButtonView = View {
        width: Fill,
        height: Fit,
        flow: Down,
        padding: Inset{ top: 5, right: 5 }
        align: Align{ x: 1.0 }

        close_button := mod.widgets.PopupCloseButton {}
    }

    // Other possible color themes that is not too glaring.
    // COLOR_POPUP_GREEN = #43bb9e;
    // COLOR_POPUP_RED = #e74c3c;
    mod.widgets.PopupDialogRightToLeftProgress = RoundedView {
        width: 275
        height: Fit
        padding: 0,
        flow: Overlay
        // Clicks on the popup shouldn't reach whatever is underneath it, or take key focus.
        cursor: MouseCursor.Default,
        grab_key_focus: false,
        show_bg: true,
        draw_bg +: {
            border_radius: uniform(4.0)
            border_color: instance((COLOR_POPUP_BORDER))
            border_size: uniform(2.0)
            color: instance((COLOR_BG_FLOATING))
            display_border: instance(1.0)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                sdf.box(
                    1.,
                    1.,
                    self.rect_size.x - 2.0,
                    self.rect_size.y - 2.0,
                    self.border_radius
                )
                sdf.fill_keep(self.color)

                // Only draw black border for blank popups
                if self.display_border > 0.5 {
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
                left_side_view := mod.widgets.LeftSideView {}
                // Main content area
                main_content := mod.widgets.MainContent {}
            }
            progress_bar := mod.widgets.PopupProgressBar {}
            // Add a small gap between the progress bar and the end of the popup 
            // to ensure the progress bar is within the popup.
            View {
                height: 0.2
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
            left_side_view := mod.widgets.LeftSideView {
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
            progress_bar := mod.widgets.PopupProgressBar {
                width: 10,
                height: Fill,
                draw_bg +: {
                    direction: 1.0,
                    border_radius: 2.0,
                }
            }
        }
    }

    mod.widgets.RobrixPopupNotificationRightToLeftProgress = set_type_default() do #(RobrixPopupNotification::register_widget(vm)) {
        ..mod.widgets.SolidView

        width: 275
        height: Fit
        flow: Down
        draw_bg +: {
            color: (COLOR_TRANSPARENT)
        }
        content: mod.widgets.PopupDialogRightToLeftProgress {}
        bg_color_blank: (COLOR_BG_FLOATING)
        bg_color_error: (COLOR_BG_DANGER_RED_SOLID)
        bg_color_info: (COLOR_INFO_BLUE)
        bg_color_success: (COLOR_BG_ACCEPT_GREEN_SOLID)
        bg_color_warning: (COLOR_WARNING_YELLOW)
        text_color_on_accent: (COLOR_TEXT_ON_ACCENT)
        text_color_on_warning: (COLOR_TEXT_ON_WARNING)
    }

    mod.widgets.RobrixPopupNotificationTopToBottomProgress = mod.widgets.RobrixPopupNotificationRightToLeftProgress {
        content: mod.widgets.PopupDialogTopToBottomProgress {}
    }

    // A widget that displays a vertical list of popups at the top right corner of the screen.
    // The progress bar slides from right to left.
    // The margins are to ensure that popups don't get rendered in the device safe inset area.
    mod.widgets.PopupList = View {
        width: Fill,
        height: Fill,
        margin: Inset{
            top: 10,
            left: (mod.widgets.SAFE_INSET_PAD_LEFT),
            right: (mod.widgets.SAFE_INSET_PAD_RIGHT),
        }
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
struct PopupEntry {
    view: View,
    close_timer: Timer,
    kind: PopupKind,
    has_progress: bool,
}

/// A widget that displays a vertical list of popups.
#[derive(Script, Widget)]
pub struct RobrixPopupNotification {
    #[uid] uid: WidgetUid,
    #[source] source: ScriptObjectRef,
    #[live] pub content: Option<LivePtr>,
    #[live] bg_color_blank: Vec4f,
    #[live] bg_color_error: Vec4f,
    #[live] bg_color_info: Vec4f,
    #[live] bg_color_success: Vec4f,
    #[live] bg_color_warning: Vec4f,
    /// The text color for popups with a strongly colored background (error, info, success).
    #[live] text_color_on_accent: Vec4f,
    #[live] text_color_on_warning: Vec4f,

    #[rust] draw_list: Option<DrawList2d>,
    #[redraw] #[live] draw_bg: DrawQuad,
    #[layout] layout: Layout,
    #[walk] walk: Walk,
    // A list of tuples containing individual widgets, its content and the close timer in the order they were added.
    #[rust] popups: Vec<PopupEntry>,
}

impl ScriptHook for RobrixPopupNotification {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        self.draw_list = Some(DrawList2d::script_new(vm));
    }

    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        apply: &Apply,
        scope: &mut Scope,
        _value: ScriptValue,
    ) {
        // A reapply (e.g., a theme change) never reaches our popups, so we re-apply their template
        // and restyle them. Other reloads would stop their progress animations, so we skip those.
        if apply.is_script_reapply() && let Some(content) = self.content {
            for popup in &mut self.popups {
                popup.view.script_apply(vm, apply, scope, content);
            }
            vm.with_cx_mut(|cx| {
                for index in 0..self.popups.len() {
                    self.apply_popup_style(cx, index);
                }
            });
        }
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
            while let Some(popup_item) = PENDING_POPUP_NOTIFICATIONS.pop() {
                self.push(cx, popup_item);
            }
        }
        if self.popups.is_empty() {
            return;
        }

        let mut remove_index = None;

        for (index, popup) in self.popups.iter_mut().enumerate() {
            popup.view.handle_event(cx, event, scope);

            if remove_index.is_none() && popup.close_timer.is_event(event).is_some() {
                remove_index = Some(index);
            }
        }

        if let Some(index) = remove_index {
            self.popups.remove(index);
            self.redraw_overlay(cx);
        }

        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let draw_list = self.draw_list.as_mut().unwrap();
        draw_list.begin_overlay_reuse(cx);
        self.draw_bg.begin(cx, walk, self.layout);
        if !self.popups.is_empty() {
            cx.begin_turtle(walk, self.layout);
            for popup in self.popups.iter_mut() {
                let walk = walk.with_margin_bottom(5.0);
                let _ = popup.view.draw_walk(cx, scope, walk);
            }
            cx.end_turtle();
        }
        self.draw_bg.end(cx);
        draw_list.end(cx);
        DrawStep::done()
    }
}

impl RobrixPopupNotification {
    fn redraw_overlay(&self, cx: &mut Cx) {
        if let Some(draw_list) = &self.draw_list {
            draw_list.redraw(cx);
        }
        self.draw_bg.redraw(cx);
    }

    /// Adds a new popup with a close button to the right side of the screen.
    ///
    /// The popup's content is a string given by the `PopupItem` parameter.
    /// New popup will be displayed below the previous ones.
    pub fn push(&mut self, cx: &mut Cx, popup_item: PopupItem) {
        let view = view_from_live_ptr(cx, self.content);
        view.label(cx, ids!(popup_label)).set_text(cx, &popup_item.message);

        let progress_bar = view.view(cx, ids!(popup_content.progress_bar));
        progress_bar.animator_cut(cx, ids!(progress.off));
        let close_timer = if let Some(duration) = popup_item.auto_dismissal_duration {
            progress_bar.animator_play_with(cx, ids!(progress.on), Play::Forward { duration });
            cx.start_timeout(duration)
        } else {
            Timer::empty()
        };
        self.popups.push(PopupEntry {
            view,
            close_timer,
            kind: popup_item.kind,
            has_progress: popup_item.auto_dismissal_duration.is_some(),
        });
        self.apply_popup_style(cx, self.popups.len() - 1);
        self.redraw_overlay(cx);
    }

    /// Styles the popup at the given index based on its kind.
    ///
    /// Its view must have just been built or re-applied from the `content` template.
    fn apply_popup_style(&mut self, cx: &mut Cx, index: usize) {
        let Some(popup) = self.popups.get_mut(index) else { return };
        let view = &mut popup.view;
        let mut popup_icon = view.widget(cx, ids!(popup_content.inner.left_side_view.popup_icon));
        let mut close_button = view.button(cx, ids!(close_button));

        // Set the icon and icon color based on the popup kind.
        let show_left_side_view = popup.kind != PopupKind::Blank;
        view.view(cx, ids!(popup_content.inner.left_side_view)).set_visible(cx, show_left_side_view);
        match popup.kind {
            PopupKind::Error => {
                script_apply_eval!(cx, popup_icon, {
                    draw_icon.svg: mod.widgets.ICON_FORBIDDEN,
                    draw_icon.color: mod.widgets.COLOR_TEXT_ON_ACCENT,
                });
            }
            PopupKind::Info => {
                script_apply_eval!(cx, popup_icon, {
                    draw_icon.svg: mod.widgets.ICON_INFO,
                    draw_icon.color: mod.widgets.COLOR_TEXT_ON_ACCENT,
                });
            }
            PopupKind::Success => {
                script_apply_eval!(cx, popup_icon, {
                    draw_icon.svg: mod.widgets.ICON_CHECKMARK,
                    draw_icon.color: mod.widgets.COLOR_TEXT_ON_ACCENT,
                });
            }
            PopupKind::Warning => {
                script_apply_eval!(cx, popup_icon, {
                    draw_icon.svg: mod.widgets.ICON_WARNING,
                    draw_icon.color: mod.widgets.COLOR_TEXT_ON_WARNING,
                });
            }
            PopupKind::Blank => {}
        }

        // Set the text and close button color based on the popup kind.
        match popup.kind {
            PopupKind::Error | PopupKind::Info | PopupKind::Success => {
                view.label(cx, ids!(popup_label)).set_text_color(cx, self.text_color_on_accent);
                script_apply_eval!(cx, close_button, {
                    draw_bg +: {
                        color:              mod.widgets.COLOR_POPUP_CLOSE_BG_ON_ACCENT,
                        color_hover:        mod.widgets.COLOR_POPUP_CLOSE_BG_ON_ACCENT_HOVER,
                        color_down:         mod.widgets.COLOR_POPUP_CLOSE_BG_ON_ACCENT_DOWN,
                        border_color:       mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_ACCENT,
                        border_color_hover: mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_ACCENT_HOVER,
                        border_color_down:  mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_ACCENT_DOWN,
                    },
                    draw_icon +: { color: mod.widgets.COLOR_POPUP_CLOSE_ICON_ON_ACCENT }
                });
            }
            PopupKind::Warning => {
                view.label(cx, ids!(popup_label)).set_text_color(cx, self.text_color_on_warning);
                script_apply_eval!(cx, close_button, {
                    draw_bg +: {
                        color:              mod.widgets.COLOR_POPUP_CLOSE_BG_ON_WARNING,
                        color_hover:        mod.widgets.COLOR_POPUP_CLOSE_BG_ON_WARNING_HOVER,
                        color_down:         mod.widgets.COLOR_POPUP_CLOSE_BG_ON_WARNING_DOWN,
                        border_color:       mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_WARNING,
                        border_color_hover: mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_WARNING_HOVER,
                        border_color_down:  mod.widgets.COLOR_POPUP_CLOSE_BORDER_ON_WARNING_DOWN,
                    },
                    draw_icon +: { color: mod.widgets.COLOR_POPUP_CLOSE_ICON_ON_WARNING }
                });
            }
            // A blank popup keeps the template's text and close button.
            PopupKind::Blank => {}
        }

        // Set the background color of the popup based on its kind.
        let bg_color = match popup.kind {
            PopupKind::Blank => self.bg_color_blank,
            PopupKind::Error => self.bg_color_error,
            PopupKind::Info => self.bg_color_info,
            PopupKind::Success => self.bg_color_success,
            PopupKind::Warning => self.bg_color_warning,
        };
        let display_border = if popup.kind == PopupKind::Blank { 1.0 } else { 0.0 };
        view.draw_bg.set_dyn_instance(cx, id!(color), &[bg_color.x, bg_color.y, bg_color.z, bg_color.w]);
        view.draw_bg.set_dyn_instance(cx, id!(display_border), &[display_border]);

        if let Some(mut progress_bar) = view.view(cx, ids!(popup_content.progress_bar)).borrow_mut() {
            let display_progress_bar = if popup.has_progress { 1.0 } else { 0.0 };
            progress_bar.draw_bg.set_dyn_instance(cx, id!(display_progress_bar), &[display_progress_bar]);
        }
    }

    /// Adds a new popup with a custom view to the right side of the screen.
    ///
    /// This allows arbitrary content to be displayed via RobrixPopupNotification's content live dsl.
    ///
    /// The `view` parameter should be constructed using `RobrixPopupNotification::content()`.
    /// The view should have a view with the id `popup_content` which should contain
    /// a `progress_bar` view with the id `progress_bar` and a `popup_label` view with the id `popup_label`.
    /// The `popup_label` view should have a `draw_text` field that will be used to display the popup's message.
    /// The custom view should also have a close button with the id `close_button` which should have a `draw_icon` field that will be used to display the close button's icon.
    ///
    /// The `auto_dismissal_duration` field of the `PopupItem` parameter will be used to automatically dismiss the popup after the given duration.
    /// If `auto_dismissal_duration` is `None`, the popup will not be automatically dismissed and the user will have to manually close it.
    /// The maximum auto dismissal duration is 3 minutes.
    pub fn push_with_custom_view(
        &mut self,
        mut popup_item: PopupItem,
        view: View,
        close_timer: Timer,
    ) {
        popup_item.auto_dismissal_duration = popup_item
            .auto_dismissal_duration
            .map(|duration| duration.min(3. * 60.));
        self.popups.push(PopupEntry {
            view,
            close_timer,
            kind: popup_item.kind,
            has_progress: popup_item.auto_dismissal_duration.is_some(),
        });
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
        for (i, popup) in self.popups.iter_mut().enumerate() {
            if popup.view.button(cx, ids!(close_button)).clicked(actions) {
                cx.stop_timer(popup.close_timer);
                self.popups.remove(i);
                self.redraw_overlay(cx);
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
