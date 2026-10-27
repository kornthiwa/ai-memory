# DormNet Architecture

## Goals

Background Windows app that detects dorm captive portals, authenticates when required, monitors the session, and recovers from transient failures — with low resource use and no secret leakage.

## Components

| Module | Responsibility |
|--------|----------------|
| `network` | Adapter readiness, HTTP connectivity probes, captive-portal heuristics |
| `auth` | `AuthProvider` trait; mock impl for tests; dorm-specific impl only after HAR evidence |
| `session` | Periodic re-check; avoid concurrent duplicate logins |
| `retry` | Exponential backoff with jitter; no retry on bad credentials |
| `creds` | Username/password via Windows Credential Manager |
| `logutil` | Structured tracing, rotating files, secret redaction |
| `single_instance` | Named mutex so only one process runs |
| `state` | Explicit state machine transitions |

## Connectivity probes

| Probe | Expected when online |
|-------|----------------------|
| `http://www.msftconnecttest.com/connecttest.txt` | Body contains `Microsoft Connect Test` |
| `http://connectivitycheck.gstatic.com/generate_204` | HTTP 204 |

Captive / blocked signals (heuristic, not every redirect):

- Unexpected HTML body on the Microsoft test URL
- Non-204 / redirect landing on a non-probe host for `generate_204`
- Timeout / DNS failure treated as transient network errors

## State machine

```
Starting
  -> WaitingForNetwork
  -> CheckingInternet
       -> MonitoringSession          (internet OK)
       -> DetectingCaptivePortal     (captive or blocked)
            -> AuthenticationRequired
                 -> Authenticating
                      -> MonitoringSession       (auth OK)
                      -> AuthenticationFailed    (bad credentials; stop retry storm)
                      -> RetryWaiting            (transient)
            -> RetryWaiting                      (transient detect errors)
  MonitoringSession -> DetectingCaptivePortal    (session lost)
  RetryWaiting -> CheckingInternet
```

Rules:

- Only one authentication attempt at a time.
- Bad credentials do not use the transient retry loop.
- Shutdown (Ctrl+C / process exit) leaves the machine cleanly from any state.
- Success after auth is confirmed by re-running connectivity probes, not by HTTP 200 alone.

## Startup model

- Release binary uses `windows_subsystem = "windows"` (no console window).
- Auto-start via Task Scheduler **At logon** for the interactive user (`scripts/install-task.ps1`).
- Not a Windows Service in Phase 1.
- Credential access requires the user session (logon trigger).

## Testing strategy

- Unit/integration tests use an in-process mock portal (`wiremock` / local HTTP).
- Tests must never send credentials to the real dorm network.
- Dorm-specific auth lands in Phase 2 after `docs/login-analysis.md` is complete.
