use tauri::Emitter;

mod bridge;
mod commands;
mod config;
mod engine;
mod intercom;
mod models;
mod registry;
mod tmux;
mod transcript;
mod transport;
mod tray;

pub use bridge::*;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            #[cfg(desktop)]
            {
                use tauri::tray::TrayIconBuilder;
                use tauri::Manager;

                // v1.14：启动桥接引擎（WebSocket 传输 + intercom 接入 + 对话表维护）
                let bridge_state = crate::engine::spawn_bridge();
                app.manage(bridge_state);

                let handle = app.handle().clone();

                if let Ok(initial_menu) = build_tray_menu(&handle) {
                    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                        .expect("failed to load tray icon bytes");
                    let _tray = TrayIconBuilder::with_id("main")
                        .icon(tray_icon)
                        .icon_as_template(true)
                        .menu(&initial_menu)
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
                            } else if event_id.starts_with("addpane:") {
                                let session_name = &event_id[8..];
                                let _ = add_pane(session_name.to_string());
                            } else if event_id == "new-workspace" || event_id == "show-main" {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                    if event_id == "new-workspace" {
                                        let _ = window.emit("trigger-new-workspace", ());
                                    }
                                }
                            } else if event_id == "quit" {
                                app.exit(0);
                            }
                        })
                        .build(app);
                }

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            to_wsl_path,
            detect_environment,
            load_config,
            save_config,
            create_session,
            open_session,
            get_tmux_sessions,
            kill_session,
            rename_session,
            capture_pane,
            add_pane,
            kill_pane,
            get_terminal_icon,
            send_pane_text,
            send_pane_key,
            list_panes,
            swap_pane,
            bridge_pairing,
            bridge_conversations
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
    fn test_run_tmux_smoke() {
        if check_tmux_installed().is_some() {
            let res = run_tmux(&["list-sessions"]);
            assert!(res.is_ok(), "run_tmux failed to execute command");
            let output = res.unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            // tmux exits non-zero when no server is running yet; both outcomes are valid.
            assert!(
                output.status.success() || is_no_server_err(&stderr),
                "tmux should either succeed or report no server, stderr: {}",
                stderr
            );
        }
    }
}
