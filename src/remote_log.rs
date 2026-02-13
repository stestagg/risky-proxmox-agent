use std::collections::VecDeque;
use std::io;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tokio::sync::Mutex;
use tracing_subscriber::fmt::MakeWriter;

use crate::config::RemoteLogConfig;

#[derive(Clone)]
pub struct RemoteLogHandle {
    state: Arc<Mutex<RemoteLogState>>,
    upload_url: Arc<str>,
    authorization_secret: Arc<str>,
    max_pending_bytes: usize,
    max_upload_bytes: usize,
    upload_delay: Duration,
    hostname: Arc<str>,
    client: reqwest::Client,
}

struct RemoteLogState {
    entries: VecDeque<Vec<u8>>,
    pending_bytes: usize,
}

impl RemoteLogHandle {
    pub fn new(config: RemoteLogConfig) -> Self {
        let hostname = std::env::var("HOSTNAME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "unknown-host".to_string());

        Self {
            state: Arc::new(Mutex::new(RemoteLogState {
                entries: VecDeque::new(),
                pending_bytes: 0,
            })),
            upload_url: Arc::from(config.upload_url),
            authorization_secret: Arc::from(config.authorization_secret),
            max_pending_bytes: config.max_pending_bytes,
            max_upload_bytes: config.max_upload_bytes,
            upload_delay: Duration::from_secs_f64(config.upload_delay_secs.max(0.1)),
            hostname: Arc::from(hostname),
            client: reqwest::Client::new(),
        }
    }

    pub fn spawn_upload_loop(&self) {
        let this = self.clone();
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        runtime.spawn(async move {
            loop {
                tokio::time::sleep(this.upload_delay).await;
                this.do_upload().await;
            }
        });
    }

    async fn do_upload(&self) {
        let next_batch = self.take_next_batch().await;
        if next_batch.is_empty() {
            return;
        }

        let mut payload = Vec::new();
        for line in next_batch {
            if !payload.is_empty() {
                payload.push(b'\n');
            }
            payload.extend_from_slice(&line);
        }

        let response = self
            .client
            .post(self.upload_url.as_ref())
            .header("Content-Type", "application/x-ndjson")
            .header("Authorization", self.authorization_secret.as_ref())
            .body(payload)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => eprintln!("[remote-log] upload returned status {}", resp.status()),
            Err(err) => eprintln!("[remote-log] upload failed: {err}"),
        }
    }

    async fn take_next_batch(&self) -> Vec<Vec<u8>> {
        let mut state = self.state.lock().await;
        let mut batch = Vec::new();
        let mut size = 0usize;

        while let Some(entry) = state.entries.front() {
            let entry_size = entry.len();
            if !batch.is_empty() && size + entry_size > self.max_upload_bytes {
                break;
            }
            size += entry_size;
            let popped = state.entries.pop_front().expect("entry existed");
            state.pending_bytes = state.pending_bytes.saturating_sub(popped.len());
            batch.push(popped);
            if size >= self.max_upload_bytes {
                break;
            }
        }

        batch
    }

    pub fn log(&self, data: Vec<u8>) {
        let hostname = self.hostname.clone();
        let this = self.clone();
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let timestamp_ms = current_timestamp_ms();
        runtime.spawn(async move {
            let normalized = normalize_line(data, &hostname, timestamp_ms);
            let mut state = this.state.lock().await;
            if state.pending_bytes + normalized.len() > this.max_pending_bytes {
                eprintln!(
                    "[remote-log] dropped entry ({} bytes) because buffer is full",
                    normalized.len()
                );
                return;
            }

            state.pending_bytes += normalized.len();
            state.entries.push_back(normalized);
        });
    }
}

fn normalize_line(data: Vec<u8>, hostname: &str, timestamp_ms: u64) -> Vec<u8> {
    match serde_json::from_slice::<Value>(&data) {
        Ok(Value::Object(mut map)) => {
            map.entry("hostname".to_string())
                .or_insert_with(|| Value::String(hostname.to_string()));
            map.entry("timestamp_ms".to_string())
                .or_insert_with(|| Value::Number(timestamp_ms.into()));
            serde_json::to_vec(&Value::Object(map)).unwrap_or_else(|_| {
                serde_json::to_vec(&serde_json::json!({
                    "hostname": hostname,
                    "timestamp_ms": timestamp_ms,
                    "message": String::from_utf8_lossy(&data)
                }))
                .unwrap_or_default()
            })
        }
        Ok(other) => serde_json::to_vec(&serde_json::json!({
            "hostname": hostname,
            "timestamp_ms": timestamp_ms,
            "log": other
        }))
        .unwrap_or_default(),
        Err(_) => serde_json::to_vec(&serde_json::json!({
            "hostname": hostname,
            "timestamp_ms": timestamp_ms,
            "message": String::from_utf8_lossy(&data)
        }))
        .unwrap_or_default(),
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

#[derive(Clone)]
pub struct RemoteLogMakeWriter {
    handle: RemoteLogHandle,
}

impl RemoteLogMakeWriter {
    pub fn new(handle: RemoteLogHandle) -> Self {
        Self { handle }
    }
}

impl<'a> MakeWriter<'a> for RemoteLogMakeWriter {
    type Writer = RemoteLogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        RemoteLogWriter {
            handle: self.handle.clone(),
            buffer: Vec::new(),
        }
    }
}

pub struct RemoteLogWriter {
    handle: RemoteLogHandle,
    buffer: Vec<u8>,
}

impl Write for RemoteLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let data = std::mem::take(&mut self.buffer);
        for line in data.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            self.handle.log(line.to_vec());
        }

        Ok(())
    }
}

impl Drop for RemoteLogWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
