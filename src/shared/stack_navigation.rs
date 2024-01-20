use makepad_widgets::widget::WidgetCache;
use makepad_widgets::*;
use crate::shared::stack_view_action::StackViewAction;
use std::collections::HashMap;

use super::stack_view_action::StackViewSubWidgetAction;

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

#[derive(Live, LiveHook, Widget)]
pub struct StackNavigationView {
    #[deref]
    view:View,

    #[live]
    offset: f64,

    #[animator]
    animator: Animator,
}

impl Widget for StackNavigationView {
    fn handle_event(&mut self, cx:&mut Cx, event:&Event, scope:&mut Scope) {
        if self.animator_handle_event(cx, event).is_animating() {
            self.view.redraw(cx);
        }

        let actions = cx.capture_actions(|cx| self.view.handle_event(cx, event, scope));
        
        // Handle "back navigation": going back to the previous (parent) root_view. This includes:
        // * Clicking the left (back) button in the header
        // * Clicking the "back" button on the mouse
        // * TODO: in the future, a swipe right gesture on touchscreen, or two-finger swipe on trackpad
        let left_button_clicked = self.button(id!(left_button)).clicked(&actions);
        let back_mouse_button_released = match event {
            Event::MouseUp(mouse) => mouse.button == 3, // the "back" button on the mouse
            _ => false,
        };
        if left_button_clicked || back_mouse_button_released {
            self.animator_play(cx, id!(slide.hide));
            cx.widget_action(
                self.widget_uid(),
                &HeapLiveIdPath::default(),
                StackViewSubWidgetAction::Hide,
            );
        }

        if self.animator.animator_in_state(cx, id!(slide.hide))
            && !self.animator.is_track_animating(cx, id!(slide))
        {
            self.apply_over(cx, live! {visible: false});
        }

        
    }

    fn draw_walk(&mut self, cx:&mut Cx2d, scope:&mut Scope, walk:Walk) -> DrawStep{
        self.view.draw_walk(
            cx,
            scope,
            walk.with_abs_pos(DVec2 {
                x: self.offset,
                y: 0.,
            }),
        )
    }
}

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

#[derive(Live, LiveRegisterWidget, WidgetRef)]
pub struct StackNavigation {
    #[deref]
    view: View,

    #[rust]
    active_stack_view: ActiveStackView,
}

impl LiveHook for StackNavigation {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        self.active_stack_view = ActiveStackView::None;
    }
}

impl Widget for StackNavigation {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        for widget_ref in self.get_active_views(cx).iter() {
            widget_ref.handle_event(cx, event, scope);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep  {
        for widget_ref in self.get_active_views(cx.cx).iter() {
            widget_ref.draw_walk(cx, scope, walk) ?;
        }
        DrawStep::done()
    }
}

impl WidgetNode for StackNavigation {
    fn walk(&mut self, cx:&mut Cx) -> Walk{
        self.view.walk(cx)
    }

    fn redraw(&mut self, cx: &mut Cx) {
        for widget_ref in self.get_active_views(cx).iter() {
            widget_ref.redraw(cx);
        }
    }
    
    fn find_widgets(&mut self, path: &[LiveId], cached: WidgetCache, results: &mut WidgetSet) {
        self.view.find_widgets(path, cached, results);
    }
}


impl StackNavigation {
    pub fn show_stack_view_by_id(&mut self, stack_view_id: LiveId, cx: &mut Cx) {
        if let ActiveStackView::None = self.active_stack_view {
            let mut stack_view_ref = self.stack_navigation_view(&[stack_view_id]);
            stack_view_ref.show(cx);
            self.active_stack_view = ActiveStackView::Active(stack_view_id);
            // Send a `Show` action to the view being shown so it can be aware of the transition.
            cx.widget_action(
                stack_view_ref.widget_uid(),
                &HeapLiveIdPath::default(),
                StackViewSubWidgetAction::Show,
            );
            self.redraw(cx);
        }
    }

    /// Returns the list of currently active views.
    ///
    /// It is possible for multiple views to be active because
    /// one view may be animating out while another view is animating in.
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

impl StackNavigationRef {
    pub fn show_stack_view_by_id(&mut self, stack_view_id: LiveId, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_stack_view_by_id(stack_view_id, cx);
        }
    }

    pub fn handle_stack_view_actions(&mut self, cx: &mut Cx, actions: &Actions, destinations: &HashMap<StackViewAction, LiveId>) {
        for action in actions {
            let stack_view_action = action.as_widget_action().cast();
            if let Some(stack_view_id) = destinations.get(&stack_view_action) {
                self.show_stack_view_by_id(*stack_view_id, cx);
                break;
            }
        }
    }

    pub fn set_title<S: AsRef<str>>(&self, stack_view_id: LiveId, title: S) {
        if let Some(mut inner) = self.borrow_mut() {
            let stack_view_ref = inner.stack_navigation_view(&[stack_view_id]);
            stack_view_ref.label(id!(title)).set_text(title.as_ref());
        }
    }
}
