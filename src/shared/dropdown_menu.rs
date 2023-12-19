use std::cell::RefCell;
use std::rc::Rc;

use makepad_widgets::makepad_derive_widget::*;
use makepad_widgets::makepad_draw::*;
use makepad_widgets::widget::*;

use crate::shared::popup_menu::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::popup_menu::*;

    DropDown = {{WechatDropDown}} {
        width: Fit,
        height: Fit,
        margin: {left: 1.0, right: 1.0, top: 1.0, bottom: 1.0},
        align: {x: 0., y: 0.},
        padding: {left: 5.0, top: 5.0, right: 4.0, bottom: 5.0}

        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }

        draw_icon: {
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

        icon_walk: { width: 20., height: Fit}

        popup_menu: <PopupMenu> {
        }

        popup_shift: vec2(-6.0,4.0)

        selected_item: -1
        animator: {
            hover = {
                default: off,
                off = {
                    from: {all: Forward {duration: 0.1}}
                    apply: {
                        draw_icon: {pressed: 0.0, hover: 0.0}
                    }
                }

                on = {
                    from: {
                        all: Forward {duration: 0.1}
                        pressed: Forward {duration: 0.01}
                    }
                    apply: {
                        draw_icon: {pressed: 0.0, hover: [{time: 0.0, value: 1.0}],}
                    }
                }

                pressed = {
                    from: {all: Forward {duration: 0.2}}
                    apply: {
                        draw_icon: {pressed: [{time: 0.0, value: 1.0}], hover: 1.0,}
                    }
                }
            }
        }
    }
}

#[derive(Live)]
pub struct WechatDropDown {
    #[animator]
    animator: Animator,

    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[live]
    draw_bg: DrawQuad,
    #[live]
    draw_icon: DrawIcon,
    #[live]
    icon_walk: Walk,

    #[live]
    bind: String,
    #[live]
    bind_enum: String,

    #[live]
    popup_menu: Option<LivePtr>,

    #[live]
    labels: Vec<String>,
    #[live]
    values: Vec<LiveValue>,
    #[live]
    icons: Vec<LiveDependency>,

    #[live]
    popup_shift: DVec2,

    #[rust]
    is_open: bool,

    #[live]
    selected_item: usize,
}

impl LiveHook for WechatDropDown {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, WechatDropDown)
    }

    fn after_apply(&mut self, cx: &mut Cx, from: ApplyFrom, _index: usize, _nodes: &[LiveNode]) {
        if self.popup_menu.is_none() || !from.is_from_doc() {
            return;
        }
        let global = cx.global::<PopupMenuGlobal>().clone();
        let mut map = global.map.borrow_mut();

        // when live styling clean up old style references
        map.retain(|k, _| cx.live_registry.borrow().generation_valid(*k));

        let list_box = self.popup_menu.unwrap();
        map.get_or_insert(cx, list_box, |cx| {
            PopupMenu::new_from_ptr(cx, Some(list_box))
        });
    }
}
#[derive(Clone, WidgetAction, Debug)]
pub enum WechatDropDownAction {
    Select(usize, LiveValue),
    None,
}

#[derive(Default, Clone)]
struct PopupMenuGlobal {
    map: Rc<RefCell<ComponentMap<LivePtr, PopupMenu>>>,
}

impl WechatDropDown {
    pub fn toggle_open(&mut self, cx: &mut Cx) {
        if self.is_open {
            self.set_closed(cx);
        } else {
            self.set_open(cx);
        }
    }

    pub fn set_open(&mut self, cx: &mut Cx) {
        self.is_open = true;
        self.draw_bg.redraw(cx);
        let global = cx.global::<PopupMenuGlobal>().clone();
        let mut map = global.map.borrow_mut();
        let lb = map.get_mut(&self.popup_menu.unwrap()).unwrap();
        let node_id = LiveId(self.selected_item as u64).into();
        lb.init_select_item(node_id);
        cx.sweep_lock(self.draw_bg.area());
    }

    pub fn set_closed(&mut self, cx: &mut Cx) {
        self.is_open = false;
        self.draw_bg.redraw(cx);
        cx.sweep_unlock(self.draw_bg.area());
    }

    pub fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WechatDropDownAction),
    ) {
        self.animator_handle_event(cx, event);

        if self.is_open && self.popup_menu.is_some() {
            let global = cx.global::<PopupMenuGlobal>().clone();
            let mut map = global.map.borrow_mut();
            let menu = map.get_mut(&self.popup_menu.unwrap()).unwrap();
            let mut close = false;
            menu.handle_event_with(
                cx,
                event,
                self.draw_bg.area(),
                &mut |cx, action| if let PopupMenuAction::WasSelected(node_id) = action {
                    self.selected_item = node_id.0 .0 as usize;
                    dispatch_action(
                        cx,
                        WechatDropDownAction::Select(
                            self.selected_item,
                            self.values
                                .get(self.selected_item)
                                .cloned()
                                .unwrap_or(LiveValue::None),
                        ),
                    );
                    self.draw_bg.redraw(cx);
                    close = true;
                },
            );
            if close {
                self.set_closed(cx);
            }

            // check if we touch outside of the popup menu
            if let Event::MouseDown(e) = event {
                if !menu.menu_contains_pos(cx, e.abs) {
                    self.set_closed(cx);
                    self.animator_play(cx, id!(hover.off));
                }
            }
        }
        // TODO: close on clicking outside of the popup menu
        match event.hits_with_sweep_area(cx, self.draw_bg.area(), self.draw_bg.area()) {
            Hit::KeyFocusLost(_) => {
                self.set_closed(cx);
                self.animator_play(cx, id!(hover.off));
                self.draw_bg.redraw(cx);
            }
            Hit::KeyDown(ke) => match ke.key_code {
                KeyCode::ArrowUp => {
                    if self.selected_item > 0 {
                        self.selected_item -= 1;
                        dispatch_action(
                            cx,
                            WechatDropDownAction::Select(
                                self.selected_item,
                                self.values[self.selected_item].clone(),
                            ),
                        );
                        self.set_closed(cx);
                        self.draw_bg.redraw(cx);
                    }
                }
                KeyCode::ArrowDown => {
                    if !self.values.is_empty() && self.selected_item < self.values.len() - 1 {
                        self.selected_item += 1;
                        dispatch_action(
                            cx,
                            WechatDropDownAction::Select(
                                self.selected_item,
                                self.values[self.selected_item].clone(),
                            ),
                        );
                        self.set_closed(cx);
                        self.draw_bg.redraw(cx);
                    }
                }
                _ => (),
            },
            Hit::FingerDown(_fe) => {
                cx.set_key_focus(self.draw_bg.area());
                self.toggle_open(cx);
                self.animator_play(cx, id!(hover.pressed));
            }
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
                self.animator_play(cx, id!(hover.on));
            }
            Hit::FingerHoverOut(_) => {
                self.animator_play(cx, id!(hover.off));
            }
            Hit::FingerUp(fe) => {
                if fe.is_over {
                    if fe.device.has_hovers() {
                        self.animator_play(cx, id!(hover.on));
                    }
                } else {
                    self.animator_play(cx, id!(hover.off));
                }
            }
            _ => (),
        };
    }

    pub fn draw_walk(&mut self, cx: &mut Cx2d, walk: Walk) {
        // cx.clear_sweep_lock(self.draw_bg.area());

        self.draw_bg.begin(cx, walk, self.layout);
        //let start_pos = cx.turtle().rect().pos;
        self.draw_icon.draw_walk(cx, self.icon_walk);
        self.draw_bg.end(cx);

        cx.add_nav_stop(self.draw_bg.area(), NavRole::DropDown, Margin::default());

        if self.is_open && self.popup_menu.is_some() {
            // cx.set_sweep_lock(self.draw_bg.area());
            let global = cx.global::<PopupMenuGlobal>().clone();
            let mut map = global.map.borrow_mut();
            let popup_menu = map.get_mut(&self.popup_menu.unwrap()).unwrap();

            popup_menu.begin(cx);

            for (i, item) in self.labels.iter().enumerate() {
                let node_id = LiveId(i as u64).into();
                popup_menu.draw_item(cx, node_id, item, self.icons[i].clone());
            }

            popup_menu.end(cx, self.draw_bg.area());
        }
    }
}

// It is named WechatDropDown because DropDown is already a widget in makepad_widgets
impl Widget for WechatDropDown {
    fn widget_to_data(
        &self,
        _cx: &mut Cx,
        actions: &WidgetActions,
        nodes: &mut LiveNodeVec,
        path: &[LiveId],
    ) -> bool {
        match actions.single_action(self.widget_uid()) {
            WechatDropDownAction::Select(_, value) => {
                nodes.write_field_value(path, value.clone());
                true
            }
            _ => false,
        }
    }

    fn data_to_widget(&mut self, cx: &mut Cx, nodes: &[LiveNode], path: &[LiveId]) {
        if let Some(value) = nodes.read_field_value(path) {
            if let Some(index) = self.values.iter().position(|v| v == value) {
                if self.selected_item != index {
                    self.selected_item = index;
                    self.redraw(cx);
                }
            } else {
                // error!("Value not in values list {:?}", value);
            }
        }
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.draw_bg.redraw(cx);
    }

    fn handle_widget_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        let uid = self.widget_uid();
        self.handle_event_with(cx, event, &mut |cx, action| {
            dispatch_action(cx, WidgetActionItem::new(action.into(), uid))
        });
    }

    fn walk(&mut self, _cx: &mut Cx) -> Walk {
        self.walk
    }

    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        self.draw_walk(cx, walk);
        WidgetDraw::done()
    }
}

#[derive(Clone, PartialEq, WidgetRef)]
pub struct WechatDropDownRef(WidgetRef);

impl WechatDropDownRef {
    pub fn item_clicked(&mut self, item_id: &[LiveId], actions: &WidgetActions) -> bool {
        if let Some(item) = actions.find_single_action(self.widget_uid()) {
            if let WechatDropDownAction::Select(_id, value) = item.action() {
                return LiveValue::Bool(true) == value.enum_eq(item_id)
            }
        }
        return false
    }
}
