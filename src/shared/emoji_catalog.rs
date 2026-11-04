//! The `EmojiCatalog` of every emoji that users can search for and pick in Robrix.
//!
//! That's every emoji in the `emojis` crate that our emoji font can draw, plus Unicode CLDR's
//! search keywords for each one (e.g., "lol" for 😂) and Discord's names for it (e.g., "slight_smile" for 🙂).

use std::{borrow::Cow, ops::Range, sync::LazyLock};
use emojis::{Emoji, EmojiVersion, Group, SkinTone};
use rustc_hash::FxHashMap;

/// The newest emoji version that makepad's bundled Noto Color Emoji font can draw.
const NEWEST_DRAWABLE_EMOJI_VERSION: EmojiVersion = EmojiVersion::new(16, 0);

/// One line per emoji: the emoji, its keywords, then its Discord names, separated by tabs.
/// The keywords and the names are separated by `|`, and skin tone variants only have names.
const KEYWORDS_TSV: &str = include_str!("emoji_keywords.tsv");

static CATALOG: LazyLock<EmojiCatalog> = LazyLock::new(EmojiCatalog::build);

/// An emoji that we offer, along with the text that searches look through to find it.
pub struct CatalogEmoji {
    pub emoji: &'static Emoji,
    name_lowercase: Cow<'static, str>,
    /// The emoji's lowercase name followed by its shortcodes, separated by spaces.
    name_and_shortcodes: String,
    /// CLDR's keywords and Discord's names for this emoji, separated by `|`.
    keywords: String,
    /// Discord's names for this emoji, separated by `|`.
    discord_names: &'static str,
    /// `discord_names` without its underscores, since Discord's search ignores them.
    squished_discord_names: String,
}

/// Every emoji that we offer, grouped into the categories that the emoji picker shows.
pub struct EmojiCatalog {
    /// In Unicode's order, which keeps each group's emoji together.
    pub emojis: Vec<CatalogEmoji>,
    /// The range of `emojis` in each group, in the order that the groups are shown.
    pub groups: Vec<(Group, Range<usize>)>,
    /// The emoji for each of Discord's names, including the names of skin tone variants.
    emojis_by_discord_name: FxHashMap<&'static str, &'static Emoji>,
}

impl EmojiCatalog {
    fn build() -> Self {
        let rows: FxHashMap<&'static str, (&'static str, &'static str)> = KEYWORDS_TSV.lines()
            // The space matters, since the keycap emoji #️⃣ also starts with a '#'.
            .filter(|line| !line.starts_with("# "))
            .filter_map(|line| {
                let (emoji, columns) = line.split_once('\t')?;
                Some((emoji, columns.split_once('\t')?))
            })
            .collect();
        let emojis_by_discord_name = rows.iter()
            .filter_map(|(emoji, (_, discord_names))| Some((emojis::get(emoji)?, *discord_names)))
            .flat_map(|(emoji, discord_names)| discord_names.split('|').map(move |name| (name, emoji)))
            .collect();
        let emojis: Vec<CatalogEmoji> = emojis::iter()
            .filter(|emoji| can_draw(emoji))
            .map(|emoji| {
                let name_lowercase = if emoji.name().chars().any(char::is_uppercase) {
                    Cow::Owned(emoji.name().to_lowercase())
                } else {
                    Cow::Borrowed(emoji.name())
                };
                let name_and_shortcodes = std::iter::once(&*name_lowercase)
                    .chain(emoji.shortcodes())
                    .collect::<Vec<_>>()
                    .join(" ");
                let (keywords, discord_names) = rows.get(emoji.as_str()).copied().unwrap_or_default();
                CatalogEmoji {
                    emoji,
                    name_lowercase,
                    name_and_shortcodes,
                    keywords: format!("{keywords}|{discord_names}"),
                    discord_names,
                    squished_discord_names: discord_names.replace('_', ""),
                }
            })
            .collect();

        let mut groups: Vec<(Group, Range<usize>)> = Vec::new();
        for (index, entry) in emojis.iter().enumerate() {
            match groups.last_mut() {
                Some((group, range)) if *group == entry.emoji.group() => range.end = index + 1,
                _ => groups.push((entry.emoji.group(), index .. index + 1)),
            }
        }
        Self { emojis, groups, emojis_by_discord_name }
    }

    /// Returns the emoji with the given shortcode, which can be any of Discord's names for it.
    /// Discord wins for the few names that the `emojis` crate uses for a different emoji.
    pub fn get_by_shortcode(&self, shortcode: &str) -> Option<&'static Emoji> {
        self.emojis_by_discord_name.get(shortcode).copied()
            .or_else(|| emojis::get_by_shortcode(shortcode))
    }

    /// Returns the emoji that match the given search `query` (which may be a `:shortcode:`), best matches first.
    /// Equally good matches are sorted by `pick_counts` (how often the user picked each one), then by Unicode's order.
    pub fn search(
        &'static self,
        query: &str,
        pick_counts: &FxHashMap<&'static str, u64>,
    ) -> Vec<&'static CatalogEmoji> {
        let query = query.trim().trim_matches(':').to_lowercase();
        // Like Discord, we ignore spaces and underscores here, and `|` is what separates Discord's names.
        let squished_query = query.replace([' ', '_', '|'], "");
        if squished_query.is_empty() {
            return Vec::new();
        }
        let query_words: Vec<&str> = split_into_words(&query).collect();
        // Shortcodes use underscores where names use spaces, e.g., "thumbs_up" vs "thumbs up".
        let spaced_query = query.split(|c: char| c == '_' || c.is_whitespace())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let underscored_query = spaced_query.replace(' ', "_");

        let mut matches: Vec<(EmojiMatch, u64, usize)> = self.emojis.iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let quality = entry.match_query(&spaced_query, &underscored_query, &squished_query, &query_words)?;
                let pick_count = pick_counts.get(entry.emoji.as_str()).copied().unwrap_or(0);
                Some((quality, pick_count, index))
            })
            .collect();
        matches.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        matches.into_iter().map(|(_, _, index)| &self.emojis[index]).collect()
    }
}

impl CatalogEmoji {
    fn match_query(
        &self,
        spaced_query: &str,
        underscored_query: &str,
        squished_query: &str,
        query_words: &[&str],
    ) -> Option<EmojiMatch> {
        // Discord's search finds any part of its names, which we fall back to when nothing better matches.
        let discord_match = self.squished_discord_names.contains(squished_query).then_some(EmojiMatch::DiscordNamePart);
        // Most emoji don't contain the query words anywhere, which is quick to rule out.
        if query_words.is_empty()
            || !query_words.iter().all(|query_word| self.name_and_shortcodes.contains(query_word) || self.keywords.contains(query_word))
        {
            return discord_match;
        }
        if self.discord_names.split('|').any(|name| name == underscored_query) {
            return Some(EmojiMatch::DiscordName);
        }
        if *self.name_lowercase == *spaced_query
            || self.emoji.shortcodes().any(|shortcode| shortcode == underscored_query)
        {
            return Some(EmojiMatch::Exact);
        }
        let name_words = || split_into_words(&self.name_and_shortcodes);
        let keyword_words = || self.keywords.split('|').flat_map(split_into_words);
        // The emoji is only as good of a match as its worst-matching query word.
        query_words.iter()
            .map(|query_word| {
                if name_words().any(|word| word == *query_word) {
                    Some(EmojiMatch::NameWords)
                } else if keyword_words().any(|word| word == *query_word) {
                    Some(EmojiMatch::KeywordWords)
                } else if name_words().any(|word| word.starts_with(query_word)) {
                    Some(EmojiMatch::NamePrefixes)
                } else if keyword_words().any(|word| word.starts_with(query_word)) {
                    Some(EmojiMatch::KeywordPrefixes)
                } else {
                    None
                }
            })
            .try_fold(EmojiMatch::NameWords, |worst, quality| Some(worst.max(quality?)))
            .or(discord_match)
    }
}

/// How well an emoji matches a search query, from best to worst.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EmojiMatch {
    /// The query is one of Discord's names for the emoji, so Discord users get the emoji that they expect.
    DiscordName,
    /// The query is the emoji's whole name or one of its shortcodes.
    Exact,
    /// Each query word is a whole word of the emoji's name or shortcodes.
    NameWords,
    /// Each query word is a whole word of the emoji's name, shortcodes, or keywords.
    KeywordWords,
    /// Each query word starts a word of the emoji's name or shortcodes.
    NamePrefixes,
    /// Each query word starts a word of the emoji's name, shortcodes, or keywords.
    KeywordPrefixes,
    /// The query is part of one of Discord's names for the emoji, ignoring spaces and underscores.
    DiscordNamePart,
}

/// Returns the catalog of every emoji that we offer, which is built upon first use.
pub fn get_emoji_catalog() -> &'static EmojiCatalog {
    &CATALOG
}

/// Returns whether our emoji font can draw the given emoji.
pub fn can_draw(emoji: &Emoji) -> bool {
    emoji.emoji_version() <= NEWEST_DRAWABLE_EMOJI_VERSION
}

/// Returns the version of `emoji` with the given skin tone,
/// or `emoji` itself if it doesn't have one that we can draw.
pub fn with_skin_tone(emoji: &'static Emoji, skin_tone: SkinTone) -> &'static Emoji {
    emoji.with_skin_tone(skin_tone)
        .filter(|toned| can_draw(toned))
        .unwrap_or(emoji)
}

/// Returns the given emoji with its default skin tone, which is the one that the catalog lists.
pub fn without_skin_tone(emoji: &'static Emoji) -> &'static Emoji {
    emoji.with_skin_tone(SkinTone::Default).unwrap_or(emoji)
}

/// Returns the main shortcode of the given emoji, such as "+1" for 👍.
///
/// Skin tone variants don't have their own shortcode, so they get the default one's.
pub fn get_shortcode(emoji: &'static Emoji) -> Option<&'static str> {
    emoji.shortcode().or_else(|| without_skin_tone(emoji).shortcode())
}

/// Returns the title of the given emoji group, as shown in the emoji picker.
pub fn group_title(group: Group) -> &'static str {
    match group {
        Group::SmileysAndEmotion => "Smileys & Emotion",
        Group::PeopleAndBody => "People & Body",
        Group::AnimalsAndNature => "Animals & Nature",
        Group::FoodAndDrink => "Food & Drink",
        Group::TravelAndPlaces => "Travel & Places",
        Group::Activities => "Activities",
        Group::Objects => "Objects",
        Group::Symbols => "Symbols",
        Group::Flags => "Flags",
    }
}

/// Splits the given lowercase text into its words, e.g., "heart-eyes" into "heart" and "eyes".
fn split_into_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty())
}
