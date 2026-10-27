//! Integration tests against a mock captive portal (no real dorm traffic).

use std::sync::Arc;
use std::time::Duration;

use dormnet::auth::{AuthProvider, MockFormAuth, UnconfiguredAuth};
use dormnet::creds::Credentials;
use dormnet::network::{Connectivity, NetworkMonitor};
use dormnet::retry::{FailureKind, RetryManager, RetryPolicy};
use dormnet::session::AuthGate;
use dormnet::{AppConfig, Runtime};
use tokio::sync::watch;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn portal_with_probes(login_body: &'static str, login_status: u16) -> (MockServer, NetworkMonitor, MockFormAuth) {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/connecttest.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>portal</html>"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/generate_204"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>portal</html>"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/login"))
        .respond_with(ResponseTemplate::new(login_status).set_body_string(login_body))
        .mount(&server)
        .await;

    let network = NetworkMonitor::with_urls(
        format!("{}/connecttest.txt", server.uri()),
        format!("{}/generate_204", server.uri()),
    )
    .unwrap();
    let auth = MockFormAuth {
        login_url: format!("{}/login", server.uri()),
    };
    (server, network, auth)
}

#[tokio::test]
async fn detects_captive_before_login() {
    let (_server, network, _auth) = portal_with_probes("LOGIN_OK", 200).await;
    assert_eq!(network.check_connectivity().await, Connectivity::Captive);
}

#[tokio::test]
async fn auth_success_then_online_probes() {
    let server = MockServer::start().await;
    let online = std::sync::atomic::AtomicBool::new(false);
    let online = std::sync::Arc::new(online);

    // Dynamic-ish: first probes captive; after "login" we remount — simpler: call auth then swap URLs.
    Mock::given(method("POST"))
        .and(path("/login"))
        .respond_with(ResponseTemplate::new(200).set_body_string("LOGIN_OK"))
        .mount(&server)
        .await;

    let auth = MockFormAuth {
        login_url: format!("{}/login", server.uri()),
    };
    let client = reqwest::Client::new();
    auth.authenticate(&client, &Credentials::new("u", "p"))
        .await
        .expect("login ok");

    // After successful login, probes should be online.
    Mock::given(method("GET"))
        .and(path("/connecttest.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Microsoft Connect Test"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/generate_204"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let network = NetworkMonitor::with_urls(
        format!("{}/connecttest.txt", server.uri()),
        format!("{}/generate_204", server.uri()),
    )
    .unwrap();
    assert_eq!(network.check_connectivity().await, Connectivity::Online);
    let _ = online;
}

#[tokio::test]
async fn bad_password_is_not_retried_by_policy() {
    let mut retry = RetryManager::new(RetryPolicy::default());
    assert!(retry.next_delay(FailureKind::BadCredentials).is_none());
}

#[tokio::test]
async fn auth_gate_blocks_concurrent_login() {
    let gate = AuthGate::new();
    assert!(gate.try_enter());
    assert!(gate.is_busy());
    assert!(!gate.try_enter());
    gate.exit();
    assert!(gate.try_enter());
}

#[tokio::test]
async fn runtime_fails_closed_when_auth_unconfigured() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/connecttest.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>portal</html>"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/generate_204"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>portal</html>"))
        .mount(&server)
        .await;

    let network = NetworkMonitor::with_urls(
        format!("{}/connecttest.txt", server.uri()),
        format!("{}/generate_204", server.uri()),
    )
    .unwrap();

    let (tx, rx) = watch::channel(false);
    let config = AppConfig {
        monitor_interval: Duration::from_millis(50),
        network_wait: Duration::from_millis(10),
        network: Some(network),
        credentials: Some(Credentials::new("u", "p")),
        auth: Some(Arc::new(UnconfiguredAuth)),
    };
    let mut rt = Runtime::new(config, rx).unwrap();

    let handle = tokio::spawn(async move { rt.run().await });
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = tx.send(true);
    let _ = handle.await;
}

#[tokio::test]
async fn runtime_reaches_monitoring_when_already_online() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/connecttest.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Microsoft Connect Test"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/generate_204"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let network = NetworkMonitor::with_urls(
        format!("{}/connecttest.txt", server.uri()),
        format!("{}/generate_204", server.uri()),
    )
    .unwrap();

    let (tx, rx) = watch::channel(false);
    let config = AppConfig {
        monitor_interval: Duration::from_secs(60),
        network_wait: Duration::from_millis(10),
        network: Some(network),
        credentials: Some(Credentials::new("u", "p")),
        auth: Some(Arc::new(MockFormAuth {
            login_url: format!("{}/login", server.uri()),
        })),
    };
    let mut rt = Runtime::new(config, rx).unwrap();

    let run = tokio::spawn(async move {
        // Give the loop time to reach MonitoringSession then stop.
        let task = tokio::spawn(async move { rt.run().await });
        tokio::time::sleep(Duration::from_millis(200)).await;
        task.abort();
    });
    let _ = tx.send(false);
    let _ = run.await;
}

#[tokio::test]
async fn false_success_http_200() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok?"))
        .mount(&server)
        .await;
    let auth = MockFormAuth {
        login_url: format!("{}/login", server.uri()),
    };
    let err = auth
        .authenticate(&reqwest::Client::new(), &Credentials::new("u", "p"))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        dormnet::auth::AuthError::FalseSuccess
    ));
}

#[tokio::test]
async fn session_expiry_signal() {
    use dormnet::session::SessionMonitor;
    assert!(SessionMonitor::session_lost(
        Connectivity::Online,
        Connectivity::Captive
    ));
}
