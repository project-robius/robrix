use makepad_widgets::*;
live_design ! { 
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;
    ReactionList = {{ReactionList}} { 
        item: <Button> { draw_text: { text_style: { font_size: 16, }, }, } 
    } 
} 
#[derive(Live, Widget)] pub struct ReactionList { 
    #[redraw] #[rust] 
    area: Area, 
    #[live] item: Option<LivePtr>, 
    #[rust] children: ComponentMap<LiveId, ButtonRef>, 
    #[layout] layout: Layout, 
    #[walk] walk: Walk, 
    #[rust] pub list: Vec<String>, 
} 
impl Widget for ReactionList { 
    fn draw_walk(& mut self, cx:&mut Cx2d, _scope:&mut Scope, walk: Walk) -> DrawStep { 
        cx.begin_turtle(walk, self.layout);
        for(index, value) in self.list.iter().enumerate() { 
            let target = self.children.get_or_insert(cx, LiveId(index as u64), |cx | { 
                WidgetRef::new_from_ptr(cx, self.item).as_button() 
            });
            target.set_text(value);
            target.draw_all(cx,&mut Scope::empty());
        }
        cx.end_turtle();
        self.children.retain_visible();
        DrawStep::done() 
    }
    fn handle_event(& mut self, cx:&mut Cx, event:&Event, scope:&mut Scope) { 
        self.children.iter().enumerate()
        .for_each(|(_index,(_id, widget_ref)) | { 
            widget_ref.handle_event(cx, event, scope);
        });
    } 
} 
impl LiveHook for ReactionList { 
    fn before_apply(& mut self, cx:&mut Cx, apply:&mut Apply, index: usize, nodes:&[LiveNode]) {
        
        self.list = vec ! ["Hello".to_string(), "GenUI".to_string(), "Rust".to_string()];
    } 
} 
impl ReactionListRef { 
    pub fn set_list(& mut self, looper: Vec<String>) { 
        if let Some(mut instance) = self.borrow_mut() { 
            instance.list = looper;
        } 
    } 
}