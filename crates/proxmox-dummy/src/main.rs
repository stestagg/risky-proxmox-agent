use std::net::SocketAddr;

use clap::Parser;
use tracing::info;

use proxmox_dummy::DummyHandle;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,
    #[arg(long, default_value_t = 0)]
    port: u16,
    #[arg(long, default_value = "pve")]
    node: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let handle = DummyHandle::new(args.node);
    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;
    info!("Dummy Proxmox server listening on {actual_addr}");
    handle.serve(listener).await?;
    Ok(())
}
