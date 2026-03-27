//! Multi-account management for Robrix.
//!
//! This module provides the infrastructure for managing multiple Matrix accounts
//! simultaneously, including:
//! - Storing and switching between multiple logged-in accounts
//! - Tracking the active (currently selected) account
//! - Managing account-specific state and sync connections

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use matrix_sdk::{Client, ruma::OwnedUserId};
use crate::persistence::ClientSessionPersisted;

/// Represents a logged-in Matrix account with its associated client and session info.
#[derive(Clone)]
pub struct Account {
    /// The Matrix client for this account
    pub client: Client,
    /// The user ID for this account
    pub user_id: OwnedUserId,
    /// The persisted session data for rebuilding the client
    pub session: ClientSessionPersisted,
    /// Display name for the account (cached from profile)
    pub display_name: Option<String>,
    /// Avatar URL for the account (cached from profile)
    pub avatar_url: Option<String>,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("user_id", &self.user_id)
            .field("display_name", &self.display_name)
            .field("avatar_url", &self.avatar_url)
            .finish_non_exhaustive()
    }
}

/// Manager for multiple Matrix accounts.
///
/// This struct handles:
/// - Storing multiple logged-in accounts
/// - Tracking which account is currently active
/// - Providing access to account-specific clients
#[derive(Default, Debug)]
pub struct AccountManager {
    /// Map of user_id to Account for all logged-in accounts
    accounts: HashMap<OwnedUserId, Account>,
    /// The currently active (selected) account's user_id
    active_account_id: Option<OwnedUserId>,
}

impl AccountManager {
    /// Creates a new empty AccountManager.
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            active_account_id: None,
        }
    }

    /// Adds a new account to the manager.
    ///
    /// If this is the first account, it becomes the active account automatically.
    /// Returns true if the account was newly added, false if it replaced an existing one.
    pub fn add_account(&mut self, account: Account) -> bool {
        let user_id = account.user_id.clone();
        let is_new = !self.accounts.contains_key(&user_id);

        // If this is the first account, make it active
        if self.accounts.is_empty() {
            self.active_account_id = Some(user_id.clone());
        }

        self.accounts.insert(user_id, account);
        is_new
    }

    /// Removes an account from the manager.
    ///
    /// If the removed account was active, switches to another available account.
    /// Returns the removed account if it existed.
    pub fn remove_account(&mut self, user_id: &OwnedUserId) -> Option<Account> {
        let removed = self.accounts.remove(user_id);

        // If we removed the active account, switch to another one
        if self.active_account_id.as_ref() == Some(user_id) {
            self.active_account_id = self.accounts.keys().next().cloned();
        }

        removed
    }

    /// Sets the active account by user_id.
    ///
    /// Returns true if the account exists and was made active, false otherwise.
    pub fn set_active_account(&mut self, user_id: &OwnedUserId) -> bool {
        if self.accounts.contains_key(user_id) {
            self.active_account_id = Some(user_id.clone());
            true
        } else {
            false
        }
    }

    /// Gets the currently active account.
    pub fn active_account(&self) -> Option<&Account> {
        self.active_account_id
            .as_ref()
            .and_then(|id| self.accounts.get(id))
    }

    /// Gets the currently active account mutably.
    pub fn active_account_mut(&mut self) -> Option<&mut Account> {
        let id = self.active_account_id.clone()?;
        self.accounts.get_mut(&id)
    }

    /// Gets the client for the currently active account.
    pub fn active_client(&self) -> Option<Client> {
        self.active_account().map(|a| a.client.clone())
    }

    /// Gets the user_id of the currently active account.
    pub fn active_user_id(&self) -> Option<&OwnedUserId> {
        self.active_account_id.as_ref()
    }

    /// Gets an account by user_id.
    pub fn get_account(&self, user_id: &OwnedUserId) -> Option<&Account> {
        self.accounts.get(user_id)
    }

    /// Gets a client by user_id.
    pub fn get_client(&self, user_id: &OwnedUserId) -> Option<Client> {
        self.accounts.get(user_id).map(|a| a.client.clone())
    }

    /// Returns an iterator over all accounts.
    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }

    /// Returns the number of logged-in accounts.
    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }

    /// Returns true if there are no logged-in accounts.
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// Returns all user IDs of logged-in accounts.
    pub fn user_ids(&self) -> Vec<OwnedUserId> {
        self.accounts.keys().cloned().collect()
    }

    /// Updates the display name for an account.
    pub fn update_display_name(&mut self, user_id: &OwnedUserId, display_name: Option<String>) {
        if let Some(account) = self.accounts.get_mut(user_id) {
            account.display_name = display_name;
        }
    }

    /// Updates the avatar URL for an account.
    pub fn update_avatar_url(&mut self, user_id: &OwnedUserId, avatar_url: Option<String>) {
        if let Some(account) = self.accounts.get_mut(user_id) {
            account.avatar_url = avatar_url;
        }
    }
}

// =============================================================================
// Global Account Manager Singleton
// =============================================================================

/// Global singleton for the account manager.
static ACCOUNT_MANAGER: OnceLock<Mutex<AccountManager>> = OnceLock::new();

/// Gets the global account manager.
fn account_manager() -> &'static Mutex<AccountManager> {
    ACCOUNT_MANAGER.get_or_init(|| Mutex::new(AccountManager::new()))
}

/// Adds an account to the global account manager.
pub fn add_account(account: Account) -> bool {
    account_manager().lock().unwrap().add_account(account)
}

/// Removes an account from the global account manager.
pub fn remove_account(user_id: &OwnedUserId) -> Option<Account> {
    account_manager().lock().unwrap().remove_account(user_id)
}

/// Sets the active account in the global account manager.
pub fn set_active_account(user_id: &OwnedUserId) -> bool {
    account_manager().lock().unwrap().set_active_account(user_id)
}

/// Gets the client for the currently active account.
pub fn get_active_client() -> Option<Client> {
    account_manager().lock().unwrap().active_client()
}

/// Gets the user_id of the currently active account.
pub fn get_active_user_id() -> Option<OwnedUserId> {
    account_manager().lock().unwrap().active_user_id().cloned()
}

/// Gets a client by user_id.
pub fn get_client_for_user(user_id: &OwnedUserId) -> Option<Client> {
    account_manager().lock().unwrap().get_client(user_id)
}

/// Returns the number of logged-in accounts.
pub fn account_count() -> usize {
    account_manager().lock().unwrap().account_count()
}

/// Returns all user IDs of logged-in accounts.
pub fn get_all_user_ids() -> Vec<OwnedUserId> {
    account_manager().lock().unwrap().user_ids()
}

/// Executes a closure with access to the account manager.
pub fn with_account_manager<F, R>(f: F) -> R
where
    F: FnOnce(&AccountManager) -> R,
{
    let manager = account_manager().lock().unwrap();
    f(&manager)
}

/// Executes a closure with mutable access to the account manager.
pub fn with_account_manager_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut AccountManager) -> R,
{
    let mut manager = account_manager().lock().unwrap();
    f(&mut manager)
}

/// Clears all accounts from the global account manager.
/// This should only be used during logout of all accounts.
pub fn clear_all_accounts() {
    let mut manager = account_manager().lock().unwrap();
    manager.accounts.clear();
    manager.active_account_id = None;
}
