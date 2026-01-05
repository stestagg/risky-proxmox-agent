#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmStatus {
    Running,
    Stopped,
    Unknown,
}

impl VmStatus {
    pub fn normalize(raw: Option<&str>) -> Self {
        match raw.unwrap_or("").to_lowercase().as_str() {
            "running" => Self::Running,
            "stopped" => Self::Stopped,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmInfo {
    pub vmid: u64,
    pub name: String,
    pub tags: Vec<String>,
    pub status: VmStatus,
    pub notes: Option<String>,
}

pub fn parse_tags(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or("")
        .split(|ch| ch == ';' || ch == ',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tags_splits_on_semicolons() {
        let tags = parse_tags(Some("alpha;beta; gamma "));
        assert_eq!(
            tags,
            vec![
                String::from("alpha"),
                String::from("beta"),
                String::from("gamma")
            ]
        );
    }

    #[test]
    fn parse_tags_handles_empty() {
        let tags = parse_tags(Some("   "));
        assert!(tags.is_empty());
    }

    #[test]
    fn normalize_status_handles_known_states() {
        assert_eq!(VmStatus::normalize(Some("running")), VmStatus::Running);
        assert_eq!(VmStatus::normalize(Some("stopped")), VmStatus::Stopped);
        assert_eq!(VmStatus::normalize(Some("paused")), VmStatus::Unknown);
        assert_eq!(VmStatus::normalize(None), VmStatus::Unknown);
    }
}
