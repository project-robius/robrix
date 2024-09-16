use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    AdaptiveView = {{AdaptiveView}} {
        width: Fill, height: Fill
        
        Shared = <View> {}
    
        Mobile = <View> {}
        Tablet = <View> {}
        Desktop = <View> {}
    }
}

#[derive(Live, Widget)]
pub struct AdaptiveView {
    #[redraw] #[rust]
    area: Area,

    #[walk]
    walk: Walk,

    #[rust]
    screen_width: f64,

    #[rust] 
    templates: ComponentMap<LiveId, LivePtr>,
    
    #[rust] 
    active_widget: Option<(LiveId, WidgetRef)>,
}

impl LiveHook for AdaptiveView {
    fn before_apply(&mut self, _cx: &mut Cx, apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        if let ApplyFrom::UpdateFromDoc {..} = apply.from {
            self.templates.clear();
        }
    }
    
    // hook the apply flow to collect our templates and apply to instanced childnodes
    fn apply_value_instance(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) -> usize {
        if nodes[index].is_instance_prop() {
            if let Some(live_ptr) = apply.from.to_live_ptr(cx, index){
                let id = nodes[index].id;
                self.templates.insert(id, live_ptr);

                if let Some((templ_id, node)) = self.active_widget.as_mut() {
                    if *templ_id == id {
                        node.apply(cx, apply, index, nodes);
                    }
                }
            }
        }
        else {
            cx.apply_error_no_matching_field(live_error_origin!(), index, nodes);
        }
        nodes.skip_node(index)
    }
}

impl Widget for AdaptiveView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.widget_match_event(cx, event, scope);
        if let Some(active_widget) = self.active_widget.as_mut() {
            active_widget.1.handle_event(cx, event, scope);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(active_widget) = self.active_widget.as_mut() else { return DrawStep::done() };

        active_widget.1.draw_walk(cx, scope, walk)
    }
}

impl AdaptiveView {
    // TODO: use sizes defined in DSL or through queries
    fn apply_after_resize(&mut self, cx: &mut Cx) {
        if self.screen_width <= 860. {
            match self.active_widget.as_ref() {
                Some((template_id, _widgetref)) => {
                    // if the active widget is not the Mobile view, set it to the Mobile view
                    // we might even want to reuse them on resize instead of creating new ones
                    if template_id == &live_id!(Mobile) {return;}

                    // widget.set_visible(false);
                }
                None => ()
            }

            let template = self.templates.get(&live_id!(Mobile)).unwrap();
            let widget_ref = WidgetRef::new_from_ptr(cx, Some(*template));            
            self.active_widget = Some((live_id!(Mobile), widget_ref));
        } else {
            match self.active_widget.as_ref() {
                Some((template_id, _widgetref)) => {
                    // if the active widget is not the desktop view, set it to the Desktop view
                    // we might even want to reuse them on resize instead of creating new ones
                    if template_id == &live_id!(Desktop) {return;}
                },
                None => ()
            }

            let template = self.templates.get(&live_id!(Desktop)).unwrap();
            let widget_ref = WidgetRef::new_from_ptr(cx, Some(*template));
            self.active_widget = Some((live_id!(Desktop), widget_ref));
        }

        self.walk = self.active_widget.as_mut().unwrap().1.walk(cx);
    }
}

impl WidgetMatchEvent for AdaptiveView {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        for action in actions {
            // Handle window geom change events to update the screen width, this is triggered at startup and on window resize
            if let WindowAction::WindowGeomChange(ce) = action.as_widget_action().cast() {
                if self.screen_width != ce.new_geom.inner_size.x {
                    self.screen_width = ce.new_geom.inner_size.x;
                    self.apply_after_resize(cx);
                    cx.redraw_all();
                }
            }
        }
    }
}
