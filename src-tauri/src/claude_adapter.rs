use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::config::get_config_dir;

pub const MANAGED_CLAUDE_VERSION: &str = "0.13.0-connect.1";
pub const MANAGED_CLAUDE_RESOURCE: &str = "ctliz-agent-intercom-claude-0.13.0-connect.1.tgz";
pub const MANAGED_CLAUDE_SHA256: &str =
    "a766f4631d92df3dc26ee81f9bec06da38c3c09bae9ea4c6b0ef3975eeeb96ba";
pub const MANAGED_ADAPTER_MARKER: &str = "0.13.0-connect.1";

const REQUIRED_FILES: &[&str] = &[
    ".claude-plugin/plugin.json",
    ".mcp.json",
    "monitors/monitors.json",
    "dist/inbox-monitor.mjs",
    "dist/claude-server.mjs",
    "dist/cci.mjs",
];
const HEALTH_CACHE_TTL: Duration = Duration::from_secs(5);
static HEALTH_CACHE: OnceLock<Mutex<Option<(Instant, ManagedClaudeState)>>> = OnceLock::new();

const RUNTIME_FILES: &[&str] = &[
    "dist/inbox-monitor.mjs",
    "dist/claude-server.mjs",
    "dist/cci.mjs",
];

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedClaudeState {
    NotInstalled,
    NeedsRepair,
    Healthy,
    Unavailable,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManagedClaudeStatus {
    pub state: ManagedClaudeState,
    pub version: String,
    pub path: Option<String>,
    pub standard_claude_available: bool,
    pub using_standard: bool,
}

#[derive(Serialize, Deserialize)]
struct InstallManifest {
    version: String,
    sha256: String,
    files: Vec<InstalledFile>,
}

#[derive(Serialize, Deserialize)]
struct InstalledFile {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
struct PluginManifest {
    monitors: String,
    #[serde(rename = "mcpServers")]
    mcp_servers: String,
}

#[derive(Deserialize)]
struct MonitorDefinition {
    command: String,
    when: String,
}

#[derive(Deserialize)]
struct MonitorsConfig(Vec<MonitorDefinition>);

#[derive(Deserialize)]
struct McpConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: std::collections::HashMap<String, McpServer>,
}

#[derive(Deserialize)]
struct McpServer {
    command: String,
    args: Vec<String>,
}

pub fn managed_claude_root() -> PathBuf {
    get_config_dir()
        .join("managed")
        .join("claude-intercom")
        .join(MANAGED_CLAUDE_VERSION)
}

pub fn managed_cci_path() -> PathBuf {
    managed_claude_root().join("dist").join("cci.mjs")
}

#[allow(dead_code)]
fn manifest_path(root: &Path) -> PathBuf {
    root.join("tmuxdeck-managed.json")
}

fn sha256(path: &Path) -> Result<String, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("ERR_MANAGED_CLAUDE_VERIFY|{}", error))?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn validate_plugin_chain(root: &Path) -> Result<(), String> {
    let plugin: PluginManifest = serde_json::from_slice(
        &std::fs::read(root.join(".claude-plugin/plugin.json"))
            .map_err(|error| format!("plugin manifest: {error}"))?,
    )
    .map_err(|error| format!("plugin manifest: {error}"))?;
    if plugin.monitors != "./monitors/monitors.json" || plugin.mcp_servers != "./.mcp.json" {
        return Err("plugin manifest does not point to the managed Monitor and MCP config".into());
    }

    let monitors: Vec<MonitorDefinition> = serde_json::from_slice::<MonitorsConfig>(
        &std::fs::read(root.join("monitors/monitors.json"))
            .map_err(|error| format!("monitor config: {error}"))?,
    )
    .map_err(|error| format!("monitor config: {error}"))?
    .0;
    if monitors.len() != 1
        || monitors[0].when != "always"
        || monitors[0].command != "node \"${CLAUDE_PLUGIN_ROOT}/dist/inbox-monitor.mjs\""
    {
        return Err("monitor config does not invoke the managed inbox runtime".into());
    }

    let mcp: McpConfig = serde_json::from_slice(
        &std::fs::read(root.join(".mcp.json")).map_err(|error| format!("MCP config: {error}"))?,
    )
    .map_err(|error| format!("MCP config: {error}"))?;
    let Some(server) = mcp.mcp_servers.get("claude-intercom") else {
        return Err("MCP config is missing claude-intercom".into());
    };
    if server.command != "node" || server.args != ["${CLAUDE_PLUGIN_ROOT}/dist/claude-server.mjs"] {
        return Err("MCP config does not invoke the managed server runtime".into());
    }
    Ok(())
}

#[allow(dead_code)]
fn verify_installed_files(root: &Path, manifest: &InstallManifest) -> Result<(), String> {
    if manifest.version != MANAGED_CLAUDE_VERSION || manifest.sha256 != MANAGED_CLAUDE_SHA256 {
        return Err("managed manifest version or artifact digest does not match".into());
    }
    if manifest.files.len() != REQUIRED_FILES.len() {
        return Err("managed manifest file inventory is incomplete".into());
    }
    for relative in REQUIRED_FILES {
        let Some(installed) = manifest.files.iter().find(|file| file.path == *relative) else {
            return Err(format!("managed manifest is missing {relative}"));
        };
        let path = root.join(relative);
        if !path.is_file() || sha256(&path)? != installed.sha256 {
            return Err(format!(
                "managed file failed integrity verification: {relative}"
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(root.join("dist/cci.mjs"))
            .map(|metadata| metadata.permissions().mode() & 0o111 == 0)
            .unwrap_or(true)
        {
            return Err("managed cci runtime is not executable".into());
        }
    }
    Ok(())
}

fn node_binary() -> Result<String, String> {
    crate::registry::find_agent_binary("node")
        .ok_or_else(|| "Node.js runtime was not found in TmuxDeck's executable search paths".into())
}

fn smoke_test_monitor(root: &Path, node: &str) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::time::Duration;

    let inbox = std::env::temp_dir().join(format!(
        "tmuxdeck-monitor-health-{}-{}.jsonl",
        std::process::id(),
        random_hex(4)?
    ));
    std::fs::write(&inbox, "").map_err(|error| error.to_string())?;
    let mut child = Command::new(node)
        .arg(root.join("dist/inbox-monitor.mjs"))
        .env("CLAUDE_INTERCOM_INBOX", &inbox)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start the inbox monitor: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "could not capture the inbox monitor output".to_string())?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let line = BufReader::new(stdout).lines().next().transpose();
        let _ = tx.send(line);
    });
    std::thread::sleep(Duration::from_millis(150));
    std::fs::write(
        &inbox,
        "{\"ts\":1,\"fromId\":\"healthcheck\",\"messageId\":\"m1\",\"expectsReply\":false,\"text\":\"tmuxdeck-monitor-ok\"}\n",
    )
    .map_err(|error| error.to_string())?;
    let result = rx.recv_timeout(Duration::from_secs(3));
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(inbox);
    match result {
        Ok(Ok(Some(line)))
            if line.contains("Intercom message from healthcheck")
                && line.contains("tmuxdeck-monitor-ok") =>
        {
            Ok(())
        }
        Ok(Ok(Some(line))) => Err(format!("inbox monitor emitted unexpected output: {line}")),
        Ok(Ok(None)) => Err("inbox monitor exited without emitting the health message".into()),
        Ok(Err(error)) => Err(format!("could not read inbox monitor output: {error}")),
        Err(_) => Err("inbox monitor did not deliver the health message".into()),
    }
}

fn validate_runtime(root: &Path) -> Result<(), String> {
    validate_plugin_chain(root)?;
    let node = node_binary()?;
    for relative in RUNTIME_FILES {
        let output = Command::new(&node)
            .arg("--check")
            .arg(root.join(relative))
            .output()
            .map_err(|error| format!("could not run node for {relative}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "managed JavaScript runtime is invalid ({relative}): {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    smoke_test_monitor(root, &node)?;
    let Some(claude) = crate::registry::find_agent_binary("claude") else {
        return Err("Claude Code is not installed".into());
    };
    let augmented_path = crate::commands::build_augmented_path_for_command(&claude);
    let output = Command::new(&claude)
        .args(["plugin", "validate"])
        .arg(root)
        .env("PATH", augmented_path)
        .output()
        .map_err(|error| format!("could not validate the Claude plugin: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Claude rejected the managed plugin: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn healthy_root(root: &Path) -> bool {
    crate::commands::adapter::verify_managed_root_integrity(
        root,
        "claude-intercom",
        MANAGED_CLAUDE_VERSION,
        crate::commands::adapter::CLAUDE_IMMUTABLE_DIGESTS,
        "@ctliz/agent-intercom-claude",
        MANAGED_CLAUDE_RESOURCE,
        MANAGED_CLAUDE_SHA256,
    ) && validate_runtime(root).is_ok()
}

fn health_cache() -> &'static Mutex<Option<(Instant, ManagedClaudeState)>> {
    HEALTH_CACHE.get_or_init(|| Mutex::new(None))
}

pub fn invalidate_managed_claude_health_cache() {
    if let Ok(mut cache) = health_cache().lock() {
        *cache = None;
    }
}

pub fn managed_claude_state() -> ManagedClaudeState {
    if !cfg!(target_os = "macos") {
        return ManagedClaudeState::Unavailable;
    }
    let Ok(mut cache) = health_cache().lock() else {
        return ManagedClaudeState::NeedsRepair;
    };
    if let Some((checked_at, state)) = *cache {
        if checked_at.elapsed() < HEALTH_CACHE_TTL {
            return state;
        }
    }
    // Keep the mutex during validation so simultaneous detect_environment and
    // get_managed_claude_status calls share one heavy health check.
    let root = managed_claude_root();
    let state = if healthy_root(&root) {
        ManagedClaudeState::Healthy
    } else if root.exists() {
        ManagedClaudeState::NeedsRepair
    } else {
        ManagedClaudeState::NotInstalled
    };
    *cache = Some((Instant::now(), state));
    state
}

pub fn healthy_managed_cci() -> Option<String> {
    (managed_claude_state() == ManagedClaudeState::Healthy)
        .then(|| managed_cci_path().to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_managed_claude_status() -> ManagedClaudeStatus {
    let state = managed_claude_state();
    let standard_claude_available = crate::registry::find_agent_binary("claude").is_some();
    ManagedClaudeStatus {
        state,
        version: MANAGED_CLAUDE_VERSION.to_string(),
        path: (state == ManagedClaudeState::Healthy)
            .then(|| managed_cci_path().to_string_lossy().to_string()),
        standard_claude_available,
        using_standard: crate::config::load_config().use_standard_claude
            && standard_claude_available,
    }
}

#[allow(dead_code)]
#[cfg(target_os = "macos")]
fn bundled_resource_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("ERR_MANAGED_CLAUDE_RESOURCE|{}", error))?;
    [
        dir.join(MANAGED_CLAUDE_RESOURCE),
        dir.join("resources").join(MANAGED_CLAUDE_RESOURCE),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| "ERR_MANAGED_CLAUDE_RESOURCE|bundled adapter archive is missing".to_string())
}

#[allow(dead_code)]
fn archive_listing_is_safe(names: &str, verbose: &str) -> bool {
    let safe_names = names.lines().all(|entry| {
        !entry.starts_with('/')
            && entry.starts_with("package/")
            && !entry.split('/').any(|part| part == "..")
    });
    let safe_types = verbose
        .lines()
        .all(|line| matches!(line.as_bytes().first(), Some(b'-' | b'd')));
    safe_names && safe_types
}

#[allow(dead_code)]
#[cfg(target_os = "macos")]
fn validate_archive(resource: &Path) -> Result<(), String> {
    let names = Command::new("/usr/bin/tar")
        .args(["-tzf"])
        .arg(resource)
        .output()
        .map_err(|error| format!("ERR_MANAGED_CLAUDE_INSTALL|{}", error))?;
    if !names.status.success() {
        return Err("ERR_MANAGED_CLAUDE_INSTALL|invalid adapter archive".into());
    }
    let mut found = std::collections::HashSet::new();
    let name_listing = String::from_utf8_lossy(&names.stdout);
    for entry in name_listing.lines() {
        found.insert(entry.trim_end_matches('/').to_string());
    }
    if !REQUIRED_FILES
        .iter()
        .all(|relative| found.contains(&format!("package/{relative}")))
    {
        return Err("ERR_MANAGED_CLAUDE_INSTALL|adapter archive is incomplete".into());
    }

    let verbose = Command::new("/usr/bin/tar")
        .args(["-tvzf"])
        .arg(resource)
        .output()
        .map_err(|error| format!("ERR_MANAGED_CLAUDE_INSTALL|{}", error))?;
    if !verbose.status.success()
        || !archive_listing_is_safe(&name_listing, &String::from_utf8_lossy(&verbose.stdout))
    {
        return Err("ERR_MANAGED_CLAUDE_INSTALL|links and special files are not allowed".into());
    }
    Ok(())
}

#[allow(dead_code)]
fn activate_staged_install(root: &Path, staging: &Path, backup: &Path) -> Result<(), String> {
    if root.exists() {
        std::fs::rename(root, backup)
            .map_err(|error| format!("ERR_MANAGED_CLAUDE_INSTALL|{}", error))?;
    }
    if let Err(error) = std::fs::rename(staging, root) {
        let _ = std::fs::rename(backup, root);
        return Err(format!("ERR_MANAGED_CLAUDE_INSTALL|{}", error));
    }
    Ok(())
}

#[allow(dead_code)]
fn build_manifest(root: &Path) -> Result<InstallManifest, String> {
    let files = REQUIRED_FILES
        .iter()
        .map(|relative| {
            Ok(InstalledFile {
                path: (*relative).to_string(),
                sha256: sha256(&root.join(relative))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(InstallManifest {
        version: MANAGED_CLAUDE_VERSION.to_string(),
        sha256: MANAGED_CLAUDE_SHA256.to_string(),
        files,
    })
}

#[tauri::command]
pub fn install_managed_claude(app: tauri::AppHandle) -> Result<ManagedClaudeStatus, String> {
    invalidate_managed_claude_health_cache();
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("ERR_MANAGED_CLAUDE_UNAVAILABLE".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        let runner = crate::commands::adapter::RealCommandRunner;
        let resources = crate::commands::adapter::TauriResourceLocator { app: &app };
        let home_dir =
            dirs::home_dir().ok_or_else(|| "ERR_MANAGED_CLAUDE_UNAVAILABLE".to_string())?;
        let config_dir = crate::config::get_config_dir();
        let pi_agent_dir = crate::commands::adapter::get_pi_agent_dir(&home_dir);
        let ctx = crate::commands::adapter::AdapterContext {
            runner: &runner,
            home_dir,
            config_dir,
            pi_agent_dir,
            is_macos: cfg!(target_os = "macos"),
            #[cfg(test)]
            injected_fail_point: crate::commands::adapter::FAIL_NONE,
        };

        crate::commands::adapter::apply_single_adapter(&ctx, &resources, "claude")
            .map_err(|e| format!("ERR_MANAGED_CLAUDE_INSTALL|{e}"))?;

        let mut config = crate::config::load_config();
        config.use_standard_claude = false;
        if let Err(error) = crate::config::save_config(config) {
            return Err(format!("ERR_MANAGED_CLAUDE_INSTALL|{error}"));
        }
        invalidate_managed_claude_health_cache();
        crate::registry::invalidate_environment_cache();
        Ok(get_managed_claude_status())
    }
}

#[tauri::command]
pub fn use_managed_claude() -> Result<(), String> {
    invalidate_managed_claude_health_cache();
    if managed_claude_state() != ManagedClaudeState::Healthy {
        return Err("ERR_MANAGED_CLAUDE_VERIFY|managed adapter is not healthy".into());
    }
    let mut config = crate::config::load_config();
    config.use_standard_claude = false;
    crate::config::save_config(config)?;
    invalidate_managed_claude_health_cache();
    Ok(())
}

fn random_hex(len: usize) -> Result<String, String> {
    let mut bytes = vec![0u8; len];
    getrandom::getrandom(&mut bytes).map_err(|error| format!("ERR_RANDOM_ID|{}", error))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn random_intercom_id() -> Result<String, String> {
    let hex = random_hex(16)?;
    Ok(format!(
        "tmuxdeck-{}-{}-4{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[13..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_safety_rejects_paths_links_and_devices() {
        assert!(archive_listing_is_safe(
            "package/dist/cci.mjs\npackage/monitors/\n",
            "-rwxr-xr-x user group 1 date package/dist/cci.mjs\ndrwxr-xr-x user group 0 date package/monitors/"
        ));
        assert!(!archive_listing_is_safe(
            "package/../escape\n",
            "-rw-r--r-- user group 1 date package/../escape"
        ));
        assert!(!archive_listing_is_safe(
            "/absolute\n",
            "-rw-r--r-- user group 1 date /absolute"
        ));
        assert!(!archive_listing_is_safe(
            "package/link\n",
            "lrwxr-xr-x user group 0 date package/link -> target"
        ));
        assert!(!archive_listing_is_safe(
            "package/device\n",
            "crw-r--r-- user group 1,2 date package/device"
        ));
    }

    #[test]
    fn failed_activation_restores_the_previous_install() {
        let parent = std::env::temp_dir().join(format!(
            "tmuxdeck-rollback-test-{}-{}",
            std::process::id(),
            random_hex(4).unwrap()
        ));
        let root = parent.join("root");
        let missing_staging = parent.join("missing-staging");
        let backup = parent.join("backup");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("sentinel"), "old healthy install").unwrap();

        assert!(activate_staged_install(&root, &missing_staging, &backup).is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("sentinel")).unwrap(),
            "old healthy install"
        );
        assert!(!backup.exists());
        let _ = std::fs::remove_dir_all(parent);
    }

    #[test]
    fn bundled_artifact_has_the_pinned_digest_and_valid_plugin_chain() {
        let resource = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(MANAGED_CLAUDE_RESOURCE);
        assert_eq!(sha256(&resource).unwrap(), MANAGED_CLAUDE_SHA256);

        let root = std::env::temp_dir().join(format!(
            "tmuxdeck-adapter-test-{}-{}",
            std::process::id(),
            random_hex(4).unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        let output = Command::new("tar")
            .args(["-xzf"])
            .arg(resource)
            .args(["--strip-components", "1", "-C"])
            .arg(&root)
            .output()
            .unwrap();
        assert!(output.status.success());
        validate_plugin_chain(&root).unwrap();
        for relative in REQUIRED_FILES {
            assert!(root.join(relative).is_file(), "missing {relative}");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn managed_manifest_detects_runtime_tampering() {
        let root = std::env::temp_dir().join(format!(
            "tmuxdeck-managed-claude-{}-{}",
            std::process::id(),
            random_hex(4).unwrap()
        ));
        for relative in REQUIRED_FILES {
            let path = root.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, relative).unwrap();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = root.join("dist/cci.mjs");
            let mut permissions = std::fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(path, permissions).unwrap();
        }
        let manifest = build_manifest(&root).unwrap();
        verify_installed_files(&root, &manifest).unwrap();
        std::fs::write(root.join("dist/cci.mjs"), "tampered").unwrap();
        assert!(verify_installed_files(&root, &manifest).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn health_cache_reuses_recent_result_and_can_be_invalidated() {
        *health_cache().lock().unwrap() = Some((Instant::now(), ManagedClaudeState::Healthy));
        assert_eq!(managed_claude_state(), ManagedClaudeState::Healthy);
        invalidate_managed_claude_health_cache();
        assert!(health_cache().lock().unwrap().is_none());
    }

    #[test]
    fn random_ids_are_non_deterministic_and_namespaced() {
        let first = random_intercom_id().unwrap();
        let second = random_intercom_id().unwrap();
        assert!(first.starts_with("tmuxdeck-"));
        assert_eq!(first.len(), 45);
        assert_ne!(first, second);
    }
}
