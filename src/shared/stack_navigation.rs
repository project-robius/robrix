use makepad_widgets::widget::WidgetCache;
use makepad_widgets::*;
use crate::shared::stack_view_action::StackViewAction;
use std::collections::HashMap;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::header::HeaderWithLeftActionButton;

    Header = <HeaderWithLeftActionButton> {
        content = {
            title_container = {
                title = {
                    text: "My Stack View"
                }
            }

            button_container = {
                left_button = {
                    width: Fit
                    icon_walk: {width: 10}
                    draw_icon: {
                        svg_file: dep("crate://self/resources/icons/back.svg")
                    }
                }
            }
        }
    }

    StackNavigationView = {{StackNavigationView}} {
        visible: false
        width: Fill, height: Fill
        flow: Down
        show_bg: true
        draw_bg: {
            color: #fff
        }

        header = <Header> {}

        // TBD Adjust this based on actual screen size
        offset: 400.0

        animator: {
            slide = {
                default: hide,
                hide = {
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    from: {all: Forward {duration: 0.3}}
                    // Bug: Constants are not working as part of an live state value
                    apply: {offset: 400.0}
                }

                show = {
                    ease: ExpDecay {d1: 0.82, d2: 0.95}
                    from: {all: Forward {duration: 0.3}}
                    apply: {offset: 0.0}
                }
            }
        }
    }

    StackNavigation = {{StackNavigation}} {
        width: Fill, height: Fill
        flow: Overlay

        root_view = <View> {}
    }
}

#[derive(Live)]
pub struct StackNavigationView {
    #[deref]
    view:View,

    #[live]
    offset: f64,

    #[animator]
    animator: Animator,
}

impl LiveHook for StackNavigationView {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, StackNavigationView);
    }
}

impl Widget for StackNavigationView {
    fn walk(&mut self, cx: &mut Cx) -> Walk {
        self.view.walk(cx)
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.view.redraw(cx)
    }

    fn find_widgets(&mut self, path: &[LiveId], cached: WidgetCache, results: &mut WidgetSet) {
        self.view.find_widgets(path, cached, results);
    }

    fn handle_widget_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        self.handle_event_with(cx, event, dispatch_action);
    }

    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        self.view.draw_walk_widget(
            cx,
            walk.with_abs_pos(DVec2 {
                x: self.offset,
                y: 0.,
            }),
        )
    }
}

impl StackNavigationView {
    pub fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        if self.animator_handle_event(cx, event).is_animating() {
            self.view.redraw(cx);
        }

        let actions = self.view.handle_widget_event(cx, event);
        if self.button(id!(left_button)).clicked(&actions) {
            self.animator_play(cx, id!(slide.hide));
        }

        for action in actions.into_iter() {
            dispatch_action(cx, action);
        }

        if self.animator.animator_in_state(cx, id!(slide.hide))
            && !self.animator.is_track_animating(cx, id!(slide))
        {
            self.apply_over(cx, live! {visible: false});
        }
    }
}

#[derive(Clone, PartialEq, WidgetRef)]
pub struct StackNavigationViewRef(pub WidgetRef);

impl StackNavigationViewRef {
    pub fn show(&mut self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.apply_over(cx, live! {visible: true});
            inner.animator_play(cx, id!(slide.show));
        }
    }

    pub fn is_showing(&self, cx: &mut Cx) -> bool {
        if let Some(inner) = self.borrow() {
            inner.animator.animator_in_state(cx, id!(slide.show))
                || inner.animator.is_track_animating(cx, id!(slide))
        } else {
            false
        }
    }

    pub fn is_animating(&self, cx: &mut Cx) -> bool {
        if let Some(inner) = self.borrow() {
            inner.animator.is_track_animating(cx, id!(slide))
        } else {
            false
        }
    }
}

#[derive(Default)]
enum ActiveStackView {
    #[default]
    None,
    Active(LiveId),
}

#[derive(Live)]
pub struct StackNavigation {
    #[deref]
    view: View,
    #[rust]
    active_stack_view: ActiveStackView,
}

impl LiveHook for StackNavigation {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, StackNavigation);
    }

    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        self.active_stack_view = ActiveStackView::None;
    }
}

impl Widget for StackNavigation {
    fn handle_widget_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        let mut actions = vec![];

        for widget_ref in self.get_active_views(cx).iter() {
            for a in widget_ref.handle_widget_event(cx, event) {
                actions.push(a);
            }
        }

        for action in actions.into_iter() {
            dispatch_action(cx, action);
        }
    }

    fn redraw(&mut self, cx: &mut Cx) {
        for widget_ref in self.get_active_views(cx).iter() {
            widget_ref.redraw(cx);
        }
    }

    fn find_widgets(&mut self, path: &[LiveId], cached: WidgetCache, results: &mut WidgetSet) {
        // We're only usingView widget ability to find widgets
        self.view.find_widgets(path, cached, results);
    }

    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        for widget_ref in self.get_active_views(cx.cx).iter() {
            widget_ref.draw_walk_widget(cx, walk) ?;
        }
        WidgetDraw::done()
    }
}

impl StackNavigation {
    pub fn show_stack_view_by_id(&mut self, stack_view_id: LiveId, cx: &mut Cx) {
        if let ActiveStackView::None = self.active_stack_view {
            let mut stack_view_ref = self.stack_navigation_view(&[stack_view_id]);
            stack_view_ref.show(cx);
            self.active_stack_view = ActiveStackView::Active(stack_view_id);
            self.redraw(cx);
        }
    }

    fn get_active_views(&mut self, cx: &mut Cx) -> Vec<WidgetRef> {
        match self.active_stack_view {
            ActiveStackView::None => {
                vec![self.view.widget(id!(root_view))]
            },
            ActiveStackView::Active(stack_view_id) => {
                let stack_view_ref = self.stack_navigation_view(&[stack_view_id]);
                let mut views = vec![];

                if stack_view_ref.is_showing(cx) {
                    if stack_view_ref.is_animating(cx) {
                        views.push(self.view.widget(id!(root_view)));
                    }
                    views.push(stack_view_ref.0.clone());
                    views
                } else {
                    self.active_stack_view = ActiveStackView::None;
                    vec![self.view.widget(id!(root_view))]
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, WidgetRef, Debug)]
pub struct StackNavigationRef(pub WidgetRef);

impl StackNavigationRef {
    pub fn show_stack_view_by_id(&mut self, stack_view_id: LiveId, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_stack_view_by_id(stack_view_id, cx);
        }
    }

    pub fn handle_stack_view_actions(&mut self, cx: &mut Cx, actions: &WidgetActions, destinations: &HashMap<StackViewAction, LiveId>) {
        for action in actions {
            let stack_view_action = action.action();
            if let Some(stack_view_id) = destinations.get(&stack_view_action) {
                self.show_stack_view_by_id(*stack_view_id, cx);
                break;
            }
        }
    }

    pub fn set_title(&self, stack_view_id: LiveId, title: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            let stack_view_ref = inner.stack_navigation_view(&[stack_view_id]);
            stack_view_ref.label(id!(title)).set_text(title);
        }
    }
}
