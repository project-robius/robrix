//! Slash commands for the message input, triggered by typing `/` at the start of a message.
//!
//! A command either produces the message content to send (based on what the user typed)
//! (e.g., `/me`, `/spoiler`), or a [`SlashCommandAction`] for the message input to run
//! within the current room (e.g., `/invite`, `/leave`).

use std::fmt::Write;
use unicode_segmentation::UnicodeSegmentation;
use ruma::{events::room::message::RoomMessageEventContent, matrix_uri::MatrixId, MatrixToUri, MatrixUri, OwnedUserId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashCommand {
    /// The command name without the leading slash, e.g. `"html"`.
    pub name: &'static str,
    /// Alternate names that invoke this same command.
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub usage: &'static str,
}

/// The full list of slash commands, in display order
pub static SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "me",
        aliases: &[],
        description: "Send an emote describing an action",
        usage: "/me <message>",
    },
    SlashCommand {
        name: "notice",
        aliases: &[],
        description: "Send the message as a notice",
        usage: "/notice <message>",
    },
    SlashCommand {
        name: "spoiler",
        aliases: &[],
        description: "Send the message as a hidden spoiler",
        usage: "/spoiler <message>",
    },
    SlashCommand {
        name: "shrug",
        aliases: &[],
        description: r"Prepend ¯\_(ツ)_/¯ to the message",
        usage: "/shrug [<message>]",
    },
    SlashCommand {
        name: "tableflip",
        aliases: &[],
        description: "Prepend (╯°□°）╯︵ ┻━┻ to the message",
        usage: "/tableflip [<message>]",
    },
    SlashCommand {
        name: "unflip",
        aliases: &[],
        description: "Prepend ┬──┬ ノ( ゜-゜ノ) to the message",
        usage: "/unflip [<message>]",
    },
    SlashCommand {
        name: "lenny",
        aliases: &[],
        description: "Prepend ( ͡° ͜ʖ ͡°) to the message",
        usage: "/lenny [<message>]",
    },
    SlashCommand {
        name: "rainbow",
        aliases: &[],
        description: "Send the message in rainbow colors",
        usage: "/rainbow <message>",
    },
    SlashCommand {
        name: "rainbowme",
        aliases: &[],
        description: "Send an emote in rainbow colors",
        usage: "/rainbowme <message>",
    },
    SlashCommand {
        name: "plain",
        aliases: &[],
        description: "Send as plain text, without Markdown or HTML formatting",
        usage: "/plain <message>",
    },
    SlashCommand {
        name: "html",
        aliases: &[],
        description: "Send the message as raw HTML",
        usage: "/html <message>",
    },
    SlashCommand {
        name: "dm",
        aliases: &["direct", "msg", "query"],
        description: "Open a direct message with a user",
        usage: "/dm <user-id>",
    },
    SlashCommand {
        name: "invite",
        aliases: &[],
        description: "Invite a user to this room",
        usage: "/invite <user-id>",
    },
    SlashCommand {
        name: "whois",
        aliases: &["who"],
        description: "Show a user's profile",
        usage: "/whois <user-id>",
    },
    SlashCommand {
        name: "ignore",
        aliases: &["block"],
        description: "Hide/block all messages from a user",
        usage: "/ignore <user-id>",
    },
    SlashCommand {
        name: "unignore",
        aliases: &["unblock"],
        description: "Stop hiding/blocking a user's messages",
        usage: "/unignore <user-id>",
    },
    SlashCommand {
        name: "nick",
        aliases: &[],
        description: "Change your display name in every room",
        usage: "/nick <display_name>",
    },
    SlashCommand {
        name: "leave",
        aliases: &["part"],
        description: "Leave this room",
        usage: "/leave",
    },

    // TODO: add more of the commands below, most of which need backend matrix requests:
    //
    // * /knock <room-address> [reason]: `MatrixRequest::Knock` already takes an alias,
    //   so this mostly needs `add_room::parse_address` made public.
    // * /kick, /ban, /unban <user-id> [reason]: one new membership-change `MatrixRequest`.
    // * /topic [<topic>], /roomname <name>: one new state-event `MatrixRequest`.
    // * /react <emoji>, /redact [reason]: need to find the latest matching timeline item,
    //   like `MessageAction::EditLatest` does.
    // * /help: show this list in the timeline.
    // * /op <user-id> [<power-level>], /deop <user-id>: power level writes, plus
    //   un-commenting the `UserPowerLevels::RoomPowerLevels` bit to gate them.
    // * /myroomnick, /myroomavatar, /roomavatar, /myavatar: read-modify-write of our own
    //   `m.room.member`, and the avatar ones also want a file picker.
    // * /join and /goto <room-address>: we can't resolve an alias to a room ID yet.
    //   See the `MatrixId::RoomAlias` TODO in `room_screen.rs`.
    // * /upgraderoom, /converttodm, /converttoroom, /jumptodate, /devtools, /discardsession.
    // * No counterpart in Robrix at all yet: /verify (we only do interactive SAS, not
    //   manual fingerprints), /addwidget, /rageshake, /status, /holdcall.
    // * Element's chat effects: /confetti, /fireworks, /rainfall, /snowfall, /hearts.
];

/// What the message input should do with the text that the user entered.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum SlashCommandOutcome {
    /// Send this message.
    Message(RoomMessageEventContent),
    /// Run this action within the current room instead of sending a message.
    Action(SlashCommandAction),
    /// Show an error to the user, don't send or do anything.
    Error(String),
}

/// Actions that a slash command can ask the current room to do.
///
/// "Current" room just refers to the room that this command was typed within.
#[derive(Debug, Clone)]
pub enum SlashCommandAction {
    /// Leave the current room.
    LeaveRoom,
    /// Invite the given user to the current room.
    InviteUser(OwnedUserId),
    /// Open a direct message room (or offer to create a new one) with the given user.
    OpenDirectMessage(OwnedUserId),
    /// Show the given user's profile pane.
    ShowUserProfile(OwnedUserId),
    /// Ignore (`true`) or unignore (`false`) the given user.
    IgnoreUser {
        user_id: OwnedUserId,
        ignore: bool,
    },
    /// Set the current user's display name across all rooms.
    SetDisplayName(String),
}

/// Returns an iterator over all slash commands matching the given query.
///
/// Matches against both the slash command name and any of its aliases,
/// listing the commands matched by name first.
pub fn matching_commands(query: &str) -> impl Iterator<Item = &'static SlashCommand> {
    let query = query.to_lowercase();
    let (by_name, by_alias): (Vec<_>, Vec<_>) = SLASH_COMMANDS.iter()
        .filter(|c| c.name.starts_with(&query) || c.aliases.iter().any(|a| a.starts_with(&query)))
        .partition(|c| c.name.starts_with(&query));
    by_name.into_iter().chain(by_alias)
}

/// Turns the text the user entered into a message to send, an action to run, or an error.
///
/// Text that isn't a command comes back as a regular Markdown message, so callers only
/// need to handle the three outcomes rather than sniffing for a leading slash themselves.
pub fn parse_input(text: &str) -> SlashCommandOutcome {
    // A doubled slash escapes the command, so "//foo" sends the literal text "/foo".
    if text.starts_with("//") {
        return SlashCommandOutcome::Message(RoomMessageEventContent::text_markdown(&text[1..]));
    }
    let Some((name, arg)) = split_command(text) else {
        return SlashCommandOutcome::Message(RoomMessageEventContent::text_markdown(text));
    };
    let Some(command) = find_command(name) else {
        return SlashCommandOutcome::Error(format!(
            "Unknown command \"/{name}\". Begin your message with \"//\" to send a literal slash."
        ));
    };

    // Only the emoticons and /leave still make sense with nothing after them.
    let arg = arg.trim();
    if arg.is_empty() && !matches!(
        command.name,
        "shrug" | "tableflip" | "unflip" | "lenny" | "leave",
    ) {
        return SlashCommandOutcome::Error(format!("Usage: {}", command.usage));
    }

    // Commands focused on a single user all take one argument, so parse it once.
    if let "dm" | "invite" | "whois" | "ignore" | "unignore" = command.name {
        let Some(user_id) = parse_user_id(arg) else {
            return SlashCommandOutcome::Error(format!(
                "\"{arg}\" isn't a valid user ID; it should look like @user:server.org"
            ));
        };
        return SlashCommandOutcome::Action(match command.name {
            "dm" => SlashCommandAction::OpenDirectMessage(user_id),
            "invite" => SlashCommandAction::InviteUser(user_id),
            "whois" => SlashCommandAction::ShowUserProfile(user_id),
            "ignore" => SlashCommandAction::IgnoreUser { user_id, ignore: true },
            _ => SlashCommandAction::IgnoreUser { user_id, ignore: false },
        });
    }

    let content = match command.name {
        "me" => RoomMessageEventContent::emote_markdown(arg),
        "notice" => RoomMessageEventContent::notice_markdown(arg),
        "plain" => RoomMessageEventContent::text_plain(arg),
        "html" => RoomMessageEventContent::text_html(html_to_plaintext(arg), arg),
        "spoiler" => RoomMessageEventContent::text_html(
            arg,
            format!("<span data-mx-spoiler>{}</span>", htmlize::escape_text(arg)),
        ),
        "rainbow" | "rainbowme" if arg.chars().count() > RAINBOW_MAX_CHARS => {
            return SlashCommandOutcome::Error(format!(
                "Your message is too long to rainbow; the limit is {RAINBOW_MAX_CHARS} characters."
            ));
        }
        "rainbow" => RoomMessageEventContent::text_html(arg, rainbow_html(arg)),
        "rainbowme" => RoomMessageEventContent::emote_html(arg, rainbow_html(arg)),
        "shrug" | "tableflip" | "unflip" | "lenny" => {
            let emoticon = match command.name {
                "shrug" => r"¯\_(ツ)_/¯",
                "tableflip" => "(╯°□°）╯︵ ┻━┻",
                "unflip" => "┬──┬ ノ( ゜-゜ノ)",
                _ => "( ͡° ͜ʖ ͡°)",
            };
            RoomMessageEventContent::text_plain(match arg.is_empty() {
                true => emoticon.to_owned(),
                false => format!("{emoticon} {arg}"),
            })
        }
        "nick" => return SlashCommandOutcome::Action(SlashCommandAction::SetDisplayName(arg.to_owned())),
        "leave" => return SlashCommandOutcome::Action(SlashCommandAction::LeaveRoom),
        _ => return SlashCommandOutcome::Error(format!("Usage: {}", command.usage)),
    };
    SlashCommandOutcome::Message(content)
}

/// Returns the command that `name` invokes, matching aliases and ignoring case.
///
/// A real name always wins over another command's alias.
fn find_command(name: &str) -> Option<&'static SlashCommand> {
    SLASH_COMMANDS.iter()
        .find(|c| c.name.eq_ignore_ascii_case(name))
        .or_else(|| SLASH_COMMANDS.iter().find(|c|
            c.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
        ))
}

/// Parses a user ID from a bare `@user:server.org`, from a pasted matrix.to link, or from
/// the Markdown link that this input's own `@` autocomplete inserts.
fn parse_user_id(arg: &str) -> Option<OwnedUserId> {
    if let Ok(user_id) = UserId::parse(arg) {
        return Some(user_id);
    }
    // Unwrap `[Alice](https://matrix.to/#/@alice:server.org)` down to just the link target.
    let uri = arg.strip_suffix(')')
        .and_then(|inner| inner.rfind("](").map(|i| &inner[i + 2..]))
        .unwrap_or(arg);
    let matrix_id = MatrixToUri::parse(uri)
        .map(|u| u.id().clone())
        .or_else(|_| MatrixUri::parse(uri).map(|u| u.id().clone()))
        .ok()?;
    match matrix_id {
        MatrixId::User(user_id) => Some(user_id),
        _ => None,
    }
}

fn split_command(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix('/')?;
    Some(match rest.split_once(char::is_whitespace) {
        Some((name, arg)) => (name, arg),
        None => (rest, ""),
    })
}

/// Builds a plaintext fallback from HTML for non-HTML clients: drops tags and
/// decodes entities (so `&amp;` shows as `&`).
///
/// This is what notifications and room previews show, so tags that break a line
/// become newlines, and markup that becomes nothing just uses the raw HTML as fallback.
fn html_to_plaintext(html: &str) -> String {
    /// Tags we turn into a line break, so "a<br/>b" doesn't come out as "ab".
    const BREAKING_TAGS: &[&str] = &["br", "p", "li", "div", "tr", "blockquote", "h1", "h2", "h3"];

    /// Advances the iterator through the corresponding closing tag.
    fn skip_raw_text(chars: &mut impl Iterator<Item = char>, tag_name: &str) {
        let close: Vec<char> = format!("</{tag_name}").chars().collect();
        let mut matched = 0;
        for ch in chars.by_ref() {
            let ch = ch.to_ascii_lowercase();
            if ch == close[matched] {
                matched += 1;
                if matched == close.len() {
                    break;
                }
            } else {
                matched = usize::from(ch == close[0]);
            }
        }
        for ch in chars {
            if ch == '>' {
                break;
            }
        }
    }

    let mut out = String::with_capacity(html.len());
    let mut tag_name = String::new();
    let mut in_tag = false;
    let mut in_name = false;
    let mut is_close_tag = false;
    // Set once the name reads as `<!--`, since a bare '>' doesn't end a comment.
    let mut in_comment = false;
    let mut prev = ['\0', '\0'];
    // Set while we're inside a quoted attribute value, where a '>' doesn't end the tag.
    let mut quote: Option<char> = None;
    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        if !in_tag {
            // Only start a tag if a tag name follows, so "i <3 you" keeps its bracket.
            let starts_tag = c == '<' && chars.peek()
                .is_some_and(|&n| n.is_ascii_alphabetic() || matches!(n, '/' | '!' | '?'));
            if starts_tag {
                in_tag = true;
                in_name = true;
                is_close_tag = chars.peek() == Some(&'/');
                in_comment = false;
                prev = ['\0', '\0'];
                tag_name.clear();
            } else {
                out.push(c);
            }
            continue;
        }
        // A comment runs all the way to "-->".
        if in_comment {
            if c == '>' && prev == ['-', '-'] {
                in_tag = false;
            }
            prev = [prev[1], c];
            continue;
        }
        match c {
            '"' | '\'' if quote.is_none() => quote = Some(c),
            _ if quote == Some(c) => quote = None,
            '>' if quote.is_none() => {
                in_tag = false;
                if BREAKING_TAGS.contains(&tag_name.as_str()) && !out.ends_with('\n') {
                    out.push('\n');
                }
                // Exclude javascript or CSS content, which is code, not text
                if !is_close_tag && matches!(tag_name.as_str(), "script" | "style") {
                    skip_raw_text(&mut chars, &tag_name);
                }
            }
            // The name runs from just after the '<' (or '</') up to the first space.
            _ if quote.is_none() && in_name => {
                if c.is_whitespace() {
                    in_name = false;
                } else if c != '/' {
                    tag_name.push(c.to_ascii_lowercase());
                    in_comment = tag_name == "!--";
                }
            }
            _ => {}
        }
    }

    let text = htmlize::unescape(out.trim()).into_owned();
    // All markup and no text (e.g. a lone <img>) would leave nothing at all to show.
    match text.is_empty() {
        true => html.to_owned(),
        false => text,
    }
}

/// Generating rainbow-formatted text is heavy (37 bytes per char), so we impose a limit
/// such that the homeserver doesn't reject it for being over the 64 KiB event size limit.
const RAINBOW_MAX_CHARS: usize = 1500;

fn rainbow_html(text: &str) -> String {
    // This is borrowed from Element's CIELAB behavior
    fn adjust_xyz(v: f64) -> f64 {
        if v > 0.2069 { v.powi(3) } else { 0.1284 * v - 0.01771 }
    }
    fn adjust_rgb(v: f64) -> u8 {
        let gamma_corrected = if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (gamma_corrected.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    const LIGHTNESS: f64 = 75.0;
    const CHROMA: f64 = 127.0;
    // Apply the color on a per-grapheme basis, not chars or bytes.
    let frequency = 2.0 * std::f64::consts::PI / text.graphemes(true).count().max(1) as f64;
    let y_lightness = (LIGHTNESS + 16.0) / 116.0;
    let y = adjust_xyz(y_lightness);

    let mut out = String::with_capacity(text.len() * 40);
    for (i, grapheme) in text.graphemes(true).enumerate() {
        // Don't apply color to whitespace, since there's nothing to see in a
        // colored tab or newline, just wasted bytes.
        if grapheme.chars().all(char::is_whitespace) {
            out.push_str(grapheme);
            continue;
        }
        let hue = i as f64 * frequency;
        let x = adjust_xyz(y_lightness + CHROMA * hue.cos() / 500.0) * 0.9505;
        let z = adjust_xyz(y_lightness - CHROMA * hue.sin() / 200.0) * 1.089;
        let red   = adjust_rgb( 3.24096994 * x - 1.53738318 * y - 0.49861076 * z);
        let green = adjust_rgb(-0.96924364 * x + 1.87596750 * y + 0.04155506 * z);
        let blue  = adjust_rgb( 0.05563008 * x - 0.20397696 * y + 1.05697151 * z);
        let _ = write!(out, "<span data-mx-color=\"#{red:02x}{green:02x}{blue:02x}\">");
        out.push_str(&htmlize::escape_text(grapheme));
        out.push_str("</span>");
    }
    out
}


#[cfg(test)]
mod tests_slash_commands {
    use super::*;
    use ruma::events::room::message::MessageType;

    fn message(text: &str) -> RoomMessageEventContent {
        match parse_input(text) {
            SlashCommandOutcome::Message(content) => content,
            other => panic!("expected a message for {text:?}, got {other:?}"),
        }
    }

    fn action(text: &str) -> SlashCommandAction {
        match parse_input(text) {
            SlashCommandOutcome::Action(action) => action,
            other => panic!("expected an action for {text:?}, got {other:?}"),
        }
    }

    fn error(text: &str) -> String {
        match parse_input(text) {
            SlashCommandOutcome::Error(err) => err,
            other => panic!("expected an error for {text:?}, got {other:?}"),
        }
    }

    /// Returns the `formatted_body` of a message, panicking if it has none.
    fn formatted(content: &RoomMessageEventContent) -> &str {
        let formatted = match &content.msgtype {
            MessageType::Text(m) => m.formatted.as_ref(),
            MessageType::Emote(m) => m.formatted.as_ref(),
            MessageType::Notice(m) => m.formatted.as_ref(),
            other => panic!("unexpected msgtype {other:?}"),
        };
        &formatted.expect("expected a formatted body").body
    }

    fn is_unformatted(content: &RoomMessageEventContent) -> bool {
        match &content.msgtype {
            MessageType::Text(m) => m.formatted.is_none(),
            MessageType::Emote(m) => m.formatted.is_none(),
            MessageType::Notice(m) => m.formatted.is_none(),
            other => panic!("unexpected msgtype {other:?}"),
        }
    }

    #[test]
    fn plain_text_is_not_a_command() {
        let content = message("hello /not a command");
        assert_eq!(content.body(), "hello /not a command");
        assert!(matches!(content.msgtype, MessageType::Text(_)));
    }

    #[test]
    fn double_slash_escapes_the_command() {
        assert_eq!(message("//me waves").body(), "/me waves");
        assert_eq!(message("//").body(), "/");
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(error("/mee hello").contains("/mee"));
        assert!(error("/usr/bin/foo is broken").contains("/usr/bin/foo"));
        // A bare slash has no command name at all.
        assert!(error("/").contains("Unknown command"));
    }

    #[test]
    fn command_names_are_case_insensitive() {
        assert!(matches!(message("/HTML <b>hi</b>").msgtype, MessageType::Text(_)));
        assert!(matches!(message("/Me waves").msgtype, MessageType::Emote(_)));
    }

    #[test]
    fn aliases_resolve_to_the_same_command() {
        assert!(matches!(action("/part"), SlashCommandAction::LeaveRoom));
        assert!(matches!(action("/msg @bob:server.org"), SlashCommandAction::OpenDirectMessage(_)));
        assert!(matches!(action("/query @bob:server.org"), SlashCommandAction::OpenDirectMessage(_)));
        assert!(matches!(
            action("/block @bob:server.org"),
            SlashCommandAction::IgnoreUser { ignore: true, .. },
        ));
        assert!(matches!(
            action("/unblock @bob:server.org"),
            SlashCommandAction::IgnoreUser { ignore: false, .. },
        ));
    }

    /// A duplicate would be shadowed by whichever command comes first in the table.
    #[test]
    fn no_name_or_alias_is_used_twice() {
        let mut seen = std::collections::HashSet::new();
        for command in SLASH_COMMANDS {
            for name in std::iter::once(&command.name).chain(command.aliases) {
                assert!(seen.insert(*name), "\"{name}\" is used by more than one command");
                assert!(!name.starts_with('/'), "\"{name}\" should not include the leading slash");
                assert_eq!(*name, name.to_lowercase(), "\"{name}\" should be lowercase");
            }
            assert!(
                command.usage.starts_with(&format!("/{}", command.name)),
                "usage {:?} doesn't match the name {:?}", command.usage, command.name,
            );
        }
    }

    #[test]
    fn matching_commands_finds_names_and_aliases() {
        let names = |query| matching_commands(query).map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(names("rainbow"), vec!["rainbow", "rainbowme"]);
        // "part" is only an alias of "leave", so it still surfaces the leave command.
        assert_eq!(names("par"), vec!["leave"]);
        assert!(names("").len() == SLASH_COMMANDS.len());
        assert!(names("zzz").is_empty());
    }

    #[test]
    fn matching_commands_lists_name_matches_before_alias_matches() {
        for query in ["", "b", "d", "l", "m", "p", "q", "u", "un", "w"] {
            let matched: Vec<_> = matching_commands(query).collect();
            let first_alias_only = matched.iter().position(|c| !c.name.starts_with(query));
            let last_by_name = matched.iter().rposition(|c| c.name.starts_with(query));
            if let (Some(alias), Some(name)) = (first_alias_only, last_by_name) {
                assert!(alias > name, "an alias match preceded a name match for {query:?}");
            }
            // A command matching by both its name and an alias is still listed once.
            let unique: std::collections::HashSet<_> = matched.iter().map(|c| c.name).collect();
            assert_eq!(unique.len(), matched.len(), "duplicate entries for {query:?}");
        }
        // "who" hits the `whois` name and its own `who` alias.
        assert_eq!(matching_commands("who").count(), 1);
    }

    #[test]
    fn missing_argument_reports_usage() {
        assert_eq!(error("/me"), "Usage: /me <message>");
        assert_eq!(error("/me   "), "Usage: /me <message>");
        assert_eq!(error("/invite"), "Usage: /invite <user-id>");
        assert_eq!(error("/nick"), "Usage: /nick <display_name>");
    }

    #[test]
    fn emoticons_stand_alone_without_an_argument() {
        assert_eq!(message("/shrug").body(), r"¯\_(ツ)_/¯");
        assert_eq!(message("/tableflip").body(), "(╯°□°）╯︵ ┻━┻");
        assert_eq!(message("/unflip").body(), "┬──┬ ノ( ゜-゜ノ)");
        assert_eq!(message("/lenny").body(), "( ͡° ͜ʖ ͡°)");
        assert!(matches!(parse_input("/leave"), SlashCommandOutcome::Action(_)));
    }

    #[test]
    fn emoticons_are_prepended_as_plain_text() {
        let content = message("/shrug this is a test message");
        assert_eq!(content.body(), r"¯\_(ツ)_/¯ this is a test message");
        // Element sends these unformatted, so Markdown in the rest stays literal.
        assert!(is_unformatted(&content));
        assert_eq!(message("/shrug **bold**").body(), r"¯\_(ツ)_/¯ **bold**");
    }

    #[test]
    fn me_and_notice_use_their_own_msgtypes() {
        let emote = message("/me waves *enthusiastically*");
        assert!(matches!(emote.msgtype, MessageType::Emote(_)));
        assert_eq!(emote.body(), "waves *enthusiastically*");
        assert_eq!(formatted(&emote), "waves <em>enthusiastically</em>");

        let notice = message("/notice heads up");
        assert!(matches!(notice.msgtype, MessageType::Notice(_)));
        assert_eq!(notice.body(), "heads up");
    }

    #[test]
    fn plain_keeps_markdown_literal_and_html_keeps_tags() {
        let plain = message("/plain **not bold**");
        assert_eq!(plain.body(), "**not bold**");
        assert!(is_unformatted(&plain));

        let html = message("/html <b>hi &amp; bye</b>");
        assert_eq!(formatted(&html), "<b>hi &amp; bye</b>");
        assert_eq!(html.body(), "hi & bye");
    }

    #[test]
    fn spoiler_wraps_and_escapes_the_message() {
        let content = message("/spoiler the butler <did> it");
        assert_eq!(content.body(), "the butler <did> it");
        assert_eq!(
            formatted(&content),
            "<span data-mx-spoiler>the butler &lt;did&gt; it</span>",
        );
    }

    /// Checks our CIELAB walk against Element's own committed snapshot.
    #[test]
    fn rainbow_matches_element_colors() {
        const EXPECTED: &str = concat!(
            r##"<span data-mx-color="#ff00be">t</span><span data-mx-color="#ff0080">h</span>"##,
            r##"<span data-mx-color="#ff0041">i</span><span data-mx-color="#ff5f00">s</span> "##,
            r##"<span data-mx-color="#faa900">i</span><span data-mx-color="#c3bf00">s</span> "##,
            r##"<span data-mx-color="#00d800">a</span> "##,
            r##"<span data-mx-color="#00e371">t</span><span data-mx-color="#00e6b6">e</span>"##,
            r##"<span data-mx-color="#00e7f8">s</span><span data-mx-color="#00e7ff">t</span> "##,
            r##"<span data-mx-color="#00deff">m</span><span data-mx-color="#00d2ff">e</span>"##,
            r##"<span data-mx-color="#00c0ff">s</span><span data-mx-color="#44a4ff">s</span>"##,
            r##"<span data-mx-color="#e87dff">a</span><span data-mx-color="#ff42ff">g</span>"##,
            r##"<span data-mx-color="#ff00fe">e</span>"##,
        );
        let content = message("/rainbow this is a test message");
        assert_eq!(content.body(), "this is a test message");
        assert_eq!(formatted(&content), EXPECTED);
        assert!(matches!(message("/rainbowme waves").msgtype, MessageType::Emote(_)));
    }

    #[test]
    fn rainbow_leaves_every_kind_of_whitespace_uncolored() {
        let content = message("/rainbow a\tb\u{00A0}c\nd");
        let body = formatted(&content);
        // Only the four letters get a span; the tab, NBSP and newline pass through as-is.
        assert_eq!(body.matches("<span").count(), 4);
        for ws in ["\t", "\u{00A0}", "\n"] {
            assert!(body.contains(&format!("</span>{ws}<span")), "{ws:?} was colored in {body:?}");
        }
    }

    #[test]
    fn rainbow_escapes_html_in_the_message() {
        let content = message("/rainbow <&>");
        let formatted_body = formatted(&content);
        assert!(formatted_body.contains("&lt;"));
        assert!(formatted_body.contains("&amp;"));
        assert!(formatted_body.contains("&gt;"));
    }

    #[test]
    fn user_commands_require_a_full_user_id() {
        assert!(error("/invite bob").contains("@user:server.org"));
        assert!(error("/ignore @bob").contains("@user:server.org"));

        let user_id = UserId::parse("@bob:server.org").unwrap();
        assert!(matches!(action("/invite @bob:server.org"), SlashCommandAction::InviteUser(u) if u == user_id));
        assert!(matches!(action("/whois @bob:server.org"), SlashCommandAction::ShowUserProfile(u) if u == user_id));
        assert!(matches!(
            action("/ignore @bob:server.org"),
            SlashCommandAction::IgnoreUser { ignore: true, .. },
        ));
        assert!(matches!(
            action("/unignore @bob:server.org"),
            SlashCommandAction::IgnoreUser { ignore: false, .. },
        ));
    }

    /// The '@' popup in the same text input rewrites `@bob` into a Markdown link,
    /// so the user-targeted commands have to accept that form too.
    #[test]
    fn user_commands_accept_an_inserted_mention_pill() {
        let user_id = UserId::parse("@bob:server.org").unwrap();
        let from_pill = action("/invite [Bob Smith](https://matrix.to/#/@bob:server.org)");
        assert!(matches!(from_pill, SlashCommandAction::InviteUser(u) if u == user_id));
        // A bare permalink, e.g. pasted in, works too.
        let from_link = action("/dm https://matrix.to/#/@bob:server.org");
        assert!(matches!(from_link, SlashCommandAction::OpenDirectMessage(u) if u == user_id));
        // A link to something that isn't a user is still rejected.
        assert!(error("/dm [A Room](https://matrix.to/#/%23room:server.org)").contains("user ID"));
    }

    #[test]
    fn html_fallback_strips_comments_and_raw_text() {
        // A '>' inside the comment must not end it early.
        assert_eq!(message("/html <!-- c > d -->visible").body(), "visible");
        assert_eq!(message("/html <!-- secret -->ok").body(), "ok");
        // Script/style hold code, not body text.
        assert_eq!(message("/html <script>alert(1)</script>hi").body(), "hi");
        assert_eq!(message("/html <style>a{b:c}</style>hi").body(), "hi");
        assert_eq!(message("/html a<script>x</script>b").body(), "ab");
    }

    #[test]
    fn rainbow_rejects_a_message_past_the_event_limit() {
        let too_long = "a".repeat(RAINBOW_MAX_CHARS + 1);
        assert!(error(&format!("/rainbow {too_long}")).contains("too long to rainbow"));
        assert!(error(&format!("/rainbowme {too_long}")).contains("too long to rainbow"));
        let at_limit = "a".repeat(RAINBOW_MAX_CHARS);
        assert_eq!(message(&format!("/rainbow {at_limit}")).body(), at_limit);
    }

    #[test]
    fn html_fallback_keeps_bare_angle_brackets() {
        assert_eq!(message("/html i <3 you").body(), "i <3 you");
        assert_eq!(message("/html 5 > 3").body(), "5 > 3");
        assert_eq!(message("/html <b>bold</b> 5 > 3").body(), "bold 5 > 3");
    }

    #[test]
    fn rainbow_keeps_grapheme_clusters_whole() {
        // A ZWJ family emoji must land in one span, not five.
        let content = message("/rainbow \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}");
        assert_eq!(formatted(&content).matches("<span").count(), 1);
        // A base letter plus a combining accent is likewise a single cluster.
        let accented = message("/rainbow e\u{0301}");
        assert_eq!(formatted(&accented).matches("<span").count(), 1);
    }

    /// A grapheme cluster like `<` plus a combining mark isn't equal to `"<"`, so escaping
    /// by whole-cluster comparison used to let a raw bracket through.
    #[test]
    fn rainbow_escapes_decorated_brackets() {
        for decorated in ["<\u{301}", "&\u{301}", ">\u{FE0F}"] {
            let content = message(&format!("/rainbow {decorated}"));
            let body = formatted(&content);
            let inner = body.rsplit_once('>').map(|(_, t)| t).unwrap_or(body);
            assert!(
                !inner.starts_with('<') && !inner.starts_with('&'),
                "unescaped {decorated:?} leaked into {body:?}",
            );
        }
    }

    #[test]
    fn html_fallback_survives_attributes_and_block_tags() {
        // A '>' inside a quoted attribute must not end the tag early.
        assert_eq!(message(r#"/html <a href="x" title="5 > 3">hi</a>"#).body(), "hi");
        // Line-breaking tags become newlines instead of running words together.
        assert_eq!(message("/html a<br/>b").body(), "a\nb");
        assert_eq!(message("/html <p>one</p><p>two</p>").body(), "one\ntwo");
        // Markup with no text at all falls back to the raw HTML, never an empty body.
        assert_eq!(
            message(r#"/html <img src="mxc://s/i">"#).body(),
            r#"<img src="mxc://s/i">"#,
        );
    }

    /// Display names are arbitrary, so the pill has to be split at its last `](`.
    #[test]
    fn user_commands_accept_a_pill_whose_label_has_brackets() {
        let user_id = UserId::parse("@bob:server.org").unwrap();
        let pill = "/invite [a](b](https://matrix.to/#/@bob:server.org)";
        assert!(matches!(action(pill), SlashCommandAction::InviteUser(u) if u == user_id));
    }

    #[test]
    fn nick_takes_the_rest_of_the_line() {
        assert!(matches!(
            action("/nick  Bob the Builder  "),
            SlashCommandAction::SetDisplayName(name) if name == "Bob the Builder",
        ));
    }
}
