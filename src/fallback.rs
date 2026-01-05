use std::time::Duration;

use tokio::time::{interval, sleep};
use tracing::{info, warn};

use crate::proxmox::types::VmStatus;
use crate::proxmox::ProxmoxClient;

const FALLBACK_POLL_INTERVAL: Duration = Duration::from_secs(30);
const FALLBACK_RECHECK_DELAY: Duration = Duration::from_secs(10);

pub fn spawn_fallback_task(client: ProxmoxClient, fallback_name: String) {
    tokio::spawn(async move {
        info!("Fallback VM polling enabled for '{}'", fallback_name);
        let mut ticker = interval(FALLBACK_POLL_INTERVAL);
        loop {
            ticker.tick().await;
            if let Err(err) = poll_and_start(&client, &fallback_name).await {
                warn!("Fallback VM poll failed: {err}");
            }
        }
    });
}

async fn poll_and_start(
    client: &ProxmoxClient,
    fallback_name: &str,
) -> Result<(), crate::proxmox::error::ProxmoxError> {
    let vms = client.list_vms().await?;
    if vms.iter().any(|vm| vm.status == VmStatus::Running) {
        return Ok(());
    }

    sleep(FALLBACK_RECHECK_DELAY).await;

    let vms = client.list_vms().await?;
    if vms.iter().any(|vm| vm.status == VmStatus::Running) {
        return Ok(());
    }

    let fallback_vm = vms.iter().find(|vm| vm.name == fallback_name);
    if let Some(vm) = fallback_vm {
        info!(
            "No running VMs detected; starting fallback VM '{}' ({})",
            vm.name, vm.vmid
        );
        client.start_vm(vm.vmid).await?;
    } else {
        warn!(
            "Fallback VM '{}' not found; skipping auto-start",
            fallback_name
        );
    }

    Ok(())
}
