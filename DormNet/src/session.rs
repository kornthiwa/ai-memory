//! Session monitoring: periodic connectivity re-check and login mutex.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::network::{Connectivity, NetworkMonitor};

/// Prevents concurrent authentication attempts.
#[derive(Clone, Default)]
pub struct AuthGate {
    busy: Arc<AtomicBool>,
}

impl AuthGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if this caller acquired the gate.
    pub fn try_enter(&self) -> bool {
        self.busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn exit(&self) {
        self.busy.store(false, Ordering::SeqCst);
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }
}

pub struct SessionMonitor {
    network: NetworkMonitor,
    interval: Duration,
    gate: AuthGate,
    last: Arc<Mutex<Option<Connectivity>>>,
}

impl SessionMonitor {
    pub fn new(network: NetworkMonitor, interval: Duration, gate: AuthGate) -> Self {
        Self {
            network,
            interval,
            gate,
            last: Arc::new(Mutex::new(None)),
        }
    }

    pub fn gate(&self) -> &AuthGate {
        &self.gate
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub async fn check_once(&self) -> Connectivity {
        let status = self.network.check_connectivity().await;
        *self.last.lock().await = Some(status);
        status
    }

    pub async fn last_status(&self) -> Option<Connectivity> {
        *self.last.lock().await
    }

    /// True when previously online and now captive/offline.
    pub fn session_lost(previous: Connectivity, current: Connectivity) -> bool {
        previous == Connectivity::Online && current != Connectivity::Online
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_gate_is_exclusive() {
        let g = AuthGate::new();
        assert!(g.try_enter());
        assert!(!g.try_enter());
        g.exit();
        assert!(g.try_enter());
    }

    #[test]
    fn detects_session_loss() {
        assert!(SessionMonitor::session_lost(
            Connectivity::Online,
            Connectivity::Captive
        ));
        assert!(!SessionMonitor::session_lost(
            Connectivity::Online,
            Connectivity::Online
        ));
    }
}
