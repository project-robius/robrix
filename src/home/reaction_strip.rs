//! A row of the user's frequently used emoji for reacting to a message,
//! ending with a button that opens the emoji picker for any other emoji.

use emojis::Emoji;
use makepad_widgets::*;
use rustc_hash::FxHashSet;

use crate::shared::{
    context_menu::BUTTON_HEIGHT,
    emoji_catalog::{with_skin_tone, without_skin_tone},
    recent_emojis::{get_frequently_used_emojis, get_skin_tone},
};

const NUM_EMOJIS: usize = 5;
/// The emoji that fill the strip after the user's own frequently used ones, in order.
/// There are more than fit, since we leave out the ones that the user already reacted with.
const DEFAULT_EMOJIS: [&str; 10] = ["👍", "❤️", "😂", "🎉", "😮", "😢", "🙏", "🔥", "👀", "👏"];

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let ReactionStripButton = RobrixIconButton {
        width: 32, height: 32
        padding: 0, spacing: 0
        align: Align{x: 0.5, y: 0.5}
        draw_bg +: {
            color: (COLOR_TRANSPARENT)
            color_hover: (COLOR_FLOATING_ITEM_BG_HOVER)
            color_down: (COLOR_CONTEXT_MENU_ITEM_BG_DOWN)
        }
        draw_icon.color: (COLOR_TEXT_BODY)
        draw_text +: {
            ink_centered: false
            text_style: EMOJI_TEXT {}
        }
    }

    mod.widgets.ReactionStrip = #(ReactionStrip::register_widget(vm)) {
        width: Fill, height: #(BUTTON_HEIGHT)
        flow: Right
        align: Align{y: 0.5}
        spacing: 6

        emoji_0 := ReactionStripButton {}
        emoji_1 := ReactionStripButton {}
        emoji_2 := ReactionStripButton {}
        emoji_3 := ReactionStripButton {}
        emoji_4 := ReactionStripButton {}
        add_reaction_button := ReactionStripButton {
            icon_walk: Walk{width: 20, height: 20}
            draw_icon +: { svg: (ICON_ADD_REACTION) }
        }
    }
}

#[derive(Clone)]
struct ChildRefs {
    emoji_buttons: [ButtonRef; NUM_EMOJIS],
    add_reaction_button: ButtonRef,
}

#[derive(Script, Widget)]
pub struct ReactionStrip {
    #[deref] view: View,
    /// The emoji on each emoji button, or `None` for a hidden button.
    #[rust] emojis: [Option<&'static Emoji>; NUM_EMOJIS],
    #[rust] child_refs: Option<ChildRefs>,
}

impl ScriptHook for ReactionStrip {
    fn on_after_reload(&mut self, vm: &mut ScriptVm) {
        self.child_refs = None;
        // A reload resets the buttons' text and visibility, so we re-apply them.
        vm.with_cx_mut(|cx| self.populate_buttons(cx));
    }
}

impl Widget for ReactionStrip {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ReactionStrip {
    fn child_refs(&mut self, cx: &mut Cx) -> &ChildRefs {
        self.child_refs.get_or_insert_with(|| {
            let view = &self.view;
            ChildRefs {
                emoji_buttons: [
                    view.button(cx, ids!(emoji_0)),
                    view.button(cx, ids!(emoji_1)),
                    view.button(cx, ids!(emoji_2)),
                    view.button(cx, ids!(emoji_3)),
                    view.button(cx, ids!(emoji_4)),
                ],
                add_reaction_button: view.button(cx, ids!(add_reaction_button)),
            }
        })
    }

    fn populate_buttons(&mut self, cx: &mut Cx) {
        let ChildRefs { emoji_buttons, .. } = self.child_refs(cx).clone();
        for (button, emoji) in emoji_buttons.iter().zip(self.emojis) {
            if let Some(emoji) = emoji {
                button.set_text(cx, emoji.as_str());
            }
            button.set_visible(cx, emoji.is_some());
        }
    }
}

impl ReactionStripRef {
    /// Shows the user's frequently used emoji, leaving out the ones in `own_reactions`, which they already sent.
    pub fn populate(&self, cx: &mut Cx, own_reactions: &[String]) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let skin_tone = get_skin_tone(cx);
        let default_emojis = DEFAULT_EMOJIS.into_iter()
            .filter_map(emojis::get)
            .map(|emoji| with_skin_tone(emoji, skin_tone));
        // Each emoji only gets one button, in just one of its skin tones, and none if the user already reacted with it.
        let mut seen: FxHashSet<&str> = own_reactions.iter()
            .filter_map(|reaction| emojis::get(reaction))
            .map(|emoji| without_skin_tone(emoji).as_str())
            .collect();
        let mut emojis = get_frequently_used_emojis(cx, NUM_EMOJIS + seen.len()).into_iter()
            .chain(default_emojis)
            .filter(|emoji| seen.insert(without_skin_tone(emoji).as_str()));
        inner.emojis = std::array::from_fn(|_| emojis.next());
        inner.populate_buttons(cx);
    }

    pub fn reset_hover(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let ChildRefs { emoji_buttons, add_reaction_button } = inner.child_refs(cx).clone();
        for button in emoji_buttons.iter().chain([&add_reaction_button]) {
            button.reset_hover(cx);
        }
    }

    pub fn clicked_emoji(&self, actions: &Actions) -> Option<&'static Emoji> {
        let inner = self.borrow()?;
        let child_refs = inner.child_refs.as_ref()?;
        child_refs.emoji_buttons.iter()
            .zip(inner.emojis)
            .find_map(|(button, emoji)| emoji.filter(|_| button.clicked(actions)))
    }

    /// Returns whether the user clicked the button for picking any other emoji.
    pub fn add_reaction_clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner|
            inner.child_refs.as_ref().is_some_and(|child_refs| child_refs.add_reaction_button.clicked(actions))
        )
    }
}
