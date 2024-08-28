use std::{cell::RefCell, collections::HashMap};

use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    AdaptiveLayoutView = {{AdaptiveLayoutView}} {
        width: Fill, height: Fill
    }
}

#[derive(Live, LiveHook)]
#[live_ignore]
pub enum ViewOptimize {
    #[pick]
    None,
    DrawList,
    Texture,
}

#[derive(Live)]
#[live_ignore]
pub enum ViewDebug {
    #[pick]
    None,
    R,
    G,
    B,
    M,
    Margin,
    P,
    Padding,
    A,
    All,
    #[live(Vec4::default())]
    Color(Vec4),
}

impl LiveHook for ViewDebug {
    fn skip_apply(
        &mut self,
        _cx: &mut Cx,
        _apply: &mut Apply,
        index: usize,
        nodes: &[LiveNode],
    ) -> Option<usize> {
        match &nodes[index].value {
            LiveValue::Vec4(v) => {
                *self = Self::Color(*v);
                Some(index + 1)
            }
            LiveValue::Color(v) => {
                *self = Self::Color(Vec4::from_u32(*v));
                Some(index + 1)
            }
            LiveValue::Bool(v) => {
                if *v {
                    *self = Self::R;
                } else {
                    *self = Self::None;
                }
                Some(index + 1)
            }
            LiveValue::Float64(v) => {
                if *v != 0.0 {
                    *self = Self::R;
                } else {
                    *self = Self::None;
                }
                Some(index + 1)
            }
            LiveValue::Int64(v) => {
                if *v != 0 {
                    *self = Self::R;
                } else {
                    *self = Self::None;
                }
                Some(index + 1)
            }
            _ => None,
        }
    }
}

#[derive(Live, LiveHook)]
#[live_ignore]
pub enum EventOrder {
    Down,
    #[pick]
    Up,
    #[live(Default::default())]
    List(Vec<LiveId>),
}

impl ViewOptimize {
    fn is_texture(&self) -> bool {
        if let Self::Texture = self {
            true
        } else {
            false
        }
    }
    fn is_draw_list(&self) -> bool {
        if let Self::DrawList = self {
            true
        } else {
            false
        }
    }
    fn needs_draw_list(&self) -> bool {
        return self.is_texture() || self.is_draw_list();
    }
}


// TODO: 
// - add a convinient way to navigate back
// - add regular walk and layout so that they can be used like in normal views, but are overriden through composition

#[derive(Live, LiveRegisterWidget, WidgetRef, WidgetSet)]
pub struct AdaptiveLayoutView {
    // draw info per UI element
    #[live]
    pub draw_bg: DrawColor,

    #[live(false)]
    pub show_bg: bool,

    // #[layout]
    #[live]
    pub current_layout: Layout,

    #[walk]
    pub current_walk: Walk,

    #[live]
    pub current_navigation_config: Option<NavigationConfig>,

    #[live]
    pub current_child_order: Vec<LiveId>,

    #[live]
    composition: AdaptiveComposition,

    //#[live] use_cache: bool,
    #[live]
    dpi_factor: Option<f64>,

    #[live]
    optimize: ViewOptimize,
    #[live]
    debug: ViewDebug,
    #[live]
    event_order: EventOrder,

    #[live(true)]
    pub visible: bool,

    #[live(true)]
    grab_key_focus: bool,
    #[live(false)]
    block_signal_event: bool,
    #[live]
    cursor: Option<MouseCursor>,
    #[live(false)]
    capture_overload: bool,
    #[live]
    scroll_bars: Option<LivePtr>,
    #[live(false)]
    design_mode: bool,

    #[rust]
    find_cache: RefCell<HashMap<u64, WidgetSet>>,

    #[rust]
    scroll_bars_obj: Option<Box<ScrollBars>>,
    #[rust]
    view_size: Option<DVec2>,

    #[rust]
    area: Area,
    #[rust]
    draw_list: Option<DrawList2d>,

    #[rust]
    texture_cache: Option<ViewTextureCache>,
    #[rust]
    defer_walks: Vec<(LiveId, DeferWalk)>,
    #[rust]
    draw_state: DrawStateWrap<DrawState>,
    #[rust]
    children: Vec<(LiveId, WidgetRef)>,
    //#[rust]
    //draw_order: Vec<LiveId>,

    #[animator]
    animator: Animator,

    #[rust] 
    current_layout_mode: LayoutMode,

    #[rust(1200)] 
    screen_width: f64,

    #[rust]
    active_view_takeover: bool,

    #[rust]
    navigation_state: NavigationState
}

// type AdaptiveChild = Vec<(LiveId, WidgetRef)>;

#[derive(Clone, Debug, Live, LiveHook, LiveRegister)]
#[live_ignore]
pub struct AdaptiveComposition {
    #[live] pub desktop: AdaptiveProps,
    #[live] pub mobile: AdaptiveProps
}

/// The configuration of this view and its children, that is resolution-specific.
#[derive(Clone, Debug, Live, LiveHook, LiveRegister)]
#[live_ignore]
pub struct AdaptiveProps {
    #[live] pub walk: Walk,
    #[live] pub layout: Layout,
    // Ideally child_order should be a part of layout.
    #[live] pub child_order: Vec<LiveId>,
    #[live] pub view_presence: ViewPresence,
    #[live] pub navigation: Option<NavigationConfig>
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
enum LayoutMode {
    #[default]
    Desktop,
    Mobile,
}

struct ViewTextureCache {
    pass: Pass,
    _depth_texture: Texture,
    color_texture: Texture,
}

impl LiveHook for AdaptiveLayoutView {
    fn before_apply(
        &mut self,
        _cx: &mut Cx,
        apply: &mut Apply,
        _index: usize,
        _nodes: &[LiveNode],
    ) {
        // Clear children cache after doc updates
        if let ApplyFrom::UpdateFromDoc { .. } = apply.from {
            //self.draw_order.clear();
            self.find_cache.get_mut().clear();
        }
    }


    fn after_apply(
        &mut self,
        cx: &mut Cx,
        _applyl: &mut Apply,
        _index: usize,
        _nodes: &[LiveNode],
    ) {
        if self.optimize.needs_draw_list() && self.draw_list.is_none() {
            self.draw_list = Some(DrawList2d::new(cx));
        }
        // Initialize scrollbar if necessary
        if self.scroll_bars.is_some() {
            if self.scroll_bars_obj.is_none() {
                self.scroll_bars_obj =
                    Some(Box::new(ScrollBars::new_from_ptr(cx, self.scroll_bars)));
            }
        }
    }

    /// Applies the instance fields, e.g. instance_field = <SomeWidget>{}
    fn apply_value_instance(
        &mut self,
        cx: &mut Cx,
        apply: &mut Apply,
        index: usize,
        nodes: &[LiveNode],
    ) -> usize { 

        let id = nodes[index].id;
        match apply.from {
            ApplyFrom::Animate | ApplyFrom::Over => {
                let node_id = nodes[index].id;
                if let Some((_,component)) = self.children.iter_mut().find(|(id,_)| *id == node_id) {
                    component.apply(cx, apply, index, nodes)
                } else {
                    nodes.skip_node(index)
                }
            }
            ApplyFrom::NewFromDoc { .. } | ApplyFrom::UpdateFromDoc { .. } => {
                if nodes[index].is_instance_prop() {
                    //self.draw_order.push(id);
                    if let Some((_,node)) = self.children.iter_mut().find(|(id2,_)| *id2 == id){
                        node.apply(cx, apply, index, nodes)
                    }
                    else{
                        self.children.push((id,WidgetRef::new(cx)));
                        self.children.last_mut().unwrap().1.apply(cx, apply, index, nodes)
                    }
                } else {
                    cx.apply_error_no_matching_field(live_error_origin!(), index, nodes);
                    nodes.skip_node(index)
                }
            }
            _ => nodes.skip_node(index),
        }
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum ViewAction {
    None,
    FingerDown(FingerDownEvent),
    FingerUp(FingerUpEvent),
    FingerMove(FingerMoveEvent),
    FingerHoverIn(FingerHoverEvent),
    FingerHoverOut(FingerHoverEvent),
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),
}

impl AdaptiveLayoutViewRef {
    pub fn finger_down(&self, actions: &Actions) -> Option<FingerDownEvent> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let ViewAction::FingerDown(fd) = item.cast() {
                return Some(fd);
            }
        }
        None
    }

    pub fn finger_up(&self, actions: &Actions) -> Option<FingerUpEvent> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let ViewAction::FingerUp(fd) = item.cast() {
                return Some(fd);
            }
        }
        None
    }

    pub fn finger_move(&self, actions: &Actions) -> Option<FingerMoveEvent> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let ViewAction::FingerMove(fd) = item.cast() {
                return Some(fd);
            }
        }
        None
    }

    pub fn finger_hover_in(&self, actions: &Actions) -> Option<FingerHoverEvent> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let ViewAction::FingerHoverIn(fd) = item.cast() {
                return Some(fd);
            }
        }
        None
    }

    pub fn finger_hover_out(&self, actions: &Actions) -> Option<FingerHoverEvent> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let ViewAction::FingerHoverOut(fd) = item.cast() {
                return Some(fd);
            }
        }
        None
    }

    pub fn key_down(&self, actions: &Actions) -> Option<KeyEvent> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let ViewAction::KeyDown(fd) = item.cast() {
                return Some(fd);
            }
        }
        None
    }

    pub fn key_up(&self, actions: &Actions) -> Option<KeyEvent> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let ViewAction::KeyUp(fd) = item.cast() {
                return Some(fd);
            }
        }
        None
    }

    pub fn animator_cut(&self, cx: &mut Cx, state: &[LiveId; 2]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.animator_cut(cx, state);
        }
    }

    pub fn animator_play(&self, cx: &mut Cx, state: &[LiveId; 2]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.animator_play(cx, state);
        }
    }

    pub fn toggle_state(
        &self,
        cx: &mut Cx,
        is_state_1: bool,
        animate: Animate,
        state1: &[LiveId; 2],
        state2: &[LiveId; 2],
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.animator_toggle(cx, is_state_1, animate, state1, state2);
        }
    }

    pub fn set_visible(&self, visible: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = visible
        }
    }

    pub fn set_visible_and_redraw(&self, cx: &mut Cx, visible: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = visible;
            inner.redraw(cx);
        }
    }

    pub fn visible(&self) -> bool {
        if let Some(inner) = self.borrow() {
            inner.visible
        } else {
            false
        }
    }

    pub fn set_texture(&self, slot: usize, texture: &Texture) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.draw_bg.set_texture(slot, texture);
        }
    }

    pub fn set_uniform(&self, cx: &Cx, uniform: &[LiveId], value: &[f32]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.draw_bg.set_uniform(cx, uniform, value);
        }
    }

    pub fn set_scroll_pos(&self, cx: &mut Cx, v: DVec2) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_scroll_pos(cx, v)
        }
    }

    pub fn area(&self) -> Area {
        if let Some(inner) = self.borrow_mut() {
            inner.area
        } else {
            Area::Empty
        }
    }

    pub fn child_count(&self) -> usize {
        if let Some(inner) = self.borrow_mut() {
            inner.children.len()
        } else {
            0
        }
    }

    pub fn is_visible(&self) -> bool {
        if let Some(adaptive_layout_view) = self.borrow_mut() {
            return adaptive_layout_view.is_visible()
        }
        false
    }
}

impl AdaptiveLayoutViewSet {
    pub fn animator_cut(&mut self, cx: &mut Cx, state: &[LiveId; 2]) {
        for item in self.iter() {
            item.animator_cut(cx, state)
        }
    }

    pub fn animator_play(&mut self, cx: &mut Cx, state: &[LiveId; 2]) {
        for item in self.iter() {
            item.animator_play(cx, state);
        }
    }

    pub fn toggle_state(
        &mut self,
        cx: &mut Cx,
        is_state_1: bool,
        animate: Animate,
        state1: &[LiveId; 2],
        state2: &[LiveId; 2],
    ) {
        for item in self.iter() {
            item.toggle_state(cx, is_state_1, animate, state1, state2);
        }
    }

    pub fn set_visible(&self, visible: bool) {
        for item in self.iter() {
            item.set_visible(visible)
        }
    }

    pub fn set_texture(&self, slot: usize, texture: &Texture) {
        for item in self.iter() {
            item.set_texture(slot, texture)
        }
    }

    pub fn set_uniform(&self, cx: &Cx, uniform: &[LiveId], value: &[f32]) {
        for item in self.iter() {
            item.set_uniform(cx, uniform, value)
        }
    }

    pub fn redraw(&self, cx: &mut Cx) {
        for item in self.iter() {
            item.redraw(cx);
        }
    }

    pub fn finger_down(&self, actions: &Actions) -> Option<FingerDownEvent> {
        for item in self.iter() {
            if let Some(e) = item.finger_down(actions) {
                return Some(e);
            }
        }
        None
    }

    pub fn finger_up(&self, actions: &Actions) -> Option<FingerUpEvent> {
        for item in self.iter() {
            if let Some(e) = item.finger_up(actions) {
                return Some(e);
            }
        }
        None
    }

    pub fn finger_move(&self, actions: &Actions) -> Option<FingerMoveEvent> {
        for item in self.iter() {
            if let Some(e) = item.finger_move(actions) {
                return Some(e);
            }
        }
        None
    }

    pub fn key_down(&self, actions: &Actions) -> Option<KeyEvent> {
        for item in self.iter() {
            if let Some(e) = item.key_down(actions) {
                return Some(e);
            }
        }
        None
    }

    pub fn key_up(&self, actions: &Actions) -> Option<KeyEvent> {
        for item in self.iter() {
            if let Some(e) = item.key_up(actions) {
                return Some(e);
            }
        }
        None
    }
}

impl WidgetNode for AdaptiveLayoutView {
    fn walk(&mut self, _cx: &mut Cx) -> Walk {
        self.current_walk
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.area.redraw(cx);
        // Redraw all children
        for (_,child) in &mut self.children {
            child.redraw(cx);
        }
    }
    
    // fn uid_to_widget(&self, uid:WidgetUid)->WidgetRef{
    //     for (_,child) in &self.children {
    //         let x = child.uid_to_widget(uid);
    //         if !x.is_empty(){return x}
    //     }
    //     WidgetRef::empty()
    // }

    /// Searches for a child widget with the given path and pushes it into the results set if found.
    /// Overrides the default version with added caching.
    fn find_widgets(&self, path: &[LiveId], cached: WidgetCache, results: &mut WidgetSet) {
        match cached {
            WidgetCache::Yes | WidgetCache::Clear => {
                if let WidgetCache::Clear = cached {
                    self.find_cache.borrow_mut().clear();
                }
                let mut hash = 0u64;
                for i in 0..path.len() {
                    hash ^= path[i].0
                }
                if let Some(widget_set) = self.find_cache.borrow().get(&hash) {
                    results.extend_from_set(widget_set);
                    return;
                }
                let mut local_results = WidgetSet::empty();
                if let Some((_,child)) = self.children.iter().find(|(id,_)| *id == path[0]) {
                    if path.len() > 1 {
                        child.find_widgets(&path[1..], WidgetCache::No, &mut local_results);
                    } else {
                        local_results.push(child.clone());
                    }
                }
                for (_,child) in &self.children {
                    child.find_widgets(path, WidgetCache::No, &mut local_results);
                }
                if !local_results.is_empty() {
                    results.extend_from_set(&local_results);
                }
                self.find_cache.borrow_mut().insert(hash, local_results);
            }
            WidgetCache::No => {
                 if let Some((_,child)) = self.children.iter().find(|(id,_)| *id == path[0]) {
                    if path.len() > 1 {
                        child.find_widgets(&path[1..], WidgetCache::No, results);
                    } else {
                        results.push(child.clone());
                    }
                }
                for (_,child) in &self.children {
                    child.find_widgets(path, WidgetCache::No, results);
                }
            }
        }
    }
}

impl Widget for AdaptiveLayoutView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.widget_match_event(cx, event, scope);
        let uid = self.widget_uid();
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        if self.block_signal_event {
            if let Event::Signal = event {
                // Skip signal
                return;
            }
        }
        
        // Handle scroll_bars actions, if any, redraw everything
        if let Some(scroll_bars) = &mut self.scroll_bars_obj {
            let mut actions = Vec::new();
            scroll_bars.handle_main_event(cx, event, &mut actions);
            if actions.len() > 0 {
                cx.redraw_area_and_children(self.area);
            };
        }

        // Propagate events to children based on event order
        match &self.event_order {
            EventOrder::Up => {
                for (id, child) in self.children.iter_mut().rev() {
                    scope.with_id(*id, |scope| {
                        child.handle_event(cx, event, scope);
                    });
                }
            }
            EventOrder::Down => {
                for (id, child) in self.children.iter_mut() {
                    scope.with_id(*id, |scope| {
                        child.handle_event(cx, event, scope);
                    })
                }
            }
            EventOrder::List(list) => {
                for id in list {
                    if let Some((_,child)) = self.children.iter_mut().find(|(id2,_)| id2 == id) {
                        scope.with_id(*id, |scope| {
                            child.handle_event(cx, event, scope);
                        })
                    }
                }
            }
        }

        // Handle mouse and key events
        if self.visible && self.cursor.is_some() || self.animator.live_ptr.is_some() {
            match event.hits_with_capture_overload(cx, self.area(), self.capture_overload) {
                Hit::FingerDown(e) => {
                    if self.grab_key_focus {
                        cx.set_key_focus(self.area());
                    }
                    cx.widget_action(uid, &scope.path, ViewAction::FingerDown(e));
                    if self.animator.live_ptr.is_some() {
                        self.animator_play(cx, id!(down.on));
                    }
                }
                Hit::FingerMove(e) => cx.widget_action(uid, &scope.path, ViewAction::FingerMove(e)),
                Hit::FingerUp(e) => {
                    cx.widget_action(uid, &scope.path, ViewAction::FingerUp(e));
                    if self.animator.live_ptr.is_some() {
                        self.animator_play(cx, id!(down.off));
                    }
                }
                Hit::FingerHoverIn(e) => {
                    cx.widget_action(uid, &scope.path, ViewAction::FingerHoverIn(e));
                    if let Some(cursor) = &self.cursor {
                        cx.set_cursor(*cursor);
                    }
                    if self.animator.live_ptr.is_some() {
                        self.animator_play(cx, id!(hover.on));
                    }
                }
                Hit::FingerHoverOut(e) => {
                    cx.widget_action(uid, &scope.path, ViewAction::FingerHoverOut(e));
                    if self.animator.live_ptr.is_some() {
                        self.animator_play(cx, id!(hover.off));
                    }
                }
                Hit::KeyDown(e) => cx.widget_action(uid, &scope.path, ViewAction::KeyDown(e)),
                Hit::KeyUp(e) => cx.widget_action(uid, &scope.path, ViewAction::KeyUp(e)),
                _ => (),
            }
        }

        if let Some(scroll_bars) = &mut self.scroll_bars_obj {
            scroll_bars.handle_scroll_event(cx, event, &mut Vec::new());
        }
    }

    fn is_visible(&self) -> bool {
        // self.visible
        match self.current_layout_mode {
            LayoutMode::Desktop => {
                // log!("{:?}, visible {:?}", self.widget_uid(), self.composition.desktop.view_presence);
                self.composition.desktop.view_presence == ViewPresence::Visible
            },
            LayoutMode::Mobile => {
                self.composition.mobile.view_presence == ViewPresence::Visible //||
                // self.composition.mobile.view_presence == ViewPresence::NavigationItem
            },
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Begin a draw state
        if self.draw_state.begin(cx, DrawState::Drawing(0, false)) {
            if !self.visible {
                self.draw_state.end();
                return DrawStep::done();
            }

            self.defer_walks.clear();

            match self.optimize {
                // Drawing is cached on a texture which wont redraw if nothing is dirty
                ViewOptimize::Texture => {
                    let walk = self.walk_from_previous_size(walk);
                    if !cx.will_redraw(self.draw_list.as_mut().unwrap(), walk) {
                        if let Some(texture_cache) = &self.texture_cache {
                            self.draw_bg
                                .draw_vars
                                .set_texture(0, &texture_cache.color_texture);
                            let mut rect = cx.walk_turtle_with_area(&mut self.area, walk);
                            // NOTE(eddyb) see comment lower below for why this is
                            // disabled (it used to match `set_pass_scaled_area`).
                            if false {
                                rect.size *= 2.0 / self.dpi_factor.unwrap_or(1.0);
                            }
                            self.draw_bg.draw_abs(cx, rect);
                            self.area = self.draw_bg.area();
                            /*if false {
                                // FIXME(eddyb) this was the previous logic,
                                // but the only tested apps that use `CachedView`
                                // are sized correctly (regardless of `dpi_factor`)
                                // *without* extra scaling here.
                                cx.set_pass_scaled_area(
                                    &texture_cache.pass,
                                    self.area,
                                    2.0 / self.dpi_factor.unwrap_or(1.0),
                                );
                            } else {*/
                                cx.set_pass_area(
                                    &texture_cache.pass,
                                    self.area,
                                );
                            //}
                        }
                        return DrawStep::done();
                    }
                    // lets start a pass
                    if self.texture_cache.is_none() {
                        self.texture_cache = Some(ViewTextureCache {
                            pass: Pass::new(cx),
                            _depth_texture: Texture::new(cx),
                            color_texture: Texture::new(cx),
                        });
                        let texture_cache = self.texture_cache.as_mut().unwrap();
                        //cache.pass.set_depth_texture(cx, &cache.depth_texture, PassClearDepth::ClearWith(1.0));
                        texture_cache.color_texture = Texture::new_with_format(
                            cx,
                            TextureFormat::RenderBGRAu8 {
                                size: TextureSize::Auto,
                            },
                        );
                        texture_cache.pass.add_color_texture(
                            cx,
                            &texture_cache.color_texture,
                            PassClearColor::ClearWith(vec4(0.0, 0.0, 0.0, 0.0)),
                        );
                    }
                    let texture_cache = self.texture_cache.as_mut().unwrap();
                    cx.make_child_pass(&texture_cache.pass);
                    cx.begin_pass(&texture_cache.pass, self.dpi_factor);
                    self.draw_list.as_mut().unwrap().begin_always(cx)
                }
                
                // If using drawlist, if the view didn't didnt change it will not be regenerated by draw
                ViewOptimize::DrawList => {
                    let walk = self.walk_from_previous_size(walk);
                    if self
                        .draw_list
                        .as_mut()
                        .unwrap()
                        .begin(cx, walk)
                        .is_not_redrawing()
                    {
                        cx.walk_turtle_with_area(&mut self.area, walk);
                        return DrawStep::done();
                    }
                }
                _ => (),
            }

            // Call draw until we return LiveId(0)
            let scroll = if let Some(scroll_bars) = &mut self.scroll_bars_obj {
                scroll_bars.begin_nav_area(cx);
                scroll_bars.get_scroll_pos()
            } else {
                self.current_layout.scroll
            };

            self.apply_current_values_from_adaptive();

            // Begin the main turtle
            if self.show_bg {
                // Begin a turtle through begining the background draw
                self.draw_bg
                    .begin(cx, walk, self.current_layout.with_scroll(scroll)); //.with_scale(2.0 / self.dpi_factor.unwrap_or(2.0)));
            } else {
                // Otherwise ignore background and start a turtle directly
                cx.begin_turtle(walk, self.current_layout.with_scroll(scroll)); //.with_scale(2.0 / self.dpi_factor.unwrap_or(2.0)));
            }

            // TODO: error handling
            match &self.current_navigation_config {
                Some(nav_config) => { // TODO: this doesn't have to be mobile //if self.current_layout_mode == LayoutMode::Mobile
                    // Handle navigation-based drawing for mobile
                    let _ = self.draw_navigable_children(cx, scope, nav_config.clone());
                },
                _ => {
                    // Normal drawing for desktop or non-navigable mobile layouts
                    let mut visible_children = self.get_visible_children();
                    let _ =  self.draw_children(cx, scope, Some(&mut visible_children));
                }
            }
        }
        
        // Debugging
        match &self.debug {
            ViewDebug::None => {}
            ViewDebug::Color(c) => {
                cx.debug.area(self.area, *c);
            }
            ViewDebug::R => {
                cx.debug.area(self.area, Vec4::R);
            }
            ViewDebug::G => {
                cx.debug.area(self.area, Vec4::G);
            }
            ViewDebug::B => {
                cx.debug.area(self.area, Vec4::B);
            }
            ViewDebug::M | ViewDebug::Margin => {
                let tl = dvec2(self.current_walk.margin.left, self.current_walk.margin.top);
                let br = dvec2(self.current_walk.margin.right, self.current_walk.margin.bottom);
                cx.debug.area_offset(self.area, tl, br, Vec4::B);
                cx.debug.area(self.area, Vec4::R);
            }
            ViewDebug::P | ViewDebug::Padding => {
                let tl = dvec2(-self.current_layout.padding.left, -self.current_walk.margin.top);
                let br = dvec2(-self.current_layout.padding.right, -self.current_layout.padding.bottom);
                cx.debug.area_offset(self.area, tl, br, Vec4::G);
                cx.debug.area(self.area, Vec4::R);
            }
            ViewDebug::All | ViewDebug::A => {
                let tl = dvec2(self.current_walk.margin.left, self.current_walk.margin.top);
                let br = dvec2(self.current_walk.margin.right, self.current_walk.margin.bottom);
                cx.debug.area_offset(self.area, tl, br, Vec4::B);
                let tl = dvec2(-self.current_layout.padding.left, -self.current_walk.margin.top);
                let br = dvec2(-self.current_layout.padding.right, -self.current_layout.padding.bottom);
                cx.debug.area_offset(self.area, tl, br, Vec4::G);
                cx.debug.area(self.area, Vec4::R);
            }
        }
        DrawStep::done()
    }
}


impl WidgetMatchEvent for AdaptiveLayoutView {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        for action in actions {
            if let WindowAction::WindowGeomChange(ce) = action.as_widget_action().cast() {
                self.screen_width = ce.new_geom.inner_size.x;
                if ce.old_geom != ce.new_geom {
                    self.apply_current_values_from_adaptive();
                    self.redraw(cx);
                }
            }

            // TODO: only handle this if the there's current navigation, and if the child and action correspond to this specific parent view
            if let AdaptiveLayoutViewAction::NavigateTo(view_id) = action.as_widget_action().cast() {
                self.active_view_takeover = true;
                self.navigation_state.active_item_id = Some(view_id);
            }
        }
    }
}

#[derive(Clone)]
enum DrawState {
    Drawing(usize, bool),
    DeferWalk(usize),
}

impl AdaptiveLayoutView {

    /// Determines the current layout based on the current screen width and user-provided values for the different 
    /// screen sizes.
    fn apply_current_values_from_adaptive(&mut self) {       
       // TODO allow overriding these values or setting custom alternatives to Mobile and Desktop
        if self.screen_width <= 860.0 { 
            self.current_layout_mode = LayoutMode::Mobile;
            self.current_layout = self.composition.mobile.layout;
            self.current_walk = self.composition.mobile.walk;
            self.current_navigation_config = self.composition.mobile.navigation.clone();
            self.current_child_order = self.composition.mobile.child_order.clone();
        } else {
            self.current_layout_mode = LayoutMode::Desktop;
            self.current_layout = self.composition.desktop.layout;
            self.current_walk = self.composition.desktop.walk;
            self.current_navigation_config = self.composition.desktop.navigation.clone();
            self.current_child_order = self.composition.desktop.child_order.clone();
        }
    }

    /// Returns a list of the visible children based on their presence configuration for the current layout mode.
    fn get_visible_children(&self) -> Vec<(LiveId, WidgetRef)> {
        let mut visible_children = Vec::new();
        for child in self.children.iter() {
            if child.1.is_visible() {
                visible_children.push(child.clone())
            }
        }
        visible_children
    }

    /// Draws the children of this view. If a subset is provided only those children will be drawn
    /// otherwise all visible children are drawn in the same fashion as [View].
    /// Children are sorted by the order in [current_child_order] if provided.
    fn draw_children(&mut self, cx: &mut Cx2d, scope: &mut Scope, children_subset: Option<&mut Vec<(LiveId, WidgetRef)>>) -> DrawStep {
        let children = children_subset.unwrap_or(&mut self.children);

        // If there is a user-specified order in which to draw the children, sort them.
        // Note that the user might have provided only a subset of the total chidren
        if !self.current_child_order.is_empty() {
            // Create a map from LiveId to its position in the draw_order
            let draw_order_map: HashMap<LiveId, usize> = self.current_child_order.iter().enumerate().map(|(i, &id)| (id, i)).collect();

            // Sort the children based on the draw_order, with non-matching ones at the end
            children.sort_by(|a, b| {
                let a_order = draw_order_map.get(&a.0);
                let b_order = draw_order_map.get(&b.0);
                match (a_order, b_order) {
                    (Some(a_index), Some(b_index)) => a_index.cmp(b_index),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
        
        // Iterate over the children, advancing by updating the draw state with each children as a step
        while let Some(DrawState::Drawing(step, resume)) = self.draw_state.get() {
            if step < children.len() {
                //let id = self.draw_order[step];
                if let Some((id,child)) = children.get_mut(step) {
                    // if child.is_visible() {
                        let walk = child.walk(cx);
                        if resume {
                            // Draw child with its id on the scope
                            scope.with_id(*id, |scope| child.draw_walk(cx, scope, walk))?;
                        } else if let Some(fw) = cx.defer_walk(walk) {
                            // If there's a deferred walk push into the vec for later drawing
                            self.defer_walks.push((*id, fw));
                        } else {
                            // Draw child with its id on the scope
                            self.draw_state.set(DrawState::Drawing(step, true));
                            scope.with_id(*id, |scope| child.draw_walk(cx, scope, walk))?;
                        }
                    // }
                }
                self.draw_state.set(DrawState::Drawing(step + 1, false));
            } else {
                self.draw_state.set(DrawState::DeferWalk(0));
            }
        }

        // Draw deferred walks
        while let Some(DrawState::DeferWalk(step)) = self.draw_state.get() {
            if step < self.defer_walks.len() {
                let (id, dw) = &mut self.defer_walks[step];
                if let Some((id, child)) = children.iter_mut().find(|(id2,_)|id2 == id) {
                    let walk = dw.resolve(cx);
                    scope.with_id(*id, |scope| child.draw_walk(cx, scope, walk))?;
                }
                self.draw_state.set(DrawState::DeferWalk(step + 1));
            } else {
                if let Some(scroll_bars) = &mut self.scroll_bars_obj {
                    scroll_bars.draw_scroll_bars(cx);
                };

                if self.show_bg {
                    if self.optimize.is_texture() {
                        panic!("dont use show_bg and texture caching at the same time");
                    }
                    self.draw_bg.end(cx);
                    self.area = self.draw_bg.area();
                } else {
                    cx.end_turtle_with_area(&mut self.area);
                };

                if let Some(scroll_bars) = &mut self.scroll_bars_obj {
                    scroll_bars.set_area(self.area);
                    scroll_bars.end_nav_area(cx);
                };

                if self.optimize.needs_draw_list() {
                    let rect = self.area.rect(cx);
                    self.view_size = Some(rect.size);
                    self.draw_list.as_mut().unwrap().end(cx);

                    if self.optimize.is_texture() {
                        let texture_cache = self.texture_cache.as_mut().unwrap();
                        cx.end_pass(&texture_cache.pass);
                        /*if cache.pass.id_equals(4){
                            self.draw_bg.draw_vars.set_uniform(cx, id!(marked),&[1.0]);
                        }
                        else{
                            self.draw_bg.draw_vars.set_uniform(cx, id!(marked),&[0.0]);
                        }*/
                        self.draw_bg
                            .draw_vars
                            .set_texture(0, &texture_cache.color_texture);
                        self.draw_bg.draw_abs(cx, rect);
                        let area = self.draw_bg.area();
                        let texture_cache = self.texture_cache.as_mut().unwrap();
                        /* if false {
                            // FIXME(eddyb) this was the previous logic,
                            // but the only tested apps that use `CachedView`
                            // are sized correctly (regardless of `dpi_factor`)
                            // *without* extra scaling here.
                            cx.set_pass_scaled_area(
                                &texture_cache.pass,
                                area,
                                2.0 / self.dpi_factor.unwrap_or(1.0),
                            );
                        } else {*/
                            cx.set_pass_area(
                                &texture_cache.pass,
                                area,
                            );
                        //}
                    }
                }
                self.draw_state.end();
            }
        }
        DrawStep::done()
    }

    fn draw_navigable_children(&mut self, cx: &mut Cx2d, scope: &mut Scope, navigation_config: NavigationConfig) -> DrawStep {
       let visible_children = self.get_visible_children();
        match navigation_config.mode {
            NavigationMode::Stack => {
                // Draw only the active navigation item or all present items for the given layout mode
                let mut children_to_draw = Vec::new();

                // If there is an active navigation item, simply draw the active child and skip the rest. 
                if let Some(child_id) = self.navigation_state.active_item_id {
                    if let Some(child) = self.children.iter().find(|c| c.0.eq(&child_id)) {
                        children_to_draw.push(child.clone());
                    } else {
                        error!("Attempted to navigate to a child that is not present in the navigation list of its parent.")
                    }
                } else {
                    // There is no active item in the navigation, draw all visible items for the given layout mode.
                    children_to_draw = visible_children;
                }

                 let _ = self.draw_children(cx, scope, Some(&mut children_to_draw)); // TODO error handling
            },
            NavigationMode::Tabs => todo!(),
            NavigationMode::Drawer => todo!(),
        }
        DrawStep::done()
    }

    pub fn set_scroll_pos(&mut self, cx: &mut Cx, v: DVec2) {
        if let Some(scroll_bars) = &mut self.scroll_bars_obj {
            scroll_bars.set_scroll_pos(cx, v);
        } else {
            self.current_layout.scroll = v;
        }
    }

    pub fn area(&self) -> Area {
        self.area
    }

    pub fn walk_from_previous_size(&self, walk: Walk) -> Walk {
        let view_size = self.view_size.unwrap_or(DVec2::default());
        Walk {
            abs_pos: walk.abs_pos,
            width: if walk.width.is_fill() {
                walk.width
            } else {
                Size::Fixed(view_size.x)
            },
            height: if walk.height.is_fill() {
                walk.height
            } else {
                Size::Fixed(view_size.y)
            },
            margin: walk.margin,
        }
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }
    
    pub fn debug_print_children(&self){
        log!("Debug print view children {:?}", self.children.len());
        for i in 0..self.children.len(){
            log!("Child: {}",self.children[i].0)
        }
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum AdaptiveLayoutViewAction {
    None,
    NavigateTo(LiveId)   
}

#[derive(Copy, Clone, Debug, Live, LiveHook, LiveRegister, PartialEq)]
#[live_ignore]
pub enum ViewPresence {
    #[pick]
    Visible,
    Hidden,
    NavigationItem,
}

#[derive(Clone, Debug, Live, LiveHook, LiveRegister, PartialEq)]
#[live_ignore]
pub struct NavigationConfig {
    #[live] mode: NavigationMode,
    #[live] items: Vec<LiveId>
}

#[derive(Copy, Clone, Debug, Live, LiveHook, LiveRegister, PartialEq)]
#[live_ignore]
pub enum NavigationMode {
    #[pick] #[live] Stack,
    #[live] Tabs,
    #[live] Drawer
}

#[derive(Default)]
struct NavigationState {
    active_item_id: Option<LiveId>
}
