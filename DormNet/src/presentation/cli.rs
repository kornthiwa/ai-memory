//! CLI entry: gui / run / login / creds.

use std::io::{self, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tokio::sync::watch;
use tracing::{error, info};

use crate::application::{AppConfig, Runtime};
use crate::domain::Credentials;
use crate::infrastructure::auth::{AuthProvider, MikroTikHotspotAuth};
use crate::infrastructure::credentials_store;
use crate::infrastructure::logging;
use crate::infrastructure::network::NetworkMonitor;
use crate::infrastructure::single_instance;
use crate::presentation::gui;

#[derive(Parser, Debug)]
#[command(name = "dormnet", about = "Dormitory MikroTik hotspot auto login")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Open credential GUI (default when double-clicked).
    Gui,
    /// Run the background monitor loop.
    Run {
        /// Seconds between session probes while online.
        #[arg(long, default_value_t = 60)]
        interval: u64,
    },
    /// One-shot login using stored credentials (does not call logout).
    Login,
    /// Manage credentials in Windows Credential Manager (CLI).
    Creds {
        #[command(subcommand)]
        action: CredsCmd,
    },
}

#[derive(Subcommand, Debug)]
enum CredsCmd {
    /// Store username/password (prompted).
    Set {
        #[arg(long)]
        username: Option<String>,
    },
    /// Delete stored credentials.
    Clear,
}

/// Parse argv and dispatch to GUI / monitor / one-shot login / creds CLI.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Gui) {
        Commands::Gui => {
            gui::run().map_err(|e| anyhow::anyhow!("gui error: {e}"))?;
            Ok(())
        }
        Commands::Creds { action } => {
            ensure_console();
            match action {
                CredsCmd::Set { username } => creds_set(username)?,
                CredsCmd::Clear => {
                    credentials_store::delete()?;
                    println!("credentials cleared");
                }
            }
            Ok(())
        }
        Commands::Login => {
            ensure_console();
            one_shot_login().await
        }
        Commands::Run { interval } => run_loop(interval).await,
    }
}

async fn one_shot_login() -> Result<()> {
    let _guard = logging::init(None).ok();
    let creds =
        credentials_store::load().context("load credentials (open GUI or: dormnet creds set)")?;
    let network = NetworkMonitor::new().context("network client")?;
    let auth = MikroTikHotspotAuth::default();
    match auth.authenticate(network.client(), &creds).await {
        Ok(_) => {
            println!("login OK (or already logged in)");
            Ok(())
        }
        Err(e) => {
            eprintln!("login failed: {e}");
            Err(anyhow::anyhow!(e))
        }
    }
}

async fn run_loop(interval: u64) -> Result<()> {
    let _lock = single_instance::acquire_default().context("single-instance lock")?;
    let _guard = logging::init(None).context("init logging")?;

    info!(version = env!("CARGO_PKG_VERSION"), "DormNet starting");

    let (tx, rx) = watch::channel(false);
    install_ctrl_c(tx);

    let config = AppConfig {
        monitor_interval: Duration::from_secs(interval),
        ..AppConfig::default()
    };

    let mut runtime = Runtime::new(config, rx)?;
    if let Err(e) = runtime.run().await {
        error!(error = %e, "runtime exited with error");
        return Err(e);
    }
    info!("DormNet stopped");
    Ok(())
}

fn creds_set(username: Option<String>) -> Result<()> {
    let username = match username {
        Some(u) => u,
        None => {
            print!("username: ");
            io::stdout().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            buf.trim().to_string()
        }
    };
    print!("password: ");
    io::stdout().flush()?;
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    let password = password.trim_end_matches(['\r', '\n']).to_string();

    credentials_store::store(&Credentials::new(username, password))?;
    println!("credentials stored in Windows Credential Manager (service=DormNet)");
    Ok(())
}

fn install_ctrl_c(tx: watch::Sender<bool>) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = tx.send(true);
        }
    });
}

fn ensure_console() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{
            AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS,
        };
        unsafe {
            if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
                let _ = AllocConsole();
            }
        }
    }
}
