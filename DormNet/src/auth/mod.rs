//! Authentication providers.

pub mod mikrotik;

use async_trait::async_trait;
use reqwest::Client;
use thiserror::Error;

use crate::creds::Credentials;

pub use mikrotik::MikroTikHotspotAuth;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("invalid username or password")]
    BadCredentials,
    #[error("server reported success status but login did not take effect")]
    FalseSuccess,
    #[error("transient authentication failure: {0}")]
    Transient(String),
    #[error("CAPTCHA or MFA required — user action needed")]
    UserActionRequired,
    #[error("configuration incomplete: dorm login endpoint unknown (capture HAR first)")]
    NotConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthOutcome {
    Success,
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn authenticate(&self, client: &Client, creds: &Credentials) -> Result<AuthOutcome, AuthError>;
}

/// Placeholder until `docs/login-analysis.md` is filled from a sanitized HAR.
pub struct UnconfiguredAuth;

#[async_trait]
impl AuthProvider for UnconfiguredAuth {
    async fn authenticate(
        &self,
        _client: &Client,
        _creds: &Credentials,
    ) -> Result<AuthOutcome, AuthError> {
        Err(AuthError::NotConfigured)
    }
}

/// HTTP form login against a mock/test portal.
///
/// Contract used by tests:
/// - POST `login_url` as `application/x-www-form-urlencoded` with `username` + `password`
/// - Success: HTTP 200 and body contains `LOGIN_OK` **or** sets cookie `session=ok`
/// - Bad credentials: HTTP 401/403 or body contains `LOGIN_FAIL`
/// - False success: HTTP 200 without success marker
pub struct MockFormAuth {
    pub login_url: String,
}

#[async_trait]
impl AuthProvider for MockFormAuth {
    async fn authenticate(
        &self,
        client: &Client,
        creds: &Credentials,
    ) -> Result<AuthOutcome, AuthError> {
        let resp = client
            .post(&self.login_url)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(format!(
                "username={}&password={}",
                urlencoding_minimal(&creds.username),
                urlencoding_minimal(&creds.password)
            ))
            .send()
            .await
            .map_err(|e| AuthError::Transient(e.to_string()))?;

        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp
            .text()
            .await
            .map_err(|e| AuthError::Transient(e.to_string()))?;

        if status.as_u16() == 401 || status.as_u16() == 403 || body.contains("LOGIN_FAIL") {
            return Err(AuthError::BadCredentials);
        }

        if body.contains("CAPTCHA") || body.contains("MFA") {
            return Err(AuthError::UserActionRequired);
        }

        let cookie_ok = headers
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .any(|c| c.contains("session=ok"));

        if status.is_success() && (body.contains("LOGIN_OK") || cookie_ok) {
            return Ok(AuthOutcome::Success);
        }

        if status.is_success() {
            return Err(AuthError::FalseSuccess);
        }

        if status.is_server_error() {
            return Err(AuthError::Transient(format!("server status {status}")));
        }

        Err(AuthError::Transient(format!("unexpected status {status}")))
    }
}

fn urlencoding_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client() -> Client {
        Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn login_success_with_marker() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login"))
            .and(body_string_contains("username=alice"))
            .respond_with(ResponseTemplate::new(200).set_body_string("LOGIN_OK"))
            .mount(&server)
            .await;

        let auth = MockFormAuth {
            login_url: format!("{}/login", server.uri()),
        };
        let outcome = auth
            .authenticate(&client(), &Credentials::new("alice", "pw"))
            .await
            .unwrap();
        assert_eq!(outcome, AuthOutcome::Success);
    }

    #[tokio::test]
    async fn bad_credentials() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login"))
            .respond_with(ResponseTemplate::new(200).set_body_string("LOGIN_FAIL"))
            .mount(&server)
            .await;

        let auth = MockFormAuth {
            login_url: format!("{}/login", server.uri()),
        };
        let err = auth
            .authenticate(&client(), &Credentials::new("alice", "bad"))
            .await
            .unwrap_err();
        assert_eq!(err, AuthError::BadCredentials);
    }

    #[tokio::test]
    async fn http_200_without_success_marker_is_false_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login"))
            .respond_with(ResponseTemplate::new(200).set_body_string("welcome?"))
            .mount(&server)
            .await;

        let auth = MockFormAuth {
            login_url: format!("{}/login", server.uri()),
        };
        let err = auth
            .authenticate(&client(), &Credentials::new("alice", "pw"))
            .await
            .unwrap_err();
        assert_eq!(err, AuthError::FalseSuccess);
    }

    #[tokio::test]
    async fn unconfigured_provider_errors() {
        let err = UnconfiguredAuth
            .authenticate(&client(), &Credentials::new("a", "b"))
            .await
            .unwrap_err();
        assert_eq!(err, AuthError::NotConfigured);
    }
}
