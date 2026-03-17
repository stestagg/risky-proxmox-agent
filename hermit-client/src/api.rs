use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Vm {
    pub vmid: u64,
    pub name: String,
    pub tags: Vec<String>,
    pub status: String,
    #[serde(default)]
    pub notes: Option<String>,
}

pub fn fetch_vms(base_url: &str) -> Result<Vec<Vm>, Box<dyn std::error::Error>> {
    let url = format!("{}/api/vms", base_url.trim_end_matches('/'));
    let vms: Vec<Vm> = ureq::get(&url).call()?.into_json()?;
    Ok(vms)
}
