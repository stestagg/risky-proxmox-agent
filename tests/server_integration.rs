use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use proxmox_dummy::{spawn_dummy_server, DummyHandle, VmEntry, VmStatus};
use reqwest::Client;
use risky_proxmox_agent::proxmox::ProxmoxClient;
use risky_proxmox_agent::server::{router, AppState};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::time::{sleep, timeout};

async fn spawn_app(router: Router) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, router).await {
            eprintln!("app server failed: {err}");
        }
    });
    addr
}

async fn wait_for_status(handle: &DummyHandle, vmid: u64, status: VmStatus) {
    let _ = timeout(Duration::from_secs(5), async {
        loop {
            if handle.status(vmid).await == Some(status) {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
}

#[derive(Debug, Deserialize)]
struct ApiVm {
    vmid: u64,
    name: String,
    tags: Vec<String>,
    status: String,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LaunchResponse {
    status: String,
}

#[tokio::test]
async fn list_vms_returns_expected_data() {
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    let handle = DummyHandle::new("pve");
    handle
        .insert_vm(VmEntry {
            vmid: 101,
            name: "alpha".to_string(),
            tags: vec!["easy-kill".to_string()],
            status: VmStatus::Running,
            notes: Some("alpha notes".to_string()),
        })
        .await;
    handle
        .insert_vm(VmEntry {
            vmid: 202,
            name: "beta".to_string(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            status: VmStatus::Stopped,
            notes: None,
        })
        .await;

    let (dummy_addr, _dummy_task) = spawn_dummy_server(handle.clone()).await.unwrap();
    let client = ProxmoxClient::new(
        format!("http://{dummy_addr}"),
        "token-id",
        "token-secret",
        false,
    )
    .unwrap();
    let app_addr = spawn_app(router(AppState::new(client))).await;

    let response = Client::new()
        .get(format!("http://{app_addr}/api/vms"))
        .send()
        .await
        .unwrap();
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        panic!("unexpected status {status}: {body}");
    }
    let response = response.json::<Vec<ApiVm>>().await.unwrap();

    let alpha = response.iter().find(|vm| vm.vmid == 101).unwrap();
    assert_eq!(alpha.name, "alpha");
    assert_eq!(alpha.status, "running");
    assert_eq!(alpha.tags, vec!["easy-kill"]);
    assert_eq!(alpha.notes.as_deref(), Some("alpha notes"));
}

#[tokio::test]
async fn launch_flow_terminates_easy_kill_and_starts_target() {
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    let handle = DummyHandle::new("pve");
    handle
        .insert_vm(VmEntry {
            vmid: 100,
            name: "easy".to_string(),
            tags: vec!["easy-kill".to_string()],
            status: VmStatus::Running,
            notes: None,
        })
        .await;
    handle
        .insert_vm(VmEntry {
            vmid: 200,
            name: "target".to_string(),
            tags: vec![],
            status: VmStatus::Stopped,
            notes: None,
        })
        .await;

    let (dummy_addr, _dummy_task) = spawn_dummy_server(handle.clone()).await.unwrap();
    let client = ProxmoxClient::new(
        format!("http://{dummy_addr}"),
        "token-id",
        "token-secret",
        false,
    )
    .unwrap();
    let app_addr = spawn_app(router(AppState::new(client))).await;

    let response = Client::new()
        .post(format!("http://{app_addr}/api/launch"))
        .json(&serde_json::json!({ "vmid": 200 }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        panic!("unexpected status {status}: {body}");
    }
    let response = response.json::<LaunchResponse>().await.unwrap();

    assert_eq!(response.status, "started");
    wait_for_status(&handle, 100, VmStatus::Stopped).await;
    wait_for_status(&handle, 200, VmStatus::Running).await;
    assert_eq!(handle.status(100).await, Some(VmStatus::Stopped));
    assert_eq!(handle.status(200).await, Some(VmStatus::Running));
}
