use makepad_widgets::*;

#[derive(Clone, WidgetAction, Eq, Hash, PartialEq)]
pub enum StackViewAction {
    None,
    ShowAddContact,
    ShowMoments,
    ShowMyProfile,
    ShowRoom,
}
