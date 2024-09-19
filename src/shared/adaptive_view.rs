use makepad_widgets::*;
use std::collections::HashMap;

const MIN_DESKTOP_WIDTH: f64 = 860.;

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

#[derive(Live, LiveRegisterWidget, WidgetRef)]
pub struct AdaptiveView {
    #[rust]
    area: Area,

    /// This widget's walk, it should always match the walk of the active widget.
    #[walk]
    walk: Walk,

    /// Wether to retain the widget variant state when it goes unused.
    /// While it avoids creating new widgets and keeps their state, be mindful of the memory usage and potential memory leaks.
    #[live]
    retain_unused_variants: bool,

    #[rust]
    previously_active_widgets: HashMap<LiveId, WidgetVariant>,

    /// A map of templates that are used to create the active widget.
    #[rust] 
    templates: ComponentMap<LiveId, LivePtr>,
    
    /// The active widget that is currently being displayed.
    #[rust] 
    active_widget: Option<WidgetVariant>,

    #[rust]
    screen_width: f64,
}

pub struct WidgetVariant {
    pub template_id: LiveId,
    pub widget_ref: WidgetRef,
}

impl WidgetNode for AdaptiveView {
    fn walk(&mut self, cx: &mut Cx) -> Walk {
        self.walk
        // if let Some(active_widget) = self.active_widget.as_ref() {
        //     active_widget.1.walk(cx)
        // } else {
        //     self.walk
        // }
    }
    fn area(&self)->Area{
        self.area
    }
    
    fn redraw(&mut self, cx: &mut Cx) {
        self.area.redraw(cx);
    }

    fn find_widgets(&self, path: &[LiveId], _cached: WidgetCache, results: &mut WidgetSet) {
        if let Some(active_widget) = self.active_widget.as_ref() {
            // We do not cache the results of the find_widgets call, as the active widget in children AdaptiveViews may change.
            // However this is not enough to prevent this AdaptiveView from being cached, since the setting is part of its parent.
            // Therefore we currently cannot rely on querying nested elements (e.g. `self.ui.button(id!(my_button))`) within an AdaptiveView, 
            // from a non-AdaptiveView parent.
            // TODO: Makepad should support a way to prevent caching for scenarios like this.
            active_widget.widget_ref.find_widgets(path, WidgetCache::Clear, results);
        }
    }
    
    fn uid_to_widget(&self, uid:WidgetUid) -> WidgetRef {
        if let Some(active_widget) = self.active_widget.as_ref() {
            active_widget.widget_ref.uid_to_widget(uid)
        }
        else {
            WidgetRef::empty()
        }
    }
}

impl LiveHook for AdaptiveView {
    fn before_apply(&mut self, cx: &mut Cx, apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        if let ApplyFrom::UpdateFromDoc {..} = apply.from {
            self.templates.clear();
        }
    }

    fn after_apply_from_doc(&mut self, cx:&mut Cx) {
        self.set_default_variant_selector(cx);
        // If we have a global variant context, apply the variant selector
        // This is useful for AdaptiveViews spawned after the initial resize event (e.g. PortalList items)
        // In Robrix we know there are parent AdaptiveViews that have already set the global variant context,
        // but we'll have to make sure that's the case in Makepad when porting this Widget.
        if cx.has_global::<VariantContext>() {
            let variant_context = cx.get_global::<VariantContext>().clone();
            self.apply_after_resize(cx, &variant_context);
        } else {
            error!("No global variant context found for AdaptiveView {:?}", self.widget_uid());
        }
    }
    
    // hook the apply flow to collect our templates and apply to instanced childnodes
    fn apply_value_instance(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) -> usize {
        if nodes[index].is_instance_prop() {
            if let Some(live_ptr) = apply.from.to_live_ptr(cx, index){
                let id = nodes[index].id;
                self.templates.insert(id, live_ptr);

                if let Some(widget_variant) = self.active_widget.as_mut() {
                    if widget_variant.template_id == id {
                        widget_variant.widget_ref.apply(cx, apply, index, nodes);
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
            active_widget.widget_ref.handle_event(cx, event, scope);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.active_widget.is_some() {
            let mut context = cx.get_global::<VariantContext>().clone(); // TODO avoid cloning
            context.peeked_parent_rect = cx.peek_walk_turtle(walk);
    
            // Apply the resize which may modify self.active_widget
            self.apply_after_resize(cx, &context);
    
            // Re-borrow is just to make the borrow checker happy
            if let Some(active_widget) = self.active_widget.as_mut() {
                active_widget.widget_ref.draw_walk(cx, scope, walk)?;
            }
        }
    
        DrawStep::done()
    }
}

impl WidgetMatchEvent for AdaptiveView {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        for action in actions {
            // Handle window geom change events to update the screen width, this is triggered at startup and on window resize
            if let WindowAction::WindowGeomChange(ce) = action.as_widget_action().cast() {
                // TODO(julian): Fix excessive cloning
                let redraw_id = cx.redraw_id;
                if self.screen_width != ce.new_geom.inner_size.x {
                    self.screen_width = ce.new_geom.inner_size.x;
                    // Cache a query context for the current `WindowGeomChange`
                    if cx.has_global::<VariantContext>() {
                        let current_context = cx.get_global::<VariantContext>();
                        // if current_context.updated_on_event_id == redraw_id { return }
                        
                        current_context.updated_on_event_id = redraw_id;
                        current_context.screen_width = self.screen_width;
                        current_context.peeked_parent_rect = Rect::default();

                        let cloned_context = current_context.clone();
                        self.apply_after_resize(cx, &cloned_context);
                    } else {
                        let variant_context = VariantContext {
                            updated_on_event_id: cx.redraw_id,
                            screen_width: self.screen_width,
                            peeked_parent_rect: Rect::default(),
                        };

                        self.apply_after_resize(cx, &variant_context);
                        cx.set_global(variant_context);
                    }

                    cx.redraw_all();
                }
            }
        }
    }
}

impl AdaptiveView {
    /// Apply the variant selector to determine which template to use.
    /// If the selector returns a template that is different from the current active widget,
    /// we create a new widget from that given template. Otherwise, we do nothing.
    fn apply_after_resize(&mut self, cx: &mut Cx, variant_context: &VariantContext) {
        let widget_uid = self.widget_uid().0;
        let result = cx.get_global::<VaraintSelectors>()
            .map
            .get_mut(&widget_uid)
            .map(|selector| selector(variant_context));

        match result {
            Some(template_id) => {
                // If the selector resulted in a widget that is already active, do nothing
                if let Some(active_widget) = self.active_widget.as_mut() {
                    if active_widget.template_id == template_id {
                        return;
                    }
                }

                // If the selector resulted in a widget that was previously active, restore it
                if self.retain_unused_variants && self.previously_active_widgets.contains_key(&template_id) {
                    let widget_variant = self.previously_active_widgets.remove(&template_id).unwrap();

                    self.walk = widget_variant.widget_ref.walk(cx);
                    self.active_widget = Some(widget_variant);
                    return;
                }

                // Create a new widget from the template
                let template = self.templates.get(&template_id).unwrap();
                let widget_ref = WidgetRef::new_from_ptr(cx, Some(*template));
                

                // Update this widget's walk to match the walk of the active widget,
                // this ensures that the new widget is not affected by `Fill` or `Fit` constraints from this parent.
                self.walk = widget_ref.walk(cx);

                if let Some(active_widget) = self.active_widget.take() {
                    if self.retain_unused_variants {
                        self.previously_active_widgets.insert(active_widget.template_id, active_widget);
                    }
                }

                self.active_widget = Some(WidgetVariant { template_id, widget_ref });
            }
            None => {
                error!("No query found for AdaptiveView {:?}", self.widget_uid());
            }
        }
    }

    /// Set a variant selector for this widget. 
    /// The selector is a closure that takes a `VariantContext` and returns a `LiveId`, corresponding to the template to use.
    pub fn set_variant_selector(&mut self, cx: &mut Cx, query: impl FnMut(&VariantContext) -> LiveId + 'static) {
        if !cx.has_global::<VaraintSelectors>() {
            cx.set_global(VaraintSelectors::default());
        }

        cx.get_global::<VaraintSelectors>().map.insert(self.widget_uid().0, Box::new(query));
    }

    pub fn set_default_variant_selector(&mut self, cx: &mut Cx) {
        self.set_variant_selector(cx, |context| {
            if context.screen_width < MIN_DESKTOP_WIDTH {
                live_id!(Mobile)
            } else {
                live_id!(Desktop)
            }
        });
    }
}

impl AdaptiveViewRef {
    /// Set a variant selector for this widget. 
    /// The selector is a closure that takes a `VariantContext` and returns a `LiveId`, corresponding to the template to use.
    pub fn set_variant_selector(&mut self, cx: &mut Cx, query: impl FnMut(&VariantContext) -> LiveId + 'static) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if !cx.has_global::<VaraintSelectors>() {
            cx.set_global(VaraintSelectors::default());
        }

        cx.get_global::<VaraintSelectors>().map.insert(inner.widget_uid().0, Box::new(query));
        let variant_context = cx.get_global::<VariantContext>().clone();
        inner.apply_after_resize(cx, &variant_context);
    }
}

/// A collection of callbacks that determine which view to display based on the current context.
/// We store them in a global context as a workaround for the lack of support for closures in `Live` (cannot store the variant in the `AdaptiveView`).
#[derive(Default)]
pub struct VaraintSelectors {
    pub map: HashMap<u64, Box<VariantSelector>>,
}

pub type VariantSelector = dyn FnMut(&VariantContext) -> LiveId;

// TODO(julian): rename
/// A context that is used to determine which view to display in an `AdaptiveView` widget.
/// Later to be expanded with more context data like platfrom information, accessibility settings, etc.
/// VariantContext is stored in a global context so that they can be accessed from multiple `AdaptiveView` widget instances.
#[derive(Clone, Debug)]
pub struct VariantContext {
    pub updated_on_event_id: u64,
    pub screen_width: f64,
    /// The [Rect] obtained from running `cx.peek_walk_turtle(walk)` before the widget is drawn.
    /// Useful for determining the parent size and position.
    pub peeked_parent_rect: Rect,
}
