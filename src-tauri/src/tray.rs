use crate::commands::get_tmux_sessions;
use crate::config::load_config;

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
        "View All ({} total)..." => "查看全部（共 {} 个）...".to_string(),
        _ => key.to_string(),
    }
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
    let active_add_pane = MenuItemBuilder::with_id(format!("addpane:{}", primary.name), tr("Add Pane")).build(app)?;

    let active_submenu = SubmenuBuilder::new(app, active_header_title)
        .item(&active_open)
        .item(&active_add_pane)
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
