mod config;
mod proxmox;
mod server;

use tracing::info;

use crate::config::Config;
use crate::proxmox::ProxmoxClient;
use crate::server::{router, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env().map_err(|err| {
        tracing::error!("{err}");
        err
    })?;

    let client = ProxmoxClient::new(
        config.pve_host,
        &config.pve_token_id,
        &config.pve_token_secret,
        config.pve_insecure_ssl,
    )?;

    let app = router(AppState::new(client));

    let addr = std::net::SocketAddr::from((config.bind, config.port));
    info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
