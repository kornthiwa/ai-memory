//! DormNet library surface for the binary and integration tests.

pub mod auth;
pub mod creds;
pub mod logutil;
pub mod network;
pub mod retry;
pub mod session;
pub mod single_instance;
pub mod state;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::watch;
use tracing::{error, info, warn};

use crate::auth::{AuthError, AuthProvider, MikroTikHotspotAuth};
use crate::creds::Credentials;
use crate::network::{Connectivity, NetworkMonitor};
use crate::retry::{FailureKind, RetryManager, RetryPolicy};
use crate::session::{AuthGate, SessionMonitor};
use crate::state::{State, TransitionReason};

pub struct AppConfig {
    pub monitor_interval: Duration,
    pub network_wait: Duration,
    /// When set (tests), use this provider instead of UnconfiguredAuth.
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

pub struct Runtime {
    state: State,
    network: NetworkMonitor,
    session: SessionMonitor,
    retry: RetryManager,
    auth: Arc<dyn AuthProvider>,
    creds: Option<Credentials>,
    network_wait: Duration,
    shutdown: watch::Receiver<bool>,
}

impl Runtime {
    pub fn new(config: AppConfig, shutdown: watch::Receiver<bool>) -> Result<Self> {
        let network = match config.network {
            Some(n) => n,
            None => NetworkMonitor::new().context("build network monitor")?,
        };
        let gate = AuthGate::new();
        let session = SessionMonitor::new(network.clone(), config.monitor_interval, gate);
        let auth = config
            .auth
            .unwrap_or_else(|| Arc::new(MikroTikHotspotAuth::default()) as Arc<dyn AuthProvider>);

        Ok(Self {
            state: State::Starting,
            network,
            session,
            retry: RetryManager::new(RetryPolicy::default()),
            auth,
            creds: config.credentials,
            network_wait: config.network_wait,
            shutdown,
        })
    }

    pub fn state(&self) -> State {
        self.state
    }

    fn transition(&mut self, next: State, reason: TransitionReason) {
        info!(from = %self.state, to = %next, %reason, "state transition");
        self.state = next;
    }

    fn shutting_down(&self) -> bool {
        *self.shutdown.borrow()
    }

    fn has_credentials(&self) -> bool {
        self.creds.is_some() || crate::creds::load().is_ok()
    }

    async fn sleep_interruptible(&self, dur: Duration) -> bool {
        tokio::select! {
            _ = tokio::time::sleep(dur) => false,
            _ = wait_shutdown(self.shutdown.clone()) => true,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.transition(State::WaitingForNetwork, TransitionReason::Boot);

        while !self.shutting_down() {
            match self.state {
                State::Starting => {
                    self.transition(State::WaitingForNetwork, TransitionReason::Boot);
                }
                State::WaitingForNetwork => {
                    if NetworkMonitor::has_usable_ipv4() {
                        self.transition(State::CheckingInternet, TransitionReason::NetworkReady);
                    } else if self.sleep_interruptible(self.network_wait).await {
                        break;
                    }
                }
                State::CheckingInternet => match self.session.check_once().await {
                    Connectivity::Online => {
                        self.retry.reset();
                        // After reboot, probes can look "online" briefly while the
                        // hotspot session is already gone — always ensure login when
                        // credentials exist (no-op if already authenticated).
                        if self.has_credentials() {
                            self.transition(State::Authenticating, TransitionReason::EnsureSession);
                        } else {
                            self.transition(State::MonitoringSession, TransitionReason::InternetOk);
                        }
                    }
                    Connectivity::Captive => {
                        self.transition(
                            State::DetectingCaptivePortal,
                            TransitionReason::CaptiveOrBlocked,
                        );
                    }
                    Connectivity::Offline => {
                        self.transition(State::RetryWaiting, TransitionReason::TransientError);
                    }
                },
                State::DetectingCaptivePortal => match self.session.check_once().await {
                    Connectivity::Online => {
                        self.retry.reset();
                        if self.has_credentials() {
                            self.transition(State::Authenticating, TransitionReason::EnsureSession);
                        } else {
                            self.transition(State::MonitoringSession, TransitionReason::InternetOk);
                        }
                    }
                    Connectivity::Captive => {
                        self.transition(
                            State::AuthenticationRequired,
                            TransitionReason::PortalDetected,
                        );
                    }
                    Connectivity::Offline => {
                        self.transition(State::RetryWaiting, TransitionReason::TransientError);
                    }
                },
                State::AuthenticationRequired => {
                    self.transition(State::Authenticating, TransitionReason::PortalDetected);
                }
                State::Authenticating => {
                    self.handle_authenticate().await;
                }
                State::Authenticated => {
                    self.transition(State::MonitoringSession, TransitionReason::AuthOk);
                }
                State::MonitoringSession => {
                    if self
                        .sleep_interruptible(self.session.interval())
                        .await
                    {
                        break;
                    }
                    let prev = self
                        .session
                        .last_status()
                        .await
                        .unwrap_or(Connectivity::Online);
                    let cur = self.session.check_once().await;
                    match cur {
                        Connectivity::Captive => {
                            warn!("captive portal while monitoring; authenticating");
                            self.transition(
                                State::DetectingCaptivePortal,
                                TransitionReason::CaptiveOrBlocked,
                            );
                        }
                        Connectivity::Offline => {
                            warn!("went offline while monitoring");
                            self.transition(State::RetryWaiting, TransitionReason::TransientError);
                        }
                        Connectivity::Online => {
                            let _ = prev; // still online
                        }
                    }
                }
                State::RetryWaiting => {
                    if let Some(delay) = self.retry.next_delay(FailureKind::Transient) {
                        info!(?delay, attempt = self.retry.attempt(), "retry waiting");
                        if self.sleep_interruptible(delay).await {
                            break;
                        }
                        self.transition(State::CheckingInternet, TransitionReason::RetryElapsed);
                    } else {
                        error!("retry limit reached");
                        self.transition(
                            State::AuthenticationFailed,
                            TransitionReason::TransientError,
                        );
                    }
                }
                State::AuthenticationFailed => {
                    error!("authentication failed permanently for this run");
                    break;
                }
            }
        }

        self.transition(self.state, TransitionReason::Shutdown);
        Ok(())
    }

    async fn handle_authenticate(&mut self) {
        if !self.session.gate().try_enter() {
            warn!("authentication already in progress; skipping");
            self.transition(State::RetryWaiting, TransitionReason::TransientError);
            return;
        }

        let result = async {
            let creds = match &self.creds {
                Some(c) => c.clone(),
                None => match crate::creds::load() {
                    Ok(c) => c,
                    Err(e) => {
                        error!(error = %e, "credentials unavailable");
                        return Err(AuthError::NotConfigured);
                    }
                },
            };
            self.auth
                .authenticate(self.network.client(), &creds)
                .await
        }
        .await;

        self.session.gate().exit();

        match result {
            Ok(_) => {
                // Confirm with probes; do not trust auth HTTP status alone.
                match self.session.check_once().await {
                    Connectivity::Online => {
                        self.retry.reset();
                        self.transition(State::Authenticated, TransitionReason::AuthOk);
                    }
                    Connectivity::Captive => {
                        warn!("auth returned success but probes still captive");
                        self.transition(State::RetryWaiting, TransitionReason::TransientError);
                    }
                    Connectivity::Offline => {
                        self.transition(State::RetryWaiting, TransitionReason::TransientError);
                    }
                }
            }
            Err(AuthError::BadCredentials) => {
                error!("bad credentials; will not retry storm");
                let _ = self.retry.next_delay(FailureKind::BadCredentials);
                self.transition(State::AuthenticationFailed, TransitionReason::BadCredentials);
            }
            Err(AuthError::UserActionRequired) => {
                error!("CAPTCHA/MFA required — waiting for user");
                self.transition(State::AuthenticationFailed, TransitionReason::BadCredentials);
            }
            Err(AuthError::NotConfigured) => {
                error!("credentials missing — run: dormnet creds set");
                self.transition(State::AuthenticationFailed, TransitionReason::BadCredentials);
            }
            Err(AuthError::FalseSuccess) | Err(AuthError::Transient(_)) => {
                warn!(?result, "transient auth failure");
                self.transition(State::RetryWaiting, TransitionReason::TransientError);
            }
        }
    }
}

async fn wait_shutdown(mut rx: watch::Receiver<bool>) {
    loop {
        if *rx.borrow() {
            return;
        }
        if rx.changed().await.is_err() {
            return;
        }
    }
}
