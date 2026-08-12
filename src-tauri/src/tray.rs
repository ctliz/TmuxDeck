use crate::commands::get_tmux_sessions;
use crate::config::load_config;
use crate::registry::detect_environment;

pub const TRAY_PANEL_LABEL: &str = "tray-panel";
pub const MAIN_WINDOW_LABEL: &str = "main";

/// 把面板摆到托盘图标正下方，并夹在当前显示器内——菜单栏图标常常靠近右上角，
/// 不夹一下面板会有一半跑到屏幕外。
pub fn position_tray_panel<R: tauri::Runtime>(win: &tauri::WebviewWindow<R>, rect: tauri::Rect) {
    const MARGIN: f64 = 8.0;
    /// 面板与菜单栏之间留一点缝，阴影才有落点；贴死了会显得像菜单栏被撑高了。
    const GAP_BELOW_MENUBAR: f64 = 6.0;

    let scale = win.scale_factor().unwrap_or(1.0);
    let icon_pos = rect.position.to_physical::<f64>(scale);
    let icon_size = rect.size.to_physical::<f64>(scale);
    let Ok(panel) = win.outer_size() else { return };

    let mut x = icon_pos.x + icon_size.width / 2.0 - f64::from(panel.width) / 2.0;
    let y = icon_pos.y + icon_size.height + GAP_BELOW_MENUBAR;

    if let Ok(Some(monitor)) = win.current_monitor() {
        let m_pos = monitor.position();
        let m_size = monitor.size();
        let min_x = f64::from(m_pos.x) + MARGIN;
        let max_x = f64::from(m_pos.x) + f64::from(m_size.width) - f64::from(panel.width) - MARGIN;
        if max_x >= min_x {
            x = x.clamp(min_x, max_x);
        }
    }

    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
}

pub fn hide_tray_panel<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri::Manager;
    if let Some(panel) = app.get_webview_window(TRAY_PANEL_LABEL) {
        let _ = panel.hide();
    }
}

// 面板显示/失焦隐藏的时间戳，用来化解两个时序竞态（见下面两个函数的注释）。
static PANEL_SHOWN_AT_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PANEL_BLUR_HID_AT_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 显示后的抑制窗口：show + set_focus 之间系统可能先抛一次 Focused(false)，
/// 不挡掉的话面板会刚弹出就自己消失。
const SHOW_SETTLE_MS: u64 = 250;
/// 失焦隐藏后的抑制窗口：见 `tray_click_should_open`。
const REOPEN_GUARD_MS: u64 = 350;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn elapsed_since(stamp: &std::sync::atomic::AtomicU64) -> u64 {
    now_ms().saturating_sub(stamp.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn mark_panel_shown() {
    PANEL_SHOWN_AT_MS.store(now_ms(), std::sync::atomic::Ordering::Relaxed);
}

/// 面板失焦时是否真的该隐藏。刚 show 出来的瞬间收到的失焦要忽略。
pub fn blur_should_hide_panel() -> bool {
    if elapsed_since(&PANEL_SHOWN_AT_MS) < SHOW_SETTLE_MS {
        return false;
    }
    PANEL_BLUR_HID_AT_MS.store(now_ms(), std::sync::atomic::Ordering::Relaxed);
    true
}

/// 左键点击托盘图标时该不该弹面板。
///
/// 面板开着时再点图标，macOS 会先把焦点给菜单栏 —— 失焦处理器已经把面板隐藏了，
/// 等我们的点击处理器跑到时窗口已经不可见，直接判断 `is_visible` 会立刻重新弹出，
/// 于是「再点一次关闭」永远失效。所以刚被失焦隐藏过的一小段时间内，把这次点击
/// 当作「关闭」而不是「打开」。
pub fn tray_click_should_open(panel_visible: bool) -> bool {
    !panel_visible && elapsed_since(&PANEL_BLUR_HID_AT_MS) >= REOPEN_GUARD_MS
}

/// 显示并聚焦主窗口，可选地触发新建工作区流程。
/// 原生右键菜单与面板共用同一条路径，避免两处各写一遍再各自漂移。
pub fn focus_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>, new_workspace: bool) {
    use tauri::{Emitter, Manager};
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    let _ = window.show();
    let _ = window.set_focus();
    if new_workspace {
        let _ = window.emit("trigger-new-workspace", ());
    }
}

/// 面板里的「主界面 / 新建工作区」。面板本身随即收起，行为对齐原生菜单。
#[tauri::command]
pub fn panel_show_main(app: tauri::AppHandle, new_workspace: bool) {
    hide_tray_panel(&app);
    focus_main_window(&app, new_workspace);
}

#[tauri::command]
pub fn panel_hide(app: tauri::AppHandle) {
    hide_tray_panel(&app);
}

#[tauri::command]
pub fn panel_quit(app: tauri::AppHandle) {
    app.exit(0);
}

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

    /// 这几个时序守卫是纯粹的状态机，值得钉死——它们坏掉的表现是「面板一闪就没」
    /// 或者「关不掉」，都很难靠肉眼回归发现。
    #[test]
    fn panel_visibility_guards_resolve_races() {
        use std::sync::atomic::Ordering;

        // 面板已经可见 -> 这次点击是关闭，不该走打开分支。
        assert!(!tray_click_should_open(true));

        // 刚显示出来就收到失焦：忽略，否则面板刚弹出就自己消失。
        mark_panel_shown();
        assert!(!blur_should_hide_panel());

        // 显示已「坐稳」后的失焦：正常隐藏。
        PANEL_SHOWN_AT_MS.store(now_ms() - SHOW_SETTLE_MS - 10, Ordering::Relaxed);
        assert!(blur_should_hide_panel());

        // 上一步的失焦隐藏刚发生 -> 紧接着的托盘点击应判定为「关闭」而非重新打开。
        assert!(!tray_click_should_open(false));

        // 守卫窗口过去之后，点击图标才重新是「打开」。
        PANEL_BLUR_HID_AT_MS.store(now_ms() - REOPEN_GUARD_MS - 10, Ordering::Relaxed);
        assert!(tray_click_should_open(false));
    }

    #[test]
    fn add_pane_agent_menu_id_encodes_workspace_and_agent() {
        assert_eq!(
            add_pane_agent_menu_id("project-alpha", "claude"),
            "addpane-agent:project-alpha:claude"
        );
    }
}
