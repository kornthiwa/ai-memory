//! DormNet Windows background agent + credential GUI.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    dormnet::presentation::cli::run().await
}
