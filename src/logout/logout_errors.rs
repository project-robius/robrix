//! Improved error types for logout operations
use std::fmt;

/// Categorized logout errors for better handling
#[derive(Debug, Clone, PartialEq)]
pub enum LogoutError {
    /// Recoverable errors - can retry or continue using app
    Recoverable(RecoverableError),
    /// Unrecoverable errors - app needs restart
    Unrecoverable(UnrecoverableError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoverableError {
    /// No access token present
    NoAccessToken,
    /// Server logout failed but can retry
    ServerLogoutFailed(String),
    /// Timeout during operation
    Timeout(String),
    /// User cancelled the operation
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnrecoverableError {
    /// Key components (e.g., the client or sync service) have been cleared
    /// during a logout attempt, so Robrix cannot continue executing properly.
    ComponentsCleared,
    /// Failed after point of no return
    PostPointOfNoReturnFailure(String),
    /// Runtime restart failed
    RuntimeRestartFailed,
}

impl fmt::Display for LogoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogoutError::Recoverable(e) => write!(f, "Recoverable error: {:?}", e),
            LogoutError::Unrecoverable(e) => write!(f, "Unrecoverable error: {:?}", e),
        }
    }
}

impl std::error::Error for LogoutError {}
