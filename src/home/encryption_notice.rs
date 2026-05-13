use makepad_widgets::*;
use matrix_sdk::room::RoomMember;

use crate::sliding_sync::current_user_id;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.EncryptionNotice = set_type_default() do #(EncryptionNotice::register_widget(vm)) {
        width: Fill,
        height: Fit,
        margin: Inset{left: 16, right: 16, top: 8, bottom: 8}
        padding: 12
        flow: Right
        spacing: 10
        align: Align{x: 0.0, y: 0.5}
        show_bg: true
        draw_bg +: {
            color: #xF0F2F5
            border_radius: 6.0
        }

        lock_filled_icon := View {
            visible: false
            width: Fit, height: Fit
            Icon {
                width: 16,
                height: 16,
                align: Align{x: 0.5, y: 0.5}
                draw_icon +: {
                    svg: (ICON_LOCK_FILLED)
                    color: #888888
                }
                icon_walk: Walk{width: 16, height: 16}
            }
        }

        lock_open_icon := View {
            visible: false
            width: Fit, height: Fit
            Icon {
                width: 16,
                height: 16,
                align: Align{x: 0.5, y: 0.5}
                draw_icon +: {
                    svg: (ICON_LOCK_OPEN)
                    color: #888888
                }
                icon_walk: Walk{width: 16, height: 16}
            }
        }

        text := View {
            width: Fill,
            height: Fit,
            flow: Down
            spacing: 3

            title := Label {
                width: Fill,
                height: Fit
                draw_text +: {
                    color: #202124
                    text_style: BOLD_TEXT { font_size: 10.5 }
                }
                text: ""
            }

            body := Label {
                width: Fill,
                height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: #444444
                    text_style: REGULAR_TEXT { font_size: 9.5 }
                }
                text: ""
            }
        }
    }
}

const ENCRYPTED_TITLE: &str = "Encryption enabled";
const UNENCRYPTED_TITLE: &str = "Encryption not enabled";
const ENCRYPTED_BODY: &str = "Messages here are end-to-end encrypted.";
const UNENCRYPTED_BODY: &str = "Messages here are not end-to-end encrypted.";
const VERIFY_PREFIX: &str = "Messages here are end-to-end encrypted. Verify ";
const VERIFY_SUFFIX: &str = " in their profile - tap on their profile picture.";
const LOADING_MEMBER_PLACEHOLDER: &str = "…";
const DISPLAY_NAME_LIMIT: usize = 30;

pub fn first_other_member_display_name(members: Option<&[RoomMember]>) -> Option<Option<String>> {
    let members = members?;
    let own_user_id = current_user_id();
    let first_other = members
        .iter()
        .find(|member| own_user_id.as_ref().is_none_or(|own| member.user_id() != own))?;

    Some(Some(
        first_other
            .display_name()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| first_other.user_id().to_string()),
    ))
}

pub fn truncate_display_name(display_name: &str) -> String {
    let mut chars = display_name.chars();
    let truncated: String = chars.by_ref().take(DISPLAY_NAME_LIMIT).collect();
    if chars.next().is_some() {
        format!("{truncated}{LOADING_MEMBER_PLACEHOLDER}")
    } else {
        truncated
    }
}

pub fn encryption_notice_copy(
    is_encrypted: bool,
    first_other_member: Option<Option<String>>,
) -> (&'static str, String) {
    if !is_encrypted {
        return (UNENCRYPTED_TITLE, UNENCRYPTED_BODY.to_string());
    }

    // Only show the "Verify <name>" sentence when we actually have a name to put in it.
    // For every other state (members not loaded yet, lonely room, no resolvable display name)
    // fall back to the generic body — never render a "…" placeholder in user-visible copy.
    match first_other_member {
        Some(Some(display_name)) => (
            ENCRYPTED_TITLE,
            format!(
                "{VERIFY_PREFIX}{}{}",
                truncate_display_name(&display_name),
                VERIFY_SUFFIX,
            ),
        ),
        _ => (ENCRYPTED_TITLE, ENCRYPTED_BODY.to_string()),
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct EncryptionNotice {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
}

impl Widget for EncryptionNotice {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl EncryptionNotice {
    pub fn set_content(
        &mut self,
        cx: &mut Cx,
        is_encrypted: bool,
        first_other_member: Option<Option<String>>,
    ) {
        let (title, body) = encryption_notice_copy(is_encrypted, first_other_member);
        self.label(cx, ids!(text.title)).set_text(cx, title);
        self.label(cx, ids!(text.body)).set_text(cx, &body);

        self.view.view(cx, ids!(lock_filled_icon)).set_visible(cx, is_encrypted);
        self.view.view(cx, ids!(lock_open_icon)).set_visible(cx, !is_encrypted);
    }
}

impl EncryptionNoticeRef {
    pub fn set_content(
        &self,
        cx: &mut Cx,
        is_encrypted: bool,
        first_other_member: Option<Option<String>>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_content(cx, is_encrypted, first_other_member);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notice_unencrypted() {
        let (title, body) = encryption_notice_copy(false, None);

        assert_eq!(title, "Encryption not enabled");
        assert_eq!(body, "Messages here are not end-to-end encrypted.");
    }

    #[test]
    fn test_notice_member_placeholder() {
        // When members are not yet loaded (first_other_member = None), we no longer
        // render a "…" placeholder; the body falls back to the generic "encrypted" sentence
        // and the verify sentence appears only after a real name resolves.
        let (title, body) = encryption_notice_copy(true, None);

        assert_eq!(title, "Encryption enabled");
        assert_eq!(body, "Messages here are end-to-end encrypted.");
    }

    #[test]
    fn test_notice_lonely_room() {
        let (title, body) = encryption_notice_copy(true, Some(None));

        assert_eq!(title, "Encryption enabled");
        assert_eq!(body, "Messages here are end-to-end encrypted.");
    }

    #[test]
    fn test_notice_encrypted() {
        let (title, body) = encryption_notice_copy(true, Some(Some("Alice".to_string())));

        assert_eq!(title, "Encryption enabled");
        assert_eq!(
            body,
            "Messages here are end-to-end encrypted. Verify Alice in their profile - tap on their profile picture."
        );
    }

    #[test]
    fn test_notice_truncates_long_display_name_to_30_chars() {
        let body = encryption_notice_copy(
            true,
            Some(Some("123456789012345678901234567890123".to_string())),
        ).1;

        assert_eq!(
            body,
            "Messages here are end-to-end encrypted. Verify 123456789012345678901234567890… in their profile - tap on their profile picture."
        );
    }
}
