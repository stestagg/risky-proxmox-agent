use std::fmt;

#[derive(Debug)]
pub enum ProxmoxError {
    Api(String),
    MissingNode(u64),
    Reqwest(reqwest::Error),
    Serde(serde_json::Error),
}

impl fmt::Display for ProxmoxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api(message) => write!(f, "Proxmox API error: {message}"),
            Self::MissingNode(vmid) => write!(f, "Missing node for VM {vmid}"),
            Self::Reqwest(err) => write!(f, "HTTP error: {err}"),
            Self::Serde(err) => write!(f, "Parse error: {err}"),
        }
    }
}

impl std::error::Error for ProxmoxError {}

impl From<reqwest::Error> for ProxmoxError {
    fn from(value: reqwest::Error) -> Self {
        Self::Reqwest(value)
    }
}

impl From<serde_json::Error> for ProxmoxError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}
