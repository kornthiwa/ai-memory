//! Structured logging with rotation and basic secret redaction.

use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Patterns that must never appear in logs as-is.
const REDACT_HINTS: &[&str] = &["password=", "passwd=", "pwd=", "authorization:", "cookie:"];

pub fn redact(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    for hint in REDACT_HINTS {
        if lower.contains(hint) {
            return "[redacted]".to_string();
        }
    }
    input.to_string()
}

pub fn default_log_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().context("local data dir unavailable")?;
    Ok(base.join("DormNet").join("logs"))
}

/// Initialize stdout + rotating file logging. Keep the returned guard alive for the process lifetime.
pub fn init(log_dir: Option<PathBuf>) -> Result<WorkerGuard> {
    let dir = match log_dir {
        Some(p) => p,
        None => default_log_dir()?,
    };
    std::fs::create_dir_all(&dir).with_context(|| format!("create log dir {}", dir.display()))?;

    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("dormnet")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&dir)
        .context("configure rolling file appender")?;

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let console = fmt::layer().with_target(false).with_writer(std::io::stderr);
    let file = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(filter)
        .with(console)
        .with(file)
        .init();

    tracing::info!(path = %dir.display(), "logging initialized");
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_password_query() {
        assert_eq!(redact("user=a&password=secret"), "[redacted]");
    }

    #[test]
    fn keeps_safe_messages() {
        assert_eq!(redact("login succeeded"), "login succeeded");
    }
}
