use makepad_widgets::makepad_derive_widget::*;
use makepad_widgets::makepad_draw::*;
use makepad_widgets::widget::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::styles::*;

    MenuItem = {{MenuItem}} {
        align: {y: 0.5},
        padding: {left: 5., top: 10., bottom: 10., right: 5.},
        spacing: 5.,
        width: Fill,
        height: Fit

        draw_bg: {
            instance color: #4
            instance color_selected: #5
            instance border_radius: 4.0

            instance selected: 0.0
            instance hover: 0.0
    
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.box(
                    1.0,
                    1.0,
                    self.rect_size.x,
                    self.rect_size.y,
                    self.border_radius
                )
                sdf.fill_keep(mix(self.color, self.color_selected, self.hover))
                return sdf.result;
            }
        }
    
        draw_name: {
            text_style: <REGULAR_TEXT>{font_size: 9},
            instance selected: 0.0
            instance hover: 0.0
    
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        THEME_COLOR_TEXT_DEFAULT,
                        THEME_COLOR_TEXT_SELECTED,
                        self.selected
                    ),
                    THEME_COLOR_TEXT_HOVER,
                    self.hover
                )
            }
        }

        icon_walk: {width: 15., height: Fit, margin: {bottom: 3.}}
        draw_icon: {
            color: #f2f2f2;
            brightness: 0.8;
        }

        animator: {
            hover = {
                default: off
                off = {
                    from: {all: Snap}
                    apply: {
                        draw_bg: {hover: 0.0}
                        draw_name: {hover: 0.0}
                    }
                }
                on = {
                    cursor: Hand
                    from: {all: Snap}
                    apply: {
                        draw_bg: {hover: 1.0}
                        draw_name: {hover: 1.0}
                    }
                }
            }

            select = {
                default: off
                off = {
                    from: {all: Snap}
                    apply: {
                        draw_bg: {selected: 0.0,}
                        draw_name: {selected: 0.0,}
                    }
                }
                on = {
                    from: {all: Snap}
                    apply: {
                        draw_bg: {selected: 1.0,}
                        draw_name: {selected: 1.0,}
                    }
                }
            }
        }
        indent_width: 10.0
    }

    PopupMenu = {{PopupMenu}} {
        menu_item: <MenuItem> {}
        flow: Down,
        padding: 5.,
        width: 140.,
        height: Fit,

        icon_walk: {width: 20., height: Fit}
        draw_icon:{
            instance hover: 0.0
            instance pressed: 0.0
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        #9,
                        #c,
                        self.hover
                    ),
                    #9,
                    self.pressed
                )
            }
        }

        draw_bg: {
            instance color: #4
            instance border_width: 0.0,
            instance border_color: #4,
            instance border_radius: 4.0

            fn get_color(self) -> vec4 {
                return self.color
            }

            fn get_border_color(self) -> vec4 {
                return self.border_color
            }

            fn pixel(self) -> vec4 {

                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                // sdf.blur = 18.0;
                sdf.box(
                    self.border_width,
                    self.border_width,
                    self.rect_size.x - (self.border_width * 2.0),
                    self.rect_size.y - (self.border_width * 2.0),
                    self.border_radius
                )
                sdf.fill_keep(self.get_color())
                return sdf.result;
            }
        }
    }
}

#[derive(Live, LiveRegister, WidgetWrap)]
pub struct PopupMenu {
    #[live]
    draw_list: DrawList2d,

    #[live]
    menu_item: Option<LivePtr>,

    #[live] #[redraw]
    draw_bg: DrawQuad,
    #[live] #[redraw]
    draw_icon: DrawIcon,
    #[live]
    icon_walk: Walk,

    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,

    #[live]
    labels: Vec<String>,
    #[live]
    values: Vec<LiveValue>,

    #[live]
    items: Vec<String>,
    #[rust]
    menu_items: ComponentMap<MenuItemId, MenuItem>,
    #[rust]
    init_select_item: Option<MenuItemId>,

    #[rust]
    first_tap: bool,
    #[rust]
    count: usize,
}

impl LiveHook for PopupMenu {
    fn after_apply(&mut self, cx: &mut Cx, from: ApplyFrom, index: usize, nodes: &[LiveNode]) {
        if let Some(index) = nodes.child_by_name(index, live_id!(list_node).as_field()) {
            for (_, node) in self.menu_items.iter_mut() {
                node.apply(cx, from, index, nodes);
            }
        }
        self.draw_list.redraw(cx);
    }
}

impl Widget for PopupMenu {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        self.draw_bg.begin(cx, walk, self.layout);
        self.draw_icon.draw_walk(cx, self.icon_walk);
        self.draw_bg.end(cx);
        DrawStep::done()
    }
}

impl PopupMenu {
    pub fn menu_contains_pos(&self, cx: &mut Cx, pos: DVec2) -> bool {
        self.draw_bg.area().get_clipped_rect(cx).contains(pos)
    }

    pub fn begin(&mut self, cx: &mut Cx2d) {
        self.draw_list.begin_overlay_reuse(cx);

        cx.begin_pass_sized_turtle(Layout::flow_down());

        self.draw_bg.begin(cx, self.walk, self.layout);
        self.count = 0;
    }

    pub fn end(&mut self, cx: &mut Cx2d, shift_area: Area) {
        self.draw_bg.end(cx);
        let area = self.draw_bg.area().get_rect(cx);
        let shift = DVec2 {
            x: -area.size.x + (shift_area.get_rect(cx).size.x * 0.7),
            y: 30.,
        };

        cx.end_pass_sized_turtle_with_shift(shift_area, shift);
        self.draw_list.end(cx);

        self.menu_items.retain_visible();
        if let Some(init_select_item) = self.init_select_item.take() {
            self.select_item_state(cx, init_select_item);
        }
    }

    pub fn draw_item(
        &mut self,
        cx: &mut Cx2d,
        item_id: MenuItemId,
        label: &str,
        icon: LiveDependency,
    ) {
        self.count += 1;

        let menu_item = self.menu_item;
        let menu_item = self
            .menu_items
            .get_or_insert(cx, item_id, |cx| MenuItem::new_from_ptr(cx, menu_item));
        menu_item.draw_item(cx, label, icon);
    }

    pub fn init_select_item(&mut self, which_id: MenuItemId) {
        self.init_select_item = Some(which_id);
        self.first_tap = true;
    }

    fn select_item_state(&mut self, cx: &mut Cx, which_id: MenuItemId) {
        for (id, item) in &mut *self.menu_items {
            if *id == which_id {
                item.animator_cut(cx, id!(select.on));
                item.animator_cut(cx, id!(hover.on));
            } else {
                item.animator_cut(cx, id!(select.off));
                item.animator_cut(cx, id!(hover.off));
            }
        }
    }

    pub fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        sweep_area: Area,
        dispatch_action: &mut dyn FnMut(&mut Cx, PopupMenuAction),
    ) {
        let mut actions = Vec::new();
        for (item_id, node) in self.menu_items.iter_mut() {
            node.handle_event_with(cx, event, sweep_area, &mut |_, e| {
                actions.push((*item_id, e))
            });
        }

        for (node_id, action) in actions {
            match action {
                MenuItemAction::WasSweeped => {
                    self.select_item_state(cx, node_id);
                    dispatch_action(cx, PopupMenuAction::WasSweeped(node_id));
                }
                MenuItemAction::WasSelected => {
                    self.select_item_state(cx, node_id);
                    dispatch_action(cx, PopupMenuAction::WasSelected(node_id));
                }
                _ => (),
            }
        }
    }
}

#[derive(Live, LiveHook, LiveRegister, WidgetWrap)]
pub struct MenuItem {
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,

    #[live] #[redraw]
    draw_bg: DrawQuad,
    #[live] #[redraw]
    draw_name: DrawText,

    #[live] #[redraw]
    draw_icon: DrawIcon,
    #[live]
    icon_walk: Walk,

    #[live]
    indent_width: f32,

    #[animator]
    animator: Animator,
    #[live]
    opened: f32,
    #[live]
    hover: f32,
    #[live]
    selected: f32,
}

#[derive(Default, Clone, Debug)]
pub enum MenuItemAction {
    WasSweeped,
    WasSelected,
    #[default]
    None,
}

#[derive(Clone, DefaultNone, Debug)]
pub enum PopupMenuAction {
    WasSweeped(MenuItemId),
    WasSelected(MenuItemId),
    None,
}

#[derive(Clone, Debug, Default, Eq, Hash, Copy, PartialEq, FromLiveId)]
pub struct MenuItemId(pub LiveId);

impl MenuItem {
    pub fn draw_item(&mut self, cx: &mut Cx2d, label: &str, icon: LiveDependency) {
        self.draw_bg.begin(cx, self.walk, self.layout);
        self.draw_icon.svg_file = icon;
        self.draw_icon.draw_walk(cx, self.icon_walk);
        self.draw_name
            .draw_walk(cx, Walk::fit(), Align { x: 0., y: 0.5 }, label);
        self.draw_bg.end(cx);
    }

    pub fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        sweep_area: Area,
        dispatch_action: &mut dyn FnMut(&mut Cx, MenuItemAction),
    ) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.draw_bg.area().redraw(cx);
        }

        match event.hits_with_options(
            cx,
            self.draw_bg.area(),
            HitOptions::new().with_sweep_area(sweep_area),
        ) {
            Hit::FingerHoverIn(_) => {
                self.animator_play(cx, id!(hover.on));
            }
            Hit::FingerHoverOut(_) => {
                self.animator_play(cx, id!(hover.off));
            }
            Hit::FingerDown(_) => {
                dispatch_action(cx, MenuItemAction::WasSweeped);
                self.animator_play(cx, id!(hover.on));
                self.animator_play(cx, id!(select.on));
            }
            Hit::FingerUp(se) => {
                if !se.is_sweep {
                    dispatch_action(cx, MenuItemAction::WasSelected);
                } else {
                    self.animator_play(cx, id!(hover.off));
                    self.animator_play(cx, id!(select.off));
                }
            }
            _ => {}
        }
    }
}
