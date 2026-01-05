mod config;
mod proxmox;

use axum::{routing::get, Router};
use tracing::info;

use crate::config::Config;

const INDEX_HTML: &str = include_str!("../templates/index.html");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env().map_err(|err| {
        tracing::error!("{err}");
        err
    })?;

    let app = Router::new().route("/", get(index));

    let addr = std::net::SocketAddr::from((config.bind, config.port));
    info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index() -> axum::response::Html<&'static str> {
    axum::response::Html(INDEX_HTML)
}
