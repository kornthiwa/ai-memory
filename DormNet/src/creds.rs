//! Secure credential storage via Windows Credential Manager (keyring).

use anyhow::{bail, Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "DormNet";
const USER_ACCOUNT: &str = "dorm-username";
const PASS_ACCOUNT: &str = "dorm-password";

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .finish()
    }
}

impl Credentials {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

fn user_entry() -> Result<Entry> {
    Entry::new(SERVICE, USER_ACCOUNT).context("open username credential entry")
}

fn pass_entry() -> Result<Entry> {
    Entry::new(SERVICE, PASS_ACCOUNT).context("open password credential entry")
}

pub fn store(creds: &Credentials) -> Result<()> {
    if creds.username.trim().is_empty() {
        bail!("username must not be empty");
    }
    user_entry()?
        .set_password(&creds.username)
        .context("store username")?;
    pass_entry()?
        .set_password(&creds.password)
        .context("store password")?;
    Ok(())
}

pub fn load() -> Result<Credentials> {
    let username = user_entry()?
        .get_password()
        .context("load username (run: dormnet creds set)")?;
    let password = pass_entry()?
        .get_password()
        .context("load password (run: dormnet creds set)")?;
    Ok(Credentials { username, password })
}

pub fn delete() -> Result<()> {
    let _ = user_entry()?.delete_credential();
    let _ = pass_entry()?.delete_credential();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_debug_redacts_password() {
        let c = Credentials::new("alice", "secret");
        assert_eq!(c.username, "alice");
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("secret"));
        assert!(dbg.contains("[redacted]"));
    }
}
