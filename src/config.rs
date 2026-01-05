use std::net::IpAddr;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "risky-proxmox-agent", about = "Risky Proxmox Agent")]
pub struct CliArgs {
    /// Bind address for the HTTP server
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: IpAddr,
    /// Port for the HTTP server
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: IpAddr,
    pub port: u16,
    pub pve_host: String,
    pub pve_token_id: String,
    pub pve_token_secret: String,
    pub pve_insecure_ssl: bool,
    pub pve_fallback_vm: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();
        let args = CliArgs::parse();

        let pve_host = read_env("PVE_HOST")?;
        let pve_token_id = read_env("PVE_TOKEN_ID")?;
        let pve_token_secret = read_env("PVE_TOKEN_SECRET")?;
        let pve_insecure_ssl = read_env_bool("PVE_INSECURE_SSL").unwrap_or(false);
        let pve_fallback_vm = read_env_optional("PVE_FALLBACK_VM");

        Ok(Self {
            bind: args.bind,
            port: args.port,
            pve_host,
            pve_token_id,
            pve_token_secret,
            pve_insecure_ssl,
            pve_fallback_vm,
        })
    }
}

fn read_env(key: &str) -> Result<String, String> {
    std::env::var(key).map_err(|_| format!("Missing required env var: {key}"))
}

fn read_env_bool(key: &str) -> Option<bool> {
    std::env::var(key)
        .ok()
        .and_then(|value| match value.to_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
}

fn read_env_optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
