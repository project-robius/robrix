use makepad_widgets::*;

/// Actions representing the possible views that can be shown,
/// i.e., views that can be "made active" or "animated in"
/// by a stack navigation container.
#[derive(Clone, DefaultNone, Eq, Hash, PartialEq, Debug)]
pub enum StackViewAction {
    None,
    ShowAddContact,
    ShowMoments,
    ShowMyProfile,
    ShowRoom,
}

/// Actions that are delivered to an incoming or outgoing "active" widget/view
/// within a stack navigation container.
#[derive(Clone, DefaultNone, Eq, Hash, PartialEq, Debug)]
pub enum StackViewSubWidgetAction {
    None,
    /// The widget is being shown.
    /// This is sent to the widget/view being animated in,
    /// at the very beginning of that animate-in process.
    Show,
    /// The widget is being hidden.
    /// This is sent to the widget/view being animated out,
    /// at the very beginning of that animate-out process.
    Hide,
}
