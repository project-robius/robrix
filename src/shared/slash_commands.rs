//! Slash commands for the message input, triggered by typing `/` at the start of a message.

use ruma::events::room::message::RoomMessageEventContent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashCommand {
    /// The command name without the leading slash, e.g. `"html"`.
    pub name: &'static str,
    pub description: &'static str,
    pub usage: &'static str,
}

/// The full list of slash commands, in display order
pub static SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "html",
        description: "Send the message as raw HTML",
        usage: "/html <message>",
    },
    SlashCommand {
        name: "plain",
        description: "Send as plain text, without Markdown formatting",
        usage: "/plain <message>",
    },
    // Not implemented yet:
    //
    // SlashCommand {
    //     name: "me",
    //     description: "[Coming soon!] Send an emote",
    //     usage: "/me <action>",
    // },
    // SlashCommand {
    //     name: "shrug",
    //     description: "[Coming soon!] Append a shrug emoticon",
    //     usage: "/shrug <message>",
    // },
    // SlashCommand {
    //     name: "spoiler",
    //     description: "[Coming soon!] Send the message as a spoiler",
    //     usage: "/spoiler <message>",
    // },
];

/// Returns an iterator over all slash commands that start with the given `query`.
pub fn matching_commands(query: &str) -> impl Iterator<Item = &'static SlashCommand> {
    let query = query.to_lowercase();
    SLASH_COMMANDS.iter().filter(move |c| c.name.starts_with(&query))
}

/// Creates and returns the message event content for the given slash command.
pub fn build_message_for_command(text: &str) -> Option<RoomMessageEventContent> {
    let (name, arg) = split_command(text)?;
    match name {
        "html" => Some(RoomMessageEventContent::text_html(html_to_plaintext(arg), arg)),
        "plain" => Some(RoomMessageEventContent::text_plain(arg)),
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
fn html_to_plaintext(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    htmlize::unescape(&out).into_owned()
}
