//! Room Member Manager - Efficient memory management using reference counting
//! Data is retained when components subscribe and automatically cleaned up when there are no subscribers
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;

/// Component needs to implement this trait to receive room member updates
pub trait RoomMemberSubscriber: 'static + Send + Sync {
    /// Called when the room member list is updated
    fn on_room_members_updated(
        &mut self, room_id: &OwnedRoomId, members: Arc<Vec<RoomMember>>,
    );
}

/// Wrapper type for subscribers
struct SubscriberEntry {
    /// Strong reference to the subscriber - Changed to Arc instead of Weak to ensure lifetime
    subscriber: Arc<Mutex<dyn RoomMemberSubscriber>>,
    /// Subscriber's unique identifier
    id: u64,
}

/// Manages room member data and notifies subscribers
#[derive(Default)]
pub struct RoomMemberManager {
    /// Stores member lists by room ID
    room_members: HashMap<OwnedRoomId, Arc<Vec<RoomMember>>>,

    /// Subscriber list (grouped by room ID)
    subscribers: HashMap<OwnedRoomId, Vec<SubscriberEntry>>,

    /// Active subscriber count per room
    active_subscribers_count: HashMap<OwnedRoomId, usize>,

    /// Used to generate unique subscriber IDs
    next_subscriber_id: u64,
}

/// Global singleton
static ROOM_MEMBER_MANAGER: OnceLock<Arc<Mutex<RoomMemberManager>>> = OnceLock::new();

impl RoomMemberManager {
    /// Get the manager singleton instance
    pub fn instance() -> Arc<Mutex<RoomMemberManager>> {
        ROOM_MEMBER_MANAGER
            .get_or_init(|| Arc::new(Mutex::new(RoomMemberManager::default())))
            .clone()
    }

    /// Update specific room's member list and notify subscribers
    pub fn update_room_members(room_id: OwnedRoomId, members: Vec<RoomMember>) {
        log!("Updating room members for room {}", room_id.clone());

        let instance = Self::instance();
        let mut manager = instance.lock().unwrap();

        // Only store data when there are active subscribers
        if manager.active_subscribers_count.get(&room_id).copied().unwrap_or(0) > 0 {
            let members_changed = if let Some(existing_members) = manager.room_members.get(&room_id) {
                if existing_members.len() != members.len() {
                    true
                } else {
                    let existing_ids: std::collections::HashSet<_> = existing_members.iter()
                        .map(|m| m.user_id().to_owned())
                        .collect();
                    let new_ids: std::collections::HashSet<_> = members.iter()
                        .map(|m| m.user_id().to_owned())
                        .collect();

                    existing_ids != new_ids
                }
            } else {
                // first update, always considered as changed
                true
            };

            if !members_changed {
                log!("Skipping room {} member update (no actual changes)", room_id);
                return;
            }

            // Create a shared reference for the member list
            let shared_members = Arc::new(members);

            // Store the shared member list
            manager.room_members.insert(room_id.clone(), shared_members.clone());

            // Collect subscribers to notify
            let mut to_notify = Vec::new();

            if let Some(subscribers) = manager.subscribers.get(&room_id) {
                log!("Processing {} subscribers for room {}", subscribers.len(), room_id);

                // Since using Arc instead of Weak, all subscribers should be valid
                for entry in subscribers {
                    to_notify.push((entry.subscriber.clone(), shared_members.clone()));
                }
            }

            // Log the update
            log!(
                "Updating room {} member data, notifying {} subscribers",
                room_id,
                to_notify.len()
            );

            // Release the lock before notifying subscribers
            drop(manager);

            // Notify all collected subscribers after the lock is released
            let room_id_ref = &room_id;
            for (subscriber, members) in to_notify {
                if let Ok(mut sub) = subscriber.lock() {
                    sub.on_room_members_updated(room_id_ref, members);
                } else {
                    log!("Warning: Unable to acquire subscriber lock");
                }
            }
        } else {
            log!("Skipping storing room {} member data (no active subscribers)", room_id);
        }
    }

    /// Subscribe to specific room member updates
    pub fn subscribe(
        room_id: OwnedRoomId, subscriber: Arc<Mutex<dyn RoomMemberSubscriber>>,
    ) -> u64 {
        let instance = Self::instance();
        let mut manager = instance.lock().unwrap();

        // Create unique subscription ID
        let subscriber_id = manager.next_subscriber_id;
        manager.next_subscriber_id += 1;

        // Increase subscriber count
        *manager.active_subscribers_count.entry(room_id.clone()).or_insert(0) += 1;

        // Create subscription entry and add to corresponding room
        let entry = SubscriberEntry { subscriber: subscriber.clone(), id: subscriber_id };

        manager.subscribers.entry(room_id.clone()).or_default().push(entry);

        log!(
            "Room {} added a new subscriber ID: {}, total subscribers: {}",
            room_id,
            subscriber_id,
            manager.active_subscribers_count.get(&room_id).copied().unwrap_or(0)
        );

        // If there is current room member data, immediately provide it to the new subscriber
        let members_clone = manager.room_members.get(&room_id).cloned();

        // To avoid executing callbacks within the lock, release the lock first
        drop(manager);

        // If there is existing data, immediately notify the new subscriber
        if let Some(members) = members_clone {
            if let Ok(mut sub) = subscriber.lock() {
                sub.on_room_members_updated(&room_id, members);
            }
        }

        // Return subscription ID
        subscriber_id
    }

    /// Unsubscribe
    pub fn unsubscribe(room_id: &OwnedRoomId, subscriber_id: u64) {
        let instance = Self::instance();
        let mut manager = instance.lock().unwrap();

        if let Some(subscribers) = manager.subscribers.get_mut(room_id) {
            // Check if the subscriber exists
            let existed = subscribers.iter().any(|entry| entry.id == subscriber_id);

            // Remove the matching subscriber
            subscribers.retain(|entry| entry.id != subscriber_id);

            // If the subscriber exists and was removed, decrease the count
            if existed {
                if let Some(count) = manager.active_subscribers_count.get_mut(room_id) {
                    *count = count.saturating_sub(1);

                    // If there are no more subscribers, clean up the data
                    if *count == 0 {
                        manager.room_members.remove(room_id);
                        manager.subscribers.remove(room_id);
                        manager.active_subscribers_count.remove(room_id);
                        log!(
                            "Automatically cleaned up room {} member data (no subscribers)",
                            room_id
                        );
                    } else {
                        log!(
                            "Unsubscribed ID: {} from room {}. Remaining subscribers: {}",
                            subscriber_id,
                            room_id,
                            *count
                        );
                    }
                }
            }
        }
    }

    /// Diagnostic method: Get the subscriber count for a room
    #[allow(dead_code)]
    pub fn get_subscriber_count(room_id: &OwnedRoomId) -> usize {
        let instance = Self::instance();
        let manager = instance.lock().unwrap();
        manager.active_subscribers_count.get(room_id).copied().unwrap_or(0)
    }

    /// Diagnostic method: Get the total number of managed rooms
    #[allow(dead_code)]
    pub fn get_managed_room_count() -> usize {
        let instance = Self::instance();
        let manager = instance.lock().unwrap();
        manager.room_members.len()
    }
}

/// Helper type for managing subscription lifecycles
pub struct RoomMemberSubscription {
    /// Room ID
    room_id: OwnedRoomId,
    /// Subscription ID
    subscription_id: u64,
    /// Flag indicating whether the subscription has been unsubscribed
    unsubscribed: bool,
}

impl RoomMemberSubscription {
    /// Create a new subscription
    pub fn new(
        room_id: OwnedRoomId, subscriber: Arc<Mutex<dyn RoomMemberSubscriber>>,
    ) -> Self {
        let subscription_id = RoomMemberManager::subscribe(room_id.clone(), subscriber);
        Self { room_id, subscription_id, unsubscribed: false }
    }

    /// Manually unsubscribe from the subscription
    pub fn unsubscribe(&mut self) {
        if !self.unsubscribed {
            RoomMemberManager::unsubscribe(&self.room_id, self.subscription_id);
            self.unsubscribed = true;
        }
    }
}

/// Auto-unsubscribe when the subscribed component is destroyed
impl Drop for RoomMemberSubscription {
    fn drop(&mut self) {
        log!("RoomMemberSubscription dropped");
        self.unsubscribe();
    }
}

/// Room member update shortcut methods
pub mod room_members {
    use super::*;

    /// Update room member data
    pub fn update(room_id: OwnedRoomId, members: Vec<RoomMember>) {
        RoomMemberManager::update_room_members(room_id, members);
    }

    /// Diagnostic: Get the subscriber count for a room
    #[allow(dead_code)]
    pub fn subscriber_count(room_id: &OwnedRoomId) -> usize {
        RoomMemberManager::get_subscriber_count(room_id)
    }

    /// Diagnostic: Get the total number of managed rooms
    #[allow(dead_code)]
    pub fn managed_room_count() -> usize {
        RoomMemberManager::get_managed_room_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Simple test subscriber implementation
    struct TestSubscriber {
        id: String,
        received_updates: AtomicUsize,
    }

    impl RoomMemberSubscriber for TestSubscriber {
        fn on_room_members_updated(
            &mut self, room_id: &OwnedRoomId, members: Arc<Vec<RoomMember>>,
        ) {
            self.received_updates.fetch_add(1, Ordering::SeqCst);
            log!(
                "TestSubscriber({}) received update for room {} with {} members",
                self.id,
                room_id,
                members.len()
            );
        }
    }

    // fn create_test_cx() -> Cx {
    //     let event_handler = Box::new(|_: &mut Cx, _: &Event| {
    //         log!("Test event handler called");
    //     });

    //     Cx::new(event_handler)
    // }

    #[test]
    fn test_subscription() {
        // Create a mock room ID
        let room_id_str = "!test_room:example.org";
        let room_id = OwnedRoomId::try_from(room_id_str).unwrap();

        // Reset the singleton for test isolation
        let manager = RoomMemberManager::instance();
        let mut guard = manager.lock().unwrap();
        *guard = RoomMemberManager::default();
        drop(guard);

        // Create test subscriber
        let subscriber = Arc::new(Mutex::new(TestSubscriber {
            id: "test_sub".to_string(),
            received_updates: AtomicUsize::new(0),
        }));

        // Manually manage subscription ID
        let sub_id = RoomMemberManager::subscribe(room_id.clone(), subscriber.clone());

        // Verify subscription was created
        assert_eq!(RoomMemberManager::get_subscriber_count(&room_id), 1);

        // Get initial subscriber update count
        let initial_count = subscriber.lock().unwrap().received_updates.load(Ordering::SeqCst);

        // Directly update room members
        RoomMemberManager::update_room_members(room_id.clone(), vec![]);

        // Verify update was received
        let new_count = subscriber.lock().unwrap().received_updates.load(Ordering::SeqCst);
        assert_eq!(new_count, initial_count + 1);

        // Manually unsubscribe
        RoomMemberManager::unsubscribe(&room_id, sub_id);

        // Verify subscription was removed
        assert_eq!(RoomMemberManager::get_subscriber_count(&room_id), 0);

        // Update room members again
        RoomMemberManager::update_room_members(room_id.clone(), vec![]);

        // Verify no additional updates were received
        let final_count = subscriber.lock().unwrap().received_updates.load(Ordering::SeqCst);
        assert_eq!(final_count, new_count);
    }
}
