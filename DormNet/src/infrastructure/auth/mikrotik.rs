//! MikroTik Hotspot auth for Netprasomplace dorm network.
//!
//! Evidence (2026-10-27, live gateway):
//! - Gateway `1.1.1.1` → `http://netprasomplace.wifi/status` title "Hotspot Status"
//! - `/login` while authenticated returns title "mikrotik hotspot > redirect"
//!   and body "You are logged in"
//! - Logout form posts to `http://Netprasomplace.wifi/logout` (DormNet never calls this)
//!
//! Protocol: standard MikroTik Hotspot PAP form POST
//! (`username`, `password`, optional `dst`, `popup`).

use async_trait::async_trait;
use reqwest::Client;
use tracing::{info, warn};

use crate::domain::Credentials;

use super::{AuthError, AuthOutcome, AuthProvider};

pub const DEFAULT_LOGIN_URLS: &[&str] = &[
    "http://1.1.1.1/login",
    "http://netprasomplace.wifi/login",
    "http://Netprasomplace.wifi/login",
];

pub const STATUS_URLS: &[&str] = &[
    "http://netprasomplace.wifi/status",
    "http://Netprasomplace.wifi/status",
    "http://1.1.1.1/status",
];

#[derive(Debug, Clone)]
pub struct MikroTikHotspotAuth {
    pub login_urls: Vec<String>,
    pub status_urls: Vec<String>,
}

impl Default for MikroTikHotspotAuth {
    fn default() -> Self {
        Self {
            login_urls: DEFAULT_LOGIN_URLS.iter().map(|s| (*s).to_string()).collect(),
            status_urls: STATUS_URLS.iter().map(|s| (*s).to_string()).collect(),
        }
    }
}

#[async_trait]
impl AuthProvider for MikroTikHotspotAuth {
    async fn authenticate(
        &self,
        client: &Client,
        creds: &Credentials,
    ) -> Result<AuthOutcome, AuthError> {
        // Already authenticated?
        if self.is_logged_in(client).await {
            info!("mikrotik status already authenticated");
            return Ok(AuthOutcome::Success);
        }

        let mut last_err = AuthError::Transient("no login url reachable".into());
        for url in &self.login_urls {
            match self.post_login(client, url, creds).await {
                Ok(AuthOutcome::Success) => {
                    if self.is_logged_in(client).await {
                        return Ok(AuthOutcome::Success);
                    }
                    // Some firmwares only flip after a moment; caller also re-probes internet.
                    info!(%url, "login POST accepted; status not yet confirmed");
                    return Ok(AuthOutcome::Success);
                }
                Err(AuthError::BadCredentials) => return Err(AuthError::BadCredentials),
                Err(AuthError::UserActionRequired) => return Err(AuthError::UserActionRequired),
                Err(e) => {
                    warn!(%url, error = %e, "login attempt failed; trying next url");
                    last_err = e;
                }
            }
        }
        Err(last_err)
    }
}

impl MikroTikHotspotAuth {
    async fn post_login(
        &self,
        client: &Client,
        url: &str,
        creds: &Credentials,
    ) -> Result<AuthOutcome, AuthError> {
        let body = format!(
            "username={}&password={}&dst=&popup=true",
            form_encode(&creds.username),
            form_encode(&creds.password)
        );

        info!(%url, username = %creds.username, "posting mikrotik hotspot login");

        let resp = client
            .post(url)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .header(reqwest::header::REFERER, url)
            .body(body)
            .send()
            .await
            .map_err(|e| AuthError::Transient(e.to_string()))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| AuthError::Transient(e.to_string()))?;
        let lower = text.to_ascii_lowercase();

        if looks_like_bad_credentials(&lower) {
            return Err(AuthError::BadCredentials);
        }
        if lower.contains("chap") && lower.contains("challenge") && !lower.contains("you are logged in")
        {
            return Err(AuthError::UserActionRequired);
        }
        if lower.contains("you are logged in")
            || lower.contains("hotspot status")
            || lower.contains("alogin")
        {
            return Ok(AuthOutcome::Success);
        }
        // MikroTik often 302/200 to status or login redirect page.
        if status.is_redirection() || status.is_success() {
            // Ambiguous: let caller confirm via status/probes.
            return Ok(AuthOutcome::Success);
        }
        Err(AuthError::Transient(format!(
            "unexpected login status {status}"
        )))
    }

    pub async fn is_logged_in(&self, client: &Client) -> bool {
        for url in &self.status_urls {
            if let Ok(resp) = client.get(url).send().await {
                if let Ok(text) = resp.text().await {
                    let lower = text.to_ascii_lowercase();
                    if lower.contains("you are logged in")
                        || (lower.contains("log off") && lower.contains("logout"))
                    {
                        return true;
                    }
                    // Unauthenticated login page usually has password field.
                    if lower.contains("name=\"password\"") || lower.contains("name='password'") {
                        return false;
                    }
                }
            }
        }
        // Fallback: try login URL — authenticated session returns redirect page.
        for url in &self.login_urls {
            if let Ok(resp) = client.get(url).send().await {
                if let Ok(text) = resp.text().await {
                    let lower = text.to_ascii_lowercase();
                    if lower.contains("you are logged in") {
                        return true;
                    }
                }
            }
        }
        false
    }
}

fn form_encode(s: &str) -> String {
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

fn looks_like_bad_credentials(lower: &str) -> bool {
    lower.contains("invalid username")
        || lower.contains("invalid password")
        || lower.contains("wrong password")
        || lower.contains("login failed")
        || lower.contains("error: invalid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_form_fields() {
        assert_eq!(form_encode("a b"), "a+b");
        assert_eq!(form_encode("p@ss"), "p%40ss");
    }

    #[test]
    fn detects_bad_password_copy() {
        assert!(looks_like_bad_credentials("invalid username or password"));
        assert!(!looks_like_bad_credentials("you are logged in"));
    }
}
