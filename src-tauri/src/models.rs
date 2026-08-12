use serde::{Deserialize, Serialize};
use crate::tmux::TmuxPane;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateOpts {
    pub name: String,
    pub dir: Option<String>,
    pub agent_id: String,
    #[serde(default)]
    pub pane_agent_ids: Vec<String>,
    pub panes: u8,
    pub terminal_id: String,
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
