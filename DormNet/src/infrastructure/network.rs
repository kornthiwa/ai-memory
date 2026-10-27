//! Network readiness and captive-portal detection via well-known probes.

use std::time::Duration;

use reqwest::{Client, StatusCode, redirect::Policy};
use thiserror::Error;

pub const MSFT_CONNECT_URL: &str = "http://www.msftconnecttest.com/connecttest.txt";
pub const MSFT_CONNECT_BODY: &str = "Microsoft Connect Test";
pub const GSTATIC_204_URL: &str = "http://connectivitycheck.gstatic.com/generate_204";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connectivity {
    /// Both probes matched expected online behavior.
    Online,
    /// Likely captive portal or filtered access.
    Captive,
    /// Adapter/DNS/timeout style failure.
    Offline,
}

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected probe response")]
    Unexpected,
}

#[derive(Debug, Clone)]
pub struct NetworkMonitor {
    client: Client,
    msft_url: String,
    gstatic_url: String,
}

impl NetworkMonitor {
    pub fn new() -> Result<Self, NetworkError> {
        Self::with_urls(MSFT_CONNECT_URL, GSTATIC_204_URL)
    }

    pub fn with_urls(msft_url: impl Into<String>, gstatic_url: impl Into<String>) -> Result<Self, NetworkError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(8))
            .redirect(Policy::none())
            .cookie_store(true)
            .no_proxy()
            .build()?;
        Ok(Self {
            client,
            msft_url: msft_url.into(),
            gstatic_url: gstatic_url.into(),
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Best-effort check that at least one non-loopback IPv4 address exists.
    pub fn has_usable_ipv4() -> bool {
        #[cfg(windows)]
        {
            // Lightweight check via std; detailed adapter enumeration is out of scope for Phase 1.
            std::net::UdpSocket::bind("0.0.0.0:0")
                .and_then(|s| {
                    s.connect("8.8.8.8:80")?;
                    Ok(s.local_addr()?.ip())
                })
                .map(|ip| match ip {
                    std::net::IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_unspecified(),
                    _ => false,
                })
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            true
        }
    }

    pub async fn check_connectivity(&self) -> Connectivity {
        let msft = self.probe_msft().await;
        let gstatic = self.probe_gstatic().await;

        match (msft, gstatic) {
            (Ok(true), Ok(true)) => Connectivity::Online,
            (Ok(false), _) | (_, Ok(false)) => Connectivity::Captive,
            (Err(_), Err(_)) => Connectivity::Offline,
            // One probe failed transiently but the other looked captive.
            (Ok(true), Err(_)) | (Err(_), Ok(true)) => Connectivity::Offline,
        }
    }

    async fn probe_msft(&self) -> Result<bool, NetworkError> {
        let resp = self.client.get(&self.msft_url).send().await?;
        if resp.status() != StatusCode::OK {
            // Redirects (3xx) with Policy::none often indicate a portal.
            if resp.status().is_redirection() {
                return Ok(false);
            }
            return Ok(false);
        }
        let body = resp.text().await?;
        let trimmed = body.trim();
        if trimmed == MSFT_CONNECT_BODY || trimmed.contains(MSFT_CONNECT_BODY) {
            Ok(true)
        } else {
            // HTML login page masquerading as 200.
            Ok(false)
        }
    }

    async fn probe_gstatic(&self) -> Result<bool, NetworkError> {
        let resp = self.client.get(&self.gstatic_url).send().await?;
        if resp.status() == StatusCode::NO_CONTENT {
            return Ok(true);
        }
        if resp.status().is_redirection() || resp.status().is_success() {
            // 200 with body / redirect → typically captive.
            return Ok(false);
        }
        Err(NetworkError::Unexpected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn online_when_both_probes_match() {
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

        let mon = NetworkMonitor::with_urls(
            format!("{}/connecttest.txt", server.uri()),
            format!("{}/generate_204", server.uri()),
        )
        .unwrap();
        assert_eq!(mon.check_connectivity().await, Connectivity::Online);
    }

    #[tokio::test]
    async fn captive_when_msft_returns_html() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/connecttest.txt"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>login</html>"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/generate_204"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>portal</html>"))
            .mount(&server)
            .await;

        let mon = NetworkMonitor::with_urls(
            format!("{}/connecttest.txt", server.uri()),
            format!("{}/generate_204", server.uri()),
        )
        .unwrap();
        assert_eq!(mon.check_connectivity().await, Connectivity::Captive);
    }

    #[tokio::test]
    async fn offline_when_server_down() {
        let mon = NetworkMonitor::with_urls(
            "http://127.0.0.1:1/connecttest.txt",
            "http://127.0.0.1:1/generate_204",
        )
        .unwrap();
        assert_eq!(mon.check_connectivity().await, Connectivity::Offline);
    }
}
