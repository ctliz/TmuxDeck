use crate::commands::get_tmux_sessions;
use crate::config::load_config;
use crate::registry::detect_environment;

pub fn is_zh_locale() -> bool {
    sys_locale::get_locale()
        .map(|l| l.to_lowercase().starts_with("zh"))
        .unwrap_or(false)
}

pub fn tr(key: &str) -> String {
    if !is_zh_locale() {
        return key.to_string();
    }
    match key {
        "No Active Workspaces" => "无活动工作区".to_string(),
        "+ New Workspace..." => "+ 新建工作区...".to_string(),
        "TmuxDeck Main Window" => "TmuxDeck 主界面".to_string(),
        "Quit TmuxDeck" => "退出 TmuxDeck".to_string(),
        "Open ({})" => "打开 ({})".to_string(),
        "Add Pane" => "新增分屏".to_string(),
        "Add Pane with Agent" => "使用 Agent 新增分屏".to_string(),
        "{} (Recommended)" => "{}（推荐）".to_string(),
        "agent.shell" => "纯 Shell".to_string(),
        "agent.custom" => "自定义 Agent".to_string(),
        "View All ({} total)..." => "查看全部（共 {} 个）...".to_string(),
        _ => key.to_string(),
    }
}

fn add_pane_agent_menu_id(workspace: &str, agent_id: &str) -> String {
    format!("addpane-agent:{}:{}", workspace, agent_id)
}

fn agent_display_name(name: &str) -> String {
    match name {
        "agent.shell" => {
            if is_zh_locale() { "纯 Shell".to_string() } else { "Plain Shell".to_string() }
        }
        "agent.custom" => {
            if is_zh_locale() { "自定义 Agent".to_string() } else { "Custom Agent".to_string() }
        }
        _ => name.to_string(),
    }
}

fn resolve_pane_agent_id(
    pane: &crate::tmux::TmuxPane,
    agents: &[crate::registry::ToolInfo],
) -> Option<String> {
    if let Some(declared) = pane.agent_id.as_deref().map(str::trim).filter(|id| !id.is_empty()) {
        return (declared != "shell").then(|| declared.to_string());
    }
    agents.iter().find_map(|agent| {
        (agent.id != "shell"
            && (pane.command.contains(&agent.id)
                || (!agent.path.is_empty() && pane.command.contains(&agent.path))))
        .then(|| agent.id.clone())
    })
}

fn dominant_agent_id(
    panes: &[crate::tmux::TmuxPane],
    agents: &[crate::registry::ToolInfo],
) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for pane in panes {
        let Some(agent_id) = resolve_pane_agent_id(pane, agents) else {
            continue;
        };
        if let Some((_, count)) = counts.iter_mut().find(|(id, _)| id == &agent_id) {
            *count += 1;
        } else {
            counts.push((agent_id, 1));
        }
    }
    counts
        .into_iter()
        .reduce(|best, item| if item.1 > best.1 { item } else { best })
        .map(|(id, _)| id)
}

pub fn build_tray_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<tauri::menu::Menu<R>, Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

    let cfg = load_config();
    let default_terminal = if !cfg.default_terminal.is_empty() {
        cfg.default_terminal
    } else {
        "ghostty".to_string()
    };

    let sessions = get_tmux_sessions().unwrap_or_default();
    let menu = MenuBuilder::new(app);

    if sessions.is_empty() {
        let no_sess_item = MenuItemBuilder::with_id("no-sessions", tr("No Active Workspaces"))
            .enabled(false)
            .build(app)?;
        let new_item =
            MenuItemBuilder::with_id("new-workspace", tr("+ New Workspace...")).build(app)?;
        let show_item =
            MenuItemBuilder::with_id("show-main", tr("TmuxDeck Main Window")).build(app)?;
        let quit_item = MenuItemBuilder::with_id("quit", tr("Quit TmuxDeck")).build(app)?;

        return Ok(menu
            .item(&no_sess_item)
            .separator()
            .item(&new_item)
            .separator()
            .item(&show_item)
            .item(&quit_item)
            .build()?);
    }

    let mut sorted_sessions = sessions.clone();
    sorted_sessions.sort_by(|a, b| {
        if a.attached != b.attached {
            return b.attached.cmp(&a.attached);
        }
        b.last_active_ts.cmp(&a.last_active_ts)
    });

    let primary = &sorted_sessions[0];
    let icon_dot = if primary.attached { "●" } else { "○" };
    let active_header_title = format!("{} Active: {}", icon_dot, primary.name);

    let active_open = MenuItemBuilder::with_id(
        format!("open:{}", primary.name),
        tr("Open ({})").replace("{}", &default_terminal),
    )
    .build(app)?;
    let environment = detect_environment();
    let recommended = dominant_agent_id(&primary.panes, &environment.agents);
    let mut add_pane_menu = SubmenuBuilder::new(app, tr("Add Pane with Agent"));
    for agent in &environment.agents {
        let display_name = agent_display_name(&agent.name);
        let title = if recommended.as_deref() == Some(agent.id.as_str()) {
            tr("{} (Recommended)").replace("{}", &display_name)
        } else {
            display_name
        };
        add_pane_menu = add_pane_menu.item(
            &MenuItemBuilder::with_id(
                add_pane_agent_menu_id(&primary.name, &agent.id),
                title,
            )
            .build(app)?,
        );
    }
    let add_pane_menu = add_pane_menu.build()?;

    let active_submenu = SubmenuBuilder::new(app, active_header_title)
        .item(&active_open)
        .item(&add_pane_menu)
        .build()?;

    let mut menu = menu.item(&active_submenu).separator();

    let limit = 8;
    for session in sorted_sessions.iter().take(limit) {
        let sess_dot = if session.attached { "●" } else { "○" };
        let title = format!("{} {}", sess_dot, session.name);
        let item = MenuItemBuilder::with_id(format!("open:{}", session.name), title).build(app)?;
        menu = menu.item(&item);
    }

    if sorted_sessions.len() > limit {
        let more_title = tr("View All ({} total)...").replace("{}", &sorted_sessions.len().to_string());
        let view_more_item = MenuItemBuilder::with_id("show-main", more_title).build(app)?;
        menu = menu.item(&view_more_item);
    }

    let menu = menu.separator();

    let new_item = MenuItemBuilder::with_id("new-workspace", tr("+ New Workspace...")).build(app)?;
    let show_item = MenuItemBuilder::with_id("show-main", tr("TmuxDeck Main Window")).build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", tr("Quit TmuxDeck")).build(app)?;

    let menu = menu
        .item(&new_item)
        .separator()
        .item(&show_item)
        .item(&quit_item);

    Ok(menu.build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(agent_id: Option<&str>, command: &str) -> crate::tmux::TmuxPane {
        crate::tmux::TmuxPane {
            id: "%1".into(),
            command: command.into(),
            agent_id: agent_id.map(String::from),
            active: false,
            session_target: "workspace".into(),
            slot: None,
            attached: false,
        }
    }

    fn agents() -> Vec<crate::registry::ToolInfo> {
        vec![
            crate::registry::ToolInfo { id: "pi".into(), name: "Pi".into(), path: "/bin/pi".into(), icon_path: None },
            crate::registry::ToolInfo { id: "claude".into(), name: "Claude Code".into(), path: "/bin/claude".into(), icon_path: None },
            crate::registry::ToolInfo { id: "shell".into(), name: "agent.shell".into(), path: "/bin/zsh".into(), icon_path: None },
        ]
    }

    #[test]
    fn dominant_agent_prefers_metadata_and_first_seen_tie() {
        let panes = vec![
            pane(Some("claude"), "0.10.0"),
            pane(Some("pi"), "bash"),
            pane(None, "/bin/pi"),
            pane(Some("shell"), "zsh"),
        ];
        assert_eq!(dominant_agent_id(&panes, &agents()), Some("pi".into()));
        assert_eq!(
            dominant_agent_id(&[pane(Some("claude"), "x"), pane(Some("pi"), "x")], &agents()),
            Some("claude".into())
        );
        assert_eq!(dominant_agent_id(&[pane(Some("shell"), "zsh")], &agents()), None);
    }

    #[test]
    fn add_pane_agent_menu_id_encodes_workspace_and_agent() {
        assert_eq!(
            add_pane_agent_menu_id("project-alpha", "claude"),
            "addpane-agent:project-alpha:claude"
        );
    }
}
