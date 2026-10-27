//! DormNet library surface for the binary and integration tests.
//!
//! Layered layout:
//! - [`domain`] — pure types (state, credentials, retry)
//! - [`application`] — runtime orchestration and session monitoring
//! - [`infrastructure`] — network, auth adapters, keyring, logging
//! - [`presentation`] — CLI + egui GUI

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;

// Compatibility re-exports (stable import paths for tests and callers).
pub use application::{AppConfig, AuthGate, Runtime, SessionMonitor};
pub use application::session;
pub use domain::{Credentials, FailureKind, RetryManager, RetryPolicy, State, TransitionReason};
pub use domain::retry;
pub use infrastructure::auth;
pub use infrastructure::credentials_store as creds;
pub use infrastructure::logging as logutil;
pub use infrastructure::network;
pub use infrastructure::single_instance;
