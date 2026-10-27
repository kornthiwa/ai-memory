//! Runtime configuration for the background monitor.

use std::sync::Arc;
use std::time::Duration;

use crate::domain::Credentials;
use crate::infrastructure::auth::AuthProvider;
use crate::infrastructure::network::NetworkMonitor;

pub struct AppConfig {
    pub monitor_interval: Duration,
    pub network_wait: Duration,
    /// When set (tests), use this provider instead of the default MikroTik auth.
    pub auth: Option<Arc<dyn AuthProvider>>,
    pub network: Option<NetworkMonitor>,
    pub credentials: Option<Credentials>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            monitor_interval: Duration::from_secs(30),
            network_wait: Duration::from_secs(3),
            auth: None,
            network: None,
            credentials: None,
        }
    }
}
