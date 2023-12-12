#[derive(Debug, Clone)]
pub enum ContactKind {
    People,
    FileTransfer,
    WeChat,
}

#[derive(Debug, Clone)]
pub struct ContactInfo {
    pub name: String,
    pub kind: ContactKind,
}
