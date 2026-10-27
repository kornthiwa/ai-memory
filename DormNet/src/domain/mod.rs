//! Domain layer: pure types and policies with no I/O.

pub mod credentials;
pub mod retry;
pub mod state;

pub use credentials::Credentials;
pub use retry::{FailureKind, RetryManager, RetryPolicy};
pub use state::{State, TransitionReason};
