use makepad_widgets::*;
use crate::shared::avatar::{Avatar};
live_design!{
    import makepad_draw::shader::std::*;
    import crate::shared::avatar::Avatar;
    EmojiSequencer = {{EmojiSequencer}} {
        button: <Button> {
            width: 14,
            height: 14,
            text: "Hello world",
            draw_text:{color:#52b2ac},
            draw_bg : { fn pixel (self) -> vec4 { return vec4 (1.0 , 0.0 , 0.0 , 1.0) ; } }
        }
        width: Fit,
        height: Fit
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, Copy, PartialEq, FromLiveId)]
pub struct ButtonId(pub LiveId);

#[derive(Live, Widget)]
pub struct EmojiSequencer {
    #[redraw] #[live] draw_text: DrawText,
    #[rust] area: Area,
    #[walk] walk: Walk,
    #[live] button: Option<LivePtr>,
    #[live(false)] hover_actions_enabled: bool,
    #[rust] buttons: ComponentMap<ButtonId, Button>,
    #[rust] list: Vec<String>
}

impl LiveHook for EmojiSequencer {
fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        for button in self.buttons.values_mut() {
            if let Some(index) = nodes.child_by_name(index, live_id!(button).as_field()) {
                button.apply(cx, apply, index, nodes);
            }
        }
        self.area.redraw(cx);
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum EmojiSequencerAction {
    None
}
impl Widget for EmojiSequencer {

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        for button in self.buttons.values_mut() {

        }
        
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::default());        
        let button = self.button;
        for (i,name) in vec![String::from("g"),String::from("k")].iter().enumerate(){
            let btn_id = LiveId(i as u64).into();
            let btn = self.buttons.get_or_insert(cx, btn_id, | cx | {
                Button::new_from_ptr(cx, button)
            });
            btn.set_text(name);
            btn.draw(cx, scope).unwrap();
        }

        cx.end_turtle_with_area(&mut self.area);
        self.buttons.retain_visible();
        DrawStep::done()
    }
    
    fn widget_to_data(&self, cx: &mut Cx, actions: &Actions, nodes: &mut LiveNodeVec, path: &[LiveId]) -> bool {
        false
    }

    fn data_to_widget(&mut self, cx: &mut Cx, nodes:&[LiveNode], path: &[LiveId]){
    }
}

impl EmojiSequencerRef {
    pub fn get_list(&self)->Vec<String>{
        if let Some(inner) = self.borrow() {
            return inner.list.clone();
        }
        return vec![];
    }
    pub fn set_list(&mut self,read_user_id:Vec<String>){
        if let Some(mut inner) = self.borrow_mut() {
            inner.list = read_user_id.iter().map(|f|f.to_string()).collect();
        }
    }
    
}
