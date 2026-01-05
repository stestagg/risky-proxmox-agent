use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::proxmox::error::ProxmoxError;
use crate::proxmox::types::{VmInfo, VmStatus};
use crate::proxmox::ProxmoxClient;

const INDEX_HTML: &str = include_str!("../assets/index.html");
const APP_JS: &str = include_str!("../assets/app.js");

#[derive(Clone)]
pub struct AppState {
    client: ProxmoxClient,
    launch_manager: Arc<LaunchManager>,
}

impl AppState {
    pub fn new(client: ProxmoxClient) -> Self {
        Self {
            client,
            launch_manager: Arc::new(LaunchManager::default()),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/app.js", get(app_js))
        .route("/api/vms", get(list_vms))
        .route("/api/launch", post(launch))
        .with_state(Arc::new(state))
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn app_js() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        APP_JS,
    )
}

async fn list_vms(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ApiVm>>, (StatusCode, Json<ApiError>)> {
    let vms = state.client.list_vms().await.map_err(map_proxmox_error)?;
    let response = vms.into_iter().map(ApiVm::from).collect();
    Ok(Json(response))
}

async fn launch(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LaunchRequest>,
) -> Result<Json<LaunchResponse>, (StatusCode, Json<ApiError>)> {
    let response = state
        .launch_manager
        .launch(&state.client, payload.vmid, payload.action)
        .await
        .map_err(map_launch_error)?;
    Ok(Json(response))
}

#[derive(Debug, Serialize)]
struct ApiVm {
    vmid: u64,
    name: String,
    tags: Vec<String>,
    status: String,
    notes: Option<String>,
}

impl From<VmInfo> for ApiVm {
    fn from(vm: VmInfo) -> Self {
        Self {
            vmid: vm.vmid,
            name: vm.name,
            tags: vm.tags,
            status: match vm.status {
                VmStatus::Running => "running".to_string(),
                VmStatus::Stopped => "stopped".to_string(),
                VmStatus::Unknown => "unknown".to_string(),
            },
            notes: vm.notes,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LaunchRequest {
    vmid: u64,
    action: Option<LaunchAction>,
}

#[derive(Debug, Serialize)]
struct LaunchResponse {
    status: LaunchStatus,
    message: String,
    running_vm: Option<RunningVmInfo>,
    allowed_actions: Vec<LaunchAction>,
}

impl LaunchResponse {
    fn started() -> Self {
        Self {
            status: LaunchStatus::Started,
            message: "Launch sequence started.".to_string(),
            running_vm: None,
            allowed_actions: Vec::new(),
        }
    }

    fn updated() -> Self {
        Self {
            status: LaunchStatus::Updated,
            message: "Launch updated to terminate current VM.".to_string(),
            running_vm: None,
            allowed_actions: Vec::new(),
        }
    }

    fn already_running() -> Self {
        Self {
            status: LaunchStatus::AlreadyRunning,
            message: "Target VM is already running.".to_string(),
            running_vm: None,
            allowed_actions: Vec::new(),
        }
    }

    fn cancelled() -> Self {
        Self {
            status: LaunchStatus::Cancelled,
            message: "Launch cancelled.".to_string(),
            running_vm: None,
            allowed_actions: Vec::new(),
        }
    }

    fn needs_action(vm: &VmInfo) -> Self {
        Self {
            status: LaunchStatus::NeedsAction,
            message: "A VM is currently running; choose an action.".to_string(),
            running_vm: Some(RunningVmInfo::from(vm)),
            allowed_actions: vec![
                LaunchAction::Shutdown,
                LaunchAction::Hibernate,
                LaunchAction::Terminate,
                LaunchAction::Cancel,
            ],
        }
    }
}

#[derive(Debug, Serialize)]
struct RunningVmInfo {
    vmid: u64,
    name: String,
}

impl From<&VmInfo> for RunningVmInfo {
    fn from(vm: &VmInfo) -> Self {
        Self {
            vmid: vm.vmid,
            name: vm.name.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum LaunchStatus {
    Started,
    NeedsAction,
    Updated,
    AlreadyRunning,
    Cancelled,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LaunchAction {
    Shutdown,
    Hibernate,
    Terminate,
    Cancel,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

fn map_proxmox_error(err: ProxmoxError) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::BAD_GATEWAY,
        Json(ApiError {
            error: err.to_string(),
        }),
    )
}

fn map_launch_error(err: LaunchError) -> (StatusCode, Json<ApiError>) {
    match err {
        LaunchError::InProgress => (
            StatusCode::CONFLICT,
            Json(ApiError {
                error: "Launch already in progress".to_string(),
            }),
        ),
        LaunchError::Proxmox(err) => map_proxmox_error(err),
    }
}

#[derive(Debug, Default)]
struct LaunchState {
    in_progress: bool,
    requested_action: Option<LaunchAction>,
}

#[derive(Debug, Default)]
struct LaunchManager {
    state: Mutex<LaunchState>,
}

impl LaunchManager {
    async fn launch(
        &self,
        client: &ProxmoxClient,
        target_vmid: u64,
        mut action: Option<LaunchAction>,
    ) -> Result<LaunchResponse, LaunchError> {
        {
            let mut state = self.state.lock().await;
            if state.in_progress {
                if matches!(action, Some(LaunchAction::Terminate)) {
                    state.requested_action = Some(LaunchAction::Terminate);
                    return Ok(LaunchResponse::updated());
                }
                return Err(LaunchError::InProgress);
            }
        }

        let vms = client.list_vms().await?;
        let running_vm = vms.into_iter().find(|vm| vm.status == VmStatus::Running);

        if let Some(ref running) = running_vm {
            if running.vmid == target_vmid {
                return Ok(LaunchResponse::already_running());
            }

            let easy_kill = running
                .tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case("easy-kill"));

            if action.is_none() && easy_kill {
                action = Some(LaunchAction::Terminate);
            }

            match action {
                None => return Ok(LaunchResponse::needs_action(running)),
                Some(LaunchAction::Cancel) => return Ok(LaunchResponse::cancelled()),
                _ => {}
            }
        } else if matches!(action, Some(LaunchAction::Cancel)) {
            return Ok(LaunchResponse::cancelled());
        }

        {
            let mut state = self.state.lock().await;
            state.in_progress = true;
            state.requested_action = action;
        }

        let outcome = self
            .run_flow(client, target_vmid, running_vm, action)
            .await;

        let mut state = self.state.lock().await;
        state.in_progress = false;
        state.requested_action = None;

        outcome?;
        Ok(LaunchResponse::started())
    }

    async fn run_flow(
        &self,
        client: &ProxmoxClient,
        target_vmid: u64,
        running_vm: Option<VmInfo>,
        mut action: Option<LaunchAction>,
    ) -> Result<(), LaunchError> {
        if let Some(running) = running_vm {
            let mut current_action = action.take().unwrap_or(LaunchAction::Terminate);
            info!(
                "Resolving running VM {} before launching {}",
                running.vmid, target_vmid
            );

            self.execute_action(client, running.vmid, current_action).await?;

            loop {
                let status = client.vm_status(running.vmid).await?;
                if status == VmStatus::Stopped {
                    break;
                }

                let requested_action = {
                    let state = self.state.lock().await;
                    state.requested_action
                };

                if requested_action == Some(LaunchAction::Terminate)
                    && current_action != LaunchAction::Terminate
                {
                    warn!(
                        "Escalating action to terminate VM {} during launch",
                        running.vmid
                    );
                    self.execute_action(client, running.vmid, LaunchAction::Terminate)
                        .await?;
                    current_action = LaunchAction::Terminate;
                }

                sleep(Duration::from_secs(2)).await;
            }
        }

        info!("Starting VM {}", target_vmid);
        client.start_vm(target_vmid).await?;
        Ok(())
    }

    async fn execute_action(
        &self,
        client: &ProxmoxClient,
        vmid: u64,
        action: LaunchAction,
    ) -> Result<(), LaunchError> {
        match action {
            LaunchAction::Shutdown => client.shutdown_vm(vmid).await?,
            LaunchAction::Hibernate => client.hibernate_vm(vmid).await?,
            LaunchAction::Terminate => client.terminate_vm(vmid).await?,
            LaunchAction::Cancel => {}
        }
        Ok(())
    }
}

#[derive(Debug)]
enum LaunchError {
    InProgress,
    Proxmox(ProxmoxError),
}

impl From<ProxmoxError> for LaunchError {
    fn from(value: ProxmoxError) -> Self {
        Self::Proxmox(value)
    }
}
