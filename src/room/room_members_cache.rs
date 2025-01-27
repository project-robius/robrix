//! A cache of room members, indexed by room ID.
//!
//! Similar to user_profile_cache, this cache is only accessible from the main UI thread.

use crossbeam_queue::SegQueue;
use makepad_widgets::{log, Cx, SignalToUI};
use matrix_sdk::{room::RoomMember, ruma::OwnedRoomId};
use std::{cell::RefCell, collections::{btree_map::Entry, BTreeMap}};
use crate::sliding_sync::{submit_async_request, MatrixRequest};

// 使用线程本地存储存储房间成员缓存
thread_local! {
    /// 按房间 ID 索引的成员缓存，仅可从主 UI 线程访问
    static ROOM_MEMBERS_CACHE: RefCell<BTreeMap<OwnedRoomId, RoomMembersCacheEntry>> = const { RefCell::new(BTreeMap::new()) };
}

// 缓存条目的状态
enum RoomMembersCacheEntry {
    /// 已发出请求，等待完成
    Requested,
    /// 成功从服务器加载的成员列表
    Loaded(Vec<RoomMember>),
}

// 待处理的更新队列
static PENDING_ROOM_MEMBERS_UPDATES: SegQueue<RoomMembersUpdate> = SegQueue::new();

// 更新类型
pub struct RoomMembersUpdate {
    pub room_id: OwnedRoomId,
    pub members: Vec<RoomMember>,
}

// 将更新添加到队列
pub fn enqueue_room_members_update(update: RoomMembersUpdate) {
    PENDING_ROOM_MEMBERS_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

// 处理所有待处理的更新
pub fn process_room_members_updates(cx: &mut Cx) {
    log!("Processing room members updates...");
    ROOM_MEMBERS_CACHE.with_borrow_mut(|cache| {
        let mut processed = 0;
        while let Some(update) = PENDING_ROOM_MEMBERS_UPDATES.pop() {
            processed += 1;
            log!("Processing update {} - room {} with {} members",
                processed, update.room_id, update.members.len());
            match cache.entry(update.room_id) {
                Entry::Occupied(mut entry) => {
                    *entry.get_mut() = RoomMembersCacheEntry::Loaded(update.members);
                }
                Entry::Vacant(entry) => {
                    entry.insert(RoomMembersCacheEntry::Loaded(update.members));
                }
            }
        }
        if processed > 0 {
            log!("Processed {} room members updates", processed);
        }
    });
}

// 获取房间成员，如果不存在则可选择发起获取请求
pub fn get_room_members(
    _cx: &mut Cx,
    room_id: OwnedRoomId,
    fetch_if_missing: bool,
) -> Option<Vec<RoomMember>> {
    ROOM_MEMBERS_CACHE.with_borrow_mut(|cache|
        match cache.entry(room_id) {
            Entry::Occupied(entry) => match entry.get() {
                RoomMembersCacheEntry::Loaded(members) => Some(members.clone()),
                RoomMembersCacheEntry::Requested => None,
            }
            Entry::Vacant(entry) => {
                if fetch_if_missing {
                    log!("Room members not found in cache for room {}, fetching from server.", entry.key());
                    // 发起获取请求
                    submit_async_request(MatrixRequest::FetchRoomMembers {
                        room_id: entry.key().clone(),
                    });
                    entry.insert(RoomMembersCacheEntry::Requested);
                }
                None
            }
        }
    )
}
