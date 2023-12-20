use makepad_widgets::*;

#[derive(Clone, DefaultNone, Eq, Hash, PartialEq, Debug)]
pub enum StackViewAction {
    None,
    ShowAddContact,
    ShowMoments,
    ShowMyProfile,
    ShowRoom,
}
