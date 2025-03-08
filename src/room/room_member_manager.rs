//! 房间成员管理器 - 使用引用计数实现高效的内存管理
//! 当组件订阅时保留数据，当没有订阅者时自动清理数据
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;

/// 组件需要实现此特性来接收房间成员更新
pub trait RoomMemberSubscriber: 'static + Send + Sync {
    /// 当房间成员列表更新时被调用
    fn on_room_members_updated(&mut self, cx: &mut Cx, room_id: &OwnedRoomId, members: Arc<Vec<RoomMember>>);
}

/// 订阅者的包装类型
struct SubscriberEntry {
    /// 订阅者的强引用 - 改为 Arc 而不是 Weak 以确保生命周期
    subscriber: Arc<Mutex<dyn RoomMemberSubscriber>>,
    /// 订阅者的唯一标识符
    id: u64,
}

/// 管理房间成员数据和通知订阅者
#[derive(Default)]
pub struct RoomMemberManager {
    /// 按房间ID存储的成员列表
    room_members: HashMap<OwnedRoomId, Arc<Vec<RoomMember>>>,

    /// 订阅者列表 (按房间ID分组)
    subscribers: HashMap<OwnedRoomId, Vec<SubscriberEntry>>,

    /// 每个房间的活跃订阅者计数
    active_subscribers_count: HashMap<OwnedRoomId, usize>,

    /// 用于生成唯一订阅者ID
    next_subscriber_id: u64,
}

/// 全局单例，使用 OnceLock 代替 lazy_static
static ROOM_MEMBER_MANAGER: OnceLock<Arc<Mutex<RoomMemberManager>>> = OnceLock::new();

impl RoomMemberManager {
    /// 获取管理器单例实例
    pub fn instance() -> Arc<Mutex<RoomMemberManager>> {
        ROOM_MEMBER_MANAGER.get_or_init(|| {
            Arc::new(Mutex::new(RoomMemberManager::default()))
        }).clone()
    }

    /// 更新特定房间的成员列表并通知订阅者
    pub fn update_room_members(cx: &mut Cx, room_id: OwnedRoomId, members: Vec<RoomMember>) {
        let instance = Self::instance();
        let mut manager = instance.lock().unwrap();

        // 只有当有活跃订阅者时才存储数据
        if manager.active_subscribers_count.get(&room_id).copied().unwrap_or(0) > 0 {
            // 为成员列表创建共享引用
            let shared_members = Arc::new(members);

            // 存储共享的成员列表
            manager.room_members.insert(room_id.clone(), shared_members.clone());

            // 收集需要通知的订阅者
            let mut to_notify = Vec::new();

            if let Some(subscribers) = manager.subscribers.get(&room_id) {
                log!("Processing {} subscribers for room {}", subscribers.len(), room_id);

                // 由于使用 Arc 而不是 Weak，所有订阅者都应该有效
                for entry in subscribers {
                    to_notify.push((entry.subscriber.clone(), shared_members.clone()));
                }
            }

            // 记录日志
            log!("更新房间 {} 的成员数据，通知 {} 个订阅者",
                 room_id, to_notify.len());

            // 释放锁，然后通知订阅者
            drop(manager);

            // 在锁释放后通知所有收集的订阅者
            let room_id_ref = &room_id;
            for (subscriber, members) in to_notify {
                if let Ok(mut sub) = subscriber.lock() {
                    sub.on_room_members_updated(cx, room_id_ref, members);
                } else {
                    log!("警告：无法获取订阅者锁");
                }
            }
        } else {
            log!("跳过存储房间 {} 的成员数据 (没有活跃订阅者)", room_id);
        }
    }

    /// 订阅特定房间的成员更新
    pub fn subscribe(
        cx: &mut Cx,
        room_id: OwnedRoomId,
        subscriber: Arc<Mutex<dyn RoomMemberSubscriber>>,
    ) -> u64 {
        let instance = Self::instance();
        let mut manager = instance.lock().unwrap();

        // 创建唯一订阅ID
        let subscriber_id = manager.next_subscriber_id;
        manager.next_subscriber_id += 1;

        // 增加订阅者计数
        *manager.active_subscribers_count.entry(room_id.clone()).or_insert(0) += 1;

        // 创建订阅条目并添加到对应房间
        let entry = SubscriberEntry {
            subscriber: subscriber.clone(), // 存储强引用
            id: subscriber_id,
        };

        manager.subscribers.entry(room_id.clone()).or_default().push(entry);

        log!("房间 {} 添加了新订阅者 ID: {}，总订阅者数: {}",
             room_id, subscriber_id, manager.active_subscribers_count.get(&room_id).copied().unwrap_or(0));

        // 如果有当前房间的成员数据，立即提供给新订阅者
        let members_clone = manager.room_members.get(&room_id).cloned();

        // 为了避免在锁内执行回调，先释放锁
        drop(manager);

        // 如果有现有数据，立即通知新订阅者
        if let Some(members) = members_clone {
            if let Ok(mut sub) = subscriber.lock() {
                sub.on_room_members_updated(cx, &room_id, members);
            }
        }

        // 返回订阅ID
        subscriber_id
    }

    /// 取消订阅
    pub fn unsubscribe(room_id: &OwnedRoomId, subscriber_id: u64) {
        let instance = Self::instance();
        let mut manager = instance.lock().unwrap();

        if let Some(subscribers) = manager.subscribers.get_mut(room_id) {
            // 检查订阅者是否存在
            let existed = subscribers.iter().any(|entry| entry.id == subscriber_id);

            // 移除匹配的订阅者
            subscribers.retain(|entry| entry.id != subscriber_id);

            // 如果订阅者存在且被移除，减少计数
            if existed {
                if let Some(count) = manager.active_subscribers_count.get_mut(room_id) {
                    *count = count.saturating_sub(1);

                    // 如果没有更多订阅者，清理数据
                    if *count == 0 {
                        manager.room_members.remove(room_id);
                        manager.subscribers.remove(room_id);
                        manager.active_subscribers_count.remove(room_id);
                        log!("自动清理房间 {} 的成员数据 (没有订阅者)", room_id);
                    } else {
                        log!("从房间 {} 取消订阅 ID: {}。剩余订阅者: {}",
                            room_id, subscriber_id, *count);
                    }
                }
            }
        }
    }

    /// 获取特定房间的成员列表
    pub fn get_room_members(room_id: &OwnedRoomId) -> Option<Arc<Vec<RoomMember>>> {
        let instance = Self::instance();
        let manager = instance.lock().unwrap();
        manager.room_members.get(room_id).cloned()
    }

    /// 诊断方法：获取房间的订阅者计数
    #[allow(dead_code)]
    pub fn get_subscriber_count(room_id: &OwnedRoomId) -> usize {
        let instance = Self::instance();
        let manager = instance.lock().unwrap();
        manager.active_subscribers_count.get(room_id).copied().unwrap_or(0)
    }

    /// 诊断方法：获取管理的房间总数
    #[allow(dead_code)]
    pub fn get_managed_room_count() -> usize {
        let instance = Self::instance();
        let manager = instance.lock().unwrap();
        manager.room_members.len()
    }
}

/// 用于管理订阅生命周期的辅助类型
pub struct RoomMemberSubscription {
    /// 房间ID
    room_id: OwnedRoomId,
    /// 订阅ID
    subscription_id: u64,
    /// 标记是否已取消订阅
    unsubscribed: bool,
}

impl RoomMemberSubscription {
    /// 创建新的订阅
    pub fn new(
        cx: &mut Cx,
        room_id: OwnedRoomId,
        subscriber: Arc<Mutex<dyn RoomMemberSubscriber>>,
    ) -> Self {
        let subscription_id = RoomMemberManager::subscribe(cx, room_id.clone(), subscriber);
        Self {
            room_id,
            subscription_id,
            unsubscribed: false,
        }
    }

    /// 手动取消订阅
    pub fn unsubscribe(&mut self) {
        if !self.unsubscribed {
            RoomMemberManager::unsubscribe(&self.room_id, self.subscription_id);
            self.unsubscribed = true;
        }
    }
}

impl Drop for RoomMemberSubscription {
    fn drop(&mut self) {
        self.unsubscribe();
    }
}

/// 房间成员更新的快捷访问方法
pub mod room_members {
    use super::*;

    /// 更新房间成员数据
    pub fn update(cx: &mut Cx, room_id: OwnedRoomId, members: Vec<RoomMember>) {
        RoomMemberManager::update_room_members(cx, room_id, members);
    }

    /// 获取房间成员数据
    pub fn get(room_id: &OwnedRoomId) -> Option<Arc<Vec<RoomMember>>> {
        RoomMemberManager::get_room_members(room_id)
    }

    /// 诊断：获取房间的订阅者计数
    #[allow(dead_code)]
    pub fn subscriber_count(room_id: &OwnedRoomId) -> usize {
        RoomMemberManager::get_subscriber_count(room_id)
    }

    /// 诊断：获取管理的房间总数
    #[allow(dead_code)]
    pub fn managed_room_count() -> usize {
        RoomMemberManager::get_managed_room_count()
    }
}
