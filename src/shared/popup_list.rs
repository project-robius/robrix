use crossbeam_queue::SegQueue;
use makepad_widgets::*;

use crate::shared::styles::*;

static POPUP_NOTIFICATION: SegQueue<PopupItem> = SegQueue::new();
const POPUP_KINDS: [(PopupKind, Vec3); 4] = [
    (PopupKind::Error, COLOR_DANGER_RED),
    (PopupKind::Info, COLOR_PRIMARY),
    (PopupKind::Success, COLOR_ACCEPT_GREEN),
    (PopupKind::Warning, COLOR_WARNING_ORANGE),
];
const ICON_SET: &[&[LiveId]] = ids!(error_icon, info_icon, success_icon, warning_icon,);
/// Displays a new popup notification with a popup item.
///
/// Popup notifications will be shown in the order they were enqueued,
/// and can be removed when manually closed by the user or automatically.
/// Maximum auto dismissal duration is 3 minutes.
pub fn enqueue_popup_notification(mut popup_item: PopupItem) {
    // Limit auto dismiss duration to 180 seconds
    popup_item.auto_dismissal_duration = popup_item
        .auto_dismissal_duration
        .map(|duration| duration.min(3. * 60.));
    POPUP_NOTIFICATION.push(popup_item);
    SignalToUI::set_ui_signal();
}

/// Kind of a popup notification.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum PopupKind {
    /// Shows no icon at all.
    #[default]
    Blank,
    /// Shows a red background and a error icon.
    Error,
    /// Shows a white background and a blue stack icon.
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
    pub message: String,
    /// Duration in seconds after which the popup will be automatically closed.
    /// Maximum duration is 3 minutes.
    /// If none, the popup will not automatically close.
    pub auto_dismissal_duration: Option<f64>,
    /// Kind of the popup defined by [`PopupKind`].
    pub kind: PopupKind,
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;
    CHECK_ICON = <View> {
        width: Fill,
        height: Fit,
        visible: false,
        <Icon> {
            draw_icon: {
                svg_file: (ICON_CHECKMARK),
                color: #ffffff,
            }
            icon_walk: { width: 18, height: 18 }
        }
    }
    CROSS_ICON = <CHECK_ICON> {
        <Icon> {
            draw_icon: {
                svg_file: (ICON_FAT_CROSS),
                color: #ffffff,
            }
        }
    }
    INFO_ICON = <CHECK_ICON> {
        <Icon> {
            draw_icon: {
                svg_file: (ICON_INFO),
                color: (COLOR_SELECT_TEXT),
            }
        }
    }
    WARNING_ICON = <CHECK_ICON> {
        <Icon> {
            draw_icon: {
                svg_file: (ICON_WARNING),
                color: #ffffff,
            }
        }
    }
    PROGRESS_BAR = <View> {
        width: Fill,
        height: 10,
        show_bg: true,
        margin: { bottom: 0 },
        padding: 0,
        draw_bg: {
            uniform direction: 0.0, // Direction of the progress bar: 0.0 is right to left, 1.0 is top to bottom.
            uniform border_radius: 4.,
            uniform border_size: 1.0,
            uniform progress_bar_color: #00000080, //Black with 50% opacity.
            uniform display_progress_bar: 1.0 // Display progress bar when there is auto_dismissal_duration.
            // Display progress bar even when mode.slide is off.
            // 0.0 animate according to anim_time and anim_duration, 1.0 displays oscillating progress bar.
            uniform debug_progress_bar: 0.0, 
            uniform anim_time: 0.0,
            uniform anim_duration: 2.0,
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
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
    MAIN_CONTENT = <View> {
        width: Fill,
        height: Fit,
        align: { x: 0.0, y: 0.5 }
        padding: {left: 0, top: 0, bottom: 10, right: 5}
        popup_label = <Label> {
            width: Fill,
            height: Fit,
            draw_text: {
                color: (COLOR_TEXT),
                text_style: <MESSAGE_TEXT_STYLE>{ font_size: 10 },
                wrap: Word
            }
        }
    }
    LEFT_SIDE_VIEW = <View> {
        width: 25,
        height: Fit,
        align: { x: 0.5, y: 0.5 }
        padding: { left: 0, top: 10, bottom: 10, right: 0 }
        success_icon = <CHECK_ICON> {}
        error_icon = <CROSS_ICON> {}
        info_icon = <INFO_ICON> {}
        warning_icon = <WARNING_ICON> {}
    }
    CLOSE_BUTTON_VIEW = <View> {
        width: Fill,
        height: Fit,
        flow: Down,
        align: { x: 0.99, y: 0.01 }
        // The "X" close button on the top right
        close_button = <RobrixIconButton> {
            width: Fit,
            height: Fit,
            padding: 0
            spacing: 0,
            align: { x: 0.5, y: 0.5 }
            draw_bg: {
                instance color: #FEFEFE00
            }
            draw_icon: {
                svg_file: (ICON_FAT_CROSS),
                color: (COLOR_TEXT),
            }
            icon_walk: {width: 12, height: 12}
        }
    }
    // Other possible color themes that is not too glaring.
    // COLOR_POPUP_GREEN = #43bb9e;
    // COLOR_POPUP_RED = #e74c3c;
    PopupDialogRightToLeftProgress = <RoundedView> {
        width: 275
        height: Fit
        padding: 0,
        flow: Overlay
        show_bg: true,
        draw_bg: {
            uniform border_radius: 4.0
            uniform border_color: #000000
            uniform border_size: 2.0
            instance background_color: #ffffff
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
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

        popup_content = <View> {
            width: Fill,
            height: Fit,
            flow: Down
            //Right side view with close button
            close_button_view = <CLOSE_BUTTON_VIEW> {}
            padding: { right: 2, top: 2}
            inner = <View> {
                width: Fill,
                height: Fit,
                padding: { top: 0, right: 5, bottom: 0, left: 10 }
                flow: Right,
                align: {
                    y: 0.5,
                }
                // Left side with icon for popup kind.
                <LEFT_SIDE_VIEW> {
                    // To offset the height of the close button_view.
                    margin: {top: -12,}
                }
                // Main content area
                main_content = <MAIN_CONTENT> {}
            }
            progress_bar = <PROGRESS_BAR> {}
        }

        animator: {
            mode = {
                default: close_slider,
                close_slider = {
                    redraw: true,
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        popup_content = {
                            progress_bar = {
                                draw_bg: {anim_time: 0.0}
                            }
                        }
                    }
                }
                slide = {
                    redraw: true,
                    // Maximum auto dismissal duration is 3 minutes.
                    from: {all: Forward {duration: 180.0}}
                    apply: {
                        popup_content = {
                            progress_bar = {
                                draw_bg: {anim_time: 180.0}
                            }
                        }
                    }
                }
            }
            hover = {
                default: off
                off = {
                    apply: { }
                }
                on = {
                    apply: { }
                }
            }
            down = {
                default: off
                off = {
                    apply: { }
                }
                on = {
                    apply: { }
                }
            }
        }
    }
    PopupDialogTopToBottomProgress = <PopupDialogRightToLeftProgress> {
        popup_content = <View> {
            width: Fill,
            height: Fit,
            flow: Right,
            spacing: 0,
            align: { x: 0.0, y: 0.5 }
            // Left side with for popup kind.
            <LEFT_SIDE_VIEW> {
                height: Fit,
                margin: {left: 10 }
                spacing: 0,
            }
            inner = <View> {
                width: 230,
                height: Fit,
                padding: 0,
                flow: Down,
                close_button_view = <CLOSE_BUTTON_VIEW> {}
                // Main content area
                main_content = <MAIN_CONTENT> {
                    padding: {left: 0}
                }                
            }
            progress_bar = <PROGRESS_BAR> {
                width: 10,
                height: Fill,
                draw_bg: {
                    uniform direction: 1.0,
                    uniform anim_time: 1.0,
                    uniform border_radius: 2.,
                }
            }
        }
    }
    pub RobrixPopupNotificationRightToLeftProgress = {{RobrixPopupNotification}} {
        width: 275
        height: Fit
        flow: Down
        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.0)
            }
        }
        content: <PopupDialogRightToLeftProgress> {}
    }
    pub RobrixPopupNotificationTopToBottomProgress = <RobrixPopupNotificationRightToLeftProgress> {
        content: <PopupDialogTopToBottomProgress> {}
    }
    // A widget that displays a vertical list of popups at the top right corner of the screen.
    // The progress bar slides from right to left.
    pub PopupList = <View> {
        width: Fill,
        height: Fill,
        align: {x: 0.99, y: 0.05}
        <RobrixPopupNotificationRightToLeftProgress>{}
    }
    // A widget that displays a vertical list of popups at the top right corner of the screen.
    // The progress bar slides from top to bottom.
    pub PopupListTopToBottomProgress = <PopupList> {
        <RobrixPopupNotificationTopToBottomProgress>{}
    }
}

/// A widget that displays a vertical list of popups.
#[derive(Live, Widget)]
pub struct RobrixPopupNotification {
    #[live]
    content: Option<LivePtr>,

    #[rust(DrawList2d::new(cx))]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    // A list of tuples containing individual widgets, its content and the close timer in the order they were added.
    #[rust]
    popups: Vec<(View, PopupItem, Timer)>,
}

impl LiveHook for RobrixPopupNotification {
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        for (view, popup_item, _) in self.popups.iter_mut() {
            if let Some(index) = nodes.child_by_name(index, live_id!(content).as_field()) {
                view.apply(cx, apply, index, nodes);
                view.label(id!(popup_label))
                    .set_text(cx, &popup_item.message);
                for (view, (popup_kind, _color)) in view.view_set(ICON_SET).iter().zip(POPUP_KINDS) {
                    if popup_item.kind == popup_kind {
                        view.set_visible(cx, true);
                    } else {
                        view.set_visible(cx, false);
                    }
                }
            }
        }
        self.draw_list.redraw(cx);
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
        self.draw_list.begin_overlay_reuse(cx);
        self.draw_bg.begin(cx, walk, self.layout);
        if !self.popups.is_empty() {
            cx.begin_turtle(walk, self.layout);
            for (view, popup_item, _) in self.popups.iter_mut() {
                let walk = walk.with_margin_bottom(5.0);
                view.label(id!(popup_label))
                    .set_text(cx, &popup_item.message);
                let _ = view.draw_walk(cx, scope, walk);
            }
            cx.end_turtle();
        }
        self.draw_bg.end(cx);
        self.draw_list.end(cx);
        DrawStep::done()
    }
}

impl RobrixPopupNotification {
    /// Adds a new popup with a close button to the right side of the screen.
    ///
    /// The popup's content is a string given by the `PopupItem` parameter.
    /// New popup will be displayed below the previous ones.
    pub fn push(&mut self, cx: &mut Cx, popup_item: PopupItem) {
        let mut view = View::new_from_ptr(cx, self.content);
        let mut background_color = COLOR_PRIMARY;
        for (view, (popup_kind, color)) in view.view_set(ICON_SET).iter().zip(POPUP_KINDS) {
            if popup_item.kind == popup_kind {
                view.set_visible(cx, true);
                background_color = color;
            } else {
                view.set_visible(cx, false);
            }
        }
        // Apply popup item kind-specific styling
        if background_color != COLOR_PRIMARY {
            view.apply_over(
                cx,
                live! {
                    popup_content = {
                        inner = {
                            main_content = {
                                popup_label = {
                                    draw_text: {
                                        color: (COLOR_PRIMARY),
                                    }
                                }
                            }
                            // For top to bottom progress bar.
                            close_button_view = {
                                close_button = {
                                    draw_icon: {
                                        color: (COLOR_PRIMARY),
                                    }
                                }
                            }
                        }
                        // For Right to left rogress bar.
                        close_button_view = {
                            close_button = {
                                draw_icon: {
                                    color: (COLOR_PRIMARY),
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
            view.apply_over(
                cx,
                live! {
                    popup_content = {
                        progress_bar = {
                            draw_bg: { anim_duration: (duration) }
                        }
                    }
                },
            );
            view.animator_play(cx, id!(mode.slide));
            cx.start_timeout(duration)
        } else {
            view.apply_over(
                cx,
                live! {
                    popup_content = {
                        progress_bar = {
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
}

impl WidgetMatchEvent for RobrixPopupNotification {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        for (i, (view, _popup_item, close_timer)) in self.popups.iter_mut().enumerate() {
            if view.button(id!(close_button)).clicked(actions) {
                cx.stop_timer(*close_timer);
                view.animator_cut(cx, id!(mode.close_slider));
                self.popups.remove(i);
                self.draw_bg.redraw(cx);
                break;
            }
        }
    }
}
