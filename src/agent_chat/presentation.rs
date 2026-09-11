//! Presentation tweaks for messages relayed from agents by the hagency bridge.
//!
//! The bridge stamps every relayed agent message in two language-independent
//! ways: a leading emoji marking the message *type* (`📋` request, `↩️` reply,
//! `ℹ️` inform) and a trailing `🔗 <permalink>` line. Both are useful as
//! structure and noisy as prose, so the timeline turns the marker into a badge
//! and drops the permalink line from the body. The agent's *role* is encoded
//! in its account name (`@ac_<team>_<role>`), so it needs no parsing at all.

use std::borrow::Cow;

use matrix_sdk::ruma::{events::room::message::FormattedBody, UserId};

use super::agents::{is_agent_localpart, AgentRole};

/// How the bridge tagged a relayed message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentReplyKind {
    Request,
    Reply,
    Inform,
}

impl AgentReplyKind {
    /// Splits a leading type marker off `body`, returning the kind and the rest.
    pub fn split_from_body(body: &str) -> (Option<Self>, &str) {
        let trimmed = body.trim_start();
        for (marker, kind) in [
            ("📋", Self::Request),
            ("↩️", Self::Reply),
            // The variation-selector-free form of ℹ️ also occurs in the wild.
            ("ℹ️", Self::Inform),
            ("ℹ", Self::Inform),
        ] {
            if let Some(rest) = trimmed.strip_prefix(marker) {
                return (Some(kind), rest.trim_start());
            }
        }
        (None, body)
    }

    /// Short badge text for this kind.
    pub fn label(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Reply => "reply",
            Self::Inform => "info",
        }
    }
}

/// Splits a trailing `🔗 https://…` permalink line off `body`.
///
/// Returns the body without that line (and without any blank lines that
/// preceded it), plus the URL if one was found.
pub fn split_permalink(body: &str) -> (Cow<'_, str>, Option<&str>) {
    let Some(last_line) = body.lines().next_back() else {
        return (Cow::Borrowed(body), None);
    };
    let Some(url) = last_line.trim().strip_prefix('🔗').map(str::trim) else {
        return (Cow::Borrowed(body), None);
    };
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return (Cow::Borrowed(body), None);
    }
    let mut lines: Vec<&str> = body.lines().collect();
    lines.pop();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    (Cow::Owned(lines.join("\n")), Some(url))
}

/// Removes the bridge's trailing `<a …>🔗 …</a>` anchor, and the `<br>`s
/// leading up to it, from a formatted body.
pub fn strip_permalink_anchor_from_html(html: &str) -> Cow<'_, str> {
    let Some(anchor_start) = html.rfind("<a ") else {
        return Cow::Borrowed(html);
    };
    let tail = &html[anchor_start..];
    if !tail.contains('🔗') || !tail.trim_end().ends_with("</a>") {
        return Cow::Borrowed(html);
    }
    let mut head = html[..anchor_start].trim_end();
    loop {
        let trimmed = head
            .strip_suffix("<br>")
            .or_else(|| head.strip_suffix("<br/>"))
            .or_else(|| head.strip_suffix("<br />"));
        match trimmed {
            Some(shorter) => head = shorter.trim_end(),
            None => break,
        }
    }
    Cow::Owned(head.to_owned())
}

/// Removes a leading type-marker emoji from a formatted body, if present.
fn strip_kind_marker_from_html(html: &str) -> Cow<'_, str> {
    let (kind, rest) = AgentReplyKind::split_from_body(html);
    if kind.is_some() {
        Cow::Owned(rest.to_owned())
    } else {
        Cow::Borrowed(html)
    }
}

/// What the timeline knows about an agent-sent message before drawing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMessagePresentation {
    /// The sender's workflow role, if its account name encodes one.
    pub role: Option<AgentRole>,
    /// The message type the bridge stamped on the body, if any.
    pub kind: Option<AgentReplyKind>,
    /// The permalink the bridge appended, if any.
    pub permalink: Option<String>,
}

impl AgentMessagePresentation {
    /// Returns `Some` if `sender` looks like an agent puppet account.
    pub fn for_sender(sender: &UserId) -> Option<Self> {
        let localpart = sender.localpart();
        is_agent_localpart(localpart).then(|| Self {
            role: AgentRole::from_name(localpart),
            kind: None,
            permalink: None,
        })
    }

    /// The text of the badge shown next to the sender's name, e.g.
    /// `coordinator · request`, `reviewer`, or just `agent`.
    pub fn badge_text(&self) -> String {
        let role = self.role.map(AgentRole::label).unwrap_or("agent");
        match self.kind {
            Some(kind) => format!("{role} · {}", kind.label()),
            None => role.to_owned(),
        }
    }

    /// Strips the type marker and trailing permalink from a message body and
    /// its formatted counterpart, recording what was removed on `self`.
    pub fn present<'a>(
        &mut self,
        body: &'a str,
        formatted: Option<&'a FormattedBody>,
    ) -> (Cow<'a, str>, Option<Cow<'a, FormattedBody>>) {
        let (kind, rest) = AgentReplyKind::split_from_body(body);
        let (rest, permalink) = split_permalink(rest);
        self.kind = kind;
        self.permalink = permalink.map(str::to_owned);
        let body = match rest {
            Cow::Borrowed(rest) if rest.len() == body.len() => Cow::Borrowed(body),
            Cow::Borrowed(rest) => Cow::Owned(rest.to_owned()),
            Cow::Owned(owned) => Cow::Owned(owned),
        };
        let formatted = formatted.map(|formatted| {
            let stripped = strip_permalink_anchor_from_html(&formatted.body);
            let stripped = match stripped {
                Cow::Borrowed(html) => strip_kind_marker_from_html(html),
                Cow::Owned(html) => Cow::Owned(strip_kind_marker_from_html(&html).into_owned()),
            };
            match stripped {
                Cow::Borrowed(_) => Cow::Borrowed(formatted),
                Cow::Owned(body) => Cow::Owned(FormattedBody {
                    format: formatted.format.clone(),
                    body,
                }),
            }
        });
        (body, formatted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_sdk::ruma::{events::room::message::MessageFormat, OwnedUserId};

    #[test]
    fn kind_marker_is_split_off() {
        assert_eq!(AgentReplyKind::split_from_body("📋 do the thing"), (Some(AgentReplyKind::Request), "do the thing"));
        assert_eq!(AgentReplyKind::split_from_body("↩️done"), (Some(AgentReplyKind::Reply), "done"));
        assert_eq!(AgentReplyKind::split_from_body("ℹ fyi"), (Some(AgentReplyKind::Inform), "fyi"));
        assert_eq!(AgentReplyKind::split_from_body("plain"), (None, "plain"));
    }

    #[test]
    fn permalink_is_split_off_plain_and_html() {
        let (body, url) = split_permalink("line one\nline two\n\n🔗 https://example.org/m/1");
        assert_eq!(body, "line one\nline two");
        assert_eq!(url, Some("https://example.org/m/1"));
        let (body, url) = split_permalink("🔗 not a url");
        assert_eq!(body, "🔗 not a url");
        assert_eq!(url, None);
        assert_eq!(
            strip_permalink_anchor_from_html("<p>hi</p><br><br><a href=\"https://x\">🔗 View formatted</a>"),
            "<p>hi</p>"
        );
        assert_eq!(strip_permalink_anchor_from_html("<a href=\"https://x\">plain link</a>"), "<a href=\"https://x\">plain link</a>");
    }

    #[test]
    fn presentation_for_agent_sender() {
        let sender = OwnedUserId::try_from("@ac_wf_coordinator:example.org").unwrap();
        let mut presentation = AgentMessagePresentation::for_sender(&sender).unwrap();
        assert_eq!(presentation.role, Some(AgentRole::Coordinator));
        let formatted = FormattedBody {
            format: MessageFormat::Html,
            body: "📋 <b>Please review</b><br><a href=\"https://example.org/m/1\">🔗 View formatted</a>".to_owned(),
        };
        let (body, formatted) = presentation.present("📋 Please review\n🔗 https://example.org/m/1", Some(&formatted));
        assert_eq!(body, "Please review");
        assert_eq!(formatted.unwrap().body, "<b>Please review</b>");
        assert_eq!(presentation.kind, Some(AgentReplyKind::Request));
        assert_eq!(presentation.permalink.as_deref(), Some("https://example.org/m/1"));
        assert_eq!(presentation.badge_text(), "coordinator · request");

        let human = OwnedUserId::try_from("@alice:example.org").unwrap();
        assert!(AgentMessagePresentation::for_sender(&human).is_none());
        let helper = OwnedUserId::try_from("@ac_helper:example.org").unwrap();
        assert_eq!(AgentMessagePresentation::for_sender(&helper).unwrap().badge_text(), "agent");
    }

    #[test]
    fn untouched_bodies_borrow() {
        let sender = OwnedUserId::try_from("@ac_wf_reviewer:example.org").unwrap();
        let mut presentation = AgentMessagePresentation::for_sender(&sender).unwrap();
        let (body, formatted) = presentation.present("nothing special", None);
        assert!(matches!(body, Cow::Borrowed(_)));
        assert!(formatted.is_none());
        assert_eq!(presentation.badge_text(), "reviewer");
    }
}
