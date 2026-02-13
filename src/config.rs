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
    pub remote_log: Option<RemoteLogConfig>,
}

#[derive(Debug, Clone)]
pub struct RemoteLogConfig {
    pub upload_url: String,
    pub authorization_secret: String,
    pub max_pending_bytes: usize,
    pub max_upload_bytes: usize,
    pub upload_delay_secs: f64,
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
        let remote_log = read_remote_log_config()?;

        Ok(Self {
            bind: args.bind,
            port: args.port,
            pve_host,
            pve_token_id,
            pve_token_secret,
            pve_insecure_ssl,
            pve_fallback_vm,
            remote_log,
        })
    }
}

fn read_remote_log_config() -> Result<Option<RemoteLogConfig>, String> {
    let upload_url = read_env_optional("REMOTE_LOG_UPLOAD_URL");
    let authorization_secret = read_env_optional("REMOTE_LOG_AUTHORIZATION_SECRET");

    match (upload_url, authorization_secret) {
        (None, None) => Ok(None),
        (Some(upload_url), Some(authorization_secret)) => Ok(Some(RemoteLogConfig {
            upload_url,
            authorization_secret,
            max_pending_bytes: read_env_usize("REMOTE_LOG_MAX_PENDING_BYTES")
                .unwrap_or(50 * 1024 * 1024),
            max_upload_bytes: read_env_usize("REMOTE_LOG_MAX_UPLOAD_BYTES")
                .unwrap_or(5 * 1024 * 1024),
            upload_delay_secs: read_env_f64("REMOTE_LOG_UPLOAD_DELAY_SECS").unwrap_or(5.0),
        })),
        _ => Err(
            "REMOTE_LOG_UPLOAD_URL and REMOTE_LOG_AUTHORIZATION_SECRET must be set together"
                .to_string(),
        ),
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

fn read_env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
}

fn read_env_f64(key: &str) -> Option<f64> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
}
