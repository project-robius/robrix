use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use crate::shared::avatar::Avatar;
live_design!{
    import makepad_draw::shader::std::*;
    import crate::shared::avatar::*;
    
    Sequencer = {{Sequencer}} {
        button: <Avatar> {
            width: 15.0,
            height: 15.0,
            text_view = { text = { draw_text: {
                text_style: { font_size: 6.0 }
            }}}
        }
        margin: {top: 3, right: 10, bottom: 3, left: 10}
        width: Fit,
        height: Fit
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, Copy, PartialEq, FromLiveId)]
pub struct AvatarId(pub LiveId);

#[derive(Live, Widget)]
pub struct Sequencer {
    #[redraw] #[live] draw_text: DrawText,
    #[rust] area: Area,
    #[walk] walk: Walk,
    #[live] button: Option<LivePtr>,
    #[live(false)] hover_actions_enabled: bool,
    #[rust] buttons: ComponentMap<AvatarId, Avatar>,
    #[rust] read_receipts: Vec<String>
}

impl LiveHook for Sequencer {
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
pub enum SequencerAction {
    HoverIn(Rect),
    HoverOut,
    None
}
impl Widget for Sequencer {

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        for button in self.buttons.values_mut() {
            match button.hit(cx, event, self.area){
                Hit::FingerHoverIn(_) => {
                    let rect = self.area.rect(cx);
                    cx.widget_action(uid, &scope.path, SequencerAction::HoverIn(rect));
                }
                Hit::FingerHoverOut(_) => {
                    cx.widget_action(uid, &scope.path, SequencerAction::HoverOut);
                }
                _=>{}
            }
        }
        
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::default());        
        let button = self.button;
        for (i, name) in self.read_receipts.iter().enumerate(){
            if i>4{
                break
            }
            let btn_id = LiveId(i as u64).into();
            let btn = self.buttons.get_or_insert(cx, btn_id, | cx | {
                Avatar::new_from_ptr(cx, button)
            });
            btn.set_text(name);            
            btn.draw(cx, scope);
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

impl SequencerRef {
    pub fn get_read_receipts(&self)->Vec<String>{
        if let Some(inner) = self.borrow() {
            return inner.read_receipts.clone();
        }
        return vec![];
    }
    pub fn set_read_receipts(&mut self,cx:&mut Cx , room_id: OwnedRoomId, read_user_id:Vec<OwnedUserId>){
        if let Some(mut inner) = self.borrow_mut() {
            inner.read_receipts = read_user_id.iter().map(| f | f.to_string().chars().nth(1).unwrap().to_string()).collect();
        }
    }
    pub fn hover_in(&self, actions:&Actions)->Option<Rect>{
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            match item.cast(){
                SequencerAction::HoverIn(rect) => Some(rect),
                _=> None
            }
        } else {
            None
        }
    }
    pub fn hover_out(&self, actions:&Actions)->bool{
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            match item.cast(){
                SequencerAction::HoverOut => true,
                _=> false
            }
        } else {
            false
        }
    }
}
