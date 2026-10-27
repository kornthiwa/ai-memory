//! Domain credential value object (no storage I/O).

use serde::{Deserialize, Serialize};

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
