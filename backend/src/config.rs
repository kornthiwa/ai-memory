use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let database_url =
            std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

        let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());

        let port = match std::env::var("PORT") {
            Ok(value) => value.parse().context("PORT must be a valid u16")?,
            Err(_) => 3000,
        };

        if database_url.is_empty() {
            bail!("DATABASE_URL must not be empty");
        }

        Ok(Self {
            database_url,
            host,
            port,
        })
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
