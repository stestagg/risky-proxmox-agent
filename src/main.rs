use risky_proxmox_agent::config::Config;
use risky_proxmox_agent::fallback::spawn_fallback_task;
use risky_proxmox_agent::proxmox::ProxmoxClient;
use risky_proxmox_agent::remote_log::{RemoteLogHandle, RemoteLogMakeWriter};
use risky_proxmox_agent::server::{router, AppState};
use tracing::{debug, info};
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
        info!("Remote log forwarding enabled");
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

    info!(
        bind = %config.bind,
        port = config.port,
        pve_host = %config.pve_host,
        insecure_ssl = config.pve_insecure_ssl,
        fallback_vm = ?config.pve_fallback_vm,
        remote_log_enabled = config.remote_log.is_some(),
        "Configuration loaded"
    );
    debug!("Tracing initialized");

    let client = ProxmoxClient::new(
        config.pve_host,
        &config.pve_token_id,
        &config.pve_token_secret,
        config.pve_insecure_ssl,
    )?;
    info!("Proxmox client initialized");

    if let Some(fallback_name) = config.pve_fallback_vm.clone() {
        info!(fallback_vm = %fallback_name, "Starting fallback monitoring task");
        spawn_fallback_task(client.clone(), fallback_name);
    } else {
        info!("Fallback monitoring task disabled");
    }

    let app = router(AppState::new(client));
    info!("HTTP routes initialized");

    let addr = std::net::SocketAddr::from((config.bind, config.port));
    info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("TCP listener bound successfully");
    axum::serve(listener, app).await?;

    Ok(())
}
