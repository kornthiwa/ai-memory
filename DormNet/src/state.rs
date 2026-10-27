//! Explicit runtime states for the DormNet control loop.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Starting,
    WaitingForNetwork,
    CheckingInternet,
    DetectingCaptivePortal,
    AuthenticationRequired,
    Authenticating,
    Authenticated,
    MonitoringSession,
    RetryWaiting,
    AuthenticationFailed,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            State::Starting => "Starting",
            State::WaitingForNetwork => "WaitingForNetwork",
            State::CheckingInternet => "CheckingInternet",
            State::DetectingCaptivePortal => "DetectingCaptivePortal",
            State::AuthenticationRequired => "AuthenticationRequired",
            State::Authenticating => "Authenticating",
            State::Authenticated => "Authenticated",
            State::MonitoringSession => "MonitoringSession",
            State::RetryWaiting => "RetryWaiting",
            State::AuthenticationFailed => "AuthenticationFailed",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionReason {
    Boot,
    NetworkReady,
    InternetOk,
    /// Probes look online, but still confirm/refresh the hotspot session.
    EnsureSession,
    CaptiveOrBlocked,
    PortalDetected,
    TransientError,
    AuthOk,
    BadCredentials,
    SessionLost,
    RetryElapsed,
    Shutdown,
}

impl fmt::Display for TransitionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
