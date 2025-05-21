use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    ICO_CLOSE = dep("crate://self/resources/icons/close.svg")
    ICO_CHECK = dep("crate://self/resources/icons/checkmark.svg")

    Progress = <View> {
        width: 20,
        height: Fill,
        flow: Overlay,

        <RoundedView> {
            width: Fill,
            height: Fill,
            draw_bg: {
                color: #42660a,
            }
        }

        progress_bar = <RoundedView> {
            height: Fill,
            width: Fill,
            draw_bg: {
                color: #639b0d,
            }
        }

        animator: {  
            mode = {
                default: close_slider,
                close_slider = {
                    redraw: true,
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        progress_bar = {
                            height: -25,     // height = 100 * 0.5 / self.duration
                        }
                    }
                }
                slide_down = {
                    redraw: true,
                    from: {all: Forward {duration: 2.5}}   // self.duratin + 0.5
                    apply: {
                        progress_bar = {
                            height: 100,
                        }
                    }
                }
            }
        }
    }

    TipContent = <View> {
        width: Fill,
        height: Fill,
        spacing: 10.0,
        flow: Right,
        align: {
            x: 0.0,
            y: 0.5,
        }
        margin: { left: 20.0 }

        <Icon> {
            draw_icon: {
                svg_file: (ICO_CHECK),
                color: #42660a,
            }
            icon_walk: { width: 16, height: 16 }
        }

        tip_label = <Label> {
            width: 250,
            draw_text: {
                color: #42660a,
                text_style: {
                    font_size: 12
                }
            }
            text: "Successfully updated transaction",
        }

        close_icon = <View> {
            width: Fit,
            height: Fit,
            cursor: Hand,
            <Icon> {
                draw_icon: {
                    svg_file: (ICO_CLOSE),
                    color: #6cc328
                }
    
                icon_walk: { width: 16, height: 16 }
            }
        }
        
    }

    PopupDialog = <RoundedView> {
        width: 375,
        height: 100,
        flow: Right,

        show_bg: true,
        draw_bg: {
            color: #d3f297,
        }

        progress = <Progress> {}
        <TipContent> {}
    }

    pub RobrixPopupNotification = {{RobrixPopupNotification}} {
        width: Fit,
        height: Fit,
        flow: Overlay,

        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.0)
            }
        }

        content: <PopupDialog> {}

        animator: {
            mode = {
                default: close,
                open = {
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: OutQuad
                    apply: {
                        abs_pos: vec2(60.0, 10.0),
                    }
                }
                close = {
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: InQuad
                    apply: {
                        abs_pos: vec2(-1000.0, 10.0),
                    }
                }
            }
        }
    }
}

#[derive(Live, Widget)]
pub struct RobrixPopupNotification {
    #[live]
    #[find]
    content: View,

    #[live]
    text: String,

    #[live(2.0)]
    duration: f64,

    /// If true, mutate live registry to set the animation duration directly.
    #[rust]
    live_apply: bool,

    #[rust(DrawList2d::new(cx))]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,

    #[rust]
    animation_timer: Timer,

    #[animator]
    animator: Animator,

}

impl LiveHook for RobrixPopupNotification {
    fn after_apply(&mut self, cx: &mut Cx, _apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        self.label(id!(tip_label)).set_text(cx, &self.text);
        self.live_apply = true;
        self.draw_list.redraw(cx);
    }
}

impl Widget for RobrixPopupNotification {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {

        if self.animation_timer.is_event(event).is_some() {
            self.close(cx);
        }

        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        let close_pane = {
            let area = self.view(id!(close_icon)).area();
            matches!(
                event,
                Event::Actions(actions) if self.button(id!(close_icon)).clicked(actions)
            )
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::FingerDown(_fde) => {
                    cx.set_key_focus(area);
                    false
                }
                Hit::FingerUp(fue) if fue.is_over && fue.was_tap() => {
                    matches!(fue.mouse_button(), Some(MouseButton::PRIMARY))
                }
                _ => false,
            }
            || matches!(event, Event::KeyUp(KeyEvent { key_code: KeyCode::Escape, .. }))
        };
        if close_pane {
            self.close(cx);
            return;
        }
        self.content.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, _walk: Walk) -> DrawStep {
        self.draw_list.begin_overlay_reuse(cx);

        cx.begin_pass_sized_turtle(self.layout);
        self.draw_bg.begin(cx, self.walk, self.layout);
        self.content.draw_all(cx, scope);
        self.draw_bg.end(cx);

        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);

        DrawStep::done()
    }
}

impl RobrixPopupNotification {
    pub fn open(&mut self, cx: &mut Cx) {
        self.animation_timer = cx.start_timeout(self.duration + 0.5);
        if self.live_apply{
            self.use_live_duration(cx);
            self.live_apply = false;
        }
        self.view(id!(progress)).animator_play(cx, id!(mode.slide_down));
        self.animator_play(cx, id!(mode.open));
        self.redraw(cx);
    }

    pub fn close(&mut self, cx: &mut Cx) {
        self.animator_play(cx, id!(mode.close));
        self.view(id!(progress)).animator_play(cx, id!(mode.close_slider));
        self.redraw(cx);
    }

    /// Set the duration of the animation live value to the `duration` field of `self`.
    ///
    /// This function is called whenever the `duration` field changes, and when the
    /// widget is first created.
    ///
    /// This function assumes that the `animator` field has been initialized and
    /// that the live file contains the `slide_down` and `close_slider` nodes.
    ///
    /// The function does not handle the case where the live file or the nodes
    /// do not exist, because this should not happen in normal usage.
    fn use_live_duration(&mut self, cx: &mut Cx) {
        let duration = self.duration;
        let live_ptr = match self.animator.live_ptr {
            Some(ptr) => ptr,
            None => return,
        };
    
        let LiveFileId(fi) = live_ptr.file_id;
        let registry = cx.live_registry.clone();
        let mut live_registry = registry.borrow_mut();
        
        let live_file = match live_registry.live_files.get_mut(fi as usize) {
            Some(file) => file,
            None => return,
        };
    
        let nodes = &mut live_file.expanded.nodes;
        
        let (slide_down_index, close_slider_index) = nodes.iter().enumerate()
            .fold((None, None), |(mut prog, mut close), (index, node)| {
                if node.id == live_id!(slide_down) && !matches!(node.value, LiveValue::Close) {
                    prog = Some(index);
                }
                if node.id == live_id!(close_slider) && !matches!(node.value, LiveValue::Close) {
                    close = Some(index);
                }
                (prog, close)
            });
    
        if let Some(index) = slide_down_index {
            if let Some(v) = nodes.child_by_path(index, &[
                live_id!(from).as_field(),
                live_id!(all).as_field(), 
                live_id!(duration).as_field()
            ]) {
                nodes[v].value = LiveValue::Float64(duration + 0.5);
            }
        }
        
        if let Some(index) = close_slider_index {
            if let Some(v) = nodes.child_by_path(index, &[
                live_id!(apply).as_field(),
                live_id!(progress_bar).as_instance(), 
                live_id!(height).as_field()
            ]) {
                nodes[v].value = LiveValue::Float64(-100.0 * 0.5 / duration);
            }
        }
    }
}

impl RobrixPopupNotificationRef {
    pub fn open(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.open(cx);
        }
    }

    pub fn close(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close(cx);
        }
    }
}
