pub mod error;
pub mod types;

use serde::de::DeserializeOwned;
use serde::Deserialize;

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
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(insecure_ssl)
            .build()?;
        Ok(Self {
            base_url: base_url.into(),
            token: format!("PVEAPIToken={token_id}={token_secret}"),
            client,
        })
    }

    pub async fn list_vms(&self) -> Result<Vec<VmInfo>, ProxmoxError> {
        let resources: Vec<ResourceVm> = self
            .get("/cluster/resources?type=vm")
            .await?;

        Ok(resources
            .into_iter()
            .map(|vm| VmInfo {
                vmid: vm.vmid,
                name: vm.name.unwrap_or_default(),
                tags: parse_tags(vm.tags.as_deref()),
                status: VmStatus::normalize(vm.status.as_deref()),
                notes: vm.description.filter(|note| !note.trim().is_empty()),
            })
            .collect())
    }

    pub async fn vm_status(&self, vmid: u64) -> Result<VmStatus, ProxmoxError> {
        let node = self.node_for_vmid(vmid).await?;
        let path = format!("/nodes/{node}/qemu/{vmid}/status/current");
        let status: StatusResponse = self.get(&path).await?;
        Ok(VmStatus::normalize(Some(&status.status)))
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

    async fn node_for_vmid(&self, vmid: u64) -> Result<String, ProxmoxError> {
        let resources: Vec<ResourceVm> = self.get("/cluster/resources?type=vm").await?;
        resources
            .into_iter()
            .find(|vm| vm.vmid == vmid)
            .and_then(|vm| vm.node)
            .ok_or(ProxmoxError::MissingNode(vmid))
    }

    async fn post_status(&self, vmid: u64, action: &str) -> Result<(), ProxmoxError> {
        let node = self.node_for_vmid(vmid).await?;
        let path = format!("/nodes/{node}/qemu/{vmid}/status/{action}");
        self.post(&path).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ProxmoxError> {
        let url = self.endpoint(path);
        let response = self
            .client
            .get(url)
            .header(reqwest::header::AUTHORIZATION, self.token.clone())
            .send()
            .await?;
        let response = Self::ensure_success(response).await?;
        let response: ApiResponse<T> = response.json().await?;
        Ok(response.data)
    }

    async fn post(&self, path: &str) -> Result<(), ProxmoxError> {
        let url = self.endpoint(path);
        let response = self
            .client
            .post(url)
            .header(reqwest::header::AUTHORIZATION, self.token.clone())
            .send()
            .await?;
        let _ = Self::ensure_success(response).await?;
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
            Err(ProxmoxError::Api(format!(
                "status {status}, body {body}"
            )))
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/api2/json{}",
            self.base_url.trim_end_matches('/'),
            path
        )
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
