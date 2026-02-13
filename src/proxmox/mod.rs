pub mod error;
pub mod types;

use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::proxmox::error::ProxmoxError;
use crate::proxmox::types::{parse_tags, VmInfo, VmStatus};

#[derive(Clone)]
pub struct ProxmoxClient {
    base_url: String,
    token: String,
    client: reqwest::Client,
}

impl ProxmoxClient {
    pub fn new(
        base_url: impl Into<String>,
        token_id: &str,
        token_secret: &str,
        insecure_ssl: bool,
    ) -> Result<Self, ProxmoxError> {
        let base_url = base_url.into();
        info!(%base_url, insecure_ssl, "Creating Proxmox HTTP client");
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(insecure_ssl)
            .build()?;
        Ok(Self {
            base_url,
            token: format!("PVEAPIToken={token_id}={token_secret}"),
            client,
        })
    }

    pub async fn list_vms(&self) -> Result<Vec<VmInfo>, ProxmoxError> {
        debug!("Fetching VM inventory from Proxmox");
        let resources: Vec<ResourceVm> = self.get("/cluster/resources?type=vm").await?;

        let vms: Vec<VmInfo> = resources
            .into_iter()
            .map(|vm| VmInfo {
                vmid: vm.vmid,
                name: vm.name.unwrap_or_default(),
                tags: parse_tags(vm.tags.as_deref()),
                status: VmStatus::normalize(vm.status.as_deref()),
                notes: vm.description.filter(|note| !note.trim().is_empty()),
            })
            .collect();
        info!(vm_count = vms.len(), "Fetched VM inventory");
        Ok(vms)
    }

    pub async fn vm_status(&self, vmid: u64) -> Result<VmStatus, ProxmoxError> {
        debug!(vmid, "Fetching VM status");
        let node = self.node_for_vmid(vmid).await?;
        let path = format!("/nodes/{node}/qemu/{vmid}/status/current");
        let status: StatusResponse = self.get(&path).await?;
        let normalized = VmStatus::normalize(Some(&status.status));
        debug!(vmid, status = ?normalized, "Fetched VM status");
        Ok(normalized)
    }

    pub async fn start_vm(&self, vmid: u64) -> Result<(), ProxmoxError> {
        self.post_status(vmid, "start").await
    }

    pub async fn stop_vm(&self, vmid: u64) -> Result<(), ProxmoxError> {
        self.post_status(vmid, "shutdown").await
    }

    pub async fn shutdown_vm(&self, vmid: u64) -> Result<(), ProxmoxError> {
        self.post_status(vmid, "shutdown").await
    }

    pub async fn hibernate_vm(&self, vmid: u64) -> Result<(), ProxmoxError> {
        self.post_status(vmid, "hibernate").await
    }

    pub async fn terminate_vm(&self, vmid: u64) -> Result<(), ProxmoxError> {
        self.post_status(vmid, "stop").await
    }

    pub async fn fork_vm(&self, vmid: u64, name: &str) -> Result<u64, ProxmoxError> {
        info!(source_vmid = vmid, new_name = %name, "Forking VM");
        let snapshot = format!(
            "fork-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        let newid = self.next_vmid().await?;
        self.create_snapshot(vmid, &snapshot).await?;
        self.clone_vm(vmid, newid, name, &snapshot).await?;
        info!(source_vmid = vmid, new_vmid = newid, snapshot = %snapshot, "Fork command sent");
        Ok(newid)
    }

    async fn node_for_vmid(&self, vmid: u64) -> Result<String, ProxmoxError> {
        debug!(vmid, "Resolving node for VM");
        let resources: Vec<ResourceVm> = self.get("/cluster/resources?type=vm").await?;
        resources
            .into_iter()
            .find(|vm| vm.vmid == vmid)
            .and_then(|vm| vm.node)
            .ok_or(ProxmoxError::MissingNode(vmid))
            .map(|node| {
                debug!(vmid, node = %node, "Resolved node for VM");
                node
            })
    }

    async fn post_status(&self, vmid: u64, action: &str) -> Result<(), ProxmoxError> {
        info!(vmid, action, "Sending VM status action");
        let node = self.node_for_vmid(vmid).await?;
        let path = format!("/nodes/{node}/qemu/{vmid}/status/{action}");
        self.post(&path).await
    }

    async fn next_vmid(&self) -> Result<u64, ProxmoxError> {
        debug!("Requesting next available VMID");
        let nextid: String = self.get("/cluster/nextid").await?;
        nextid
            .parse()
            .map_err(|err| ProxmoxError::Api(format!("Invalid next VMID: {err}")))
            .map(|id| {
                debug!(next_vmid = id, "Received next VMID");
                id
            })
    }

    async fn create_snapshot(&self, vmid: u64, snapshot: &str) -> Result<(), ProxmoxError> {
        info!(vmid, snapshot, "Creating VM snapshot for fork");
        let node = self.node_for_vmid(vmid).await?;
        let path = format!("/nodes/{node}/qemu/{vmid}/snapshot");
        let body = SnapshotRequest { snapname: snapshot };
        self.post_form(&path, &body).await
    }

    async fn clone_vm(
        &self,
        vmid: u64,
        newid: u64,
        name: &str,
        snapshot: &str,
    ) -> Result<(), ProxmoxError> {
        info!(source_vmid = vmid, new_vmid = newid, new_name = %name, snapshot, "Cloning VM from snapshot");
        let node = self.node_for_vmid(vmid).await?;
        let path = format!("/nodes/{node}/qemu/{vmid}/clone");
        let body = CloneRequest {
            newid,
            name,
            full: 1,
            snapname: snapshot,
        };
        self.post_form(&path, &body).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ProxmoxError> {
        let url = self.endpoint(path);
        debug!(method = "GET", %url, "Sending Proxmox request");
        let response = self
            .client
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, self.token.clone())
            .send()
            .await?;
        let response = Self::ensure_success(response).await?;
        debug!(method = "GET", %url, status = %response.status(), "Proxmox request succeeded");
        let response: ApiResponse<T> = response.json().await?;
        Ok(response.data)
    }

    async fn post(&self, path: &str) -> Result<(), ProxmoxError> {
        let url = self.endpoint(path);
        debug!(method = "POST", %url, "Sending Proxmox request");
        let response = self
            .client
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.token.clone())
            .send()
            .await?;
        let response = Self::ensure_success(response).await?;
        debug!(method = "POST", %url, status = %response.status(), "Proxmox request succeeded");
        Ok(())
    }

    async fn post_form<T: Serialize>(&self, path: &str, body: &T) -> Result<(), ProxmoxError> {
        let url = self.endpoint(path);
        debug!(method = "POST", %url, "Sending Proxmox form request");
        let response = self
            .client
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.token.clone())
            .form(body)
            .send()
            .await?;
        let response = Self::ensure_success(response).await?;
        debug!(method = "POST", %url, status = %response.status(), "Proxmox form request succeeded");
        Ok(())
    }

    async fn ensure_success(
        response: reqwest::Response,
    ) -> Result<reqwest::Response, ProxmoxError> {
        if response.status().is_success() {
            Ok(response)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(%status, body = %body, "Proxmox request returned non-success status");
            Err(ProxmoxError::Api(format!("status {status}, body {body}")))
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/api2/json{}", self.base_url.trim_end_matches('/'), path)
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
struct ResourceVm {
    vmid: u64,
    name: Option<String>,
    tags: Option<String>,
    status: Option<String>,
    node: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    status: String,
}

#[derive(Debug, Serialize)]
struct SnapshotRequest<'a> {
    snapname: &'a str,
}

#[derive(Debug, Serialize)]
struct CloneRequest<'a> {
    newid: u64,
    name: &'a str,
    full: u8,
    snapname: &'a str,
}
