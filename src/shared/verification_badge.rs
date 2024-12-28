use makepad_widgets::*;
use matrix_sdk::encryption::VerificationState;

// First, define the verification icons component layout
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::my_tooltip::*;

    VERIFICATION_YES = dep("crate://self/resources/icons/verification_yes.svg")
    VERIFICATION_NO = dep("crate://self/resources/icons/verification_no.svg")
    VERIFICATION_UNK = dep("crate://self/resources/icons/verification_unk.svg")

    VerificationIcon = <Icon> {
        icon_walk: { width: 23 }
    }

    pub IconYes = <View> {
        visible: false
        width: 31, height: 31
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_YES),
                fn get_color(self) -> vec4 {
                    return #x00BF00;
                }
            }
        }
    }

    pub IconNo = <View> {
        visible: false
        width: 31, height: 31
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_NO),
                fn get_color(self) -> vec4 {
                    return #xBF0000;
                }
            }
        }
    }

    pub IconUnk = <View> {
        visible: false
        width: 31, height: 31
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_UNK),
                fn get_color(self) -> vec4 {
                    return #x333333;
                }
            }
        }
    }

    pub VerificationBadge = {{VerificationBadge}} {
        width: Fit, height: Fit
        flow: Overlay
        align: { x: 0.5, y: 0.5 }

        visible: true

        verification_icons = <View> {
            flow: Overlay
            align: { x: 0.5, y: 0.5 }
            width: 31, height: 31

            icon_yes = <IconYes> {}
            icon_no = <IconNo> {}
            icon_unk = <IconUnk> {}
        }
    }
}

// Define the verification states and messages
#[derive(Default)]
pub enum VerificationText {
    #[default]
    Unknown,
    Verified,
    Unverified,
}

impl VerificationText {
    pub fn get_text(&self) -> &'static str {
        match self {
            VerificationText::Verified => "This device is fully verified.",
            VerificationText::Unverified => " This device is unverified. To view your encrypted message history, please verify it from another client.",
            VerificationText::Unknown => " Verification state is unknown.",
        }
    }

    pub fn from_state(state: VerificationState) -> Self {
        match state {
            VerificationState::Verified => Self::Verified,
            VerificationState::Unverified => Self::Unverified,
            VerificationState::Unknown => Self::Unknown,
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct VerificationBadge {
    #[deref]
    view: View,
    #[rust(VerificationState::Unknown)]
    pub verification_state: VerificationState,
}

impl VerificationBadge {
    pub fn get_icons_rect(&self, cx: &Cx) -> Rect {
        self.view(id!(verification_icons)).area().rect(cx)
    }
}

impl Widget for VerificationBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl VerificationBadgeRef {
    pub fn set_verification_state(&mut self, cx: &mut Cx, state: VerificationState) {
        if let Some(mut inner) = self.0.borrow_mut::<VerificationBadge>() {
            if inner.verification_state != state {
                inner.verification_state = state;
                inner.update_icon_visibility();
                inner.redraw(cx);
            }
        }
    }
}

impl VerificationBadge {
    pub fn update_icon_visibility(&mut self) {
        let (yes, no, unk) = match self.verification_state {
            VerificationState::Unknown => (false, false, true),
            VerificationState::Unverified => (false, true, false),
            VerificationState::Verified => (true, false, false),
        };

        self.view(id!(icon_yes)).set_visible(yes);
        self.view(id!(icon_no)).set_visible(no);
        self.view(id!(icon_unk)).set_visible(unk);
    }
}
