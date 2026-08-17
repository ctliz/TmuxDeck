use tauri::Emitter;

mod audit;
mod bridge;
mod bridge_state;
mod claude_adapter;
mod commands;
mod config;
mod connection;
mod engine;
mod intercom;
mod models;
mod notify;
mod registry;
mod scope;
mod team;
mod tmux;
mod transcript;
mod transport;
mod tray;
mod usage;

pub use bridge::*;
pub use bridge_state::*;
pub use claude_adapter::*;
pub use commands::*;
pub use config::*;
pub use engine::*;
pub use intercom::*;
pub use models::*;
pub use registry::*;
pub use tmux::*;
pub use transcript::*;
pub use transport::*;
pub use tray::*;
pub use usage::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| match event {
            // 关闭只隐藏，App 常驻菜单栏。
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
                // 主窗口被 orderOut 后，AppKit 会把 key window 交给 App 里的下一个窗口，
                // 而「成为 key」会顺带把已隐藏的托盘面板 order front —— 表现为关掉主窗口
                // 面板就自己冒出来。面板只应由点击托盘图标打开，这里兜住这条意外路径。
                if window.label() == MAIN_WINDOW_LABEL {
                    use tauri::Manager;
                    hide_tray_panel(window.app_handle());
                }
            }
            // 托盘面板失焦即收起，对齐原生菜单「点击别处就关」的直觉。
            tauri::WindowEvent::Focused(false)
                if window.label() == TRAY_PANEL_LABEL && blur_should_hide_panel() =>
            {
                let _ = window.hide();
            }
            _ => {}
        })
        .setup(|app| {
            #[cfg(desktop)]
            {
                let _ = crate::team::reconcile_orphan_manifests();
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
                use tauri::Manager;

                // v1.14：启动桥接引擎（WebSocket 传输 + intercom 接入 + 对话表维护）
                let bridge_state = crate::bridge_state::spawn_bridge(app.handle().clone());
                app.manage(bridge_state);

                let handle = app.handle().clone();

                // 菜单在所有平台都构建（启动时一次，很便宜），但只在非 macOS 上挂到图标上，
                // 原因见下方注释。macOS 上它仅作为保留代码路径存在。
                if let Ok(_initial_menu) = build_tray_menu(&handle) {
                    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                        .expect("failed to load tray icon bytes");

                    #[allow(unused_mut)]
                    let mut builder = TrayIconBuilder::with_id("main")
                        .icon(tray_icon)
                        .icon_as_template(true);

                    // macOS 上原生菜单与点击驱动的面板无法共存于同一个图标：
                    // NSStatusItem 一旦 setMenu，AppKit 会在 mouseDown 时自己弹菜单，
                    // tray-icon 用来拦截点击的覆盖视图根本收不到事件，show_menu_on_left_click(false)
                    // 也就形同虚设（实测只有 Enter/Move，没有 Click）。所以这里不挂菜单。
                    // 其他平台没有这个限制，继续保留原生菜单作为兜底路径。
                    #[cfg(not(target_os = "macos"))]
                    {
                        builder = builder.menu(&_initial_menu).show_menu_on_left_click(false);
                    }

                    let _tray = builder
                        .on_tray_icon_event(|tray, event| {
                            // macOS 上原生菜单未挂载，右键若不接管就完全没反应，
                            // 所以左右键都开面板；其他平台右键留给原生菜单。
                            let TrayIconEvent::Click {
                                button,
                                button_state: MouseButtonState::Up,
                                rect,
                                ..
                            } = event
                            else {
                                return;
                            };
                            let opens_panel = cfg!(target_os = "macos")
                                && (button == MouseButton::Left || button == MouseButton::Right);
                            if !opens_panel {
                                return;
                            }
                            let app = tray.app_handle();
                            let Some(panel) = app.get_webview_window(TRAY_PANEL_LABEL) else {
                                return;
                            };
                            let visible = panel.is_visible().unwrap_or(false);
                            if !tray_click_should_open(visible) {
                                if visible {
                                    let _ = panel.hide();
                                }
                                return;
                            }
                            position_tray_panel(&panel, rect);
                            mark_panel_shown();
                            let _ = panel.show();
                            let _ = panel.set_focus();
                            // 通知前端立刻拉一次最新数据，而不是等下一个轮询周期。
                            let _ = panel.emit("tray-panel-opened", ());
                        })
                        .on_menu_event(|app, event| {
                            let event_id = event.id().as_ref();
                            if event_id.starts_with("open:") {
                                let session_name = &event_id[5..];
                                let cfg = load_config();
                                let term = if !cfg.default_terminal.is_empty() {
                                    cfg.default_terminal
                                } else {
                                    "ghostty".to_string()
                                };
                                let _ = open_session(session_name.to_string(), term);
                            } else if event_id.starts_with("addpane-agent:") {
                                let mut parts = event_id.splitn(3, ':');
                                if let (Some("addpane-agent"), Some(session_name), Some(agent_id)) =
                                    (parts.next(), parts.next(), parts.next())
                                {
                                    let _ = add_pane(
                                        session_name.to_string(),
                                        Some(agent_id.to_string()),
                                    );
                                }
                            } else if event_id.starts_with("addpane:") {
                                let session_name = &event_id[8..];
                                let _ = add_pane(session_name.to_string(), None);
                            } else if event_id == "new-workspace" || event_id == "show-main" {
                                focus_main_window(app, event_id == "new-workspace");
                            } else if event_id == "quit" {
                                app.exit(0);
                            }
                        })
                        .build(app);
                }

                // Agent token 用量：冷启动首轮要扫日志（Codex 近 1GB），必须放后台，
                // 采集完通过事件通知面板，前端全程读快照不阻塞。
                #[cfg(target_os = "macos")]
                {
                    let usage_handle = app.handle().clone();
                    std::thread::spawn(move || loop {
                        let snapshot = crate::usage::refresh_usage_snapshot();
                        let _ = usage_handle.emit("usage-updated", &snapshot);
                        std::thread::sleep(std::time::Duration::from_secs(60));
                    });
                }

                // 只有真正挂了原生菜单的平台才需要定时重建它；macOS 上菜单未挂载，
                // 面板自己按需拉数据，这个线程纯属空转。
                #[cfg(not(target_os = "macos"))]
                {
                    let refresh_handle = app.handle().clone();
                    std::thread::spawn(move || loop {
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        if let Some(tray) = refresh_handle.tray_by_id("main") {
                            if let Ok(new_menu) = build_tray_menu(&refresh_handle) {
                                let _ = tray.set_menu(Some(new_menu));
                            }
                        }
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            to_wsl_path,
            detect_environment,
            get_managed_claude_status,
            install_managed_claude,
            use_managed_claude,
            use_standard_claude,
            load_config,
            save_config,
            create_session,
            open_session,
            get_tmux_sessions,
            kill_session,
            rename_session,
            capture_pane,
            add_pane,
            add_panes,
            kill_pane,
            kill_slot,
            get_terminal_icon,
            send_pane_text,
            send_pane_key,
            list_panes,
            swap_pane,
            swap_native_slots,
            bridge_pairing,
            bridge_conversations,
            get_usage_snapshot,
            panel_show_main,
            panel_hide,
            panel_quit,
            check_workspace_adapters,
            apply_workspace_install_plan
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                if !has_visible_windows {
                    crate::notify::open_from_notification(app);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_session_name() {
        assert_eq!(sanitize_session_name(""), Err("ERR_NAME_EMPTY".to_string()));
        assert_eq!(sanitize_session_name("   "), Err("ERR_NAME_EMPTY".to_string()));

        assert_eq!(sanitize_session_name("foo@bar#baz!"), Ok("foo-bar-baz".to_string()));
        assert_eq!(sanitize_session_name("  hello   world  "), Ok("hello-world".to_string()));

        let long_name = "a".repeat(70);
        let result = sanitize_session_name(&long_name).unwrap();
        assert_eq!(result.len(), 60);
        assert_eq!(result, "a".repeat(60));

        assert_eq!(sanitize_session_name("---"), Err("ERR_NAME_INVALID".to_string()));
        assert_eq!(sanitize_session_name("!!!"), Err("ERR_NAME_INVALID".to_string()));
    }

    #[test]
    fn test_validate_pane_id() {
        assert!(validate_pane_id("%123"));
        assert!(validate_pane_id("%0"));

        assert!(!validate_pane_id("%abc"));
        assert!(!validate_pane_id(""));
        assert!(!validate_pane_id("123"));
        assert!(!validate_pane_id("%"));
    }

    #[test]
    fn test_swap_pane_validation() {
        assert_eq!(swap_panes("invalid", "%1"), Err("ERR_PANE_INVALID".to_string()));
        assert_eq!(swap_panes("%1", "invalid"), Err("ERR_PANE_INVALID".to_string()));
    }

    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("\x1b[31mHello\x1b[0m"), "Hello");
        assert_eq!(strip_ansi("\x1b[1m\x1b[32mNested\x1b[0m"), "Nested");
        assert_eq!(strip_ansi("Plain text"), "Plain text");
    }

    #[test]
    fn test_is_no_server_err() {
        assert!(is_no_server_err("no server running on /private/tmp/tmux-501/default"));
        assert!(is_no_server_err("error connecting to /private/tmp/tmux-501/default (No such file or directory)"));
        assert!(is_no_server_err("failed to connect to server"));
        assert!(is_no_server_err("No such file or directory (tmux socket)"));

        assert!(!is_no_server_err("can't find session: foo"));
        assert!(!is_no_server_err("duplicate session: bar"));
        assert!(!is_no_server_err(""));
    }

    #[test]
    fn test_is_session_missing_err() {
        // 正例：tmux 对已消失 session 的典型报错
        assert!(is_session_missing_err("can't find session: Tmux-Deck"));
        assert!(is_session_missing_err("invalid or unknown session: %foo"));
        assert!(is_session_missing_err("no session"));
        assert!(is_session_missing_err("can't find session: alpha__td_slot_inside__td_slot_01"));

        // 反例：其他错误 / 空串
        assert!(!is_session_missing_err("no server running on /tmp/tmux-501/default"));
        assert!(!is_session_missing_err("error connecting to /tmp/tmux-501/default"));
        assert!(!is_session_missing_err("duplicate session: bar"));
        assert!(!is_session_missing_err(""));

        // 与 is_no_server_err 互斥
        assert!(!is_no_server_err("can't find session: foo"));
        assert!(!is_session_missing_err("no server running on /tmp/tmux-501/default"));
    }

    #[test]
    fn test_run_tmux_smoke() {
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(tmux_path) = check_tmux_installed() {
                let mut bytes = [0u8; 4];
                let _ = getrandom::getrandom(&mut bytes);
                let nonce: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                let socket = format!("tmuxdeck-test-{}-{nonce}", std::process::id());
                let res = std::process::Command::new(&tmux_path)
                    .args(["-L", &socket, "list-sessions"])
                    .output();
                assert!(res.is_ok(), "isolated tmux execution failed");
                let output = res.unwrap();
                let stderr = String::from_utf8_lossy(&output.stderr);
                // tmux exits non-zero when no server is running yet; both outcomes are valid.
                assert!(
                    output.status.success() || is_no_server_err(&stderr),
                    "isolated tmux should either succeed or report no server, stderr: {stderr}"
                );
                // Clean up any isolated server that might have been spawned
                let _ = std::process::Command::new(&tmux_path)
                    .args(["-L", &socket, "kill-server"])
                    .output();
            }
        }
    }
}
