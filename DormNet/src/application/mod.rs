//! Application layer: use-cases and orchestration.

pub mod config;
pub mod runtime;
pub mod session;

pub use config::AppConfig;
pub use runtime::Runtime;
pub use session::{AuthGate, SessionMonitor};
