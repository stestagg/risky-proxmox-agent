use tracing::info;

use risky_proxmox_agent::config::Config;
use risky_proxmox_agent::fallback::spawn_fallback_task;
use risky_proxmox_agent::proxmox::ProxmoxClient;
use risky_proxmox_agent::server::{router, AppState};

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

    if let Some(fallback_name) = config.pve_fallback_vm.clone() {
        spawn_fallback_task(client.clone(), fallback_name);
    }

    let app = router(AppState::new(client));

    let addr = std::net::SocketAddr::from((config.bind, config.port));
    info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
