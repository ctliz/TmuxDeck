use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use crate::agent_terminal_actor::{spawn as spawn_pty_actor, PtyActorHandle};
use crate::ghostty_vt::{self as ghostty, ffi};
use crate::tmux::{check_tmux_installed, run_tmux, validate_pane_id};

pub const EVENT_TERMINAL_FRAME: &str = "agent-terminal-frame";
pub const EVENT_TERMINAL_EXIT: &str = "agent-terminal-exit";

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCellPayload {
    pub text: String,
    pub fg: String,
    pub bg: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCursorPayload {
    pub x: u16,
    pub y: u16,
    pub visible: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalRowUpdatePayload {
    pub row: u16,
    pub cells: Vec<TerminalCellPayload>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalScrollbarPayload {
    pub total: usize,
    pub offset: usize,
    pub length: usize,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalFramePayload {
    pub terminal_id: String,
    pub cols: u16,
    pub rows: u16,
    pub full: bool,
    pub updates: Vec<TerminalRowUpdatePayload>,
    pub cursor: TerminalCursorPayload,
    pub mouse_reporting: bool,
    pub scrollbar: TerminalScrollbarPayload,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExitPayload {
    pub terminal_id: String,
}

fn color_hex(color: ghostty::RgbColor) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

struct GhosttyCore {
    terminal: ghostty::Terminal,
    render_state: ghostty::RenderState,
    row_iterator: ghostty::RowIterator,
    row_cells: ghostty::RowCells,
    key_encoder: ghostty::KeyEncoder,
    mouse_encoder: ghostty::MouseEncoder,
    last_mouse_reporting: Option<bool>,
    last_scrollbar: Option<TerminalScrollbarPayload>,
}

impl GhosttyCore {
    fn new(
        cols: u16,
        rows: u16,
        write_pty: impl Fn(Vec<u8>) + Send + 'static,
    ) -> Result<Self, String> {
        let mut terminal = ghostty::Terminal::new(cols, rows, 5000)
            .map_err(|error| format!("ERR_GHOSTTY_TERMINAL|{error}"))?;
        terminal
            .set_write_pty_callback(move |bytes| write_pty(bytes.to_vec()))
            .map_err(|error| format!("ERR_GHOSTTY_CALLBACK|{error}"))?;

        Ok(Self {
            terminal,
            render_state: ghostty::RenderState::new()
                .map_err(|error| format!("ERR_GHOSTTY_RENDER|{error}"))?,
            row_iterator: ghostty::RowIterator::new()
                .map_err(|error| format!("ERR_GHOSTTY_ROWS|{error}"))?,
            row_cells: ghostty::RowCells::new()
                .map_err(|error| format!("ERR_GHOSTTY_CELLS|{error}"))?,
            key_encoder: ghostty::KeyEncoder::new()
                .map_err(|error| format!("ERR_GHOSTTY_KEY_ENCODER|{error}"))?,
            mouse_encoder: ghostty::MouseEncoder::new()
                .map_err(|error| format!("ERR_GHOSTTY_MOUSE_ENCODER|{error}"))?,
            last_mouse_reporting: None,
            last_scrollbar: None,
        })
    }

    fn process(&mut self, bytes: &[u8]) {
        self.terminal.write(bytes);
    }

    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), String> {
        self.terminal
            .resize(cols, rows, 1, 1)
            .map_err(|error| format!("ERR_GHOSTTY_RESIZE|{error}"))
    }

    fn paste(&self, data: &str) -> Vec<u8> {
        if self.terminal.mode_get(2004).unwrap_or(false) {
            let mut bytes = Vec::with_capacity(data.len() + 12);
            bytes.extend_from_slice(b"\x1b[200~");
            bytes.extend_from_slice(data.as_bytes());
            bytes.extend_from_slice(b"\x1b[201~");
            bytes
        } else {
            data.as_bytes().to_vec()
        }
    }

    fn focus(&self, focused: bool) -> Result<Vec<u8>, String> {
        if !self.terminal.mode_get(1004).unwrap_or(false) {
            return Ok(Vec::new());
        }
        ghostty::encode_focus(if focused {
            ghostty::FocusEvent::Gained
        } else {
            ghostty::FocusEvent::Lost
        })
        .map_err(|error| format!("ERR_GHOSTTY_FOCUS_ENCODE|{error}"))
    }

    fn mouse(
        &mut self,
        action: &str,
        button: i8,
        x: f32,
        y: f32,
        cell_width: f32,
        cell_height: f32,
        ctrl: bool,
        alt: bool,
        shift: bool,
        meta: bool,
    ) -> Result<Vec<u8>, String> {
        if !self.terminal.mouse_tracking_enabled().unwrap_or(false) {
            return Ok(Vec::new());
        }

        let action = match action {
            "press" => ghostty::MOUSE_ACTION_PRESS,
            "release" => ghostty::MOUSE_ACTION_RELEASE,
            "motion" => ghostty::MOUSE_ACTION_MOTION,
            _ => return Err("ERR_INVALID_MOUSE_ACTION".to_string()),
        };
        let button = match button {
            -1 => None,
            0 => Some(ghostty::MOUSE_BUTTON_LEFT),
            1 => Some(ghostty::MOUSE_BUTTON_MIDDLE),
            2 => Some(ghostty::MOUSE_BUTTON_RIGHT),
            3 => Some(ghostty::MOUSE_BUTTON_WHEEL_UP),
            4 => Some(ghostty::MOUSE_BUTTON_WHEEL_DOWN),
            _ => return Err("ERR_INVALID_MOUSE_BUTTON".to_string()),
        };

        let cell_width = cell_width.round().max(1.0) as u32;
        let cell_height = cell_height.round().max(1.0) as u32;
        let cols = u32::from(self.terminal.cols().unwrap_or(1));
        let rows = u32::from(self.terminal.rows().unwrap_or(1));
        self.mouse_encoder.set_from_terminal(&self.terminal);
        self.mouse_encoder.set_size(
            cols.saturating_mul(cell_width),
            rows.saturating_mul(cell_height),
            cell_width,
            cell_height,
        );

        let mut event = ghostty::MouseEvent::new()
            .map_err(|error| format!("ERR_GHOSTTY_MOUSE_EVENT|{error}"))?;
        event.set_action(action);
        if let Some(button) = button {
            event.set_button(button);
        } else {
            event.clear_button();
        }
        let mut mods = 0u16;
        if shift {
            mods |= ghostty::MOD_SHIFT;
        }
        if ctrl {
            mods |= ghostty::MOD_CTRL;
        }
        if alt {
            mods |= ghostty::MOD_ALT;
        }
        if meta {
            mods |= ghostty::MOD_SUPER;
        }
        event.set_mods(mods);
        event.set_position(x.max(0.0), y.max(0.0));
        self.mouse_encoder
            .encode(&event)
            .map_err(|error| format!("ERR_GHOSTTY_MOUSE_ENCODE|{error}"))
    }

    fn scroll(&mut self, delta: isize) {
        self.terminal.scroll_viewport_delta(delta);
    }

    fn copy_selection(
        &self,
        start_x: u16,
        start_y: u16,
        end_x: u16,
        end_y: u16,
        rectangle: bool,
    ) -> Result<String, String> {
        let cols = self.terminal.cols().unwrap_or(1).max(1);
        let rows = self.terminal.rows().unwrap_or(1).max(1);
        let mut start = (start_x.min(cols - 1), start_y.min(rows - 1));
        let mut end = (end_x.min(cols - 1), end_y.min(rows - 1));
        if (start.1, start.0) > (end.1, end.0) {
            std::mem::swap(&mut start, &mut end);
        }
        self.terminal
            .read_text_viewport(
                (start.0, u32::from(start.1)),
                (end.0, u32::from(end.1)),
                rectangle,
            )
            .map_err(|error| format!("ERR_GHOSTTY_COPY|{error}"))
    }

    fn encode_key(
        &mut self,
        code: &str,
        key: &str,
        ctrl: bool,
        alt: bool,
        shift: bool,
        meta: bool,
    ) -> Result<Vec<u8>, String> {
        let Some(physical_key) = ghostty_key_from_dom_code(code) else {
            return Err("ERR_UNSUPPORTED_KEY".to_string());
        };

        self.key_encoder.set_from_terminal(&self.terminal);
        let mut event =
            ghostty::KeyEvent::new().map_err(|error| format!("ERR_GHOSTTY_KEY_EVENT|{error}"))?;
        event.set_action(ffi::GhosttyKeyAction_GHOSTTY_KEY_ACTION_PRESS);
        event.set_key(physical_key);

        let mut mods = 0u16;
        if shift {
            mods |= ghostty::MOD_SHIFT;
        }
        if ctrl {
            mods |= ghostty::MOD_CTRL;
        }
        if alt {
            mods |= ghostty::MOD_ALT;
        }
        if meta {
            mods |= ghostty::MOD_SUPER;
        }
        event.set_mods(mods);

        if key.chars().count() == 1 && !key.chars().next().is_some_and(char::is_control) {
            event.set_utf8(key);
        }
        if let Some(codepoint) = unshifted_codepoint(code) {
            event.set_unshifted_codepoint(codepoint);
        }

        self.key_encoder
            .encode(&event)
            .map_err(|error| format!("ERR_GHOSTTY_KEY_ENCODE|{error}"))
    }

    fn frame(
        &mut self,
        terminal_id: &str,
        force_full: bool,
    ) -> Result<Option<TerminalFramePayload>, String> {
        self.render_state
            .update(&self.terminal)
            .map_err(|error| format!("ERR_GHOSTTY_RENDER_UPDATE|{error}"))?;

        let mouse_reporting = self.terminal.mouse_tracking_enabled().unwrap_or(false);
        let scrollbar = self
            .terminal
            .scrollbar()
            .map(|scrollbar| TerminalScrollbarPayload {
                total: scrollbar.total,
                offset: scrollbar.offset,
                length: scrollbar.len,
            })
            .unwrap_or(TerminalScrollbarPayload {
                total: 0,
                offset: 0,
                length: 0,
            });
        let metadata_changed = self.last_mouse_reporting != Some(mouse_reporting)
            || self.last_scrollbar.as_ref() != Some(&scrollbar);

        let dirty = self
            .render_state
            .dirty()
            .map_err(|error| format!("ERR_GHOSTTY_DIRTY|{error}"))?;
        let full = force_full || dirty == ghostty::Dirty::Full;
        if !full && dirty == ghostty::Dirty::Clean && !metadata_changed {
            return Ok(None);
        }

        let cols = self
            .render_state
            .cols()
            .map_err(|error| format!("ERR_GHOSTTY_COLS|{error}"))?;
        let rows = self
            .render_state
            .rows()
            .map_err(|error| format!("ERR_GHOSTTY_ROWS|{error}"))?;
        let colors = self
            .render_state
            .colors()
            .map_err(|error| format!("ERR_GHOSTTY_COLORS|{error}"))?;
        let default_fg = color_hex(colors.foreground);
        let default_bg = color_hex(colors.background);

        let cursor_position = self
            .render_state
            .cursor_viewport()
            .map_err(|error| format!("ERR_GHOSTTY_CURSOR|{error}"))?;
        let cursor = TerminalCursorPayload {
            x: cursor_position.map(|cursor| cursor.x).unwrap_or(0),
            y: cursor_position.map(|cursor| cursor.y).unwrap_or(0),
            visible: cursor_position.is_some()
                && self.render_state.cursor_visible().unwrap_or(false),
        };

        let mut updates = Vec::new();
        {
            let mut rows_iter = self
                .render_state
                .populate_row_iterator(&mut self.row_iterator)
                .map_err(|error| format!("ERR_GHOSTTY_ROW_ITERATOR|{error}"))?;

            for y in 0..rows {
                if !rows_iter.next() {
                    if full {
                        updates.push(TerminalRowUpdatePayload {
                            row: y,
                            cells: (0..cols)
                                .map(|_| blank_cell(&default_fg, &default_bg))
                                .collect(),
                        });
                    }
                    continue;
                }

                let row_dirty = rows_iter
                    .dirty()
                    .map_err(|error| format!("ERR_GHOSTTY_ROW_DIRTY|{error}"))?;
                if full || row_dirty {
                    let cells = {
                        let mut row = rows_iter
                            .populate_cells(&mut self.row_cells)
                            .map_err(|error| format!("ERR_GHOSTTY_ROW_CELLS|{error}"))?;
                        let mut cells = Vec::with_capacity(usize::from(cols));
                        for _ in 0..cols {
                            if !row.next() {
                                cells.push(blank_cell(&default_fg, &default_bg));
                                continue;
                            }

                            let basic = row
                                .basic_data()
                                .map_err(|error| format!("ERR_GHOSTTY_CELL_STYLE|{error}"))?;
                            let mut text = row.grapheme_text().unwrap_or_default();
                            if basic.style.invisible
                                || matches!(basic.wide, ghostty::CellWide::SpacerTail)
                            {
                                text.clear();
                            }

                            let mut fg = row
                                .fg_color()
                                .ok()
                                .flatten()
                                .map(color_hex)
                                .unwrap_or_else(|| default_fg.clone());
                            let mut bg = row
                                .bg_color()
                                .ok()
                                .flatten()
                                .map(color_hex)
                                .unwrap_or_else(|| default_bg.clone());
                            if basic.style.inverse {
                                std::mem::swap(&mut fg, &mut bg);
                            }

                            cells.push(TerminalCellPayload {
                                text,
                                fg,
                                bg,
                                bold: basic.style.bold,
                                italic: basic.style.italic,
                                underline: basic.style.underlined,
                            });
                        }
                        cells
                    };
                    updates.push(TerminalRowUpdatePayload { row: y, cells });
                    rows_iter
                        .clear_dirty()
                        .map_err(|error| format!("ERR_GHOSTTY_CLEAR_ROW_DIRTY|{error}"))?;
                }
            }
        }

        self.render_state
            .set_dirty(ghostty::Dirty::Clean)
            .map_err(|error| format!("ERR_GHOSTTY_CLEAR_DIRTY|{error}"))?;
        self.last_mouse_reporting = Some(mouse_reporting);
        self.last_scrollbar = Some(scrollbar.clone());

        Ok(Some(TerminalFramePayload {
            terminal_id: terminal_id.to_string(),
            cols,
            rows,
            full,
            updates,
            cursor,
            mouse_reporting,
            scrollbar,
        }))
    }
}

fn blank_cell(fg: &str, bg: &str) -> TerminalCellPayload {
    TerminalCellPayload {
        text: String::new(),
        fg: fg.to_string(),
        bg: bg.to_string(),
        bold: false,
        italic: false,
        underline: false,
    }
}

fn ghostty_key_from_dom_code(code: &str) -> Option<u32> {
    use ffi::*;
    let key = match code {
        "Backquote" => GhosttyKey_GHOSTTY_KEY_BACKQUOTE,
        "Backslash" => GhosttyKey_GHOSTTY_KEY_BACKSLASH,
        "BracketLeft" => GhosttyKey_GHOSTTY_KEY_BRACKET_LEFT,
        "BracketRight" => GhosttyKey_GHOSTTY_KEY_BRACKET_RIGHT,
        "Comma" => GhosttyKey_GHOSTTY_KEY_COMMA,
        "Equal" => GhosttyKey_GHOSTTY_KEY_EQUAL,
        "Minus" => GhosttyKey_GHOSTTY_KEY_MINUS,
        "Period" => GhosttyKey_GHOSTTY_KEY_PERIOD,
        "Quote" => GhosttyKey_GHOSTTY_KEY_QUOTE,
        "Semicolon" => GhosttyKey_GHOSTTY_KEY_SEMICOLON,
        "Slash" => GhosttyKey_GHOSTTY_KEY_SLASH,
        "Backspace" => GhosttyKey_GHOSTTY_KEY_BACKSPACE,
        "Enter" | "NumpadEnter" => GhosttyKey_GHOSTTY_KEY_ENTER,
        "Space" => GhosttyKey_GHOSTTY_KEY_SPACE,
        "Tab" => GhosttyKey_GHOSTTY_KEY_TAB,
        "Delete" => GhosttyKey_GHOSTTY_KEY_DELETE,
        "End" => GhosttyKey_GHOSTTY_KEY_END,
        "Home" => GhosttyKey_GHOSTTY_KEY_HOME,
        "Insert" => GhosttyKey_GHOSTTY_KEY_INSERT,
        "PageDown" => GhosttyKey_GHOSTTY_KEY_PAGE_DOWN,
        "PageUp" => GhosttyKey_GHOSTTY_KEY_PAGE_UP,
        "ArrowDown" => GhosttyKey_GHOSTTY_KEY_ARROW_DOWN,
        "ArrowLeft" => GhosttyKey_GHOSTTY_KEY_ARROW_LEFT,
        "ArrowRight" => GhosttyKey_GHOSTTY_KEY_ARROW_RIGHT,
        "ArrowUp" => GhosttyKey_GHOSTTY_KEY_ARROW_UP,
        "Escape" => GhosttyKey_GHOSTTY_KEY_ESCAPE,
        _ if code.len() == 4 && code.starts_with("Key") => {
            let letter = code.as_bytes()[3];
            if !letter.is_ascii_uppercase() {
                return None;
            }
            GhosttyKey_GHOSTTY_KEY_A + u32::from(letter - b'A')
        }
        _ if code.len() == 6 && code.starts_with("Digit") => {
            let digit = code.as_bytes()[5];
            if !digit.is_ascii_digit() {
                return None;
            }
            GhosttyKey_GHOSTTY_KEY_DIGIT_0 + u32::from(digit - b'0')
        }
        _ if let Some(number) = code
            .strip_prefix('F')
            .and_then(|value| value.parse::<u32>().ok()) =>
        {
            if !(1..=25).contains(&number) {
                return None;
            }
            GhosttyKey_GHOSTTY_KEY_F1 + number - 1
        }
        _ => return None,
    };
    Some(key)
}

fn unshifted_codepoint(code: &str) -> Option<u32> {
    if code.len() == 4 && code.starts_with("Key") {
        return Some(u32::from(code.as_bytes()[3].to_ascii_lowercase()));
    }
    if code.len() == 6 && code.starts_with("Digit") {
        return Some(u32::from(code.as_bytes()[5]));
    }
    match code {
        "Space" => Some(' ' as u32),
        "Backquote" => Some('`' as u32),
        "Backslash" => Some('\\' as u32),
        "BracketLeft" => Some('[' as u32),
        "BracketRight" => Some(']' as u32),
        "Comma" => Some(',' as u32),
        "Equal" => Some('=' as u32),
        "Minus" => Some('-' as u32),
        "Period" => Some('.' as u32),
        "Quote" => Some('\'' as u32),
        "Semicolon" => Some(';' as u32),
        "Slash" => Some('/' as u32),
        _ => None,
    }
}

struct ZoomRollback {
    pane_id: String,
    active: bool,
}

impl Drop for ZoomRollback {
    fn drop(&mut self) {
        if self.active {
            let _ = run_tmux(&["resize-pane", "-t", &self.pane_id, "-Z"]);
        }
    }
}

struct TerminalSession {
    pane_id: String,
    zoomed_by_us: bool,
    actor: PtyActorHandle,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    core: Mutex<GhosttyCore>,
}

#[derive(Default)]
pub struct AgentTerminalManager {
    terminals: Mutex<HashMap<String, Arc<TerminalSession>>>,
}

pub type AgentTerminalState = Arc<AgentTerminalManager>;

fn parse_session_and_zoom(output: &str) -> Result<(String, bool), String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err("ERR_SESSION_PARSE|empty output".to_string());
    }
    let (session, zoomed) = if let Some((s, z)) = trimmed.split_once('|') {
        (s, z)
    } else if let Some((s, z)) = trimmed.split_once('\t') {
        (s, z)
    } else if let Some((s, z)) = trimmed.split_once("\\t") {
        (s, z)
    } else if let Some((s, z)) = trimmed.split_once(' ') {
        (s, z)
    } else {
        return Err(format!("ERR_SESSION_PARSE|{}", trimmed));
    };

    let session = session.trim();
    if session.is_empty() {
        return Err("ERR_SESSION_NAME_EMPTY".to_string());
    }
    Ok((session.to_string(), zoomed.trim() == "1"))
}

fn query_pane_session_and_zoom(pane_id: &str) -> Result<(String, bool), String> {
    if !validate_pane_id(pane_id) {
        return Err("ERR_INVALID_PANE_ID".to_string());
    }
    let out = run_tmux(&[
        "display-message",
        "-p",
        "-t",
        pane_id,
        "#{session_name}|#{window_zoomed_flag}",
    ])
    .map_err(|error| format!("ERR_TMUX_EXEC|{error}"))?;

    if !out.status.success() {
        return Err(format!(
            "ERR_PANE_NOT_FOUND|{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    parse_session_and_zoom(&String::from_utf8_lossy(&out.stdout))
}

impl AgentTerminalManager {
    pub fn open_terminal(
        self: &Arc<Self>,
        app: &AppHandle,
        terminal_id: &str,
        pane_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(), String> {
        let terminal_id = terminal_id.trim();
        if terminal_id.is_empty() {
            return Err("ERR_TERMINAL_ID_EMPTY".to_string());
        }
        if self
            .terminals
            .lock()
            .map_err(|_| "ERR_LOCK".to_string())?
            .contains_key(terminal_id)
        {
            return Err("ERR_TERMINAL_ID_DUPLICATE".to_string());
        }

        let (session_name, _) = query_pane_session_and_zoom(pane_id)?;
        let _ = run_tmux(&["select-pane", "-t", pane_id]);
        let _ = run_tmux(&["set-option", "-t", &session_name, "status", "off"]);
        let _ = run_tmux(&["set-option", "-t", &session_name, "mouse", "on"]);
        let _ = run_tmux(&["set-option", "-t", &session_name, "window-size", "latest"]);
        let _ = run_tmux(&["set-option", "-t", &session_name, "fill-character", " "]);
        let _ = run_tmux(&["set-option", "-s", "set-clipboard", "on"]);
        let _ = run_tmux(&[
            "bind-key",
            "-T",
            "copy-mode",
            "MouseDragEnd1Pane",
            "send-keys",
            "-X",
            "copy-selection",
        ]);
        let _ = run_tmux(&[
            "bind-key",
            "-T",
            "copy-mode-vi",
            "MouseDragEnd1Pane",
            "send-keys",
            "-X",
            "copy-selection",
        ]);
        let _ = run_tmux(&[
            "set-window-option",
            "-t",
            &session_name,
            "aggressive-resize",
            "on",
        ]);
        let (_, currently_zoomed) =
            query_pane_session_and_zoom(pane_id).unwrap_or((session_name.clone(), false));
        let zoomed_by_us = if !currently_zoomed {
            run_tmux(&["resize-pane", "-t", pane_id, "-Z"])
                .map(|output| output.status.success())
                .unwrap_or(false)
        } else {
            false
        };
        let mut zoom_rollback = ZoomRollback {
            pane_id: pane_id.to_string(),
            active: zoomed_by_us,
        };

        let cols = cols.max(1);
        let rows = rows.max(1);
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("ERR_OPEN_PTY|{error}"))?;

        #[cfg(target_os = "windows")]
        let mut command = {
            let mut cmd = CommandBuilder::new("wsl.exe");
            cmd.args(["--", "tmux", "attach-session", "-t", &session_name]);
            cmd
        };
        #[cfg(not(target_os = "windows"))]
        let mut command = {
            let tmux_bin = check_tmux_installed().unwrap_or_else(|| "tmux".to_string());
            let mut cmd = CommandBuilder::new(&tmux_bin);
            cmd.args(["attach-session", "-t", &session_name]);
            cmd.env("PATH", crate::commands::build_augmented_path());
            cmd
        };

        command.env_remove("TMUX");
        command.env_remove("TMUX_PANE");
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "TmuxDeck");

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("ERR_SPAWN_PTY|{error}"))?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("ERR_PTY_READER|{error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("ERR_PTY_WRITER|{error}"))?;
        let actor = spawn_pty_actor(pair.master, writer);
        let callback_actor = actor.clone();
        let core = GhosttyCore::new(cols, rows, move |bytes| {
            callback_actor.write_async(bytes);
        })?;

        let session = Arc::new(TerminalSession {
            pane_id: pane_id.to_string(),
            zoomed_by_us,
            actor,
            child: Mutex::new(child),
            core: Mutex::new(core),
        });
        self.terminals
            .lock()
            .map_err(|_| "ERR_LOCK".to_string())?
            .insert(terminal_id.to_string(), Arc::clone(&session));
        zoom_rollback.active = false;

        let app = app.clone();
        let manager = Arc::clone(self);
        let terminal_id = terminal_id.to_string();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 16 * 1024];
            loop {
                let count = match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                let frame = session.core.lock().ok().and_then(|mut core| {
                    core.process(&buffer[..count]);
                    core.frame(&terminal_id, false).ok().flatten()
                });
                if let Some(frame) = frame {
                    let _ = app.emit(EVENT_TERMINAL_FRAME, frame);
                }
            }
            let _ = manager.close_terminal_internal(&terminal_id);
            let _ = app.emit(
                EVENT_TERMINAL_EXIT,
                TerminalExitPayload {
                    terminal_id: terminal_id.clone(),
                },
            );
        });

        Ok(())
    }

    pub fn write_terminal(&self, terminal_id: &str, data: &str) -> Result<(), String> {
        self.session(terminal_id)?
            .actor
            .write(data.as_bytes().to_vec())
    }

    pub fn paste_terminal(&self, terminal_id: &str, data: &str) -> Result<(), String> {
        let session = self.session(terminal_id)?;
        let bytes = session
            .core
            .lock()
            .map_err(|_| "ERR_LOCK".to_string())?
            .paste(data);
        session.actor.write(bytes)
    }

    pub fn focus_terminal(&self, terminal_id: &str, focused: bool) -> Result<(), String> {
        let session = self.session(terminal_id)?;
        let bytes = session
            .core
            .lock()
            .map_err(|_| "ERR_LOCK".to_string())?
            .focus(focused)?;
        session.actor.write(bytes)
    }

    pub fn key_terminal(
        &self,
        terminal_id: &str,
        code: &str,
        key: &str,
        ctrl: bool,
        alt: bool,
        shift: bool,
        meta: bool,
    ) -> Result<(), String> {
        let session = self.session(terminal_id)?;
        let bytes = session
            .core
            .lock()
            .map_err(|_| "ERR_LOCK".to_string())?
            .encode_key(code, key, ctrl, alt, shift, meta)?;
        session.actor.write(bytes)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn mouse_terminal(
        &self,
        terminal_id: &str,
        action: &str,
        button: i8,
        x: f32,
        y: f32,
        cell_width: f32,
        cell_height: f32,
        ctrl: bool,
        alt: bool,
        shift: bool,
        meta: bool,
    ) -> Result<(), String> {
        let session = self.session(terminal_id)?;
        let bytes = session
            .core
            .lock()
            .map_err(|_| "ERR_LOCK".to_string())?
            .mouse(
                action,
                button,
                x,
                y,
                cell_width,
                cell_height,
                ctrl,
                alt,
                shift,
                meta,
            )?;
        session.actor.write(bytes)
    }

    pub fn scroll_terminal(
        &self,
        app: &AppHandle,
        terminal_id: &str,
        delta: isize,
    ) -> Result<(), String> {
        let session = self.session(terminal_id)?;
        let frame = {
            let mut core = session.core.lock().map_err(|_| "ERR_LOCK".to_string())?;
            if delta >= 900_000 {
                core.terminal.scroll_viewport_bottom();
            } else {
                core.scroll(delta);
            }
            core.frame(terminal_id, true)?
                .ok_or_else(|| "ERR_GHOSTTY_EMPTY_FRAME".to_string())?
        };
        app.emit(EVENT_TERMINAL_FRAME, frame)
            .map_err(|error| format!("ERR_FRAME_EMIT|{error}"))
    }

    pub fn copy_terminal(
        &self,
        terminal_id: &str,
        start_x: u16,
        start_y: u16,
        end_x: u16,
        end_y: u16,
        rectangle: bool,
    ) -> Result<String, String> {
        self.session(terminal_id)?
            .core
            .lock()
            .map_err(|_| "ERR_LOCK".to_string())?
            .copy_selection(start_x, start_y, end_x, end_y, rectangle)
    }

    pub fn resize_terminal(
        &self,
        app: &AppHandle,
        terminal_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(), String> {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let session = self.session(terminal_id)?;
        session.actor.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let frame = {
            let mut core = session.core.lock().map_err(|_| "ERR_LOCK".to_string())?;
            core.resize(cols, rows)?;
            core.frame(terminal_id, true)?
                .ok_or_else(|| "ERR_GHOSTTY_EMPTY_FRAME".to_string())?
        };
        app.emit(EVENT_TERMINAL_FRAME, frame)
            .map_err(|error| format!("ERR_FRAME_EMIT|{error}"))
    }

    pub fn close_terminal(&self, terminal_id: &str) -> Result<(), String> {
        self.close_terminal_internal(terminal_id);
        Ok(())
    }

    fn session(&self, terminal_id: &str) -> Result<Arc<TerminalSession>, String> {
        self.terminals
            .lock()
            .map_err(|_| "ERR_LOCK".to_string())?
            .get(terminal_id)
            .cloned()
            .ok_or_else(|| "ERR_TERMINAL_NOT_FOUND".to_string())
    }

    fn close_terminal_internal(&self, terminal_id: &str) -> Option<Arc<TerminalSession>> {
        let session = self.terminals.lock().ok()?.remove(terminal_id);
        if let Some(session) = &session {
            session.actor.shutdown();
            if let Ok(mut child) = session.child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        session
    }

    #[cfg(test)]
    pub fn active_terminal_count(&self) -> usize {
        self.terminals
            .lock()
            .map(|terminals| terminals.len())
            .unwrap_or(0)
    }
}

#[tauri::command]
pub fn open_agent_terminal(
    app: AppHandle,
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
    pane_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.open_terminal(&app, &terminal_id, &pane_id, cols, rows)
}

#[tauri::command]
pub fn write_agent_terminal(
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
    data: String,
) -> Result<(), String> {
    state.write_terminal(&terminal_id, &data)
}

#[tauri::command]
pub fn paste_agent_terminal(
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
    data: String,
) -> Result<(), String> {
    state.paste_terminal(&terminal_id, &data)
}

#[tauri::command]
pub fn focus_agent_terminal(
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
    focused: bool,
) -> Result<(), String> {
    state.focus_terminal(&terminal_id, focused)
}

#[tauri::command]
pub fn key_agent_terminal(
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
    code: String,
    key: String,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
) -> Result<(), String> {
    state.key_terminal(&terminal_id, &code, &key, ctrl, alt, shift, meta)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn mouse_agent_terminal(
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
    action: String,
    button: i8,
    x: f32,
    y: f32,
    cell_width: f32,
    cell_height: f32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
) -> Result<(), String> {
    state.mouse_terminal(
        &terminal_id,
        &action,
        button,
        x,
        y,
        cell_width,
        cell_height,
        ctrl,
        alt,
        shift,
        meta,
    )
}

#[tauri::command]
pub fn scroll_agent_terminal(
    app: AppHandle,
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
    delta: isize,
) -> Result<(), String> {
    state.scroll_terminal(&app, &terminal_id, delta)
}

#[tauri::command]
pub fn copy_agent_terminal(
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
    start_x: u16,
    start_y: u16,
    end_x: u16,
    end_y: u16,
    rectangle: bool,
) -> Result<String, String> {
    state.copy_terminal(&terminal_id, start_x, start_y, end_x, end_y, rectangle)
}

#[tauri::command]
pub fn resize_agent_terminal(
    app: AppHandle,
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.resize_terminal(&app, &terminal_id, cols, rows)
}

#[tauri::command]
pub fn close_agent_terminal(
    state: tauri::State<AgentTerminalState>,
    terminal_id: String,
) -> Result<(), String> {
    state.close_terminal(&terminal_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_frame_serializes_for_the_canvas() {
        let frame = TerminalFramePayload {
            terminal_id: "term_1".to_string(),
            cols: 1,
            rows: 1,
            full: true,
            updates: vec![TerminalRowUpdatePayload {
                row: 0,
                cells: vec![TerminalCellPayload {
                    text: "A".to_string(),
                    fg: "#ffffff".to_string(),
                    bg: "#000000".to_string(),
                    bold: true,
                    italic: false,
                    underline: false,
                }],
            }],
            cursor: TerminalCursorPayload {
                x: 0,
                y: 0,
                visible: true,
            },
            mouse_reporting: false,
            scrollbar: TerminalScrollbarPayload {
                total: 1,
                offset: 0,
                length: 1,
            },
        };
        let json = serde_json::to_value(frame).unwrap();
        assert_eq!(json["terminalId"], "term_1");
        assert_eq!(json["full"], true);
        assert_eq!(json["updates"][0]["cells"][0]["text"], "A");
        assert_eq!(json["mouseReporting"], false);
        assert_eq!(json["scrollbar"]["length"], 1);
    }

    #[test]
    fn dom_key_codes_map_to_ghostty_keys() {
        assert_eq!(
            ghostty_key_from_dom_code("KeyA"),
            Some(ffi::GhosttyKey_GHOSTTY_KEY_A)
        );
        assert_eq!(
            ghostty_key_from_dom_code("Digit9"),
            Some(ffi::GhosttyKey_GHOSTTY_KEY_DIGIT_9)
        );
        assert_eq!(
            ghostty_key_from_dom_code("ArrowUp"),
            Some(ffi::GhosttyKey_GHOSTTY_KEY_ARROW_UP)
        );
        assert_eq!(
            ghostty_key_from_dom_code("F12"),
            Some(ffi::GhosttyKey_GHOSTTY_KEY_F12)
        );
        assert_eq!(ghostty_key_from_dom_code("Unknown"), None);
    }

    #[test]
    fn test_parse_session_and_zoom() {
        assert_eq!(
            parse_session_and_zoom("my_session|0\n"),
            Ok(("my_session".to_string(), false))
        );
        assert_eq!(
            parse_session_and_zoom("cutter-team__td_slot_01|1"),
            Ok(("cutter-team__td_slot_01".to_string(), true))
        );
        assert_eq!(
            parse_session_and_zoom("cutter-team\t1\n"),
            Ok(("cutter-team".to_string(), true))
        );
        assert_eq!(
            parse_session_and_zoom("cutter-team\\t0"),
            Ok(("cutter-team".to_string(), false))
        );
        assert_eq!(
            parse_session_and_zoom("cutter-team 0"),
            Ok(("cutter-team".to_string(), false))
        );
        assert!(parse_session_and_zoom("").is_err());
        assert!(parse_session_and_zoom("  \n").is_err());
    }

    #[test]
    fn invalid_pane_id_is_rejected() {
        assert!(query_pane_session_and_zoom("invalid").is_err());
        assert!(query_pane_session_and_zoom("").is_err());
    }

    #[test]
    fn missing_terminal_operations_fail_cleanly() {
        let manager = AgentTerminalManager::default();
        assert_eq!(
            manager.write_terminal("missing", "data"),
            Err("ERR_TERMINAL_NOT_FOUND".to_string())
        );
        assert_eq!(manager.close_terminal("missing"), Ok(()));
        assert_eq!(manager.active_terminal_count(), 0);
    }

    #[test]
    fn ghostty_core_parses_and_renders_text() {
        let mut core = GhosttyCore::new(8, 2, |_| {}).unwrap();
        core.process(b"hello");
        let frame = core.frame("term_test", true).unwrap().unwrap();
        assert_eq!(frame.cols, 8);
        assert_eq!(frame.rows, 2);
        assert!(frame.full);
        assert_eq!(frame.updates[0].cells[0].text, "h");
        assert_eq!(frame.updates[0].cells[4].text, "o");
        assert!(core.frame("term_test", false).unwrap().is_none());

        core.process(b"!");
        let patch = core.frame("term_test", false).unwrap().unwrap();
        assert!(!patch.full);
        assert_eq!(patch.updates.len(), 1);
        assert_eq!(patch.updates[0].row, 0);
    }

    #[test]
    fn mouse_reporting_changes_emit_metadata_and_encode_mouse() {
        let mut core = GhosttyCore::new(8, 2, |_| {}).unwrap();
        core.frame("term_test", true).unwrap();
        core.process(b"\x1b[?1000h\x1b[?1006h");

        let patch = core.frame("term_test", false).unwrap().unwrap();
        assert!(patch.mouse_reporting);
        let encoded = core
            .mouse("press", 0, 0.0, 0.0, 1.0, 1.0, false, false, false, false)
            .unwrap();
        assert_eq!(encoded, b"\x1b[<0;1;1M");
    }

    #[test]
    fn ghostty_copies_viewport_selection() {
        let mut core = GhosttyCore::new(8, 2, |_| {}).unwrap();
        core.process(b"hello");
        assert_eq!(core.copy_selection(0, 0, 4, 0, false).unwrap(), "hello");
        assert_eq!(core.copy_selection(4, 0, 0, 0, false).unwrap(), "hello");
    }

    #[test]
    fn terminal_queries_are_answered_without_the_frontend() {
        let responses = Arc::new(Mutex::new(Vec::new()));
        let callback_responses = Arc::clone(&responses);
        let mut core = GhosttyCore::new(8, 2, move |bytes| {
            callback_responses.lock().unwrap().extend(bytes);
        })
        .unwrap();
        core.process(b"\x1b[6n");
        assert!(String::from_utf8_lossy(&responses.lock().unwrap()).contains('R'));
    }

    #[test]
    fn test_encode_enter() {
        let mut core = GhosttyCore::new(80, 24, |_| {}).unwrap();
        let enter_bytes = core.encode_key("Enter", "Enter", false, false, false, false).unwrap();
        assert_eq!(enter_bytes, b"\r");

        let shift_enter = core.encode_key("Enter", "Enter", false, false, true, false).unwrap();
        assert!(!shift_enter.is_empty());
    }
}
