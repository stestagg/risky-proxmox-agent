use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VmStatus {
    Running,
    Stopped,
}

impl VmStatus {
    fn as_str(&self) -> &'static str {
        match self {
            VmStatus::Running => "running",
            VmStatus::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmEntry {
    pub vmid: u64,
    pub name: String,
    pub tags: Vec<String>,
    pub status: VmStatus,
    pub notes: Option<String>,
}

#[derive(Debug, Default)]
struct DummyState {
    node: String,
    vms: HashMap<u64, VmEntry>,
}

#[derive(Clone, Default)]
pub struct DummyHandle {
    state: Arc<Mutex<DummyState>>,
}

impl DummyHandle {
    pub fn new(node: impl Into<String>) -> Self {
        let mut state = DummyState::default();
        state.node = node.into();
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub async fn insert_vm(&self, vm: VmEntry) {
        let mut state = self.state.lock().await;
        state.vms.insert(vm.vmid, vm);
    }

    pub async fn set_status(&self, vmid: u64, status: VmStatus) {
        let mut state = self.state.lock().await;
        if let Some(vm) = state.vms.get_mut(&vmid) {
            vm.status = status;
        }
    }

    pub async fn status(&self, vmid: u64) -> Option<VmStatus> {
        let state = self.state.lock().await;
        state.vms.get(&vmid).map(|vm| vm.status)
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/api2/json/nodes/:node/qemu", get(list_vms))
            .route(
                "/api2/json/nodes/:node/qemu/:vmid/status/current",
                get(current_status),
            )
            .route(
                "/api2/json/nodes/:node/qemu/:vmid/status/start",
                post(start_vm),
            )
            .route(
                "/api2/json/nodes/:node/qemu/:vmid/status/shutdown",
                post(shutdown_vm),
            )
            .route(
                "/api2/json/nodes/:node/qemu/:vmid/status/stop",
                post(stop_vm),
            )
            .route("/api2/json/cluster/resources", get(list_cluster_resources))
            .with_state(self.state.clone())
    }

    pub async fn serve(self, listener: tokio::net::TcpListener) -> Result<(), std::io::Error> {
        axum::serve(listener, self.router()).await
    }
}

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Debug, Serialize)]
struct ResourceVm {
    vmid: u64,
    name: Option<String>,
    tags: Option<String>,
    status: Option<String>,
    node: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
struct StatusPayload {
    status: String,
}

#[derive(Debug, Deserialize)]
struct ResourceQuery {
    #[serde(rename = "type")]
    resource_type: Option<String>,
    vmid: Option<u64>,
}

async fn list_vms(
    Path(node): Path<String>,
    State(state): State<Arc<Mutex<DummyState>>>,
) -> Result<Json<ApiResponse<Vec<ResourceVm>>>, StatusCode> {
    let state = state.lock().await;
    if node != state.node {
        return Err(StatusCode::NOT_FOUND);
    }
    let vms = state
        .vms
        .values()
        .map(|vm| ResourceVm {
            vmid: vm.vmid,
            name: Some(vm.name.clone()),
            tags: Some(vm.tags.join(";")),
            status: Some(vm.status.as_str().to_string()),
            node: Some(state.node.clone()),
            description: vm.notes.clone(),
        })
        .collect::<Vec<_>>();
    Ok(Json(ApiResponse { data: vms }))
}

async fn current_status(
    Path((node, vmid)): Path<(String, u64)>,
    State(state): State<Arc<Mutex<DummyState>>>,
) -> Result<Json<ApiResponse<StatusPayload>>, StatusCode> {
    let state = state.lock().await;
    if node != state.node {
        return Err(StatusCode::NOT_FOUND);
    }
    let vm = state.vms.get(&vmid).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(ApiResponse {
        data: StatusPayload {
            status: vm.status.as_str().to_string(),
        },
    }))
}

async fn start_vm(
    Path((node, vmid)): Path<(String, u64)>,
    State(state): State<Arc<Mutex<DummyState>>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let mut state = state.lock().await;
    if node != state.node {
        return Err(StatusCode::NOT_FOUND);
    }
    let vm = state.vms.get_mut(&vmid).ok_or(StatusCode::NOT_FOUND)?;
    vm.status = VmStatus::Running;
    Ok(Json(ApiResponse {
        data: serde_json::Value::Null,
    }))
}

async fn shutdown_vm(
    Path((node, vmid)): Path<(String, u64)>,
    State(state): State<Arc<Mutex<DummyState>>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let mut state = state.lock().await;
    if node != state.node {
        return Err(StatusCode::NOT_FOUND);
    }
    let vm = state.vms.get_mut(&vmid).ok_or(StatusCode::NOT_FOUND)?;
    vm.status = VmStatus::Stopped;
    Ok(Json(ApiResponse {
        data: serde_json::Value::Null,
    }))
}

async fn stop_vm(
    Path((node, vmid)): Path<(String, u64)>,
    State(state): State<Arc<Mutex<DummyState>>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    shutdown_vm(Path((node, vmid)), State(state)).await
}

async fn list_cluster_resources(
    State(state): State<Arc<Mutex<DummyState>>>,
    Query(query): Query<ResourceQuery>,
) -> Result<Json<ApiResponse<Vec<ResourceVm>>>, StatusCode> {
    if let Some(resource_type) = query.resource_type.as_deref() {
        if resource_type != "vm" {
            return Ok(Json(ApiResponse { data: Vec::new() }));
        }
    }
    let state = state.lock().await;
    let vms = state
        .vms
        .values()
        .filter(|vm| query.vmid.map(|id| vm.vmid == id).unwrap_or(true))
        .map(|vm| ResourceVm {
            vmid: vm.vmid,
            name: Some(vm.name.clone()),
            tags: Some(vm.tags.join(";")),
            status: Some(vm.status.as_str().to_string()),
            node: Some(state.node.clone()),
            description: vm.notes.clone(),
        })
        .collect::<Vec<_>>();
    Ok(Json(ApiResponse { data: vms }))
}

pub async fn spawn_dummy_server(
    handle: DummyHandle,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>), std::io::Error> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let join_handle = tokio::spawn(async move {
        if let Err(err) = handle.serve(listener).await {
            tracing::error!("dummy server failed: {err}");
        }
    });
    Ok((addr, join_handle))
}
