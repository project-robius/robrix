use makepad_widgets::makepad_platform::{LiveId, live_id};
use std::iter;

#[derive(Clone, Debug)]
pub struct ChatEntry {
    pub id: u64,
    pub username: String,
    pub avatar: LiveId,
    pub latest_message: MessagePreview,
    pub timestamp: String,
}

#[derive(Clone, Debug)]
pub enum MessagePreview {
    Audio,
    Image,
    Video,
    Text(String),
}

impl MessagePreview {
    pub fn text(&self) -> &str {
        match self {
            MessagePreview::Audio => "[Audio]",
            MessagePreview::Image => "[Image]",
            MessagePreview::Video => "[Video]",
            MessagePreview::Text(text) => text,
        }
    }
}

#[derive(Clone, Debug)]
pub enum MessageDirection {
    Outgoing,
    Incoming,
}

#[derive(Clone, Debug)]
pub struct MessageEntry {
    pub direction: MessageDirection,
    pub chat_id: u64,
    pub avatar: LiveId,
    pub text: String,
}

pub struct Db {
    messages: Vec<MessageEntry>,
    chats: Vec<ChatEntry>,
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}

impl Db {
    pub fn new() -> Self {
        let messages: Vec<MessageEntry> = (0..200)
        .flat_map(|i| {
            vec![
                MessageEntry {
                    direction: MessageDirection::Incoming,
                    avatar: live_id!(jorgebejar),
                    chat_id: (i * 2) % 50 + 1,
                    text: "体議速人幅触無持編聞組込".to_string(),
                },
                MessageEntry {
                    direction: MessageDirection::Outgoing,
                    avatar: live_id!(rikarends),
                    chat_id: (i * 2) % 50 + 1,
                    text: "減活乗治外進".to_string(),
                },
                MessageEntry {
                    direction: MessageDirection::Incoming,
                    avatar: live_id!(jorgebejar),
                    chat_id: (i * 2) % 50 + 1,
                    text: "福読併棋一御質慰".to_string(),
                },
                MessageEntry {
                    direction: MessageDirection::Outgoing,
                    avatar: live_id!(rikarends),
                    chat_id: (i * 2) % 50 + 1,
                    text: "嶋可済政実玉全強無示餌".to_string(),
                },
                MessageEntry {
                    direction: MessageDirection::Outgoing,
                    avatar: live_id!(johndoe),
                    chat_id: (i * 2) % 50 + 2,
                    text: "福読併棋一御質慰".to_string(),
                },
                MessageEntry {
                    direction: MessageDirection::Incoming,
                    avatar: live_id!(julianmontesdeoca),
                    chat_id: (i * 2) % 50 + 2,
                    text: "消再野誰強心無嶋可済実玉全示餌".to_string(),
                },
                MessageEntry {
                    direction: MessageDirection::Outgoing,
                    avatar: live_id!(johndoe),
                    chat_id: (i * 2) % 50 + 2,
                    text: "体議速人幅触無持編聞組込".to_string(),
                },
                MessageEntry {
                    direction: MessageDirection::Incoming,
                    avatar: live_id!(julianmontesdeoca),
                    chat_id: (i * 2) % 50 + 2,
                    text: "減活乗治外進".to_string(),
                },
            ]
        })
        .collect();
        
        Db {
            messages,
            chats: vec![
            ChatEntry {
                id: 1,
                username: "Olive Yew".to_string(),
                avatar: live_id!(rikarends),
                latest_message: MessagePreview::Text("Hi!".to_string()),
                timestamp: "14:09".to_string(),
            },
            ChatEntry {
                id: 2,
                username: "John Doe".to_string(),
                avatar: live_id!(johndoe),
                latest_message: MessagePreview::Image,
                timestamp: "11:20".to_string(),
            },
            ChatEntry {
                id: 3,
                username: "Peg Legge".to_string(),
                avatar: live_id!(jorgebejar),
                latest_message: MessagePreview::Audio,
                timestamp: "friday".to_string(),
            },
            ChatEntry {
                id: 4,
                username: "Barb Akew".to_string(),
                avatar: live_id!(julianmontesdeoca),
                latest_message: MessagePreview::Video,
                timestamp: "friday".to_string(),
            },
            ChatEntry {
                id: 5,
                username: "Chris P. Bacon".to_string(),
                avatar: live_id!(edwardtan),
                latest_message: MessagePreview::Text("thanks ed, see you there.".to_string()),
                timestamp: "thursday".to_string(),
            },
            ChatEntry {
                id: 6,
                username: "WeChat Team".to_string(),
                avatar: live_id!(wechatteam),
                latest_message: MessagePreview::Text("Welcome to WeChat!".to_string()),
                timestamp: "18/07".to_string(),
            },
            ChatEntry {
                id: 7,
                username: "Andrew Lin".to_string(),
                avatar: live_id!(jorgebejar),
                latest_message: MessagePreview::Text(
                    "Awesome, I'll make sure they know about it".to_string(),
                ),
                timestamp: "18/07".to_string(),
            },
            ChatEntry {
                id: 8,
                username: "Christian Huxley".to_string(),
                avatar: live_id!(jorgebejar),
                latest_message: MessagePreview::Image,
                timestamp: "15/07".to_string(),
            },
            ChatEntry {
                id: 9,
                username: "Ana Leddie".to_string(),
                avatar: live_id!(jorgebejar),
                latest_message: MessagePreview::Image,
                timestamp: "14/07".to_string(),
            },
            ChatEntry {
                id: 10,
                username: "Adam Adler".to_string(),
                avatar: live_id!(jorgebejar),
                latest_message: MessagePreview::Video,
                timestamp: "10/07".to_string(),
            },
            ChatEntry {
                id: 11,
                username: "Gabriel Hayes".to_string(),
                avatar: live_id!(jorgebejar),
                latest_message: MessagePreview::Text("wow I haven't seen that".to_string()),
                timestamp: "10/07".to_string(),
            },
            ChatEntry {
                id: 12,
                username: "Eric Ford".to_string(),
                avatar: live_id!(jorgebejar),
                latest_message: MessagePreview::Text("Nice to see you here!".to_string()),
                timestamp: "10/07".to_string(),
            },
            ],
        }
    }
    
    pub fn get_all_chats(&self) -> Vec<ChatEntry> {
        iter::repeat(self.chats.clone()).take(50).flatten().collect()
    }
    
    pub fn get_chat(&self, chat_id: u64) -> Option<&ChatEntry> {
        self.chats.iter().find(|m| m.id == chat_id)
    }
    
    pub fn get_messages_by_chat_id(&self, chat_id: u64) -> Vec<MessageEntry> {
        self.messages
        .iter()
        .filter(|m| m.chat_id == chat_id)
        .cloned()
        .collect()
    }
}
