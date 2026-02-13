use risky_proxmox_agent::config::Config;
use risky_proxmox_agent::fallback::spawn_fallback_task;
use risky_proxmox_agent::proxmox::ProxmoxClient;
use risky_proxmox_agent::remote_log::{RemoteLogHandle, RemoteLogMakeWriter};
use risky_proxmox_agent::server::{router, AppState};
use tracing::info;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env().map_err(|err| {
        eprintln!("{err}");
        err
    })?;

    let env_filter = tracing_subscriber::EnvFilter::from_default_env();
    let stdout_layer = tracing_subscriber::fmt::layer().with_filter(env_filter.clone());

    if let Some(remote_config) = config.remote_log.clone() {
        let remote = RemoteLogHandle::new(remote_config);
        remote.spawn_upload_loop();

        let remote_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .with_writer(RemoteLogMakeWriter::new(remote))
            .with_filter(env_filter);

        tracing_subscriber::registry()
            .with(stdout_layer)
            .with(remote_layer)
            .init();
    } else {
        tracing_subscriber::registry().with(stdout_layer).init();
    }

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
