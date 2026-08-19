use crate::tmux::TmuxPane;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateOpts {
    pub name: String,
    pub dir: Option<String>,
    pub agent_id: String,
    #[serde(default)]
    pub pane_agent_ids: Vec<String>,
    pub panes: u8,
    pub terminal_id: String,
    #[serde(default)]
    pub headless: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_opts_deserialization_defaults_headless_to_false() {
        let json_without_headless = r#"{
            "name": "project-101",
            "dir": "/tmp",
            "agent_id": "pi",
            "panes": 4,
            "terminal_id": "ghostty"
        }"#;
        let opts: CreateOpts =
            serde_json::from_str(json_without_headless).expect("deserialize without headless");
        assert!(!opts.headless);
        assert_eq!(opts.pane_agent_ids.len(), 0);

        let json_with_headless_true = r#"{
            "name": "project-102",
            "dir": null,
            "agent_id": "claude",
            "panes": 2,
            "terminal_id": "terminal",
            "headless": true
        }"#;
        let opts_true: CreateOpts =
            serde_json::from_str(json_with_headless_true).expect("deserialize with headless true");
        assert!(opts_true.headless);

        let json_with_headless_false = r#"{
            "name": "project-103",
            "dir": null,
            "agent_id": "codex",
            "panes": 1,
            "terminal_id": "ghostty",
            "headless": false
        }"#;
        let opts_false: CreateOpts = serde_json::from_str(json_with_headless_false)
            .expect("deserialize with headless false");
        assert!(!opts_false.headless);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TmuxSession {
    pub id: String,
    pub name: String,
    pub windows_count: usize,
    pub panes_count: usize,
    pub attached: bool,
    pub created_at: String,
    pub last_active_ts: i64,
    pub panes: Vec<TmuxPane>,
    pub native_split: bool,
    pub terminal_id: Option<String>,
}
