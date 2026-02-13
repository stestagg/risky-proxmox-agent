use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{MatchedPath, State},
    http::{Request, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, warn, Span};

use crate::proxmox::error::ProxmoxError;
use crate::proxmox::types::{VmInfo, VmStatus};
use crate::proxmox::ProxmoxClient;

const INDEX_HTML: &str = include_str!("../assets/index.html");
const APP_JS: &str = include_str!("../assets/app.js");
const BACKGROUND_JPG: &[u8] = include_bytes!("../assets/background.jpg");

#[derive(Clone)]
pub struct AppState {
    client: ProxmoxClient,
    launch_manager: Arc<LaunchManager>,
    shutdown_manager: Arc<ShutdownManager>,
}

impl AppState {
    pub fn new(client: ProxmoxClient) -> Self {
        Self {
            client,
            launch_manager: Arc::new(LaunchManager::default()),
            shutdown_manager: Arc::new(ShutdownManager::default()),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/app.js", get(app_js))
        .route(
            "/assets/background.jpg",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "image/jpeg")],
                    BACKGROUND_JPG,
                )
            }),
        )
        .route("/api/vms", get(list_vms))
        .route("/api/launch", post(launch))
        .route("/api/fork", post(fork_vm))
        .route("/api/host-shutdown", post(host_shutdown))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let matched_path = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map(MatchedPath::as_str)
                        .unwrap_or("<unmatched>");
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        path = %request.uri().path(),
                        matched_path,
                    )
                })
                .on_request(|request: &Request<_>, _span: &Span| {
                    info!(
                        method = %request.method(),
                        path = %request.uri().path(),
                        query = ?request.uri().query(),
                        "Incoming HTTP request"
                    );
                })
                .on_response(
                    |response: &axum::http::Response<_>, latency: Duration, _span: &Span| {
                        info!(
                            status = %response.status(),
                            latency_ms = latency.as_millis(),
                            "HTTP request completed"
                        );
                    },
                )
                .on_failure(
                    |error: tower_http::classify::ServerErrorsFailureClass,
                     latency: Duration,
                     _span: &Span| {
                        error!(
                            failure = ?error,
                            latency_ms = latency.as_millis(),
                            "HTTP request failed"
                        );
                    },
                ),
        )
        .with_state(Arc::new(state))
}

async fn index() -> Html<&'static str> {
    debug!("Serving index page");
    Html(INDEX_HTML)
}

async fn app_js() -> impl IntoResponse {
    debug!("Serving app JavaScript");
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        APP_JS,
    )
}

async fn list_vms(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ApiVm>>, (StatusCode, Json<ApiError>)> {
    info!("Listing VMs");
    let vms = state.client.list_vms().await.map_err(map_proxmox_error)?;
    info!(vm_count = vms.len(), "VM list retrieved");
    let response = vms.into_iter().map(ApiVm::from).collect();
    Ok(Json(response))
}

async fn launch(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LaunchRequest>,
) -> Result<Json<LaunchResponse>, (StatusCode, Json<ApiError>)> {
    info!(target_vmid = payload.vmid, action = ?payload.action, "Launch request received");
    let response = state
        .launch_manager
        .launch(&state.client, payload.vmid, payload.action)
        .await
        .map_err(map_launch_error)?;
    info!(target_vmid = payload.vmid, status = ?response.status, "Launch request completed");
    Ok(Json(response))
}

async fn fork_vm(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ForkRequest>,
) -> Result<Json<ForkResponse>, (StatusCode, Json<ApiError>)> {
    info!(source_vmid = payload.vmid, new_name = %payload.name, "Fork request received");
    let new_vmid = state
        .client
        .fork_vm(payload.vmid, &payload.name)
        .await
        .map_err(map_proxmox_error)?;
    wait_for_vm(&state.client, new_vmid)
        .await
        .map_err(map_proxmox_error)?;
    info!(new_vmid, "Fork request completed");
    Ok(Json(ForkResponse::created(new_vmid)))
}

async fn host_shutdown(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ShutdownRequest>,
) -> Result<Json<ShutdownResponse>, (StatusCode, Json<ApiError>)> {
    info!(action = ?payload.action, "Host shutdown request received");
    let response = state
        .shutdown_manager
        .shutdown(&state.client, payload.action)
        .await
        .map_err(map_shutdown_error)?;
    info!(status = ?response.status, "Host shutdown request completed");
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

#[derive(Debug, Deserialize)]
struct ShutdownRequest {
    action: Option<LaunchAction>,
}

#[derive(Debug, Serialize)]
struct ShutdownResponse {
    status: ShutdownStatus,
    message: String,
    running_vm: Option<RunningVmInfo>,
    allowed_actions: Vec<LaunchAction>,
}

impl ShutdownResponse {
    fn started() -> Self {
        Self {
            status: ShutdownStatus::Started,
            message: "Host shutdown sequence started.".to_string(),
            running_vm: None,
            allowed_actions: Vec::new(),
        }
    }

    fn cancelled() -> Self {
        Self {
            status: ShutdownStatus::Cancelled,
            message: "Host shutdown cancelled.".to_string(),
            running_vm: None,
            allowed_actions: Vec::new(),
        }
    }

    fn needs_action(vm: &VmInfo) -> Self {
        Self {
            status: ShutdownStatus::NeedsAction,
            message: "A VM is currently running; choose an action before shutdown.".to_string(),
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
#[serde(rename_all = "snake_case")]
enum ShutdownStatus {
    Started,
    NeedsAction,
    Cancelled,
}

#[derive(Debug, Deserialize)]
struct ForkRequest {
    vmid: u64,
    name: String,
}

#[derive(Debug, Serialize)]
struct ForkResponse {
    status: ForkStatus,
    message: String,
    vmid: u64,
}

impl ForkResponse {
    fn created(vmid: u64) -> Self {
        Self {
            status: ForkStatus::Created,
            message: "VM fork created.".to_string(),
            vmid,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ForkStatus {
    Created,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

fn map_proxmox_error(err: ProxmoxError) -> (StatusCode, Json<ApiError>) {
    warn!(error = %err, "Proxmox API call failed");
    (
        StatusCode::BAD_GATEWAY,
        Json(ApiError {
            error: err.to_string(),
        }),
    )
}

fn map_launch_error(err: LaunchError) -> (StatusCode, Json<ApiError>) {
    match err {
        LaunchError::InProgress => {
            warn!("Rejected launch request while another launch is in progress");
            (
                StatusCode::CONFLICT,
                Json(ApiError {
                    error: "Launch already in progress".to_string(),
                }),
            )
        }
        LaunchError::Proxmox(err) => map_proxmox_error(err),
    }
}

fn map_shutdown_error(err: ShutdownError) -> (StatusCode, Json<ApiError>) {
    match err {
        ShutdownError::InProgress => {
            warn!("Rejected shutdown request while another shutdown is in progress");
            (
                StatusCode::CONFLICT,
                Json(ApiError {
                    error: "Shutdown already in progress".to_string(),
                }),
            )
        }
        ShutdownError::Proxmox(err) => map_proxmox_error(err),
        ShutdownError::ShutdownFailed(err) => {
            warn!(error = %err, "Host shutdown workflow failed");
            (StatusCode::BAD_GATEWAY, Json(ApiError { error: err }))
        }
    }
}

async fn wait_for_vm(client: &ProxmoxClient, vmid: u64) -> Result<(), ProxmoxError> {
    info!(vmid, "Waiting for forked VM to appear in Proxmox inventory");
    for attempt in 1..=30 {
        let vms = client.list_vms().await?;
        if vms.iter().any(|vm| vm.vmid == vmid) {
            info!(vmid, attempt, "Forked VM is now visible");
            return Ok(());
        }
        debug!(vmid, attempt, "Forked VM not visible yet; retrying");
        sleep(Duration::from_secs(2)).await;
    }
    warn!(vmid, "Timed out waiting for forked VM to appear");
    Err(ProxmoxError::Api(format!(
        "Timed out waiting for VM {vmid} to appear"
    )))
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
                warn!(target_vmid, action = ?action, "Launch requested while another launch is in progress");
                if matches!(action, Some(LaunchAction::Terminate)) {
                    info!(
                        target_vmid,
                        "Queued terminate escalation for in-progress launch"
                    );
                    state.requested_action = Some(LaunchAction::Terminate);
                    return Ok(LaunchResponse::updated());
                }
                return Err(LaunchError::InProgress);
            }
        }

        info!(target_vmid, action = ?action, "Evaluating launch preconditions");
        let vms = client.list_vms().await?;
        let running_vm = vms.into_iter().find(|vm| vm.status == VmStatus::Running);

        if let Some(ref running) = running_vm {
            if running.vmid == target_vmid {
                info!(target_vmid, "Launch target is already running");
                return Ok(LaunchResponse::already_running());
            }

            let easy_kill = running
                .tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case("easy-kill"));

            if action.is_none() && easy_kill {
                info!(
                    running_vmid = running.vmid,
                    "Auto-selecting terminate for easy-kill VM"
                );
                action = Some(LaunchAction::Terminate);
            }

            match action {
                None => {
                    info!(
                        running_vmid = running.vmid,
                        target_vmid, "Launch requires user action due to running VM"
                    );
                    return Ok(LaunchResponse::needs_action(running));
                }
                Some(LaunchAction::Cancel) => {
                    info!(target_vmid, "Launch cancelled by client");
                    return Ok(LaunchResponse::cancelled());
                }
                _ => {}
            }
        } else if matches!(action, Some(LaunchAction::Cancel)) {
            info!(target_vmid, "Launch cancelled without active running VM");
            return Ok(LaunchResponse::cancelled());
        }

        {
            let mut state = self.state.lock().await;
            state.in_progress = true;
            state.requested_action = action;
            info!(target_vmid, action = ?action, "Launch flow marked in progress");
        }

        let outcome = self.run_flow(client, target_vmid, running_vm, action).await;

        let mut state = self.state.lock().await;
        state.in_progress = false;
        state.requested_action = None;

        outcome?;
        info!(target_vmid, "Launch flow completed successfully");
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

            self.execute_action(client, running.vmid, current_action)
                .await?;

            loop {
                let status = client.vm_status(running.vmid).await?;
                debug!(running_vmid = running.vmid, status = ?status, "Waiting for running VM to stop");
                if status == VmStatus::Stopped {
                    info!(
                        running_vmid = running.vmid,
                        "Running VM is stopped; proceeding with launch"
                    );
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

        info!(target_vmid, "Starting target VM");
        client.start_vm(target_vmid).await?;
        Ok(())
    }

    async fn execute_action(
        &self,
        client: &ProxmoxClient,
        vmid: u64,
        action: LaunchAction,
    ) -> Result<(), LaunchError> {
        info!(vmid, action = ?action, "Executing VM action for launch flow");
        match action {
            LaunchAction::Shutdown => client.shutdown_vm(vmid).await?,
            LaunchAction::Hibernate => client.hibernate_vm(vmid).await?,
            LaunchAction::Terminate => client.terminate_vm(vmid).await?,
            LaunchAction::Cancel => {}
        }
        info!(vmid, action = ?action, "Launch flow VM action command sent");
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

#[derive(Debug, Default)]
struct ShutdownState {
    in_progress: bool,
}

#[derive(Debug, Default)]
struct ShutdownManager {
    state: Mutex<ShutdownState>,
}

impl ShutdownManager {
    async fn shutdown(
        &self,
        client: &ProxmoxClient,
        action: Option<LaunchAction>,
    ) -> Result<ShutdownResponse, ShutdownError> {
        {
            let state = self.state.lock().await;
            if state.in_progress {
                warn!(action = ?action, "Host shutdown requested while shutdown already in progress");
                return Err(ShutdownError::InProgress);
            }
        }

        info!(action = ?action, "Evaluating host shutdown preconditions");
        let vms = client.list_vms().await?;
        let running_vm = vms.into_iter().find(|vm| vm.status == VmStatus::Running);

        if let Some(ref running) = running_vm {
            if action.is_none() {
                info!(
                    running_vmid = running.vmid,
                    "Host shutdown requires VM action selection"
                );
                return Ok(ShutdownResponse::needs_action(running));
            }
            if matches!(action, Some(LaunchAction::Cancel)) {
                info!("Host shutdown cancelled by client");
                return Ok(ShutdownResponse::cancelled());
            }
        } else if matches!(action, Some(LaunchAction::Cancel)) {
            info!("Host shutdown cancelled before work started");
            return Ok(ShutdownResponse::cancelled());
        }

        {
            let mut state = self.state.lock().await;
            state.in_progress = true;
            info!(action = ?action, "Host shutdown flow marked in progress");
        }

        let outcome = self.run_flow(client, running_vm, action).await;

        let mut state = self.state.lock().await;
        state.in_progress = false;

        outcome?;
        info!("Host shutdown flow completed successfully");
        Ok(ShutdownResponse::started())
    }

    async fn run_flow(
        &self,
        client: &ProxmoxClient,
        running_vm: Option<VmInfo>,
        action: Option<LaunchAction>,
    ) -> Result<(), ShutdownError> {
        if let Some(running) = running_vm {
            let selected_action = action.unwrap_or(LaunchAction::Terminate);
            info!("Resolving running VM {} before host shutdown", running.vmid);

            self.execute_action(client, running.vmid, selected_action)
                .await?;

            for attempt in 1..=60 {
                let status = client.vm_status(running.vmid).await?;
                debug!(running_vmid = running.vmid, attempt, status = ?status, "Waiting for VM to stop before host shutdown");
                if status == VmStatus::Stopped {
                    info!(
                        running_vmid = running.vmid,
                        "VM stopped before host shutdown"
                    );
                    break;
                }
                sleep(Duration::from_secs(2)).await;
            }

            let status = client.vm_status(running.vmid).await?;
            debug!(running_vmid = running.vmid, status = ?status, "Final VM status check before host shutdown");
            if status != VmStatus::Stopped {
                return Err(ShutdownError::ShutdownFailed(format!(
                    "Timed out waiting for VM {} to stop",
                    running.vmid
                )));
            }
        }

        info!("Initiating host shutdown command");
        tokio::task::spawn_blocking(|| {
            match Command::new("shutdown").arg("-h").arg("now").status() {
                Ok(status) => {
                    if !status.success() {
                        warn!("Shutdown command exited with status {status}");
                    } else {
                        info!("Shutdown command executed successfully");
                    }
                }
                Err(err) => {
                    warn!("Failed to execute shutdown command: {err}");
                }
            }
        });
        Ok(())
    }

    async fn execute_action(
        &self,
        client: &ProxmoxClient,
        vmid: u64,
        action: LaunchAction,
    ) -> Result<(), ShutdownError> {
        info!(vmid, action = ?action, "Executing VM action");
        match action {
            LaunchAction::Shutdown => client.shutdown_vm(vmid).await?,
            LaunchAction::Hibernate => client.hibernate_vm(vmid).await?,
            LaunchAction::Terminate => client.terminate_vm(vmid).await?,
            LaunchAction::Cancel => {}
        }
        info!(vmid, action = ?action, "VM action command sent");
        Ok(())
    }
}

#[derive(Debug)]
enum ShutdownError {
    InProgress,
    Proxmox(ProxmoxError),
    ShutdownFailed(String),
}

impl From<ProxmoxError> for ShutdownError {
    fn from(value: ProxmoxError) -> Self {
        Self::Proxmox(value)
    }
}
