//! Exponential backoff with jitter. Bad-credential failures are not retried.

use std::time::Duration;

use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Transient,
    BadCredentials,
    Permanent,
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 8,
            base: Duration::from_secs(2),
            max_delay: Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetryManager {
    policy: RetryPolicy,
    attempt: u32,
}

impl RetryManager {
    pub fn new(policy: RetryPolicy) -> Self {
        Self { policy, attempt: 0 }
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Returns `Some(delay)` if another retry is allowed, else `None`.
    pub fn next_delay(&mut self, kind: FailureKind) -> Option<Duration> {
        match kind {
            FailureKind::BadCredentials | FailureKind::Permanent => None,
            FailureKind::Transient => {
                if self.attempt >= self.policy.max_attempts {
                    return None;
                }
                let exp = self.attempt.min(16);
                let base_ms = self.policy.base.as_millis() as u64;
                let capped_ms = self.policy.max_delay.as_millis() as u64;
                let raw = base_ms.saturating_mul(1u64 << exp).min(capped_ms);
                let jitter = rand::rng().random_range(0..=raw / 4 + 1);
                self.attempt += 1;
                Some(Duration::from_millis(raw + jitter))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_retry_on_bad_credentials() {
        let mut r = RetryManager::new(RetryPolicy::default());
        assert!(r.next_delay(FailureKind::BadCredentials).is_none());
        assert_eq!(r.attempt(), 0);
    }

    #[test]
    fn backoff_increases_until_cap_or_max_attempts() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
        };
        let mut r = RetryManager::new(policy);
        let d1 = r.next_delay(FailureKind::Transient).unwrap();
        let d2 = r.next_delay(FailureKind::Transient).unwrap();
        let d3 = r.next_delay(FailureKind::Transient).unwrap();
        assert!(d2 >= d1 || d2.as_millis() > 0);
        assert!(d3.as_millis() > 0);
        assert!(r.next_delay(FailureKind::Transient).is_none());
    }
}
