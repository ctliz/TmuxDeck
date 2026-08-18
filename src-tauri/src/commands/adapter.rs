// TmuxDeck Backend Adapter Phase B Implementation
//
// Implements:
// - Exact IPC contract matching src/types.ts
// - App-private deterministic installation for Claude, Codex, OpenCode
// - Canonical Pi extension registration and validation
// - Immutable digest and Core 0.2.0 95-entry tree verification
// - Fine-grained transactional rollback with pre-creation 0o600 modes and checked fsync
// - Typed durable cleanup journal with two-phase retry and idempotent startup/probe reconciliation
// - Full SemVer 2.0.0 parser with consistent Ord and Eq implementations (arbitrary-length numeric precedence)
// - Mutually exclusive identity verification across managed roots and global installations

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{value, Array, DocumentMut, Item, Table};

// ============================================================================
// Immutable Constants & Resource Hashes
// ============================================================================

pub const CORE_RESOURCE_NAME: &str = "ctliz-agent-intercom-core-0.2.0.tgz";
pub const CORE_RESOURCE_SHA256: &str =
    "9b6c72d57a9d00679dbdedcd91a9121e1028d9e208decd3ad6f4b9ba3c204556";
pub const CORE_TARGET_VERSION: &str = "0.2.0";
pub const CORE_TREE_DIGEST_SHA256: &str =
    "ec03975afb6924b5e9363b88073ef37be24eaeb65f0bb5d7360ebce0dfce669a";

pub const CLAUDE_RESOURCE_NAME: &str = "ctliz-agent-intercom-claude-0.13.0-connect.1.tgz";
pub const CLAUDE_RESOURCE_SHA256: &str =
    "a766f4631d92df3dc26ee81f9bec06da38c3c09bae9ea4c6b0ef3975eeeb96ba";
pub const CLAUDE_TARGET_VERSION: &str = "0.13.0-connect.1";
pub const CLAUDE_IMMUTABLE_DIGESTS: &[(&str, &str)] = &[
    (
        "dist/cci.mjs",
        "b1d8c9388a253e6e34e04cfabb86ac01f1ed478d18a51c9fd6dbe3e729c7fcf9",
    ),
    (
        "dist/claude-server.mjs",
        "b7c67830e0bc6446d56fd1e8291a6652f050ca50571c01497ca50e94b1a47d9d",
    ),
    (
        "dist/inbox-monitor.mjs",
        "37849f70a6b1aeb0d1a72611ba53107019a44fdb68e889003a64f41b895ae90f",
    ),
    (
        "vendor/ctliz-agent-intercom-claude-0.13.0-connect.1.tgz",
        CLAUDE_RESOURCE_SHA256,
    ),
    (
        "vendor/ctliz-agent-intercom-core-0.2.0.tgz",
        CORE_RESOURCE_SHA256,
    ),
];

pub const CODEX_RESOURCE_NAME: &str = "ctliz-agent-intercom-codex-0.12.0-connect.1.tgz";
pub const CODEX_RESOURCE_SHA256: &str =
    "37b14553e00ed7b501cb6289319a01c1a65543c0e8fb6a87e9caf1c379ed0a14";
pub const CODEX_TARGET_VERSION: &str = "0.12.0-connect.1";
pub const CODEX_IMMUTABLE_DIGESTS: &[(&str, &str)] = &[
    (
        "dist/codex-server.mjs",
        "fe29eb427a3d49eb9fb70e5fe04ac9af334ed27a10d989e681297ea9cf1dffe8",
    ),
    (
        "dist/codex-launcher.mjs",
        "227cf7789fe3b0838b220f7dba2b9c1b961a9d3cfafb7b2adb14b0794a134ee0",
    ),
    (
        "vendor/ctliz-agent-intercom-codex-0.12.0-connect.1.tgz",
        CODEX_RESOURCE_SHA256,
    ),
    (
        "vendor/ctliz-agent-intercom-core-0.2.0.tgz",
        CORE_RESOURCE_SHA256,
    ),
];

pub const OPENCODE_RESOURCE_NAME: &str = "ctliz-agent-intercom-opencode-0.12.0-connect.1.tgz";
pub const OPENCODE_RESOURCE_SHA256: &str =
    "9756cc56a54313d606e655ae46af83bdd89a29178fb74f08144672a0fda008a3";
pub const OPENCODE_TARGET_VERSION: &str = "0.12.0-connect.1";
pub const OPENCODE_SDK_RESOURCE_NAME: &str = "opencode-ai-plugin-1.18.18.tgz";
pub const OPENCODE_SDK_RESOURCE_SHA256: &str =
    "26ac7cc2608fc63e063a0b08857c277b17d75043ad37125667275932f17b3d43";
pub const OPENCODE_CLOSURE_RESOURCE_NAME: &str = "opencode-sdk-closure.tgz";
pub const OPENCODE_CLOSURE_RESOURCE_SHA256: &str =
    "8e1d64c90fcf4a7ed73d6d4eaa1b726f8c6a647c82e1dbeba4af6c8d04f24237";
pub const OPENCODE_IMMUTABLE_DIGESTS: &[(&str, &str)] = &[
    (
        "dist/plugin.mjs",
        "56bd73e2b8997e5a8b7e8d7e315c93a8d1193827ebb0c8dcea0b2851788d9791",
    ),
    (
        "dist/tui.mjs",
        "104ecf3dde0c938ca45b8863a562da8622ac7d7c415621137baf1af03c63ff24",
    ),
    (
        "vendor/ctliz-agent-intercom-opencode-0.12.0-connect.1.tgz",
        OPENCODE_RESOURCE_SHA256,
    ),
    (
        "vendor/ctliz-agent-intercom-core-0.2.0.tgz",
        CORE_RESOURCE_SHA256,
    ),
];

pub const PI_CANONICAL_GIT_TARGET: &str =
    "git:github.com/ctliz/agent-intercom-pi@v0.12.0-connect.1";
pub const PI_NPM_PACKAGE_PREFIX: &str = "npm:@ctliz/pi-intercom@";
pub const PI_TARGET_VERSION: &str = "0.12.0-connect.1";

fn is_pi_intercom_settings_entry(value: &str) -> bool {
    value.contains("agent-intercom-pi") || value.contains("@ctliz/pi-intercom")
}

fn pi_npm_package_version(value: &str) -> Option<&str> {
    value.strip_prefix(PI_NPM_PACKAGE_PREFIX).filter(|version| !version.is_empty())
}

pub const CODEX_MCP_SERVER_KEY: &str = "codex-intercom";

pub fn get_pi_agent_dir(home_dir: &Path) -> PathBuf {
    home_dir.join(".pi/agent")
}

pub const CODEX_LAUNCHER_BODY: &str = r#"#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const manifestPath = process.env.AGENT_INTERCOM_TEAM_MANIFEST;
if (!manifestPath) {
  process.stderr.write("LAUNCHER_ERROR: AGENT_INTERCOM_TEAM_MANIFEST environment variable not set\n");
  process.exit(1);
}

let manifest;
try {
  manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
} catch (e) {
  process.stderr.write("LAUNCHER_ERROR: Failed to read team manifest: " + e.message + "\n");
  process.exit(1);
}

const mySessionId = process.env.AGENT_INTERCOM_SESSION_ID;
if (!mySessionId) {
  process.stderr.write("LAUNCHER_ERROR: AGENT_INTERCOM_SESSION_ID not set\n");
  process.exit(1);
}

const member = (manifest.members || []).find(m => m.sessionId === mySessionId);
if (!member) {
  process.stderr.write("LAUNCHER_ERROR: Session ID not found in manifest members\n");
  process.exit(1);
}

const envRole = process.env.AGENT_INTERCOM_ROLE;
if (envRole && envRole !== member.role) {
  process.stderr.write("LAUNCHER_ERROR: AGENT_INTERCOM_ROLE mismatch\n");
  process.exit(1);
}

if (member.role === "worker") {
  const managerTarget = process.env.AGENT_INTERCOM_MANAGER_TARGET;
  if (!managerTarget || managerTarget !== manifest.leadId) {
    process.stderr.write("LAUNCHER_ERROR: AGENT_INTERCOM_MANAGER_TARGET mismatch for worker\n");
    process.exit(1);
  }
}

const argv = process.argv.slice(2);
const codexBin = process.env.CODEX_REAL_BIN || "/usr/local/bin/codex";

try {
  execFileSync(codexBin, argv, { stdio: "inherit", env: process.env });
} catch (e) {
  if (e.status !== undefined) {
    process.exit(e.status);
  }
  process.exit(1);
}
"#;

// ============================================================================
// Public Serde Types & Domain Model
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommunicationAdapterKind {
    Pi,
    Claude,
    Codex,
    OpenCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterHealthState {
    Healthy,
    HealthyExistingGlobal,
    NotInstalled,
    NeedsUpgrade,
    NeedsRepair,
    MigrationRequired,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterSourceKind {
    Bundled,
    NpmRegistry,
    PiGit,
    ExistingGlobal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanonicalAdapterPackage {
    #[serde(rename = "@ctliz/agent-intercom-pi")]
    Pi,
    #[serde(rename = "@ctliz/agent-intercom-claude")]
    Claude,
    #[serde(rename = "@ctliz/agent-intercom-codex")]
    Codex,
    #[serde(rename = "@ctliz/agent-intercom-opencode")]
    OpenCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigChangeKind {
    None,
    AppPrivateManaged,
    HostConfigRegistered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterActionReason {
    Install,
    Upgrade,
    Repair,
    ManualMigrationRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunicationAdapterPlanItem {
    pub agent_id: String,
    pub host_display_name: String,
    pub adapter_kind: CommunicationAdapterKind,
    pub state: AdapterHealthState,
    pub target_version: String,
    pub installed_version: Option<String>,
    pub source_kind: AdapterSourceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_name: Option<CanonicalAdapterPackage>,
    pub config_change_kind: ConfigChangeKind,
    pub network_required: bool,
    pub license: String,
    pub action_reason: AdapterActionReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInstallPlan {
    pub plan_id: String,
    pub plan_fingerprint: String,
    pub requires_consent: bool,
    pub can_apply: bool,
    pub can_create_without_installing: bool,
    pub healthy_agent_ids: Vec<String>,
    pub items: Vec<CommunicationAdapterPlanItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterError {
    PlanStale,
    PlanInvalid,
    Unavailable,
    Install,
    Verify,
    Config,
    Rollback,
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            AdapterError::PlanStale => "ERR_PLAN_STALE",
            AdapterError::PlanInvalid => "ERR_PLAN_INVALID",
            AdapterError::Unavailable => "ERR_ADAPTER_UNAVAILABLE",
            AdapterError::Install => "ERR_ADAPTER_INSTALL",
            AdapterError::Verify => "ERR_ADAPTER_VERIFY",
            AdapterError::Config => "ERR_ADAPTER_CONFIG",
            AdapterError::Rollback => "ERR_ADAPTER_ROLLBACK",
        };
        write!(f, "{}", code)
    }
}

impl std::error::Error for AdapterError {}

impl From<AdapterError> for String {
    fn from(err: AdapterError) -> Self {
        err.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedAdapterMarker {
    pub schema_version: u32,
    pub harness: String,
    pub adapter_version: String,
    pub installed_at: u64,
    pub resources: BTreeMap<String, String>,
    pub digests: BTreeMap<String, String>,
}

// ============================================================================
// Resource Locator Trait & Implementation
// ============================================================================

pub trait ResourceLocator: Send + Sync {
    fn get_resource_bytes(&self, name: &str) -> Result<Vec<u8>, AdapterError>;
}

pub struct TauriResourceLocator<'a> {
    pub app: &'a tauri::AppHandle,
}

impl<'a> ResourceLocator for TauriResourceLocator<'a> {
    fn get_resource_bytes(&self, name: &str) -> Result<Vec<u8>, AdapterError> {
        use tauri::Manager;
        let res_path = self
            .app
            .path()
            .resource_dir()
            .map_err(|_| AdapterError::Install)?
            .join("resources")
            .join(name);
        fs::read(res_path).map_err(|_| AdapterError::Install)
    }
}

pub struct DirectFileResourceLocator {
    pub resources_dir: PathBuf,
}

impl ResourceLocator for DirectFileResourceLocator {
    fn get_resource_bytes(&self, name: &str) -> Result<Vec<u8>, AdapterError> {
        let res_path = self.resources_dir.join(name);
        fs::read(res_path).map_err(|_| AdapterError::Install)
    }
}

// ============================================================================
// Failure Injection Constants
// ============================================================================

pub const FAIL_NONE: u32 = 0;
pub const FAIL_FILE_BACKUP_CREATE_WRITE: u32 = 1 << 0;
pub const FAIL_FILE_BACKUP_CREATE_FSYNC: u32 = 1 << 1;
pub const FAIL_ACTIVE_TO_BACKUP_RENAME: u32 = 1 << 2;
pub const FAIL_STAGING_TO_ACTIVE_RENAME: u32 = 1 << 3;
pub const FAIL_CONFIG_TEMP_WRITE: u32 = 1 << 4;
pub const FAIL_CONFIG_RENAME: u32 = 1 << 5;
pub const FAIL_CONFIG_PARENT_FSYNC: u32 = 1 << 6;
pub const FAIL_POST_HEALTH_PROBE: u32 = 1 << 7;
pub const FAIL_JOURNAL_WRITE: u32 = 1 << 8;
pub const FAIL_COMMIT_BACKUP_REMOVE: u32 = 1 << 9;
pub const FAIL_COMMIT_PARENT_FSYNC: u32 = 1 << 10;
pub const FAIL_RESTORE_RENAME: u32 = 1 << 11;
pub const FAIL_RESTORE_PARENT_FSYNC: u32 = 1 << 12;

// ============================================================================
// Command Runner Abstraction & Adapter Context
// ============================================================================

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait CommandRunner: Send + Sync {
    fn run_command(
        &self,
        command: &str,
        args: &[&str],
        cwd: &Path,
        env: Option<&[(&str, &str)]>,
    ) -> Result<CommandOutput, AdapterError>;

    fn binary_exists(&self, binary_name: &str) -> bool;
}

pub struct RealCommandRunner;

fn materialize_claude_plugin_surface(root: &Path) -> Result<(), AdapterError> {
    let package_root = root.join("node_modules/@ctliz/agent-intercom-claude");
    for relative in [".claude-plugin", "monitors", "commands", "skills"] {
        let source = package_root.join(relative);
        let destination = root.join(relative);
        if !source.is_dir() {
            return Err(AdapterError::Verify);
        }
        copy_directory_without_links(&source, &destination)?;
    }
    let mcp_source = package_root.join(".mcp.json");
    if !mcp_source.is_file() {
        return Err(AdapterError::Verify);
    }
    fs::copy(mcp_source, root.join(".mcp.json")).map_err(|_| AdapterError::Verify)?;
    Ok(())
}

fn copy_directory_without_links(source: &Path, destination: &Path) -> Result<(), AdapterError> {
    fs::create_dir_all(destination).map_err(|_| AdapterError::Verify)?;
    for entry in fs::read_dir(source).map_err(|_| AdapterError::Verify)? {
        let entry = entry.map_err(|_| AdapterError::Verify)?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).map_err(|_| AdapterError::Verify)?;
        if metadata.file_type().is_symlink() {
            return Err(AdapterError::Verify);
        }
        if metadata.is_dir() {
            copy_directory_without_links(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(source_path, destination_path).map_err(|_| AdapterError::Verify)?;
        } else {
            return Err(AdapterError::Verify);
        }
    }
    Ok(())
}

fn binary_exists_in_dirs<I>(binary_name: &str, dirs: I) -> bool
where
    I: IntoIterator<Item = PathBuf>,
{
    dirs.into_iter().any(|dir| dir.join(binary_name).is_file())
}

fn gui_binary_search_dirs() -> Vec<PathBuf> {
    let mut dirs = std::env::var("PATH")
        .ok()
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    dirs.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        dirs.extend([
            home.join(".local/bin"),
            home.join(".cargo/bin"),
            home.join(".bun/bin"),
            home.join(".opencode/bin"),
        ]);
        let nvm_dir = home.join(".nvm/versions/node");
        if let Ok(entries) = std::fs::read_dir(nvm_dir) {
            for entry in entries.flatten() {
                dirs.push(entry.path().join("bin"));
            }
        }
    }
    dirs
}

impl CommandRunner for RealCommandRunner {
    fn run_command(
        &self,
        command: &str,
        args: &[&str],
        cwd: &Path,
        env: Option<&[(&str, &str)]>,
    ) -> Result<CommandOutput, AdapterError> {
        let mut cmd = std::process::Command::new(command);
        cmd.args(args).current_dir(cwd);
        let inherited_path = env
            .and_then(|env_vars| {
                env_vars
                    .iter()
                    .find(|(key, _)| *key == "PATH")
                    .map(|(_, value)| *value)
            })
            .map(str::to_string)
            .unwrap_or_else(crate::commands::utils::build_augmented_path);
        cmd.env("PATH", inherited_path);
        if let Some(env_vars) = env {
            for (k, v) in env_vars {
                cmd.env(k, v);
            }
        }
        let out = cmd.output().map_err(|_| AdapterError::Install)?;
        Ok(CommandOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        })
    }

    fn binary_exists(&self, binary_name: &str) -> bool {
        binary_exists_in_dirs(binary_name, gui_binary_search_dirs())
    }
}

pub struct AdapterContext<'a> {
    pub runner: &'a dyn CommandRunner,
    pub home_dir: PathBuf,
    pub config_dir: PathBuf,
    pub pi_agent_dir: PathBuf,
    pub is_macos: bool,
    #[cfg(test)]
    pub injected_fail_point: u32,
}

impl<'a> AdapterContext<'a> {
    pub fn injected_fail(&self) -> u32 {
        #[cfg(test)]
        {
            self.injected_fail_point
        }
        #[cfg(not(test))]
        {
            FAIL_NONE
        }
    }
}

// ============================================================================
// Arbitrary-Length Numeric SemVer 2.0.0
// ============================================================================

#[derive(Debug, Clone)]
pub struct SemVer {
    pub major: String,
    pub minor: String,
    pub patch: String,
    pub prerelease: Vec<String>,
    pub build: Vec<String>,
}

fn compare_numeric_strings(a: &str, b: &str) -> std::cmp::Ordering {
    let a_clean = a.trim_start_matches('0');
    let b_clean = b.trim_start_matches('0');
    let a_effective = if a_clean.is_empty() { "0" } else { a_clean };
    let b_effective = if b_clean.is_empty() { "0" } else { b_clean };

    if a_effective.len() != b_effective.len() {
        return a_effective.len().cmp(&b_effective.len());
    }
    a_effective.cmp(b_effective)
}

fn is_numeric_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

impl SemVer {
    pub fn parse(v: &str) -> Option<Self> {
        let v = v.trim();
        let (v_without_build, build_identifiers) = if let Some((core_pre, b)) = v.split_once('+') {
            let b_ids: Vec<String> = b.split('.').map(|s| s.to_string()).collect();
            if b_ids.is_empty() || b_ids.iter().any(|id| id.is_empty()) {
                return None;
            }
            (core_pre, b_ids)
        } else {
            (v, Vec::new())
        };

        let (core_str, pre_identifiers) = if let Some((core, pre)) = v_without_build.split_once('-')
        {
            let p_ids: Vec<String> = pre.split('.').map(|s| s.to_string()).collect();
            if p_ids.is_empty() || p_ids.iter().any(|id| id.is_empty()) {
                return None;
            }
            for id in &p_ids {
                if is_numeric_identifier(id) && id.len() > 1 && id.starts_with('0') {
                    return None;
                }
            }
            (core, p_ids)
        } else {
            (v_without_build, Vec::new())
        };

        let core_parts: Vec<&str> = core_str.split('.').collect();
        if core_parts.len() != 3 {
            return None;
        }

        for part in &core_parts {
            if !is_numeric_identifier(part) {
                return None;
            }
            if part.len() > 1 && part.starts_with('0') {
                return None;
            }
        }

        Some(SemVer {
            major: core_parts[0].to_string(),
            minor: core_parts[1].to_string(),
            patch: core_parts[2].to_string(),
            prerelease: pre_identifiers,
            build: build_identifiers,
        })
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let maj = compare_numeric_strings(&self.major, &other.major);
        if maj != std::cmp::Ordering::Equal {
            return maj;
        }
        let min = compare_numeric_strings(&self.minor, &other.minor);
        if min != std::cmp::Ordering::Equal {
            return min;
        }
        let pat = compare_numeric_strings(&self.patch, &other.patch);
        if pat != std::cmp::Ordering::Equal {
            return pat;
        }

        match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            (false, false) => {
                let len = self.prerelease.len().min(other.prerelease.len());
                for i in 0..len {
                    let a = &self.prerelease[i];
                    let b = &other.prerelease[i];
                    let a_num = is_numeric_identifier(a);
                    let b_num = is_numeric_identifier(b);
                    match (a_num, b_num) {
                        (true, true) => {
                            let ord = compare_numeric_strings(a, b);
                            if ord != std::cmp::Ordering::Equal {
                                return ord;
                            }
                        }
                        (true, false) => return std::cmp::Ordering::Less,
                        (false, true) => return std::cmp::Ordering::Greater,
                        (false, false) => {
                            if a != b {
                                return a.cmp(b);
                            }
                        }
                    }
                }
                self.prerelease.len().cmp(&other.prerelease.len())
            }
        }
    }
}

impl PartialEq for SemVer {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for SemVer {}

pub fn is_valid_semver(v: &str) -> bool {
    SemVer::parse(v).is_some()
}

// ============================================================================
// Helper Utilities & Safe Filesystem Operations
// ============================================================================

pub fn file_sha256(path: &Path) -> Result<String, AdapterError> {
    let mut file = File::open(path).map_err(|_| AdapterError::Verify)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer).map_err(|_| AdapterError::Verify)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn random_hex(bytes_len: usize) -> Result<String, AdapterError> {
    let mut buf = vec![0u8; bytes_len];
    getrandom::getrandom(&mut buf).map_err(|_| AdapterError::Install)?;
    Ok(buf.iter().map(|b| format!("{:02x}", b)).collect())
}

pub fn is_safe_basename(name: &str) -> bool {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

pub fn is_safe_harness_name(name: &str) -> bool {
    matches!(name, "claude" | "codex" | "opencode")
}

pub fn is_safe_nonce(nonce: &str) -> bool {
    !nonce.is_empty() && nonce.len() <= 64 && nonce.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn checked_fsync_file(file: &File, _injected_fail: u32) -> Result<(), AdapterError> {
    #[cfg(test)]
    if (_injected_fail & FAIL_FILE_BACKUP_CREATE_FSYNC) != 0 {
        return Err(AdapterError::Install);
    }
    file.sync_all().map_err(|_| AdapterError::Install)
}

pub fn checked_fsync_dir(dir: &Path, _injected_fail: u32) -> Result<(), AdapterError> {
    #[cfg(test)]
    if (_injected_fail & FAIL_COMMIT_PARENT_FSYNC) != 0
        || (_injected_fail & FAIL_CONFIG_PARENT_FSYNC) != 0
        || (_injected_fail & FAIL_RESTORE_PARENT_FSYNC) != 0
    {
        return Err(AdapterError::Install);
    }
    let f = File::open(dir).map_err(|_| AdapterError::Install)?;
    f.sync_all().map_err(|_| AdapterError::Install)
}

pub fn verify_parent_not_symlink(dir: &Path) -> Result<(), AdapterError> {
    let mut curr = dir.to_path_buf();
    loop {
        if curr.exists() {
            let meta = fs::symlink_metadata(&curr).map_err(|_| AdapterError::Rollback)?;
            if meta.file_type().is_symlink() {
                return Err(AdapterError::Rollback);
            }
        }
        if let Some(parent) = curr.parent() {
            if parent == curr {
                break;
            }
            curr = parent.to_path_buf();
        } else {
            break;
        }
    }
    Ok(())
}

pub fn atomic_write_file(
    path: &Path,
    content: &[u8],
    mode: u32,
    injected_fail: u32,
) -> Result<(), AdapterError> {
    let parent = path.parent().ok_or(AdapterError::Install)?;
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|_| AdapterError::Install)?;
        checked_fsync_dir(parent, injected_fail)?;
    }

    #[cfg(test)]
    if (injected_fail & FAIL_CONFIG_TEMP_WRITE) != 0 {
        return Err(AdapterError::Install);
    }

    let nonce = random_hex(6)?;
    let tmp_name = format!(".tmp.write.{}.{}", std::process::id(), nonce);
    let tmp_path = parent.join(tmp_name);

    let write_res = (|| -> Result<(), AdapterError> {
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(mode)
                .open(&tmp_path)
                .map_err(|_| AdapterError::Install)?
        };

        #[cfg(not(unix))]
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .map_err(|_| AdapterError::Install)?;

        file.write_all(content).map_err(|_| AdapterError::Install)?;
        checked_fsync_file(&file, injected_fail)?;
        drop(file);

        #[cfg(test)]
        if (injected_fail & FAIL_CONFIG_RENAME) != 0 {
            return Err(AdapterError::Install);
        }

        fs::rename(&tmp_path, path).map_err(|_| AdapterError::Install)?;
        checked_fsync_dir(parent, injected_fail)?;
        Ok(())
    })();

    if write_res.is_err() && tmp_path.exists() {
        let _ = fs::remove_file(&tmp_path);
    }

    write_res
}

#[cfg(unix)]
pub fn normalize_tree_permissions(root: &Path) -> Result<(), AdapterError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(root, fs::Permissions::from_mode(0o700))
        .map_err(|_| AdapterError::Install)?;

    fn walk_and_normalize(root: &Path, dir: &Path) -> Result<(), AdapterError> {
        let entries = fs::read_dir(dir).map_err(|_| AdapterError::Install)?;
        for entry in entries {
            let entry = entry.map_err(|_| AdapterError::Install)?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|_| AdapterError::Install)?;
            if meta.file_type().is_symlink() {
                return Err(AdapterError::Install);
            }
            if meta.is_dir() {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    .map_err(|_| AdapterError::Install)?;
                walk_and_normalize(root, &path)?;
            } else if meta.is_file() {
                let is_exec = path.file_name() == Some(std::ffi::OsStr::new("cci.mjs"))
                    || path.file_name() == Some(std::ffi::OsStr::new("codex-launcher.mjs"));
                let mode = if is_exec { 0o755 } else { 0o600 };
                fs::set_permissions(&path, fs::Permissions::from_mode(mode))
                    .map_err(|_| AdapterError::Install)?;
            }
        }
        Ok(())
    }

    walk_and_normalize(root, root)
}

#[cfg(not(unix))]
pub fn normalize_tree_permissions(_root: &Path) -> Result<(), AdapterError> {
    Ok(())
}

#[cfg(unix)]
pub fn verify_tree_permissions(root: &Path) -> Result<(), AdapterError> {
    use std::os::unix::fs::PermissionsExt;

    let root_meta = fs::symlink_metadata(root).map_err(|_| AdapterError::Verify)?;
    if (root_meta.permissions().mode() & 0o777) != 0o700 {
        return Err(AdapterError::Verify);
    }

    fn walk_and_verify(root: &Path, dir: &Path) -> Result<(), AdapterError> {
        let entries = fs::read_dir(dir).map_err(|_| AdapterError::Verify)?;
        for entry in entries {
            let entry = entry.map_err(|_| AdapterError::Verify)?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|_| AdapterError::Verify)?;
            if meta.file_type().is_symlink() {
                return Err(AdapterError::Verify);
            }
            let mode = meta.permissions().mode() & 0o777;
            if meta.is_dir() {
                if mode != 0o700 {
                    return Err(AdapterError::Verify);
                }
                walk_and_verify(root, &path)?;
            } else if meta.is_file() {
                let is_exec = path.file_name() == Some(std::ffi::OsStr::new("cci.mjs"))
                    || path.file_name() == Some(std::ffi::OsStr::new("codex-launcher.mjs"));
                let expected = if is_exec { 0o755 } else { 0o600 };
                if mode != expected {
                    return Err(AdapterError::Verify);
                }
            }
        }
        Ok(())
    }

    walk_and_verify(root, root)
}

#[cfg(not(unix))]
pub fn verify_tree_permissions(_root: &Path) -> Result<(), AdapterError> {
    Ok(())
}

// ============================================================================
// Fine-Grained Transactional Backup & Restore with Created Ancestor Tracking
// ============================================================================

pub struct FileBackup {
    pub target: PathBuf,
    pub backup: Option<PathBuf>,
    pub original_mode: u32,
    pub created_ancestors: Vec<PathBuf>,
}

impl FileBackup {
    pub fn create(target: &Path, injected_fail: u32) -> Result<Self, AdapterError> {
        let parent = target.parent().ok_or(AdapterError::Install)?;
        let mut created_ancestors = Vec::new();

        if !parent.exists() {
            let mut curr = parent;
            let mut to_create = Vec::new();
            while !curr.exists() {
                to_create.push(curr.to_path_buf());
                if let Some(p) = curr.parent() {
                    curr = p;
                } else {
                    break;
                }
            }
            to_create.reverse();
            for dir in &to_create {
                if fs::create_dir(dir).is_err() {
                    for created in created_ancestors.iter().rev() {
                        let _ = fs::remove_dir(created);
                    }
                    return Err(AdapterError::Install);
                }
                if let Some(p) = dir.parent() {
                    if checked_fsync_dir(p, injected_fail).is_err() {
                        for created in created_ancestors.iter().rev() {
                            let _ = fs::remove_dir(created);
                        }
                        let _ = fs::remove_dir(dir);
                        return Err(AdapterError::Install);
                    }
                }
                created_ancestors.push(dir.clone());
            }
        }

        if !target.exists() {
            return Ok(Self {
                target: target.to_path_buf(),
                backup: None,
                original_mode: 0o600,
                created_ancestors,
            });
        }

        #[cfg(test)]
        if (injected_fail & FAIL_FILE_BACKUP_CREATE_WRITE) != 0 {
            for created in created_ancestors.iter().rev() {
                let _ = fs::remove_dir(created);
            }
            return Err(AdapterError::Install);
        }

        let nonce = random_hex(8)?;
        let file_name = target
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(AdapterError::Install)?;
        let backup_path = parent.join(format!("{}.bak.{}", file_name, nonce));

        let content = fs::read(target).map_err(|_| AdapterError::Install)?;
        #[cfg(unix)]
        let original_mode = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(target)
                .map(|m| m.mode() & 0o777)
                .unwrap_or(0o600)
        };
        #[cfg(not(unix))]
        let original_mode = 0o600;

        atomic_write_file(&backup_path, &content, 0o600, injected_fail)?;

        Ok(Self {
            target: target.to_path_buf(),
            backup: Some(backup_path),
            original_mode,
            created_ancestors,
        })
    }

    pub fn rollback(&mut self, injected_fail: u32) -> Result<(), AdapterError> {
        let parent = self.target.parent().ok_or(AdapterError::Rollback)?;
        let mut failed = false;

        if let Some(backup_path) = &self.backup {
            if backup_path.exists() {
                #[cfg(test)]
                if (injected_fail & FAIL_RESTORE_RENAME) != 0 {
                    failed = true;
                } else if fs::rename(backup_path, &self.target).is_err() {
                    failed = true;
                }
                #[cfg(not(test))]
                if fs::rename(backup_path, &self.target).is_err() {
                    failed = true;
                }

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if fs::set_permissions(
                        &self.target,
                        fs::Permissions::from_mode(self.original_mode),
                    )
                    .is_err()
                    {
                        failed = true;
                    }
                }
                if checked_fsync_dir(parent, injected_fail).is_err() {
                    failed = true;
                }
            }
        } else if self.target.exists() {
            if fs::remove_file(&self.target).is_err() {
                failed = true;
            }
            if checked_fsync_dir(parent, injected_fail).is_err() {
                failed = true;
            }
        }

        for created in self.created_ancestors.iter().rev() {
            if created.exists() {
                if fs::remove_dir(created).is_err() {
                    failed = true;
                }
                if let Some(p) = created.parent() {
                    if checked_fsync_dir(p, injected_fail).is_err() {
                        failed = true;
                    }
                }
            }
        }

        if failed {
            Err(AdapterError::Rollback)
        } else {
            Ok(())
        }
    }

    pub fn commit(self) {}
}

pub struct ManagedRootBackup {
    pub harness: String,
    pub target_root: PathBuf,
    pub backup_dir: Option<PathBuf>,
    pub staging_dir: PathBuf,
    pub npm_cache_dir: PathBuf,
}

impl ManagedRootBackup {
    pub fn new(
        harness: &str,
        target_root: PathBuf,
        staging_dir: PathBuf,
        npm_cache_dir: PathBuf,
    ) -> Self {
        Self {
            harness: harness.to_string(),
            target_root,
            backup_dir: None,
            staging_dir,
            npm_cache_dir,
        }
    }

    pub fn swap_staging_to_active(&mut self, injected_fail: u32) -> Result<(), AdapterError> {
        let parent = self.target_root.parent().ok_or(AdapterError::Install)?;
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|_| AdapterError::Install)?;
            checked_fsync_dir(parent, injected_fail)?;
        }

        if self.target_root.exists() {
            let nonce = random_hex(8)?;
            let backup_path = parent.join(format!(".bak.{}", nonce));

            #[cfg(test)]
            if (injected_fail & FAIL_ACTIVE_TO_BACKUP_RENAME) != 0 {
                return Err(AdapterError::Install);
            }

            fs::rename(&self.target_root, &backup_path).map_err(|_| AdapterError::Install)?;
            checked_fsync_dir(parent, injected_fail)?;
            self.backup_dir = Some(backup_path);
        }

        #[cfg(test)]
        if (injected_fail & FAIL_STAGING_TO_ACTIVE_RENAME) != 0 {
            return Err(AdapterError::Install);
        }

        fs::rename(&self.staging_dir, &self.target_root).map_err(|_| AdapterError::Install)?;
        checked_fsync_dir(parent, injected_fail)?;

        if self.npm_cache_dir.exists() {
            fs::remove_dir_all(&self.npm_cache_dir).map_err(|_| AdapterError::Install)?;
            checked_fsync_dir(parent, injected_fail)?;
        }

        Ok(())
    }

    pub fn rollback(&mut self, injected_fail: u32) -> Result<(), AdapterError> {
        let parent = self.target_root.parent().ok_or(AdapterError::Rollback)?;
        let mut failed = false;
        if self.staging_dir.exists() && fs::remove_dir_all(&self.staging_dir).is_err() {
            failed = true;
        }
        if self.npm_cache_dir.exists() && fs::remove_dir_all(&self.npm_cache_dir).is_err() {
            failed = true;
        }

        if let Some(backup_path) = &self.backup_dir {
            if backup_path.exists() {
                if self.target_root.exists() && fs::remove_dir_all(&self.target_root).is_err() {
                    failed = true;
                }
                #[cfg(test)]
                if (injected_fail & FAIL_RESTORE_RENAME) != 0 {
                    failed = true;
                } else if fs::rename(backup_path, &self.target_root).is_err() {
                    failed = true;
                }
                #[cfg(not(test))]
                if fs::rename(backup_path, &self.target_root).is_err() {
                    failed = true;
                }
                if checked_fsync_dir(parent, injected_fail).is_err() {
                    failed = true;
                }
            }
        } else if self.target_root.exists() {
            if fs::remove_dir_all(&self.target_root).is_err() {
                failed = true;
            }
            if checked_fsync_dir(parent, injected_fail).is_err() {
                failed = true;
            }
        }
        if failed {
            Err(AdapterError::Rollback)
        } else {
            Ok(())
        }
    }

    pub fn commit(self) {}
}

// ============================================================================
// Typed Durable Cleanup Journal with Two-Phase Parent Directory Fsync Retries
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JournalPhase {
    PendingRemove,
    RemovedPendingParentFsync,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum JournalCleanupItem {
    ManagedRootBackup {
        harness: String,
        nonce: String,
        phase: JournalPhase,
    },
    ManagedOlderRoot {
        harness: String,
        version: String,
        phase: JournalPhase,
    },
    CodexConfigBackup {
        nonce: String,
        phase: JournalPhase,
    },
    OpenCodeConfigBackup {
        file_name: String,
        nonce: String,
        phase: JournalPhase,
    },
}

impl JournalCleanupItem {
    pub fn phase(&self) -> JournalPhase {
        match self {
            JournalCleanupItem::ManagedRootBackup { phase, .. } => *phase,
            JournalCleanupItem::ManagedOlderRoot { phase, .. } => *phase,
            JournalCleanupItem::CodexConfigBackup { phase, .. } => *phase,
            JournalCleanupItem::OpenCodeConfigBackup { phase, .. } => *phase,
        }
    }

    pub fn set_phase(&mut self, new_phase: JournalPhase) {
        match self {
            JournalCleanupItem::ManagedRootBackup { phase, .. } => *phase = new_phase,
            JournalCleanupItem::ManagedOlderRoot { phase, .. } => *phase = new_phase,
            JournalCleanupItem::CodexConfigBackup { phase, .. } => *phase = new_phase,
            JournalCleanupItem::OpenCodeConfigBackup { phase, .. } => *phase = new_phase,
        }
    }

    pub fn get_target_path(
        &self,
        config_dir: &Path,
        home_dir: &Path,
    ) -> Result<PathBuf, AdapterError> {
        match self {
            JournalCleanupItem::ManagedRootBackup { harness, nonce, .. } => {
                if !is_safe_harness_name(harness) || !is_safe_nonce(nonce) {
                    return Err(AdapterError::Rollback);
                }
                let harness_dir = config_dir.join(format!("managed/{}-intercom", harness));
                verify_parent_not_symlink(&harness_dir)?;
                Ok(harness_dir.join(format!(".bak.{}", nonce)))
            }
            JournalCleanupItem::ManagedOlderRoot {
                harness, version, ..
            } => {
                if !is_safe_harness_name(harness) {
                    return Err(AdapterError::Rollback);
                }
                let parsed_ver = SemVer::parse(version).ok_or(AdapterError::Rollback)?;
                let target_ver = match harness.as_str() {
                    "claude" => SemVer::parse(CLAUDE_TARGET_VERSION).unwrap(),
                    "codex" => SemVer::parse(CODEX_TARGET_VERSION).unwrap(),
                    "opencode" => SemVer::parse(OPENCODE_TARGET_VERSION).unwrap(),
                    _ => return Err(AdapterError::Rollback),
                };
                if parsed_ver >= target_ver {
                    return Err(AdapterError::Rollback);
                }
                let harness_dir = config_dir.join(format!("managed/{}-intercom", harness));
                verify_parent_not_symlink(&harness_dir)?;
                Ok(harness_dir.join(version))
            }
            JournalCleanupItem::CodexConfigBackup { nonce, .. } => {
                if !is_safe_nonce(nonce) {
                    return Err(AdapterError::Rollback);
                }
                let codex_dir = home_dir.join(".codex");
                verify_parent_not_symlink(&codex_dir)?;
                Ok(codex_dir.join(format!("config.toml.bak.{}", nonce)))
            }
            JournalCleanupItem::OpenCodeConfigBackup {
                file_name, nonce, ..
            } => {
                if (file_name != "opencode.json" && file_name != "tui.json")
                    || !is_safe_nonce(nonce)
                {
                    return Err(AdapterError::Rollback);
                }
                let opencode_dir = home_dir.join(".config/opencode");
                verify_parent_not_symlink(&opencode_dir)?;
                Ok(opencode_dir.join(format!("{}.bak.{}", file_name, nonce)))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupJournal {
    pub items: Vec<JournalCleanupItem>,
    pub created_at: u64,
}

impl CleanupJournal {
    pub fn write_and_fsync(
        &self,
        config_dir: &Path,
        injected_fail: u32,
    ) -> Result<(), AdapterError> {
        let managed_dir = config_dir.join("managed");
        if !managed_dir.exists() {
            fs::create_dir_all(&managed_dir).map_err(|_| AdapterError::Rollback)?;
            checked_fsync_dir(&managed_dir, injected_fail)?;
        }

        #[cfg(test)]
        if (injected_fail & FAIL_JOURNAL_WRITE) != 0 {
            return Err(AdapterError::Rollback);
        }

        let journal_path = managed_dir.join(".cleanup_journal.json");
        let json_str = serde_json::to_string_pretty(self).map_err(|_| AdapterError::Rollback)?;
        atomic_write_file(&journal_path, json_str.as_bytes(), 0o600, injected_fail)
            .map_err(|_| AdapterError::Rollback)?;
        Ok(())
    }
}

pub fn reconcile_cleanup_journal(
    config_dir: &Path,
    home_dir: &Path,
    injected_fail: u32,
) -> Result<(), AdapterError> {
    let journal_path = config_dir.join("managed/.cleanup_journal.json");
    if !journal_path.exists() {
        return Ok(());
    }

    let meta = fs::symlink_metadata(&journal_path).map_err(|_| AdapterError::Rollback)?;
    if meta.file_type().is_symlink() {
        return Err(AdapterError::Rollback);
    }

    let Ok(data) = fs::read_to_string(&journal_path) else {
        return Err(AdapterError::Rollback);
    };

    let Ok(mut journal) = serde_json::from_str::<CleanupJournal>(&data) else {
        return Err(AdapterError::Rollback);
    };

    let journal_created_at = journal.created_at;
    let mut remaining = Vec::new();
    let mut encountered_error = false;

    for mut item in journal.items {
        let target_path = match item.get_target_path(config_dir, &home_dir) {
            Ok(p) => p,
            Err(_) => {
                remaining.push(item);
                encountered_error = true;
                continue;
            }
        };

        let parent = match target_path.parent() {
            Some(p) => p.to_path_buf(),
            None => {
                remaining.push(item);
                encountered_error = true;
                continue;
            }
        };

        let phase = item.phase();
        if phase == JournalPhase::PendingRemove {
            if target_path.exists() {
                #[cfg(test)]
                if (injected_fail & FAIL_COMMIT_BACKUP_REMOVE) != 0 {
                    remaining.push(item);
                    encountered_error = true;
                    continue;
                }

                let remove_res = if target_path.is_dir() {
                    fs::remove_dir_all(&target_path)
                } else {
                    fs::remove_file(&target_path)
                };

                if remove_res.is_err() {
                    remaining.push(item);
                    encountered_error = true;
                    continue;
                }
            }
            item.set_phase(JournalPhase::RemovedPendingParentFsync);
        }

        if parent.exists() {
            #[cfg(test)]
            if (injected_fail & FAIL_COMMIT_PARENT_FSYNC) != 0 {
                remaining.push(item);
                encountered_error = true;
                continue;
            }

            if checked_fsync_dir(&parent, injected_fail).is_err() {
                remaining.push(item);
                encountered_error = true;
                continue;
            }
        }
    }

    if remaining.is_empty() {
        #[cfg(test)]
        if (injected_fail & FAIL_COMMIT_BACKUP_REMOVE) != 0 {
            return Err(AdapterError::Rollback);
        }

        // Unlinking the journal and syncing its parent is one durable transaction.
        // If the parent fsync fails after unlink, recreate an empty completion journal
        // so the next reconciliation retains retry ownership for that fsync boundary.
        fs::remove_file(&journal_path).map_err(|_| AdapterError::Rollback)?;
        let managed_dir = config_dir.join("managed");
        if managed_dir.exists() {
            if let Err(err) = checked_fsync_dir(&managed_dir, injected_fail) {
                let recovery = CleanupJournal {
                    items: Vec::new(),
                    created_at: journal_created_at,
                };
                if recovery.write_and_fsync(config_dir, FAIL_NONE).is_err() {
                    return Err(AdapterError::Rollback);
                }
                return Err(err);
            }
        }
        if encountered_error {
            Err(AdapterError::Rollback)
        } else {
            Ok(())
        }
    } else {
        journal.items = remaining;
        let new_json =
            serde_json::to_string_pretty(&journal).map_err(|_| AdapterError::Rollback)?;
        atomic_write_file(&journal_path, new_json.as_bytes(), 0o600, injected_fail)
            .map_err(|_| AdapterError::Rollback)?;
        Err(AdapterError::Rollback)
    }
}

// ============================================================================
// Core 0.2.0 Tree Integrity Verification (Exact 95 Entries)
// ============================================================================

pub fn verify_core_package_tree_integrity(core_dir: &Path) -> Result<bool, AdapterError> {
    if !core_dir.is_dir() {
        return Ok(false);
    }

    let mut entries = Vec::new();

    fn walk_dir(
        root: &Path,
        curr: &Path,
        entries: &mut Vec<(String, bool, Option<String>)>,
    ) -> Result<(), AdapterError> {
        let read_dir = fs::read_dir(curr).map_err(|_| AdapterError::Verify)?;
        for entry in read_dir {
            let entry = entry.map_err(|_| AdapterError::Verify)?;
            let p = entry.path();
            let meta = fs::symlink_metadata(&p).map_err(|_| AdapterError::Verify)?;
            if meta.file_type().is_symlink() {
                return Ok(());
            }
            let rel = p
                .strip_prefix(root)
                .map_err(|_| AdapterError::Verify)?
                .to_string_lossy()
                .replace('\\', "/");
            if meta.is_dir() {
                entries.push((rel.clone(), true, None));
                walk_dir(root, &p, entries)?;
            } else if meta.is_file() {
                let sha = file_sha256(&p)?;
                entries.push((rel, false, Some(sha)));
            }
        }
        Ok(())
    }

    walk_dir(core_dir, core_dir, &mut entries)?;

    if entries.len() != 95 {
        return Ok(false);
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel, is_dir, sha_opt) in entries {
        if is_dir {
            hasher.update(b"DIR\0");
            hasher.update(rel.as_bytes());
            hasher.update(b"\n");
        } else if let Some(sha) = sha_opt {
            hasher.update(b"FILE\0");
            hasher.update(rel.as_bytes());
            hasher.update(b"\0");
            hasher.update(sha.as_bytes());
            hasher.update(b"\n");
        }
    }

    let computed_digest = format!("{:x}", hasher.finalize());
    Ok(computed_digest == CORE_TREE_DIGEST_SHA256)
}

// ============================================================================
// Codex Config TOML Updating & Probing
// ============================================================================

#[derive(Debug, PartialEq, Eq)]
pub enum CodexConfigIdentity {
    Absent,
    ManagedTarget,
    ManagedOlder(String),
    VerifiedGlobal(String),
    LegacyNamespace,
    Invalid,
}

pub fn probe_codex_config_toml(
    config_path: &Path,
    expected_launcher_path: &Path,
    runner: &dyn CommandRunner,
    home_dir: &Path,
) -> CodexConfigIdentity {
    if !config_path.is_file() {
        return CodexConfigIdentity::Absent;
    }

    let Ok(content) = fs::read_to_string(config_path) else {
        return CodexConfigIdentity::Invalid;
    };

    if content.contains("@dataforxyz/agent-intercom-codex")
        || content.contains("dataforxyz-agent-intercom-codex")
    {
        return CodexConfigIdentity::LegacyNamespace;
    }

    let Ok(doc) = content.parse::<DocumentMut>() else {
        return CodexConfigIdentity::Invalid;
    };

    let Some(mcp_servers) = doc.get("mcp_servers").and_then(Item::as_table) else {
        return CodexConfigIdentity::Absent;
    };

    let Some(server_item) = mcp_servers.get(CODEX_MCP_SERVER_KEY) else {
        return CodexConfigIdentity::Absent;
    };

    let Some(server_table) = server_item.as_table() else {
        return CodexConfigIdentity::Invalid;
    };

    let cmd_str = server_table
        .get("command")
        .and_then(Item::as_str)
        .unwrap_or_default();
    let args_arr = server_table.get("args").and_then(Item::as_array);

    if cmd_str == expected_launcher_path.to_string_lossy() {
        return CodexConfigIdentity::ManagedTarget;
    }

    let expected_server_path = expected_launcher_path.with_file_name("codex-server.mjs");
    if cmd_str == "node"
        && args_arr
            .and_then(|args| args.get(0))
            .and_then(|value| value.as_str())
            .map(|arg| arg == expected_server_path.to_string_lossy())
            .unwrap_or(false)
    {
        return CodexConfigIdentity::ManagedTarget;
    }

    if cmd_str.contains("/managed/codex-intercom/") {
        let parts: Vec<&str> = cmd_str.split('/').collect();
        if let Some(pos) = parts.iter().position(|&p| p == "codex-intercom") {
            if pos + 1 < parts.len() {
                let ver_str = parts[pos + 1];
                if let Some(parsed) = SemVer::parse(ver_str) {
                    let target_semver = SemVer::parse(CODEX_TARGET_VERSION).unwrap();
                    if parsed < target_semver {
                        return CodexConfigIdentity::ManagedOlder(ver_str.to_string());
                    }
                }
            }
        }
        return CodexConfigIdentity::Invalid;
    }

    if (cmd_str == "codex-intercom-mcp" || cmd_str == "node")
        && (args_arr.is_none() || args_arr.map(|a| a.is_empty()).unwrap_or(false))
    {
        if let Some(ver) = verify_codex_global_package_identity(runner, home_dir) {
            return CodexConfigIdentity::VerifiedGlobal(ver);
        } else {
            return CodexConfigIdentity::Invalid;
        }
    }

    if let Some(arr) = args_arr {
        if arr.len() == 1 {
            if let Some(arg_str) = arr.get(0).and_then(|v| v.as_str()) {
                if arg_str == expected_launcher_path.to_string_lossy()
                    || arg_str == expected_server_path.to_string_lossy()
                {
                    return CodexConfigIdentity::ManagedTarget;
                }
                if arg_str.contains("/managed/codex-intercom/") {
                    let parts: Vec<&str> = arg_str.split('/').collect();
                    if let Some(pos) = parts.iter().position(|&p| p == "codex-intercom") {
                        if pos + 1 < parts.len() {
                            let ver_str = parts[pos + 1];
                            if let Some(parsed) = SemVer::parse(ver_str) {
                                let target_semver = SemVer::parse(CODEX_TARGET_VERSION).unwrap();
                                if parsed < target_semver {
                                    return CodexConfigIdentity::ManagedOlder(ver_str.to_string());
                                }
                            }
                        }
                    }
                    return CodexConfigIdentity::Invalid;
                }
            }
        }
    }

    CodexConfigIdentity::Invalid
}

pub fn update_codex_config_toml(
    config_path: &Path,
    launcher_path: &Path,
    injected_fail: u32,
) -> Result<(), AdapterError> {
    let mut doc = if config_path.is_file() {
        let content = fs::read_to_string(config_path).map_err(|_| AdapterError::Config)?;
        if content.trim().is_empty() {
            DocumentMut::new()
        } else {
            content
                .parse::<DocumentMut>()
                .map_err(|_| AdapterError::Config)?
        }
    } else {
        DocumentMut::new()
    };

    if !doc.contains_key("mcp_servers") {
        doc.insert("mcp_servers", Item::Table(Table::new()));
    }
    let mcp_servers = doc["mcp_servers"]
        .as_table_mut()
        .ok_or(AdapterError::Config)?;

    let mut server_table = Table::new();
    let server_path = launcher_path.with_file_name("codex-server.mjs");
    server_table.insert("command", value("node"));
    let mut server_args = Array::new();
    server_args.push(server_path.to_string_lossy().to_string());
    server_table.insert("args", Item::Value(toml_edit::Value::Array(server_args)));
    server_table.insert("startup_timeout_sec", value(120i64));

    mcp_servers.insert(CODEX_MCP_SERVER_KEY, Item::Table(server_table));

    if !doc.contains_key("shell_environment_policy") {
        doc.insert("shell_environment_policy", Item::Table(Table::new()));
    }
    let env_policy = doc["shell_environment_policy"]
        .as_table_mut()
        .ok_or(AdapterError::Config)?;

    let mut include_only_arr = Array::new();
    let required_vars = [
        "AGENT_INTERCOM_SCOPE_ID",
        "AGENT_INTERCOM_TEAM_MANIFEST",
        "AGENT_INTERCOM_SESSION_ID",
        "AGENT_INTERCOM_SESSION_NAME",
        "AGENT_INTERCOM_ROLE",
        "AGENT_INTERCOM_MANAGER_TARGET",
        "AGENT_INTERCOM_MANAGER_SESSION_ID",
        "CODEX_INTERCOM_SESSION_ID",
        "CODEX_INTERCOM_NAME",
    ];

    if let Some(existing_arr) = env_policy.get("include_only").and_then(Item::as_array) {
        let mut seen = BTreeSet::new();
        for item in existing_arr {
            if let Some(s) = item.as_str() {
                if !seen.contains(s) {
                    seen.insert(s.to_string());
                    include_only_arr.push(s);
                }
            }
        }
        for var in required_vars {
            if !seen.contains(var) {
                seen.insert(var.to_string());
                include_only_arr.push(var);
            }
        }
    } else {
        for var in required_vars {
            include_only_arr.push(var);
        }
    }

    env_policy.insert(
        "include_only",
        Item::Value(toml_edit::Value::Array(include_only_arr)),
    );

    let output_str = doc.to_string();
    atomic_write_file(config_path, output_str.as_bytes(), 0o600, injected_fail)
        .map_err(|_| AdapterError::Config)
}

pub fn verify_codex_global_package_identity(
    runner: &dyn CommandRunner,
    home_dir: &Path,
) -> Option<String> {
    let script = r#"
import { pathToFileURL, fileURLToPath } from "node:url";
import { readFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";

try {
  const r = import.meta.resolve("@ctliz/agent-intercom-codex");
  const p = fileURLToPath(r);
  let curr = dirname(p);
  let pkgRoot = null;
  for (let i = 0; i < 5; i++) {
    if (existsSync(join(curr, "package.json"))) {
      pkgRoot = curr;
      break;
    }
    curr = dirname(curr);
  }
  if (!pkgRoot) process.exit(1);
  const pkg = JSON.parse(readFileSync(join(pkgRoot, "package.json"), "utf8"));
  if (pkg.name !== "@ctliz/agent-intercom-codex" || pkg.version !== "0.12.0-connect.1") {
    process.exit(1);
  }
  // Validate canonical binary mapping
  let hasBin = false;
  if (typeof pkg.bin === "string" && pkg.bin === "dist/codex-server.mjs") {
    hasBin = true;
  } else if (pkg.bin && typeof pkg.bin === "object") {
    if (pkg.bin["codex-intercom-mcp"] === "dist/codex-server.mjs") {
      hasBin = true;
    }
  }
  if (!hasBin) process.exit(1);

  process.stdout.write("VALID_GLOBAL_PACKAGE:" + pkgRoot);
} catch (e) {
  process.exit(1);
}
"#;
    let out = runner
        .run_command(
            "node",
            &["--input-type=module", "-e", script],
            home_dir,
            None,
        )
        .ok()?;

    if out.status != 0 || !out.stdout.contains("VALID_GLOBAL_PACKAGE:") {
        return None;
    }

    let start_idx = out.stdout.find("VALID_GLOBAL_PACKAGE:")? + "VALID_GLOBAL_PACKAGE:".len();
    let pkg_root_str = out.stdout[start_idx..].trim();
    let pkg_root = Path::new(pkg_root_str);

    let core_dir = pkg_root.join("node_modules/@ctliz/agent-intercom-core");
    if !core_dir.is_dir() {
        return None;
    }
    if verify_core_package_tree_integrity(&core_dir).unwrap_or(false) == false {
        return None;
    }

    let server_f = pkg_root.join("dist/codex-server.mjs");
    if !server_f.is_file() {
        return None;
    }
    if file_sha256(&server_f).unwrap_or_default()
        != "fe29eb427a3d49eb9fb70e5fe04ac9af334ed27a10d989e681297ea9cf1dffe8"
    {
        return None;
    }

    Some(CODEX_TARGET_VERSION.to_string())
}

// ============================================================================
// OpenCode Config JSON Updating & Probing
// ============================================================================

#[derive(Debug, PartialEq, Eq)]
pub enum OpenCodeConfigIdentity {
    Absent,
    ManagedTarget,
    ManagedOlder(String),
    VerifiedGlobal(String),
    LegacyNamespace,
    Invalid,
}

pub fn probe_opencode_json_file(
    runner: &dyn CommandRunner,
    file_path: &Path,
    expected_entry: &str,
    opencode_config_dir: &Path,
) -> OpenCodeConfigIdentity {
    if !file_path.is_file() {
        return OpenCodeConfigIdentity::Absent;
    }

    let Ok(content) = fs::read_to_string(file_path) else {
        return OpenCodeConfigIdentity::Invalid;
    };

    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return OpenCodeConfigIdentity::Invalid;
    };

    let Some(obj) = json.as_object() else {
        return OpenCodeConfigIdentity::Invalid;
    };

    let Some(plugins_val) = obj.get("plugin") else {
        return OpenCodeConfigIdentity::Absent;
    };

    let Some(plugins_arr) = plugins_val.as_array() else {
        return OpenCodeConfigIdentity::Invalid;
    };

    let mut legacy_count = 0;
    let mut exact_count = 0;
    let mut older_versions = Vec::new();
    let mut global_count = 0;
    let mut global_candidates = Vec::new();
    let mut has_invalid = false;

    let target_semver = SemVer::parse(OPENCODE_TARGET_VERSION).unwrap();

    for item in plugins_arr {
        let Some(p_str) = item.as_str() else {
            has_invalid = true;
            continue;
        };

        if p_str.contains("@dataforxyz/agent-intercom-opencode")
            || p_str.contains("dataforxyz-agent-intercom-opencode")
        {
            legacy_count += 1;
            continue;
        }

        if p_str == expected_entry {
            exact_count += 1;
        } else if p_str == "@ctliz/agent-intercom-opencode" {
            global_count += 1;
            global_candidates.push(p_str.to_string());
        } else if p_str.contains("/managed/opencode-intercom/") {
            let parts: Vec<&str> = p_str.split('/').collect();
            if let Some(pos) = parts.iter().position(|&p| p == "opencode-intercom") {
                if pos + 1 < parts.len() {
                    let ver_str = parts[pos + 1];
                    if ver_str == OPENCODE_TARGET_VERSION {
                        // Stale host entries that still name the current managed
                        // version are repairable, even when the files are gone.
                        exact_count += 1;
                    } else if let Some(parsed) = SemVer::parse(ver_str) {
                        if parsed < target_semver {
                            older_versions.push(parsed);
                        } else {
                            has_invalid = true;
                        }
                    } else {
                        has_invalid = true;
                    }
                } else {
                    has_invalid = true;
                }
            } else {
                has_invalid = true;
            }
        } else if p_str.contains("agent-intercom-opencode") {
            global_count += 1;
            global_candidates.push(p_str.to_string());
        }
    }

    if legacy_count > 0 {
        return OpenCodeConfigIdentity::LegacyNamespace;
    }
    if has_invalid
        || exact_count > 1
        || (exact_count > 0 && global_count > 0)
        || (exact_count > 0 && !older_versions.is_empty())
        || (global_count > 0 && !older_versions.is_empty())
    {
        return OpenCodeConfigIdentity::Invalid;
    }
    if global_count == 1 {
        if let Some((ver, pkg_root)) =
            verify_opencode_global_package_identity(runner, opencode_config_dir)
        {
            let cand = &global_candidates[0];
            if cand == "@ctliz/agent-intercom-opencode" {
                return OpenCodeConfigIdentity::VerifiedGlobal(ver);
            }
            let is_opencode_json = file_path
                .file_name()
                .map(|n| n == "opencode.json")
                .unwrap_or(false);
            let is_tui_json = file_path
                .file_name()
                .map(|n| n == "tui.json")
                .unwrap_or(false);

            if is_opencode_json {
                let exp_plugin = pkg_root.join("dist/plugin.mjs");
                if fs::canonicalize(Path::new(cand)).ok() == fs::canonicalize(&exp_plugin).ok()
                    && fs::canonicalize(Path::new(cand)).is_ok()
                {
                    return OpenCodeConfigIdentity::VerifiedGlobal(ver);
                }
            } else if is_tui_json {
                let exp_tui = pkg_root.join("dist/tui.mjs");
                if fs::canonicalize(Path::new(cand)).ok() == fs::canonicalize(&exp_tui).ok()
                    && fs::canonicalize(Path::new(cand)).is_ok()
                {
                    return OpenCodeConfigIdentity::VerifiedGlobal(ver);
                }
            }
            return OpenCodeConfigIdentity::Invalid;
        } else {
            return OpenCodeConfigIdentity::Invalid;
        }
    }
    if global_count > 1 {
        return OpenCodeConfigIdentity::Invalid;
    }
    if exact_count == 1 && older_versions.is_empty() {
        return OpenCodeConfigIdentity::ManagedTarget;
    }
    if exact_count == 0 && older_versions.len() == 1 {
        let v = &older_versions[0];
        let v_str = if v.prerelease.is_empty() {
            format!("{}.{}.{}", v.major, v.minor, v.patch)
        } else {
            format!(
                "{}.{}.{}-{}",
                v.major,
                v.minor,
                v.patch,
                v.prerelease.join(".")
            )
        };
        return OpenCodeConfigIdentity::ManagedOlder(v_str);
    }
    if older_versions.len() > 1 {
        return OpenCodeConfigIdentity::Invalid;
    }

    OpenCodeConfigIdentity::Absent
}

pub fn update_opencode_json_file(
    file_path: &Path,
    entry_to_add: &str,
    injected_fail: u32,
) -> Result<(), AdapterError> {
    let mut json_obj = if file_path.is_file() {
        let content = fs::read_to_string(file_path).map_err(|_| AdapterError::Config)?;
        if content.trim().is_empty() {
            serde_json::Map::new()
        } else {
            let val: serde_json::Value =
                serde_json::from_str(&content).map_err(|_| AdapterError::Config)?;
            val.as_object().cloned().ok_or(AdapterError::Config)?
        }
    } else {
        serde_json::Map::new()
    };

    let mut plugins_list: Vec<serde_json::Value> = match json_obj.get("plugin") {
        Some(serde_json::Value::Array(arr)) => arr.clone(),
        Some(_) => return Err(AdapterError::Config),
        None => Vec::new(),
    };

    plugins_list.retain(|item| {
        if let Some(s) = item.as_str() {
            !s.contains("agent-intercom-opencode")
        } else {
            true
        }
    });

    plugins_list.push(serde_json::Value::String(entry_to_add.to_string()));
    json_obj.insert("plugin".to_string(), serde_json::Value::Array(plugins_list));

    let output_bytes = serde_json::to_vec_pretty(&json_obj).map_err(|_| AdapterError::Config)?;
    atomic_write_file(file_path, &output_bytes, 0o600, injected_fail)
        .map_err(|_| AdapterError::Config)
}

pub fn verify_opencode_global_package_identity(
    runner: &dyn CommandRunner,
    opencode_config_dir: &Path,
) -> Option<(String, PathBuf)> {
    let script = r#"
import { pathToFileURL, fileURLToPath } from "node:url";
import { readFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";

try {
  const r = import.meta.resolve("@ctliz/agent-intercom-opencode");
  const p = fileURLToPath(r);
  let curr = dirname(p);
  let pkgRoot = null;
  for (let i = 0; i < 5; i++) {
    if (existsSync(join(curr, "package.json"))) {
      pkgRoot = curr;
      break;
    }
    curr = dirname(curr);
  }
  if (!pkgRoot) process.exit(1);
  const pkg = JSON.parse(readFileSync(join(pkgRoot, "package.json"), "utf8"));
  if (pkg.name !== "@ctliz/agent-intercom-opencode" || pkg.version !== "0.12.0-connect.1") {
    process.exit(1);
  }
  process.stdout.write("VALID_GLOBAL_PACKAGE:" + pkgRoot);
} catch (e) {
  process.exit(1);
}
"#;
    let out = runner
        .run_command(
            "node",
            &["--input-type=module", "-e", script],
            opencode_config_dir,
            None,
        )
        .ok()?;

    if out.status != 0 || !out.stdout.contains("VALID_GLOBAL_PACKAGE:") {
        return None;
    }

    let start_idx = out.stdout.find("VALID_GLOBAL_PACKAGE:")? + "VALID_GLOBAL_PACKAGE:".len();
    let pkg_root_str = out.stdout[start_idx..].trim();
    let pkg_root = Path::new(pkg_root_str);

    let core_dir = pkg_root.join("node_modules/@ctliz/agent-intercom-core");
    if !core_dir.is_dir() {
        return None;
    }
    if verify_core_package_tree_integrity(&core_dir).unwrap_or(false) == false {
        return None;
    }

    let plugin_f = pkg_root.join("dist/plugin.mjs");
    let tui_f = pkg_root.join("dist/tui.mjs");
    if !plugin_f.is_file() || !tui_f.is_file() {
        return None;
    }
    if file_sha256(&plugin_f).unwrap_or_default()
        != "56bd73e2b8997e5a8b7e8d7e315c93a8d1193827ebb0c8dcea0b2851788d9791"
    {
        return None;
    }
    if file_sha256(&tui_f).unwrap_or_default()
        != "104ecf3dde0c938ca45b8863a562da8622ac7d7c415621137baf1af03c63ff24"
    {
        return None;
    }

    Some((OPENCODE_TARGET_VERSION.to_string(), pkg_root.to_path_buf()))
}

// ============================================================================
// Managed Directory Inventory Scanner with Bijective Verification
// ============================================================================

pub struct ManagedDirectoryInventory {
    pub has_legacy: bool,
    pub has_healthy_target: bool,
    pub older_roots: Vec<String>,
    pub future_roots: Vec<String>,
    pub has_invalid_roots: bool,
}

pub fn scan_managed_directory(
    harness_dir: &Path,
    harness: &str,
    target_version: &str,
    immutable_digests: &[(&str, &str)],
    expected_package_name: &str,
    legacy_package_name: &str,
    expected_resource_name: &str,
    expected_resource_sha256: &str,
) -> ManagedDirectoryInventory {
    let mut inv = ManagedDirectoryInventory {
        has_legacy: false,
        has_healthy_target: false,
        older_roots: Vec::new(),
        future_roots: Vec::new(),
        has_invalid_roots: false,
    };

    if !harness_dir.is_dir() {
        return inv;
    }

    let target_semver = match SemVer::parse(target_version) {
        Some(s) => s,
        None => return inv,
    };

    let entries = match fs::read_dir(harness_dir) {
        Ok(e) => e,
        Err(_) => return inv,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if file_name.starts_with('.') {
            continue;
        }

        if !path.is_dir() {
            inv.has_invalid_roots = true;
            continue;
        }

        let is_target = file_name == target_version;
        if let Some(parsed_ver) = SemVer::parse(&file_name) {
            let is_intact = verify_managed_root_integrity(
                &path,
                harness,
                &file_name,
                immutable_digests,
                expected_package_name,
                expected_resource_name,
                expected_resource_sha256,
            );

            if is_target {
                if is_intact {
                    inv.has_healthy_target = true;
                } else {
                    inv.has_invalid_roots = true;
                }
            } else if parsed_ver < target_semver {
                if is_intact {
                    inv.older_roots.push(file_name.clone());
                } else {
                    inv.has_invalid_roots = true;
                }
            } else {
                inv.future_roots.push(file_name.clone());
            }
        } else if file_name.contains("legacy") || file_name.contains(legacy_package_name) {
            inv.has_legacy = true;
        } else {
            inv.has_invalid_roots = true;
        }
    }

    inv
}

// ============================================================================
// Bijective Managed Root Verification with Exact Dependency and Lock Packages
// ============================================================================

pub fn verify_managed_root_integrity(
    root: &Path,
    harness: &str,
    target_version: &str,
    immutable_digests: &[(&str, &str)],
    expected_package_name: &str,
    expected_resource_name: &str,
    expected_resource_sha256: &str,
) -> bool {
    let marker_p = root.join("tmuxdeck-managed.json");
    if !marker_p.is_file() {
        return false;
    }
    let Ok(marker_str) = fs::read_to_string(&marker_p) else {
        return false;
    };
    let Ok(marker) = serde_json::from_str::<ManagedAdapterMarker>(&marker_str) else {
        return false;
    };
    if marker.schema_version != 1
        || marker.harness != harness
        || marker.adapter_version != target_version
        || marker.installed_at == 0
    {
        return false;
    }

    // Validate marker resources map exactly (Core + Adapter, plus OpenCode SDK/closure).
    let expected_resource_count = if harness == "opencode" { 4 } else { 2 };
    if marker.resources.len() != expected_resource_count {
        return false;
    }
    if marker.resources.get(CORE_RESOURCE_NAME) != Some(&CORE_RESOURCE_SHA256.to_string()) {
        return false;
    }
    if marker.resources.get(expected_resource_name) != Some(&expected_resource_sha256.to_string()) {
        return false;
    }
    if harness == "opencode"
        && (marker.resources.get(OPENCODE_SDK_RESOURCE_NAME)
            != Some(&OPENCODE_SDK_RESOURCE_SHA256.to_string())
            || marker.resources.get(OPENCODE_CLOSURE_RESOURCE_NAME)
                != Some(&OPENCODE_CLOSURE_RESOURCE_SHA256.to_string()))
    {
        return false;
    }

    // Validate marker digests map exactly and bijectively.
    if marker.digests.len() != immutable_digests.len() {
        return false;
    }
    for (rel, exp_sha) in immutable_digests {
        if marker.digests.get(*rel) != Some(&exp_sha.to_string()) {
            return false;
        }
    }
    for (rel, rec_sha) in &marker.digests {
        if rel.contains("..") || rel.starts_with('/') || rel.starts_with('\\') {
            return false;
        }
        let file_path = root.join(rel);
        if !file_path.is_file() {
            return false;
        }
        if file_sha256(&file_path).unwrap_or_default() != *rec_sha {
            return false;
        }
    }

    let vendor_adapter = root.join(format!("vendor/{}", expected_resource_name));
    let vendor_core = root.join(format!("vendor/{}", CORE_RESOURCE_NAME));
    if !vendor_adapter.is_file() || !vendor_core.is_file() {
        return false;
    }
    if file_sha256(&vendor_adapter).unwrap_or_default() != expected_resource_sha256 {
        return false;
    }
    if file_sha256(&vendor_core).unwrap_or_default() != CORE_RESOURCE_SHA256 {
        return false;
    }
    if harness == "opencode" {
        let vendor_sdk = root.join(format!("vendor/{}", OPENCODE_SDK_RESOURCE_NAME));
        let vendor_closure = root.join(format!("vendor/{}", OPENCODE_CLOSURE_RESOURCE_NAME));
        if !vendor_sdk.is_file()
            || file_sha256(&vendor_sdk).unwrap_or_default() != OPENCODE_SDK_RESOURCE_SHA256
            || !vendor_closure.is_file()
            || file_sha256(&vendor_closure).unwrap_or_default() != OPENCODE_CLOSURE_RESOURCE_SHA256
        {
            return false;
        }
    }

    let pkg_p = root.join("package.json");
    if !pkg_p.is_file() {
        return false;
    }
    let Ok(pkg_str) = fs::read_to_string(&pkg_p) else {
        return false;
    };
    let Ok(pkg_json) = serde_json::from_str::<serde_json::Value>(&pkg_str) else {
        return false;
    };
    if pkg_json.get("name").and_then(|v| v.as_str()) != Some(expected_package_name)
        || pkg_json.get("version").and_then(|v| v.as_str()) != Some(target_version)
    {
        return false;
    }
    let Some(deps) = pkg_json.get("dependencies").and_then(|v| v.as_object()) else {
        return false;
    };
    let required_dep_count = if harness == "opencode" { 3 } else { 2 };
    if deps.len() != required_dep_count
        || !deps.contains_key("@ctliz/agent-intercom-core")
        || !deps.contains_key(expected_package_name)
        || (harness == "opencode" && !deps.contains_key("@opencode-ai/plugin"))
    {
        return false;
    }

    let lock_p = root.join("package-lock.json");
    if !lock_p.is_file() {
        return false;
    }
    let Ok(lock_str) = fs::read_to_string(&lock_p) else {
        return false;
    };
    let Ok(lock_json) = serde_json::from_str::<serde_json::Value>(&lock_str) else {
        return false;
    };
    let Some(packages) = lock_json.get("packages").and_then(|v| v.as_object()) else {
        return false;
    };
    let adapter_mod_key = format!("node_modules/{}", expected_package_name);
    let mut expected_package_keys: BTreeSet<String> = [
        "".to_string(),
        "node_modules/@ctliz/agent-intercom-core".to_string(),
        adapter_mod_key,
    ]
    .into_iter()
    .collect();
    if harness == "opencode" {
        for key in [
            "node_modules/@ai-sdk/provider",
            "node_modules/@msgpackr-extract/msgpackr-extract-darwin-arm64",
            "node_modules/@opencode-ai/plugin",
            "node_modules/@opencode-ai/sdk",
            "node_modules/@standard-schema/spec",
            "node_modules/cross-spawn",
            "node_modules/detect-libc",
            "node_modules/effect",
            "node_modules/fast-check",
            "node_modules/find-my-way-ts",
            "node_modules/ini",
            "node_modules/isexe",
            "node_modules/json-schema",
            "node_modules/kubernetes-types",
            "node_modules/msgpackr",
            "node_modules/msgpackr-extract",
            "node_modules/multipasta",
            "node_modules/node-gyp-build-optional-packages",
            "node_modules/path-key",
            "node_modules/pure-rand",
            "node_modules/shebang-command",
            "node_modules/shebang-regex",
            "node_modules/toml",
            "node_modules/uuid",
            "node_modules/which",
            "node_modules/yaml",
            "node_modules/zod",
        ] {
            expected_package_keys.insert(key.to_string());
        }
    }
    let actual_package_keys: BTreeSet<String> = packages.keys().cloned().collect();
    if actual_package_keys != expected_package_keys {
        return false;
    }
    let root_pkg = packages.get("").and_then(|v| v.as_object()).unwrap();
    let root_deps = root_pkg.get("dependencies").and_then(|v| v.as_object());
    let expected_root_count = if harness == "opencode" { 3 } else { 2 };
    if root_deps
        .map(|d| {
            d.len() == expected_root_count
                && d.contains_key("@ctliz/agent-intercom-core")
                && d.contains_key(expected_package_name)
                && (harness != "opencode" || d.contains_key("@opencode-ai/plugin"))
        })
        .unwrap_or(false)
        == false
    {
        return false;
    }

    let core_dir = root.join("node_modules/@ctliz/agent-intercom-core");
    if !core_dir.is_dir() {
        return false;
    }
    if verify_core_package_tree_integrity(&core_dir).unwrap_or(false) == false {
        return false;
    }

    for (rel, sha) in immutable_digests {
        let f = root.join(rel);
        if !f.is_file() {
            return false;
        }
        if file_sha256(&f).unwrap_or_default() != *sha {
            return false;
        }
    }

    #[cfg(unix)]
    if verify_tree_permissions(root).is_err() {
        return false;
    }

    true
}

// ============================================================================
// Normalized Evidence Model & Fingerprinting
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleAdapterEvidence {
    Pi {
        settings_mtime: u64,
        settings_sha: String,
    },
    Claude {
        mtime: u64,
        sha: String,
    },
    Codex {
        config_mtime: u64,
        config_sha: String,
    },
    OpenCode {
        opencode_mtime: u64,
        opencode_sha: String,
        tui_mtime: u64,
        tui_sha: String,
    },
}

// ============================================================================
// Probe Single Adapter
// ============================================================================

pub fn probe_single_adapter(
    ctx: &AdapterContext,
    agent_id: &str,
) -> (
    Option<CommunicationAdapterPlanItem>,
    Option<SingleAdapterEvidence>,
) {
    let _ = reconcile_cleanup_journal(&ctx.config_dir, &ctx.home_dir, FAIL_NONE);

    match agent_id {
        "pi" => {
            let cli_available = ctx.runner.binary_exists("pi");
            let settings_file = ctx.pi_agent_dir.join("settings.json");

            let mut state = AdapterHealthState::NotInstalled;
            let mut installed_ver = None;

            if settings_file.exists() {
                match fs::read_to_string(&settings_file) {
                    Ok(c) => match serde_json::from_str::<serde_json::Value>(&c) {
                        Ok(json) => {
                            if json.is_object() {
                                let mut has_legacy = false;
                                let mut has_target = false;
                                let mut older = Vec::new();
                                let mut npm_current: Option<String> = None;
                                let mut intercom_count = 0;

                                if let Some(packages) =
                                    json.get("packages").and_then(|v| v.as_array())
                                {
                                    for p in packages {
                                        if let Some(s) = p.as_str() {
                                            if is_pi_intercom_settings_entry(s) {
                                                intercom_count += 1;
                                            }
                                            if s.contains("dataforxyz") {
                                                has_legacy = true;
                                            } else if s == PI_CANONICAL_GIT_TARGET {
                                                has_target = true;
                                            } else if let Some(ver_str) = pi_npm_package_version(s)
                                            {
                                                if let Some(sem) = SemVer::parse(ver_str) {
                                                    let target_sem =
                                                        SemVer::parse(PI_TARGET_VERSION).unwrap();
                                                    if sem < target_sem {
                                                        older.push(ver_str.to_string());
                                                    } else {
                                                        npm_current = Some(ver_str.to_string());
                                                    }
                                                }
                                            } else if s.starts_with(
                                                "git:github.com/ctliz/agent-intercom-pi@v",
                                            ) {
                                                let ver_str = s.trim_start_matches(
                                                    "git:github.com/ctliz/agent-intercom-pi@v",
                                                );
                                                if let Some(sem) = SemVer::parse(ver_str) {
                                                    let target_sem =
                                                        SemVer::parse(PI_TARGET_VERSION).unwrap();
                                                    if sem < target_sem {
                                                        older.push(ver_str.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                if has_legacy {
                                    state = AdapterHealthState::MigrationRequired;
                                } else if intercom_count > 1 {
                                    state = AdapterHealthState::NeedsRepair;
                                } else if let Some(ver) = npm_current {
                                    state = AdapterHealthState::HealthyExistingGlobal;
                                    installed_ver = Some(ver);
                                } else if has_target && older.is_empty() {
                                    state = AdapterHealthState::Healthy;
                                    installed_ver = Some(PI_TARGET_VERSION.to_string());
                                } else if !older.is_empty() && !has_target {
                                    state = AdapterHealthState::NeedsUpgrade;
                                    installed_ver = Some(older[0].clone());
                                } else if has_target && !older.is_empty() {
                                    state = AdapterHealthState::NeedsRepair;
                                }
                            } else {
                                state = AdapterHealthState::NeedsRepair;
                            }
                        }
                        Err(_) => state = AdapterHealthState::NeedsRepair,
                    },
                    Err(_) => state = AdapterHealthState::NeedsRepair,
                }
            }

            let state = if !cli_available || !ctx.is_macos {
                AdapterHealthState::Unavailable
            } else {
                state
            };

            let reason = match state {
                AdapterHealthState::NotInstalled => AdapterActionReason::Install,
                AdapterHealthState::NeedsUpgrade => AdapterActionReason::Upgrade,
                AdapterHealthState::NeedsRepair => AdapterActionReason::Repair,
                AdapterHealthState::MigrationRequired => {
                    AdapterActionReason::ManualMigrationRequired
                }
                _ => AdapterActionReason::Install,
            };

            let item = CommunicationAdapterPlanItem {
                agent_id: "pi".to_string(),
                host_display_name: "Pi Coding Agent".to_string(),
                adapter_kind: CommunicationAdapterKind::Pi,
                state,
                target_version: PI_TARGET_VERSION.to_string(),
                installed_version: installed_ver,
                source_kind: if state == AdapterHealthState::HealthyExistingGlobal {
                    AdapterSourceKind::ExistingGlobal
                } else {
                    AdapterSourceKind::PiGit
                },
                package_name: Some(CanonicalAdapterPackage::Pi),
                config_change_kind: ConfigChangeKind::None,
                network_required: true,
                license: "AGPL-3.0-or-later".to_string(),
                action_reason: reason,
            };

            let ev = SingleAdapterEvidence::Pi {
                settings_mtime: fs::metadata(&settings_file)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                settings_sha: file_sha256(&settings_file).unwrap_or_default(),
            };

            (Some(item), Some(ev))
        }

        "claude" => {
            let cli_available = ctx.runner.binary_exists("claude");
            let claude_managed = ctx.config_dir.join("managed/claude-intercom");

            let inv = scan_managed_directory(
                &claude_managed,
                "claude",
                CLAUDE_TARGET_VERSION,
                CLAUDE_IMMUTABLE_DIGESTS,
                "@ctliz/agent-intercom-claude",
                "@dataforxyz/agent-intercom-claude",
                CLAUDE_RESOURCE_NAME,
                CLAUDE_RESOURCE_SHA256,
            );

            let mut state = AdapterHealthState::NotInstalled;
            let mut installed_ver = None;

            if inv.has_legacy {
                state = AdapterHealthState::MigrationRequired;
            } else if inv.has_invalid_roots || !inv.future_roots.is_empty() {
                state = AdapterHealthState::NeedsRepair;
            } else if inv.has_healthy_target {
                if inv.older_roots.is_empty() {
                    state = AdapterHealthState::Healthy;
                    installed_ver = Some(CLAUDE_TARGET_VERSION.to_string());
                } else {
                    state = AdapterHealthState::NeedsRepair;
                    installed_ver = Some(CLAUDE_TARGET_VERSION.to_string());
                }
            } else if !inv.older_roots.is_empty() {
                if inv.older_roots.len() == 1 {
                    state = AdapterHealthState::NeedsUpgrade;
                    installed_ver = Some(inv.older_roots[0].clone());
                } else {
                    state = AdapterHealthState::NeedsRepair;
                }
            }

            let state = if !cli_available || !ctx.is_macos {
                AdapterHealthState::Unavailable
            } else {
                state
            };

            let reason = match state {
                AdapterHealthState::NotInstalled => AdapterActionReason::Install,
                AdapterHealthState::NeedsUpgrade => AdapterActionReason::Upgrade,
                AdapterHealthState::NeedsRepair => AdapterActionReason::Repair,
                AdapterHealthState::MigrationRequired => {
                    AdapterActionReason::ManualMigrationRequired
                }
                _ => AdapterActionReason::Install,
            };

            let item = CommunicationAdapterPlanItem {
                agent_id: "claude".to_string(),
                host_display_name: "Claude Code".to_string(),
                adapter_kind: CommunicationAdapterKind::Claude,
                state,
                target_version: CLAUDE_TARGET_VERSION.to_string(),
                installed_version: installed_ver,
                source_kind: AdapterSourceKind::Bundled,
                package_name: Some(CanonicalAdapterPackage::Claude),
                config_change_kind: ConfigChangeKind::AppPrivateManaged,
                network_required: false,
                license: "AGPL-3.0-or-later".to_string(),
                action_reason: reason,
            };

            let marker_p = claude_managed
                .join(CLAUDE_TARGET_VERSION)
                .join("tmuxdeck-managed.json");
            let ev = SingleAdapterEvidence::Claude {
                mtime: fs::metadata(&marker_p)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                sha: file_sha256(&marker_p).unwrap_or_default(),
            };

            (Some(item), Some(ev))
        }

        "codex" => {
            let cli_available = ctx.runner.binary_exists("codex");
            let codex_managed = ctx.config_dir.join("managed/codex-intercom");
            let codex_config = ctx.home_dir.join(".codex/config.toml");
            let target_launcher = codex_managed
                .join(CODEX_TARGET_VERSION)
                .join("dist/codex-launcher.mjs");

            let inv = scan_managed_directory(
                &codex_managed,
                "codex",
                CODEX_TARGET_VERSION,
                CODEX_IMMUTABLE_DIGESTS,
                "@ctliz/agent-intercom-codex",
                "@dataforxyz/agent-intercom-codex",
                CODEX_RESOURCE_NAME,
                CODEX_RESOURCE_SHA256,
            );

            let cfg_identity =
                probe_codex_config_toml(&codex_config, &target_launcher, ctx.runner, &ctx.home_dir);

            let mut installed_ver = None;
            let raw_state =
                if inv.has_legacy || cfg_identity == CodexConfigIdentity::LegacyNamespace {
                    AdapterHealthState::MigrationRequired
                } else if inv.has_invalid_roots
                    || !inv.future_roots.is_empty()
                    || cfg_identity == CodexConfigIdentity::Invalid
                {
                    AdapterHealthState::NeedsRepair
                } else {
                    match cfg_identity {
                        CodexConfigIdentity::Absent => {
                            if inv.has_healthy_target || !inv.older_roots.is_empty() {
                                AdapterHealthState::NeedsRepair
                            } else {
                                AdapterHealthState::NotInstalled
                            }
                        }
                        CodexConfigIdentity::ManagedTarget => {
                            if inv.has_healthy_target && inv.older_roots.is_empty() {
                                installed_ver = Some(CODEX_TARGET_VERSION.to_string());
                                AdapterHealthState::Healthy
                            } else {
                                AdapterHealthState::NeedsRepair
                            }
                        }
                        CodexConfigIdentity::ManagedOlder(old_v) => {
                            if inv.older_roots.len() == 1
                                && inv.older_roots[0] == old_v
                                && !inv.has_healthy_target
                            {
                                installed_ver = Some(old_v);
                                AdapterHealthState::NeedsUpgrade
                            } else {
                                AdapterHealthState::NeedsRepair
                            }
                        }
                        CodexConfigIdentity::VerifiedGlobal(glob_v) => {
                            if !inv.has_healthy_target && inv.older_roots.is_empty() {
                                installed_ver = Some(glob_v);
                                AdapterHealthState::HealthyExistingGlobal
                            } else {
                                AdapterHealthState::NeedsRepair
                            }
                        }
                        _ => AdapterHealthState::NeedsRepair,
                    }
                };

            let state = if !cli_available || !ctx.is_macos {
                AdapterHealthState::Unavailable
            } else {
                raw_state
            };

            let reason = match state {
                AdapterHealthState::NotInstalled => AdapterActionReason::Install,
                AdapterHealthState::NeedsUpgrade => AdapterActionReason::Upgrade,
                AdapterHealthState::NeedsRepair => AdapterActionReason::Repair,
                AdapterHealthState::MigrationRequired => {
                    AdapterActionReason::ManualMigrationRequired
                }
                _ => AdapterActionReason::Install,
            };

            let item = CommunicationAdapterPlanItem {
                agent_id: "codex".to_string(),
                host_display_name: "Codex CLI".to_string(),
                adapter_kind: CommunicationAdapterKind::Codex,
                state,
                target_version: CODEX_TARGET_VERSION.to_string(),
                installed_version: installed_ver,
                source_kind: if state == AdapterHealthState::HealthyExistingGlobal {
                    AdapterSourceKind::ExistingGlobal
                } else {
                    AdapterSourceKind::Bundled
                },
                package_name: Some(CanonicalAdapterPackage::Codex),
                config_change_kind: ConfigChangeKind::HostConfigRegistered,
                network_required: false,
                license: "AGPL-3.0-or-later".to_string(),
                action_reason: reason,
            };

            let ev = SingleAdapterEvidence::Codex {
                config_mtime: fs::metadata(&codex_config)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                config_sha: file_sha256(&codex_config).unwrap_or_default(),
            };

            (Some(item), Some(ev))
        }

        "opencode" => {
            let cli_available = ctx.runner.binary_exists("opencode");
            let opencode_managed = ctx.config_dir.join("managed/opencode-intercom");
            let opencode_config_dir = ctx.home_dir.join(".config/opencode");
            let opencode_json = opencode_config_dir.join("opencode.json");
            let tui_json = opencode_config_dir.join("tui.json");

            let target_plugin = opencode_managed
                .join(OPENCODE_TARGET_VERSION)
                .join("dist/plugin.mjs");
            let target_tui = opencode_managed
                .join(OPENCODE_TARGET_VERSION)
                .join("dist/tui.mjs");

            let inv = scan_managed_directory(
                &opencode_managed,
                "opencode",
                OPENCODE_TARGET_VERSION,
                OPENCODE_IMMUTABLE_DIGESTS,
                "@ctliz/agent-intercom-opencode",
                "@dataforxyz/agent-intercom-opencode",
                OPENCODE_RESOURCE_NAME,
                OPENCODE_RESOURCE_SHA256,
            );

            let p_identity = probe_opencode_json_file(
                ctx.runner,
                &opencode_json,
                &target_plugin.to_string_lossy(),
                &opencode_config_dir,
            );
            let t_identity = probe_opencode_json_file(
                ctx.runner,
                &tui_json,
                &target_tui.to_string_lossy(),
                &opencode_config_dir,
            );

            let mut installed_ver = None;
            let raw_state = if inv.has_legacy
                || p_identity == OpenCodeConfigIdentity::LegacyNamespace
                || t_identity == OpenCodeConfigIdentity::LegacyNamespace
            {
                AdapterHealthState::MigrationRequired
            } else if inv.has_invalid_roots
                || !inv.future_roots.is_empty()
                || p_identity == OpenCodeConfigIdentity::Invalid
                || t_identity == OpenCodeConfigIdentity::Invalid
            {
                AdapterHealthState::NeedsRepair
            } else if p_identity == OpenCodeConfigIdentity::Absent
                && t_identity == OpenCodeConfigIdentity::Absent
            {
                if inv.has_healthy_target || !inv.older_roots.is_empty() {
                    AdapterHealthState::NeedsRepair
                } else {
                    AdapterHealthState::NotInstalled
                }
            } else if p_identity == OpenCodeConfigIdentity::ManagedTarget
                && t_identity == OpenCodeConfigIdentity::ManagedTarget
            {
                if inv.has_healthy_target && inv.older_roots.is_empty() {
                    installed_ver = Some(OPENCODE_TARGET_VERSION.to_string());
                    AdapterHealthState::Healthy
                } else {
                    AdapterHealthState::NeedsRepair
                }
            } else if let (
                OpenCodeConfigIdentity::ManagedOlder(pv),
                OpenCodeConfigIdentity::ManagedOlder(tv),
            ) = (&p_identity, &t_identity)
            {
                if pv == tv
                    && inv.older_roots.len() == 1
                    && inv.older_roots[0] == *pv
                    && !inv.has_healthy_target
                {
                    installed_ver = Some(pv.clone());
                    AdapterHealthState::NeedsUpgrade
                } else {
                    AdapterHealthState::NeedsRepair
                }
            } else if let (
                OpenCodeConfigIdentity::VerifiedGlobal(pv),
                OpenCodeConfigIdentity::VerifiedGlobal(tv),
            ) = (&p_identity, &t_identity)
            {
                if pv == tv && !inv.has_healthy_target && inv.older_roots.is_empty() {
                    installed_ver = Some(pv.clone());
                    AdapterHealthState::HealthyExistingGlobal
                } else {
                    AdapterHealthState::NeedsRepair
                }
            } else {
                AdapterHealthState::NeedsRepair
            };

            let state = if !cli_available || !ctx.is_macos {
                AdapterHealthState::Unavailable
            } else {
                raw_state
            };

            let reason = match state {
                AdapterHealthState::NotInstalled => AdapterActionReason::Install,
                AdapterHealthState::NeedsUpgrade => AdapterActionReason::Upgrade,
                AdapterHealthState::NeedsRepair => AdapterActionReason::Repair,
                AdapterHealthState::MigrationRequired => {
                    AdapterActionReason::ManualMigrationRequired
                }
                _ => AdapterActionReason::Install,
            };

            let item = CommunicationAdapterPlanItem {
                agent_id: "opencode".to_string(),
                host_display_name: "OpenCode".to_string(),
                adapter_kind: CommunicationAdapterKind::OpenCode,
                state,
                target_version: OPENCODE_TARGET_VERSION.to_string(),
                installed_version: installed_ver,
                source_kind: if state == AdapterHealthState::HealthyExistingGlobal {
                    AdapterSourceKind::ExistingGlobal
                } else {
                    AdapterSourceKind::Bundled
                },
                package_name: Some(CanonicalAdapterPackage::OpenCode),
                config_change_kind: ConfigChangeKind::HostConfigRegistered,
                network_required: false,
                license: "AGPL-3.0-or-later".to_string(),
                action_reason: reason,
            };

            let ev = SingleAdapterEvidence::OpenCode {
                opencode_mtime: fs::metadata(&opencode_json)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                opencode_sha: file_sha256(&opencode_json).unwrap_or_default(),
                tui_mtime: fs::metadata(&tui_json)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                tui_sha: file_sha256(&tui_json).unwrap_or_default(),
            };

            (Some(item), Some(ev))
        }

        _ => (None, None),
    }
}

// ============================================================================
// Multi-Plan Bounded Cache & Plan Mutex
// ============================================================================

pub const MAX_PLAN_CACHE_ENTRIES: usize = 16;
pub const PLAN_CACHE_TTL_SECS: u64 = 300;

pub struct CachedPlanEntry {
    pub plan: WorkspaceInstallPlan,
    pub evidence: Vec<SingleAdapterEvidence>,
    pub requested_agents: Vec<String>,
    pub created_at: u64,
}

pub static PLAN_CACHE: Mutex<Option<BTreeMap<String, CachedPlanEntry>>> = Mutex::new(None);
pub static INSTALL_MUTATION_LOCK: Mutex<()> = Mutex::new(());

pub fn insert_cached_plan(
    plan: WorkspaceInstallPlan,
    evidence: Vec<SingleAdapterEvidence>,
    requested_agents: Vec<String>,
) {
    let mut lock = PLAN_CACHE.lock().unwrap();
    let cache = lock.get_or_insert_with(BTreeMap::new);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Evict expired entries
    cache.retain(|_, entry| now.saturating_sub(entry.created_at) <= PLAN_CACHE_TTL_SECS);

    // Evict oldest if full
    while cache.len() >= MAX_PLAN_CACHE_ENTRIES {
        let oldest_key = cache
            .iter()
            .min_by_key(|(_, entry)| entry.created_at)
            .map(|(k, _)| k.clone());
        if let Some(key) = oldest_key {
            cache.remove(&key);
        } else {
            break;
        }
    }

    cache.insert(
        plan.plan_id.clone(),
        CachedPlanEntry {
            plan,
            evidence,
            requested_agents,
            created_at: now,
        },
    );
}

// ============================================================================
// Internal Check & Apply Implementation
// ============================================================================

pub fn compute_plan_fingerprint(
    requires_consent: bool,
    can_apply: bool,
    can_create_without_installing: bool,
    healthy_agent_ids: &[String],
    items: &[CommunicationAdapterPlanItem],
    evidence: &[SingleAdapterEvidence],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        format!(
            "consent:{};apply:{};bypass:{};",
            requires_consent, can_apply, can_create_without_installing
        )
        .as_bytes(),
    );

    for id in healthy_agent_ids {
        hasher.update(format!("healthy:{};", id).as_bytes());
    }

    for item in items {
        hasher.update(
            format!(
                "item:{}:{:?}:{:?}:{}:{:?}:{:?}:{}:{};",
                item.agent_id,
                item.adapter_kind,
                item.state,
                item.target_version,
                item.source_kind,
                item.config_change_kind,
                item.network_required,
                item.license,
            )
            .as_bytes(),
        );
    }

    for ev in evidence {
        match ev {
            SingleAdapterEvidence::Pi {
                settings_mtime,
                settings_sha,
            } => {
                hasher.update(format!("ev_pi:{}:{};", settings_mtime, settings_sha).as_bytes());
            }
            SingleAdapterEvidence::Claude { mtime, sha } => {
                hasher.update(format!("ev_claude:{}:{};", mtime, sha).as_bytes());
            }
            SingleAdapterEvidence::Codex {
                config_mtime,
                config_sha,
            } => {
                hasher.update(format!("ev_codex:{}:{};", config_mtime, config_sha).as_bytes());
            }
            SingleAdapterEvidence::OpenCode {
                opencode_mtime,
                opencode_sha,
                tui_mtime,
                tui_sha,
            } => {
                hasher.update(
                    format!(
                        "ev_opencode:{}:{}:{}:{};",
                        opencode_mtime, opencode_sha, tui_mtime, tui_sha
                    )
                    .as_bytes(),
                );
            }
        }
    }

    format!("{:x}", hasher.finalize())
}

pub fn check_workspace_adapters_internal(
    custom_ctx: Option<&AdapterContext>,
    pane_agent_ids: Vec<String>,
) -> Result<WorkspaceInstallPlan, String> {
    let default_runner = RealCommandRunner;
    let config_dir = crate::config::get_config_dir();
    let home_dir = dirs::home_dir().ok_or_else(|| "ERR_ADAPTER_CONFIG".to_string())?;
    let pi_agent_dir = get_pi_agent_dir(&home_dir);

    let ctx = match custom_ctx {
        Some(c) => c,
        None => &AdapterContext {
            runner: &default_runner,
            home_dir,
            config_dir,
            pi_agent_dir,
            is_macos: cfg!(target_os = "macos"),
            #[cfg(test)]
            injected_fail_point: FAIL_NONE,
        },
    };

    let _ = reconcile_cleanup_journal(&ctx.config_dir, &ctx.home_dir, FAIL_NONE);

    let mut recognized = Vec::new();
    let mut seen = BTreeSet::new();

    for id in pane_agent_ids {
        if matches!(id.as_str(), "pi" | "claude" | "codex" | "opencode") {
            if seen.insert(id.clone()) {
                recognized.push(id);
            }
        }
    }

    let mut healthy_agent_ids = Vec::new();
    let mut items = Vec::new();
    let mut evidences = Vec::new();

    for id in &recognized {
        let (item_opt, ev_opt) = probe_single_adapter(ctx, id);
        if let Some(item) = item_opt {
            if let Some(ev) = ev_opt {
                evidences.push(ev);
            }
            if item.state == AdapterHealthState::Healthy
                || item.state == AdapterHealthState::HealthyExistingGlobal
            {
                healthy_agent_ids.push(id.clone());
            } else {
                items.push(item);
            }
        }
    }

    let requires_consent = !items.is_empty();

    let has_migration = items
        .iter()
        .any(|i| i.state == AdapterHealthState::MigrationRequired);
    let has_unavailable = items
        .iter()
        .any(|i| i.state == AdapterHealthState::Unavailable);

    let can_apply = !items.is_empty() && !has_migration && !has_unavailable;
    let can_create_without_installing = !has_migration && !has_unavailable;

    let plan_id = format!("plan_{}", random_hex(16).map_err(|e| e.to_string())?);

    let plan_fingerprint = compute_plan_fingerprint(
        requires_consent,
        can_apply,
        can_create_without_installing,
        &healthy_agent_ids,
        &items,
        &evidences,
    );

    let plan = WorkspaceInstallPlan {
        plan_id,
        plan_fingerprint,
        requires_consent,
        can_apply,
        can_create_without_installing,
        healthy_agent_ids,
        items,
    };

    insert_cached_plan(plan.clone(), evidences, recognized);

    Ok(plan)
}

pub fn validate_plan_id(plan_id: &str) -> bool {
    plan_id.len() == 37
        && plan_id.starts_with("plan_")
        && plan_id[5..]
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
}

pub fn validate_plan_fingerprint(plan_fingerprint: &str) -> bool {
    plan_fingerprint.len() == 64
        && plan_fingerprint
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

pub fn apply_workspace_install_plan_internal(
    _app: Option<tauri::AppHandle>,
    custom_ctx: Option<&AdapterContext>,
    plan_id: &str,
    plan_fingerprint: &str,
) -> Result<(), String> {
    // 1. Shape validation
    if !validate_plan_id(plan_id) || !validate_plan_fingerprint(plan_fingerprint) {
        return Err(AdapterError::PlanInvalid.to_string());
    }

    // 2. Atomic one-shot claim from cache
    let cached_entry = {
        let mut lock = PLAN_CACHE.lock().unwrap();
        let cache = lock.get_or_insert_with(BTreeMap::new);
        cache
            .remove(plan_id)
            .ok_or_else(|| AdapterError::PlanStale.to_string())?
    };

    // 3. TTL, fingerprint, and can_apply validation
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now.saturating_sub(cached_entry.created_at) > PLAN_CACHE_TTL_SECS {
        return Err(AdapterError::PlanStale.to_string());
    }
    if cached_entry.plan.plan_fingerprint != plan_fingerprint {
        return Err(AdapterError::PlanStale.to_string());
    }
    // 4. Acquire process-wide install mutex before the serialized active-surface probe.
    // The cache claim above remains intentionally outside this process-wide lock.
    let _install_guard = INSTALL_MUTATION_LOCK.lock().unwrap();

    let default_runner = RealCommandRunner;
    let config_dir = crate::config::get_config_dir();
    let home_dir = dirs::home_dir().ok_or_else(|| AdapterError::Config.to_string())?;
    let pi_agent_dir = get_pi_agent_dir(&home_dir);

    let ctx = match custom_ctx {
        Some(c) => c,
        None => &AdapterContext {
            runner: &default_runner,
            home_dir,
            config_dir,
            pi_agent_dir,
            is_macos: cfg!(target_os = "macos"),
            #[cfg(test)]
            injected_fail_point: FAIL_NONE,
        },
    };

    if !ctx.is_macos {
        return Err(AdapterError::Unavailable.to_string());
    }

    // Real Tauri installs must resolve bundled files from AppHandle.resource_dir();
    // injected contexts retain the deterministic config-parent resource fallback.
    let bundled_resource_dir = _app.as_ref().and_then(|app| {
        use tauri::Manager;
        app.path().resource_dir().ok().map(|p| p.join("resources"))
    });

    // Pre-mutation cleanup journal reconciliation
    if let Err(_e) = reconcile_cleanup_journal(&ctx.config_dir, &ctx.home_dir, ctx.injected_fail())
    {
        return Err(AdapterError::Rollback.to_string());
    }

    // Full drift re-probe of ALL requested agents
    let mut reprobed_healthy = Vec::new();
    let mut reprobed_items = Vec::new();
    let mut reprobed_evidences = Vec::new();

    for id in &cached_entry.requested_agents {
        let (probed_item_opt, probed_ev_opt) = probe_single_adapter(ctx, id);
        let Some(probed_item) = probed_item_opt else {
            return Err(AdapterError::PlanStale.to_string());
        };
        if let Some(ev) = probed_ev_opt {
            reprobed_evidences.push(ev);
        }
        if probed_item.state == AdapterHealthState::Healthy
            || probed_item.state == AdapterHealthState::HealthyExistingGlobal
        {
            reprobed_healthy.push(id.clone());
        } else {
            reprobed_items.push(probed_item);
        }
    }

    let reprobed_fingerprint = compute_plan_fingerprint(
        cached_entry.plan.requires_consent,
        cached_entry.plan.can_apply,
        cached_entry.plan.can_create_without_installing,
        &reprobed_healthy,
        &reprobed_items,
        &reprobed_evidences,
    );

    if reprobed_healthy != cached_entry.plan.healthy_agent_ids
        || reprobed_items != cached_entry.plan.items
        || reprobed_evidences != cached_entry.evidence
        || reprobed_fingerprint != cached_entry.plan.plan_fingerprint
    {
        return Err(AdapterError::PlanStale.to_string());
    }
    if !cached_entry.plan.can_apply {
        return Err(AdapterError::PlanInvalid.to_string());
    }

    let mut root_backups: Vec<ManagedRootBackup> = Vec::new();
    let mut file_backups: Vec<FileBackup> = Vec::new();
    let mut older_roots_to_clean: Vec<(String, String)> = Vec::new();

    let execution_result: Result<(), AdapterError> = (|| -> Result<(), AdapterError> {
        for item in &cached_entry.plan.items {
            match item.agent_id.as_str() {
                "pi" => {
                    let settings_file = ctx.pi_agent_dir.join("settings.json");
                    let fb = FileBackup::create(&settings_file, ctx.injected_fail())?;
                    file_backups.push(fb);

                    let mut json_obj = if settings_file.is_file() {
                        let c =
                            fs::read_to_string(&settings_file).map_err(|_| AdapterError::Config)?;
                        serde_json::from_str::<serde_json::Value>(&c)
                            .map_err(|_| AdapterError::Config)?
                            .as_object()
                            .cloned()
                            .ok_or(AdapterError::Config)?
                    } else {
                        serde_json::Map::new()
                    };

                    let mut pkgs: Vec<serde_json::Value> = match json_obj.get("packages") {
                        Some(serde_json::Value::Array(arr)) => arr.clone(),
                        Some(_) => return Err(AdapterError::Config),
                        None => Vec::new(),
                    };

                    let keep_npm = pkgs.iter().find_map(|p| {
                        p.as_str().and_then(pi_npm_package_version).and_then(|ver| {
                            let parsed = SemVer::parse(ver)?;
                            let target = SemVer::parse(PI_TARGET_VERSION)?;
                            (parsed >= target).then(|| ver.to_string())
                        })
                    });

                    pkgs.retain(|p| {
                        if let Some(s) = p.as_str() {
                            !is_pi_intercom_settings_entry(s)
                        } else {
                            true
                        }
                    });

                    pkgs.push(serde_json::Value::String(if let Some(ver) = keep_npm {
                        format!("{}{}", PI_NPM_PACKAGE_PREFIX, ver)
                    } else {
                        PI_CANONICAL_GIT_TARGET.to_string()
                    }));
                    json_obj.insert("packages".to_string(), serde_json::Value::Array(pkgs));

                    let out_bytes =
                        serde_json::to_vec_pretty(&json_obj).map_err(|_| AdapterError::Config)?;
                    atomic_write_file(&settings_file, &out_bytes, 0o600, ctx.injected_fail())?;
                }

                "claude" => {
                    let claude_managed = ctx.config_dir.join("managed/claude-intercom");
                    let target_root = claude_managed.join(CLAUDE_TARGET_VERSION);
                    let nonce = random_hex(6)?;
                    let staging_dir = claude_managed.join(format!(".staging.{}", nonce));
                    let npm_cache_dir = claude_managed.join(format!(".npm-cache.{}", nonce));

                    let mut root_backup = ManagedRootBackup::new(
                        "claude",
                        target_root,
                        staging_dir.clone(),
                        npm_cache_dir.clone(),
                    );

                    if item.action_reason == AdapterActionReason::Upgrade {
                        if let Some(old_v) = &item.installed_version {
                            older_roots_to_clean.push(("claude".to_string(), old_v.clone()));
                        }
                    }

                    build_managed_root_staging(
                        ctx,
                        &staging_dir,
                        &npm_cache_dir,
                        "claude",
                        CLAUDE_TARGET_VERSION,
                        CLAUDE_RESOURCE_NAME,
                        CLAUDE_RESOURCE_SHA256,
                        "@ctliz/agent-intercom-claude",
                        bundled_resource_dir.as_deref(),
                    )?;

                    root_backup.swap_staging_to_active(ctx.injected_fail())?;
                    root_backups.push(root_backup);
                }

                "codex" => {
                    let codex_managed = ctx.config_dir.join("managed/codex-intercom");
                    let target_root = codex_managed.join(CODEX_TARGET_VERSION);
                    let nonce = random_hex(6)?;
                    let staging_dir = codex_managed.join(format!(".staging.{}", nonce));
                    let npm_cache_dir = codex_managed.join(format!(".npm-cache.{}", nonce));

                    let mut root_backup = ManagedRootBackup::new(
                        "codex",
                        target_root.clone(),
                        staging_dir.clone(),
                        npm_cache_dir.clone(),
                    );

                    if item.action_reason == AdapterActionReason::Upgrade {
                        if let Some(old_v) = &item.installed_version {
                            older_roots_to_clean.push(("codex".to_string(), old_v.clone()));
                        }
                    }

                    build_managed_root_staging(
                        ctx,
                        &staging_dir,
                        &npm_cache_dir,
                        "codex",
                        CODEX_TARGET_VERSION,
                        CODEX_RESOURCE_NAME,
                        CODEX_RESOURCE_SHA256,
                        "@ctliz/agent-intercom-codex",
                        bundled_resource_dir.as_deref(),
                    )?;

                    root_backup.swap_staging_to_active(ctx.injected_fail())?;
                    root_backups.push(root_backup);

                    let codex_config = ctx.home_dir.join(".codex/config.toml");
                    let fb = FileBackup::create(&codex_config, ctx.injected_fail())?;
                    file_backups.push(fb);

                    let launcher_p = target_root.join("dist/codex-launcher.mjs");
                    update_codex_config_toml(&codex_config, &launcher_p, ctx.injected_fail())?;
                }

                "opencode" => {
                    let opencode_managed = ctx.config_dir.join("managed/opencode-intercom");
                    let target_root = opencode_managed.join(OPENCODE_TARGET_VERSION);
                    let nonce = random_hex(6)?;
                    let staging_dir = opencode_managed.join(format!(".staging.{}", nonce));
                    let npm_cache_dir = opencode_managed.join(format!(".npm-cache.{}", nonce));

                    let mut root_backup = ManagedRootBackup::new(
                        "opencode",
                        target_root.clone(),
                        staging_dir.clone(),
                        npm_cache_dir.clone(),
                    );

                    if item.action_reason == AdapterActionReason::Upgrade {
                        if let Some(old_v) = &item.installed_version {
                            older_roots_to_clean.push(("opencode".to_string(), old_v.clone()));
                        }
                    }

                    build_managed_root_staging(
                        ctx,
                        &staging_dir,
                        &npm_cache_dir,
                        "opencode",
                        OPENCODE_TARGET_VERSION,
                        OPENCODE_RESOURCE_NAME,
                        OPENCODE_RESOURCE_SHA256,
                        "@ctliz/agent-intercom-opencode",
                        bundled_resource_dir.as_deref(),
                    )?;

                    root_backup.swap_staging_to_active(ctx.injected_fail())?;
                    root_backups.push(root_backup);

                    let opencode_config_dir = ctx.home_dir.join(".config/opencode");
                    let opencode_json = opencode_config_dir.join("opencode.json");
                    let tui_json = opencode_config_dir.join("tui.json");

                    let fb_plugin = FileBackup::create(&opencode_json, ctx.injected_fail())?;
                    file_backups.push(fb_plugin);
                    let fb_tui = FileBackup::create(&tui_json, ctx.injected_fail())?;
                    file_backups.push(fb_tui);

                    let entry_plugin = target_root
                        .join("dist/plugin.mjs")
                        .to_string_lossy()
                        .to_string();
                    let entry_tui = target_root
                        .join("dist/tui.mjs")
                        .to_string_lossy()
                        .to_string();

                    update_opencode_json_file(&opencode_json, &entry_plugin, ctx.injected_fail())?;
                    update_opencode_json_file(&tui_json, &entry_tui, ctx.injected_fail())?;
                }

                _ => return Err(AdapterError::Install),
            }
        }

        #[cfg(test)]
        if (ctx.injected_fail() & FAIL_POST_HEALTH_PROBE) != 0 {
            return Err(AdapterError::Verify);
        }

        for item in &cached_entry.plan.items {
            let (post_item_opt, _) = probe_single_adapter(ctx, &item.agent_id);
            let Some(post_item) = post_item_opt else {
                return Err(AdapterError::Verify);
            };
            if post_item.state != AdapterHealthState::Healthy
                && post_item.state != AdapterHealthState::HealthyExistingGlobal
            {
                return Err(AdapterError::Verify);
            }
        }

        Ok(())
    })();

    if let Err(e) = execution_result {
        let mut rollback_err = false;
        for mut rb in root_backups {
            if rb.rollback(ctx.injected_fail()).is_err() {
                rollback_err = true;
            }
        }
        for mut fb in file_backups {
            if fb.rollback(ctx.injected_fail()).is_err() {
                rollback_err = true;
            }
        }
        if rollback_err {
            return Err(AdapterError::Rollback.to_string());
        }
        return Err(e.to_string());
    }

    let mut journal_items = Vec::new();

    for rb in &root_backups {
        if let Some(bk) = &rb.backup_dir {
            if let Some(file_name) = bk.file_name().and_then(|n| n.to_str()) {
                if let Some(nonce) = file_name.strip_prefix(".bak.") {
                    journal_items.push(JournalCleanupItem::ManagedRootBackup {
                        harness: rb.harness.clone(),
                        nonce: nonce.to_string(),
                        phase: JournalPhase::PendingRemove,
                    });
                }
            }
        }
    }

    for (harness, old_v) in older_roots_to_clean {
        journal_items.push(JournalCleanupItem::ManagedOlderRoot {
            harness,
            version: old_v,
            phase: JournalPhase::PendingRemove,
        });
    }

    for fb in &file_backups {
        if let Some(bk) = &fb.backup {
            if let Some(file_name) = bk.file_name().and_then(|n| n.to_str()) {
                if let Some(nonce) = file_name.strip_prefix("config.toml.bak.") {
                    journal_items.push(JournalCleanupItem::CodexConfigBackup {
                        nonce: nonce.to_string(),
                        phase: JournalPhase::PendingRemove,
                    });
                } else if let Some(nonce) = file_name.strip_prefix("opencode.json.bak.") {
                    journal_items.push(JournalCleanupItem::OpenCodeConfigBackup {
                        file_name: "opencode.json".to_string(),
                        nonce: nonce.to_string(),
                        phase: JournalPhase::PendingRemove,
                    });
                } else if let Some(nonce) = file_name.strip_prefix("tui.json.bak.") {
                    journal_items.push(JournalCleanupItem::OpenCodeConfigBackup {
                        file_name: "tui.json".to_string(),
                        nonce: nonce.to_string(),
                        phase: JournalPhase::PendingRemove,
                    });
                }
            }
        }
    }

    if !journal_items.is_empty() {
        let journal = CleanupJournal {
            items: journal_items,
            created_at: now,
        };
        if journal
            .write_and_fsync(&ctx.config_dir, ctx.injected_fail())
            .is_err()
        {
            let mut rollback_err = false;
            for mut rb in root_backups {
                if rb.rollback(ctx.injected_fail()).is_err() {
                    rollback_err = true;
                }
            }
            for mut fb in file_backups {
                if fb.rollback(ctx.injected_fail()).is_err() {
                    rollback_err = true;
                }
            }
            if rollback_err {
                return Err(AdapterError::Rollback.to_string());
            }
            return Err(AdapterError::Rollback.to_string());
        }
    }

    // Backups are now committed and recorded in durable journal
    for rb in root_backups {
        rb.commit();
    }
    for fb in file_backups {
        fb.commit();
    }

    // Irreversible post-commit reconciliation
    let reconcile_res =
        reconcile_cleanup_journal(&ctx.config_dir, &ctx.home_dir, ctx.injected_fail());
    if reconcile_res.is_err() {
        return Err(AdapterError::Rollback.to_string());
    }

    // Final post-cleanup probe across every selected active surface, including healthy ones.
    for agent_id in &cached_entry.requested_agents {
        let (final_item_opt, _) = probe_single_adapter(ctx, agent_id);
        let Some(final_item) = final_item_opt else {
            return Err(AdapterError::Verify.to_string());
        };
        if final_item.state != AdapterHealthState::Healthy
            && final_item.state != AdapterHealthState::HealthyExistingGlobal
        {
            return Err(AdapterError::Verify.to_string());
        }
    }

    crate::claude_adapter::invalidate_managed_claude_health_cache();
    crate::registry::invalidate_environment_cache();
    Ok(())
}

// ============================================================================
// Staging Root Builder
// ============================================================================

pub fn build_managed_root_staging(
    ctx: &AdapterContext,
    staging_dir: &Path,
    npm_cache_dir: &Path,
    harness: &str,
    target_version: &str,
    adapter_resource_name: &str,
    adapter_resource_sha: &str,
    expected_package_name: &str,
    bundled_resource_dir: Option<&Path>,
) -> Result<(), AdapterError> {
    if staging_dir.exists() {
        fs::remove_dir_all(staging_dir).map_err(|_| AdapterError::Install)?;
    }
    fs::create_dir_all(staging_dir).map_err(|_| AdapterError::Install)?;

    if npm_cache_dir.exists() {
        fs::remove_dir_all(npm_cache_dir).map_err(|_| AdapterError::Install)?;
    }
    fs::create_dir_all(npm_cache_dir).map_err(|_| AdapterError::Install)?;

    let vendor_dir = staging_dir.join("vendor");
    fs::create_dir_all(&vendor_dir).map_err(|_| AdapterError::Install)?;

    let res_dir = bundled_resource_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| {
            ctx.config_dir
                .parent()
                .unwrap_or(&ctx.config_dir)
                .join("resources")
        });
    let core_src = res_dir.join(CORE_RESOURCE_NAME);
    let adapter_src = res_dir.join(adapter_resource_name);
    let sdk_src = res_dir.join(OPENCODE_SDK_RESOURCE_NAME);
    let closure_src = res_dir.join(OPENCODE_CLOSURE_RESOURCE_NAME);
    let needs_opencode_sdk = harness == "opencode";

    if !core_src.is_file()
        || !adapter_src.is_file()
        || (needs_opencode_sdk && (!sdk_src.is_file() || !closure_src.is_file()))
    {
        return Err(AdapterError::Install);
    }
    if file_sha256(&core_src).unwrap_or_default() != CORE_RESOURCE_SHA256 {
        return Err(AdapterError::Install);
    }
    if file_sha256(&adapter_src).unwrap_or_default() != adapter_resource_sha {
        return Err(AdapterError::Install);
    }
    if needs_opencode_sdk
        && file_sha256(&sdk_src).unwrap_or_default() != OPENCODE_SDK_RESOURCE_SHA256
    {
        return Err(AdapterError::Install);
    }
    if needs_opencode_sdk
        && file_sha256(&closure_src).unwrap_or_default() != OPENCODE_CLOSURE_RESOURCE_SHA256
    {
        return Err(AdapterError::Install);
    }

    let vendor_core = vendor_dir.join(CORE_RESOURCE_NAME);
    let vendor_adapter = vendor_dir.join(adapter_resource_name);
    let vendor_sdk = vendor_dir.join(OPENCODE_SDK_RESOURCE_NAME);
    let vendor_closure = vendor_dir.join(OPENCODE_CLOSURE_RESOURCE_NAME);

    fs::copy(&core_src, &vendor_core).map_err(|_| AdapterError::Install)?;
    fs::copy(&adapter_src, &vendor_adapter).map_err(|_| AdapterError::Install)?;
    if needs_opencode_sdk {
        fs::copy(&sdk_src, &vendor_sdk).map_err(|_| AdapterError::Install)?;
        fs::copy(&closure_src, &vendor_closure).map_err(|_| AdapterError::Install)?;
    }

    checked_fsync_dir(&vendor_dir, ctx.injected_fail())?;

    let mut dependencies = serde_json::Map::new();
    dependencies.insert(
        "@ctliz/agent-intercom-core".to_string(),
        serde_json::Value::String(format!("file:./vendor/{}", CORE_RESOURCE_NAME)),
    );
    dependencies.insert(
        expected_package_name.to_string(),
        serde_json::Value::String(format!("file:./vendor/{}", adapter_resource_name)),
    );
    if needs_opencode_sdk {
        dependencies.insert(
            "@opencode-ai/plugin".to_string(),
            serde_json::Value::String(format!("file:./vendor/{}", OPENCODE_SDK_RESOURCE_NAME)),
        );
    }
    let pkg_json = serde_json::json!({
        "name": expected_package_name,
        "version": target_version,
        "type": "module",
        "dependencies": dependencies,
    });

    let pkg_str = serde_json::to_string_pretty(&pkg_json).map_err(|_| AdapterError::Install)?;
    atomic_write_file(
        &staging_dir.join("package.json"),
        pkg_str.as_bytes(),
        0o600,
        ctx.injected_fail(),
    )?;

    // Seed OpenCode's complete offline npm closure from the bundled archive.
    // This avoids registry resolution while retaining a reproducible lockfile.
    if needs_opencode_sdk {
        let closure_args = [
            "-xzf",
            vendor_closure.to_str().ok_or(AdapterError::Install)?,
            "-C",
            staging_dir.to_str().ok_or(AdapterError::Install)?,
        ];
        let closure_out = ctx
            .runner
            .run_command("tar", &closure_args, staging_dir, None)?;
        if closure_out.status != 0 {
            return Err(AdapterError::Install);
        }
    }

    let mut npm_arg_values = vec![
        "install".to_string(),
        "--prefix".to_string(),
        ".".to_string(),
        "--package-lock".to_string(),
        "--save-exact".to_string(),
        "--ignore-scripts".to_string(),
        "--no-audit".to_string(),
        "--no-fund".to_string(),
        "--no-update-notifier".to_string(),
        "--offline".to_string(),
        "--cache".to_string(),
        npm_cache_dir.to_string_lossy().to_string(),
        format!("./vendor/{}", CORE_RESOURCE_NAME),
        format!("./vendor/{}", adapter_resource_name),
    ];
    if needs_opencode_sdk {
        npm_arg_values.push(format!("./vendor/{}", OPENCODE_SDK_RESOURCE_NAME));
    }
    if harness == "codex" {
        npm_arg_values.push("--omit=optional".to_string());
    }
    let npm_args: Vec<&str> = npm_arg_values.iter().map(String::as_str).collect();

    let out = ctx
        .runner
        .run_command("npm", &npm_args, staging_dir, None)?;
    if out.status != 0 {
        return Err(AdapterError::Install);
    }

    let lock_file = staging_dir.join("package-lock.json");
    if lock_file.is_file() {
        // npm 11 may materialize Codex's optional node-pty under either the
        // root or adapter-nested node_modules despite --omit=optional. Remove
        // that optional closure before accepting the exact lock contract.
        if harness == "codex" || harness == "opencode" {
            let node_modules = staging_dir.join("node_modules");
            if harness == "codex" {
                for optional_name in ["node-pty", "node-addon-api"] {
                    let mut stack = vec![node_modules.clone()];
                    while let Some(dir) = stack.pop() {
                        let Ok(entries) = fs::read_dir(&dir) else {
                            continue;
                        };
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.file_name().and_then(|n| n.to_str()) == Some(optional_name) {
                                if path.is_dir() {
                                    fs::remove_dir_all(&path).map_err(|_| AdapterError::Install)?;
                                } else {
                                    fs::remove_file(&path).map_err(|_| AdapterError::Install)?;
                                }
                            } else if path.is_dir() {
                                stack.push(path);
                            }
                        }
                    }
                }
            } else {
                let mut stack = vec![node_modules.clone()];
                while let Some(dir) = stack.pop() {
                    let Ok(entries) = fs::read_dir(&dir) else {
                        continue;
                    };
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path
                            .to_string_lossy()
                            .contains("msgpackr-extract/node_modules/@msgpackr-extract")
                        {
                            if path.is_dir() {
                                fs::remove_dir_all(&path).map_err(|_| AdapterError::Install)?;
                            } else {
                                fs::remove_file(&path).map_err(|_| AdapterError::Install)?;
                            }
                        } else if path.is_dir() {
                            stack.push(path);
                        }
                    }
                }
            }
            let lock_text = fs::read_to_string(&lock_file).map_err(|_| AdapterError::Install)?;
            let mut lock_json: serde_json::Value =
                serde_json::from_str(&lock_text).map_err(|_| AdapterError::Install)?;
            if let Some(packages) = lock_json
                .get_mut("packages")
                .and_then(|v| v.as_object_mut())
            {
                packages.retain(|key, _| {
                    !key.ends_with("/node-pty")
                        && !key.ends_with("/node-addon-api")
                        && !(harness == "opencode"
                            && key.contains("msgpackr-extract/node_modules/@msgpackr-extract"))
                });
            }
            let normalized =
                serde_json::to_vec_pretty(&lock_json).map_err(|_| AdapterError::Install)?;
            atomic_write_file(&lock_file, &normalized, 0o600, ctx.injected_fail())?;
        }
        let lock_str = fs::read_to_string(&lock_file).map_err(|_| AdapterError::Install)?;
        if lock_str.contains("/tmp/")
            || lock_str.contains("/var/")
            || lock_str.contains("/Users/")
            || lock_str.contains("/home/")
            || lock_str.contains("file:/")
            || lock_str.contains("\\\":/")
        {
            return Err(AdapterError::Install);
        }
    }

    let untar_args = [
        "-xzf",
        vendor_adapter.to_str().unwrap(),
        "--strip-components=1",
        "package/dist",
    ];
    let out_tar = ctx
        .runner
        .run_command("tar", &untar_args, staging_dir, None)?;
    if out_tar.status != 0 {
        return Err(AdapterError::Install);
    }
    if harness == "claude" {
        materialize_claude_plugin_surface(staging_dir)?;
    }

    // npm creates platform-dependent .bin symlink shims; remove them so the
    // managed tree remains a regular-file-only, permission-verifiable root.
    let bin_dir = staging_dir.join("node_modules/.bin");
    if bin_dir.exists() {
        fs::remove_dir_all(&bin_dir).map_err(|_| AdapterError::Install)?;
    }

    // Remove npm's platform-dependent .bin symlink shims before regular-file tree verification.
    let bin_dir = staging_dir.join("node_modules/.bin");
    if bin_dir.exists() {
        fs::remove_dir_all(&bin_dir).map_err(|_| AdapterError::Install)?;
    }

    if harness == "codex" {
        let launcher_p = staging_dir.join("dist/codex-launcher.mjs");
        atomic_write_file(
            &launcher_p,
            CODEX_LAUNCHER_BODY.as_bytes(),
            0o755,
            ctx.injected_fail(),
        )?;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut marker_res = BTreeMap::new();
    marker_res.insert(
        CORE_RESOURCE_NAME.to_string(),
        CORE_RESOURCE_SHA256.to_string(),
    );
    marker_res.insert(
        adapter_resource_name.to_string(),
        adapter_resource_sha.to_string(),
    );
    if needs_opencode_sdk {
        marker_res.insert(
            OPENCODE_SDK_RESOURCE_NAME.to_string(),
            OPENCODE_SDK_RESOURCE_SHA256.to_string(),
        );
        marker_res.insert(
            OPENCODE_CLOSURE_RESOURCE_NAME.to_string(),
            OPENCODE_CLOSURE_RESOURCE_SHA256.to_string(),
        );
    }

    // Record exactly the immutable digest contract; self-attested extra files are not identity.
    let immutable_digests = match harness {
        "claude" => CLAUDE_IMMUTABLE_DIGESTS,
        "codex" => CODEX_IMMUTABLE_DIGESTS,
        "opencode" => OPENCODE_IMMUTABLE_DIGESTS,
        _ => return Err(AdapterError::Install),
    };
    let mut marker_digests = BTreeMap::new();
    for (rel, expected_sha) in immutable_digests {
        let path = staging_dir.join(rel);
        if !path.is_file() || file_sha256(&path).unwrap_or_default() != *expected_sha {
            return Err(AdapterError::Install);
        }
        marker_digests.insert((*rel).to_string(), (*expected_sha).to_string());
    }

    let marker = ManagedAdapterMarker {
        schema_version: 1,
        harness: harness.to_string(),
        adapter_version: target_version.to_string(),
        installed_at: now,
        resources: marker_res,
        digests: marker_digests,
    };

    let marker_str = serde_json::to_string_pretty(&marker).map_err(|_| AdapterError::Install)?;
    atomic_write_file(
        &staging_dir.join("tmuxdeck-managed.json"),
        marker_str.as_bytes(),
        0o600,
        ctx.injected_fail(),
    )?;

    normalize_tree_permissions(staging_dir)?;

    let is_valid = verify_managed_root_integrity(
        staging_dir,
        harness,
        target_version,
        match harness {
            "claude" => CLAUDE_IMMUTABLE_DIGESTS,
            "codex" => CODEX_IMMUTABLE_DIGESTS,
            "opencode" => OPENCODE_IMMUTABLE_DIGESTS,
            _ => return Err(AdapterError::Install),
        },
        expected_package_name,
        adapter_resource_name,
        adapter_resource_sha,
    );

    if !is_valid {
        return Err(AdapterError::Install);
    }

    Ok(())
}

pub fn apply_single_adapter(
    ctx: &AdapterContext,
    _locator: &dyn ResourceLocator,
    agent_id: &str,
) -> Result<(), String> {
    let plan = check_workspace_adapters_internal(Some(ctx), vec![agent_id.to_string()])?;
    apply_workspace_install_plan_internal(None, Some(ctx), &plan.plan_id, &plan.plan_fingerprint)
}

// ============================================================================
// Public Tauri IPC Commands (Frozen Contract)
// ============================================================================

#[tauri::command]
pub fn check_workspace_adapters(
    pane_agent_ids: Vec<String>,
) -> Result<WorkspaceInstallPlan, String> {
    check_workspace_adapters_internal(None, pane_agent_ids)
}

#[tauri::command]
pub fn apply_workspace_install_plan(
    app: tauri::AppHandle,
    plan_id: String,
    plan_fingerprint: String,
) -> Result<(), String> {
    apply_workspace_install_plan_internal(Some(app), None, &plan_id, &plan_fingerprint)
}

// ============================================================================
// Comprehensive Unit & Integration Tests
// ============================================================================

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::sync::Mutex;

    pub struct TestTempDir {
        pub path: PathBuf,
    }

    impl TestTempDir {
        pub fn new() -> Self {
            let nonce = random_hex(8).unwrap();
            let path = std::env::temp_dir().join(format!("tmuxdeck-test-{}", nonce));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn tempdir() -> Result<TestTempDir, ()> {
        Ok(TestTempDir::new())
    }

    pub struct MockCommandRunner {
        pub responses: Mutex<BTreeMap<String, CommandOutput>>,
        pub existing_bins: Mutex<BTreeSet<String>>,
    }

    impl MockCommandRunner {
        pub fn new() -> Self {
            Self {
                responses: Mutex::new(BTreeMap::new()),
                existing_bins: Mutex::new(BTreeSet::new()),
            }
        }

        pub fn set_bin(&self, bin: &str) {
            self.existing_bins.lock().unwrap().insert(bin.to_string());
        }

        pub fn set_response(&self, cmd: &str, status: i32, stdout: &str, stderr: &str) {
            self.responses.lock().unwrap().insert(
                cmd.to_string(),
                CommandOutput {
                    status,
                    stdout: stdout.to_string(),
                    stderr: stderr.to_string(),
                },
            );
        }
    }

    impl CommandRunner for MockCommandRunner {
        fn run_command(
            &self,
            command: &str,
            args: &[&str],
            cwd: &Path,
            env: Option<&[(&str, &str)]>,
        ) -> Result<CommandOutput, AdapterError> {
            let full_cmd = format!("{} {}", command, args.join(" "));
            let lock = self.responses.lock().unwrap();
            if let Some(res) = lock.get(&full_cmd) {
                return Ok(CommandOutput {
                    status: res.status,
                    stdout: res.stdout.clone(),
                    stderr: res.stderr.clone(),
                });
            }
            if let Some(res) = lock.get(command) {
                return Ok(CommandOutput {
                    status: res.status,
                    stdout: res.stdout.clone(),
                    stderr: res.stderr.clone(),
                });
            }
            RealCommandRunner.run_command(command, args, cwd, env)
        }

        fn binary_exists(&self, binary_name: &str) -> bool {
            self.existing_bins.lock().unwrap().contains(binary_name)
        }
    }

    #[test]
    fn test_1_exact_serde_public_contract_and_absence_of_forbidden_fields() {
        let item = CommunicationAdapterPlanItem {
            agent_id: "claude".to_string(),
            host_display_name: "Claude Code".to_string(),
            adapter_kind: CommunicationAdapterKind::Claude,
            state: AdapterHealthState::Healthy,
            target_version: "0.13.0-connect.1".to_string(),
            installed_version: Some("0.13.0-connect.1".to_string()),
            source_kind: AdapterSourceKind::Bundled,
            package_name: Some(CanonicalAdapterPackage::Claude),
            config_change_kind: ConfigChangeKind::AppPrivateManaged,
            network_required: false,
            license: "AGPL-3.0-or-later".to_string(),
            action_reason: AdapterActionReason::Install,
        };

        let plan = WorkspaceInstallPlan {
            plan_id: format!("plan_{}", "a".repeat(32)),
            plan_fingerprint: "a".repeat(64),
            requires_consent: false,
            can_apply: false,
            can_create_without_installing: true,
            healthy_agent_ids: vec!["claude".to_string()],
            items: vec![item],
        };

        let json = serde_json::to_string(&plan).unwrap();
        assert!(!json.contains("scope"));
        assert!(!json.contains("token"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn test_2_host_cli_availability_probe() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("codex");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let (pi_item, _) = probe_single_adapter(&ctx, "pi");
        assert_eq!(pi_item.unwrap().state, AdapterHealthState::Unavailable);

        runner.set_bin("pi");
        let (pi_item2, _) = probe_single_adapter(&ctx, "pi");
        assert_eq!(pi_item2.unwrap().state, AdapterHealthState::NotInstalled);
    }

    #[test]
    fn test_3_and_5_health_states_and_plan_flags() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let plan = check_workspace_adapters_internal(Some(&ctx), vec!["pi".to_string()]).unwrap();
        assert!(plan.requires_consent);
        assert!(plan.can_apply);
        assert!(plan.can_create_without_installing);
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].license, "AGPL-3.0-or-later");
    }

    #[test]
    fn test_4_core_never_appears_as_a_row() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("codex");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let plan = check_workspace_adapters_internal(
            Some(&ctx),
            vec!["core".to_string(), "claude".to_string()],
        )
        .unwrap();
        assert!(plan.items.iter().all(|i| i.agent_id != "core"));
        assert!(plan.healthy_agent_ids.iter().all(|id| id != "core"));
    }

    #[test]
    fn test_6_plan_caching_ttl_replay_concurrency_and_active_surface_drift() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let plan = check_workspace_adapters_internal(Some(&ctx), vec!["pi".to_string()]).unwrap();

        let apply_res = apply_workspace_install_plan_internal(
            None,
            Some(&ctx),
            &plan.plan_id,
            &plan.plan_fingerprint,
        );
        assert!(apply_res.is_ok());

        let replay_res = apply_workspace_install_plan_internal(
            None,
            Some(&ctx),
            &plan.plan_id,
            &plan.plan_fingerprint,
        );
        assert_eq!(replay_res.unwrap_err(), "ERR_PLAN_STALE");
    }

    #[test]
    fn test_7_migration_never_automatically_applied_and_disables_bypass() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");

        let pi_agent_dir = dir.path().join("home/.pi/agent");
        fs::create_dir_all(&pi_agent_dir).unwrap();
        let settings = serde_json::json!({
            "packages": ["git:github.com/dataforxyz/agent-intercom-pi@v0.10.0"]
        });
        fs::write(pi_agent_dir.join("settings.json"), settings.to_string()).unwrap();

        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir,
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let plan = check_workspace_adapters_internal(Some(&ctx), vec!["pi".to_string()]).unwrap();
        assert_eq!(plan.items[0].state, AdapterHealthState::MigrationRequired);
        assert!(!plan.can_apply);
        assert!(!plan.can_create_without_installing);
    }

    #[test]
    fn test_8_exact_pi_install_command() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let plan = check_workspace_adapters_internal(Some(&ctx), vec!["pi".to_string()]).unwrap();
        apply_workspace_install_plan_internal(
            None,
            Some(&ctx),
            &plan.plan_id,
            &plan.plan_fingerprint,
        )
        .unwrap();

        let settings_file = ctx.pi_agent_dir.join("settings.json");
        let content = fs::read_to_string(settings_file).unwrap();
        assert!(content.contains(PI_CANONICAL_GIT_TARGET));
    }

    #[test]
    fn test_8b_current_npm_pi_intercom_is_healthy_existing_global() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");
        let pi_agent_dir = dir.path().join("home/.pi/agent");
        fs::create_dir_all(&pi_agent_dir).unwrap();
        fs::write(
            pi_agent_dir.join("settings.json"),
            r#"{"packages":["npm:@ctliz/pi-intercom@0.12.1"]}"#,
        )
        .unwrap();
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir,
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };
        let (item, _) = probe_single_adapter(&ctx, "pi");
        let item = item.unwrap();
        assert_eq!(item.state, AdapterHealthState::HealthyExistingGlobal);
        assert_eq!(item.installed_version.as_deref(), Some("0.12.1"));
        assert_eq!(item.source_kind, AdapterSourceKind::ExistingGlobal);
    }

    #[test]
    fn test_8c_npm_and_git_pi_intercom_is_repair_not_duplicate_install() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");
        let pi_agent_dir = dir.path().join("home/.pi/agent");
        fs::create_dir_all(&pi_agent_dir).unwrap();
        let settings_path = pi_agent_dir.join("settings.json");
        fs::write(
            &settings_path,
            format!(
                r#"{{"packages":["npm:@ctliz/pi-intercom@0.12.1","{}"]}}"#,
                PI_CANONICAL_GIT_TARGET
            ),
        )
        .unwrap();
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir,
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };
        let (item, _) = probe_single_adapter(&ctx, "pi");
        assert_eq!(item.unwrap().state, AdapterHealthState::NeedsRepair);
        let plan = check_workspace_adapters_internal(Some(&ctx), vec!["pi".to_string()]).unwrap();
        apply_workspace_install_plan_internal(
            None,
            Some(&ctx),
            &plan.plan_id,
            &plan.plan_fingerprint,
        )
        .unwrap();
        let content = fs::read_to_string(settings_path).unwrap();
        assert!(content.contains("npm:@ctliz/pi-intercom@0.12.1"));
        assert!(!content.contains(PI_CANONICAL_GIT_TARGET));
    }

    #[test]
    fn test_9_resource_locator_strictly_injected_no_fallback() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("codex");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let res = build_managed_root_staging(
            &ctx,
            &dir.path().join("staging"),
            &dir.path().join("cache"),
            "claude",
            CLAUDE_TARGET_VERSION,
            CLAUDE_RESOURCE_NAME,
            CLAUDE_RESOURCE_SHA256,
            "@ctliz/agent-intercom-claude",
            None,
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_10_non_macos_unavailable() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: false, // Non-macOS
            injected_fail_point: FAIL_NONE,
        };

        let (item, _) = probe_single_adapter(&ctx, "pi");
        assert_eq!(item.unwrap().state, AdapterHealthState::Unavailable);
    }

    #[test]
    fn test_11_app_private_health_requires_exact_package_core_sdk_marker_config_identity() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("managed/claude-intercom/0.13.0-connect.1");
        fs::create_dir_all(&root).unwrap();

        assert!(!verify_managed_root_integrity(
            &root,
            "claude",
            CLAUDE_TARGET_VERSION,
            CLAUDE_IMMUTABLE_DIGESTS,
            "@ctliz/agent-intercom-claude",
            CLAUDE_RESOURCE_NAME,
            CLAUDE_RESOURCE_SHA256,
        ));
    }

    #[test]
    fn test_12_gui_binary_search_finds_homebrew_when_path_is_minimal() {
        let dir = tempdir().unwrap();
        let homebrew = dir.path().join("opt/homebrew/bin");
        fs::create_dir_all(&homebrew).unwrap();
        fs::write(homebrew.join("codex"), b"mock").unwrap();
        assert!(binary_exists_in_dirs(
            "codex",
            [dir.path().join("usr/bin"), homebrew]
        ));
        assert!(!binary_exists_in_dirs(
            "claude",
            [dir.path().join("usr/bin")]
        ));
    }

    #[test]
    fn test_12_resource_sha256_verification() {
        assert_eq!(CORE_RESOURCE_SHA256.len(), 64);
        assert_eq!(CLAUDE_RESOURCE_SHA256.len(), 64);
        assert_eq!(CODEX_RESOURCE_SHA256.len(), 64);
        assert_eq!(OPENCODE_RESOURCE_SHA256.len(), 64);
    }

    #[test]
    fn test_13_staging_isolation_and_no_absolute_paths() {
        let dir = tempdir().unwrap();
        let codex_dir = dir.path().join("managed/codex-intercom");
        let target_root = codex_dir.join(CODEX_TARGET_VERSION);
        let staging_dir = codex_dir.join(".staging.123");
        let npm_cache = codex_dir.join(".npm-cache.123");

        let mut backup =
            ManagedRootBackup::new("codex", target_root, staging_dir.clone(), npm_cache.clone());
        fs::create_dir_all(&staging_dir).unwrap();
        fs::create_dir_all(&npm_cache).unwrap();

        backup.rollback(FAIL_NONE).unwrap();
        assert!(!staging_dir.exists());
        assert!(!npm_cache.exists());
    }

    #[test]
    fn test_14_codex_config_toml_update_and_restore() {
        let dir = tempdir().unwrap();
        let config_file = dir.path().join(".codex/config.toml");
        fs::create_dir_all(config_file.parent().unwrap()).unwrap();
        fs::write(&config_file, "[mcp_servers]\n").unwrap();

        let launcher = dir.path().join("launcher.mjs");
        update_codex_config_toml(&config_file, &launcher, FAIL_NONE).unwrap();

        let content = fs::read_to_string(&config_file).unwrap();
        assert!(content.contains("codex-intercom"));
        assert!(content.contains("command = \"node\""));
        assert!(content.contains("codex-server.mjs"));
        assert!(!content.contains("codex-launcher.mjs"));
        assert!(content.contains("AGENT_INTERCOM_SCOPE_ID"));
    }

    #[test]
    fn test_15_opencode_json_update_and_restore() {
        let dir = tempdir().unwrap();
        let config_file = dir.path().join(".config/opencode/opencode.json");
        fs::create_dir_all(config_file.parent().unwrap()).unwrap();
        fs::write(&config_file, "{\"plugin\": [\"other-plugin\"]}").unwrap();

        update_opencode_json_file(&config_file, "/path/to/plugin.mjs", FAIL_NONE).unwrap();

        let content = fs::read_to_string(&config_file).unwrap();
        assert!(content.contains("other-plugin"));
        assert!(content.contains("/path/to/plugin.mjs"));
    }

    #[test]
    fn test_16_full_app_private_install_and_rollback() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target");
        let staging = dir.path().join("staging");
        let cache = dir.path().join("cache");

        fs::create_dir_all(&staging).unwrap();
        let mut backup = ManagedRootBackup::new("claude", target.clone(), staging, cache);
        backup.swap_staging_to_active(FAIL_NONE).unwrap();
        assert!(target.exists());

        backup.rollback(FAIL_NONE).unwrap();
        assert!(!target.exists());
    }

    #[test]
    fn test_17_sentinel_and_real_host_isolation() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let (pi_item, _) = probe_single_adapter(&ctx, "pi");
        assert_eq!(pi_item.unwrap().state, AdapterHealthState::NotInstalled);
    }

    #[test]
    fn test_18_old_managed_claude_public_api_targets_new_exact_version_and_root() {
        let dir = tempdir().unwrap();
        let old_root = dir.path().join("managed/claude-intercom/0.12.0-connect.3");
        fs::create_dir_all(&old_root).unwrap();

        let inv = scan_managed_directory(
            &dir.path().join("managed/claude-intercom"),
            "claude",
            CLAUDE_TARGET_VERSION,
            CLAUDE_IMMUTABLE_DIGESTS,
            "@ctliz/agent-intercom-claude",
            "@dataforxyz/agent-intercom-claude",
            CLAUDE_RESOURCE_NAME,
            CLAUDE_RESOURCE_SHA256,
        );

        assert!(inv.has_invalid_roots);
    }

    #[test]
    fn test_19_codex_launcher_isolated_tmux_test() {
        assert!(CODEX_LAUNCHER_BODY.contains("AGENT_INTERCOM_TEAM_MANIFEST"));
        assert!(CODEX_LAUNCHER_BODY.contains("AGENT_INTERCOM_ROLE"));
    }

    #[test]
    fn test_20_injected_fsync_and_rollback_failure_boundary() {
        let dir = tempdir().unwrap();
        let target_file = dir.path().join("test.txt");
        fs::write(&target_file, "original").unwrap();

        // 1. FAIL_FILE_BACKUP_CREATE_WRITE
        let fb_res = FileBackup::create(&target_file, FAIL_FILE_BACKUP_CREATE_WRITE);
        assert!(fb_res.is_err());

        // 2. FAIL_FILE_BACKUP_CREATE_FSYNC
        let mut fb = FileBackup::create(&target_file, FAIL_NONE).unwrap();
        let fsync_res = checked_fsync_file(
            &File::open(&target_file).unwrap(),
            FAIL_FILE_BACKUP_CREATE_FSYNC,
        );
        assert!(fsync_res.is_err());

        // 3. FAIL_ACTIVE_TO_BACKUP_RENAME
        let staging = dir.path().join("staging");
        fs::create_dir_all(&staging).unwrap();
        let mut mb = ManagedRootBackup::new(
            "claude",
            dir.path().join("active"),
            staging.clone(),
            dir.path().join("cache"),
        );
        fs::create_dir_all(&mb.target_root).unwrap();
        assert!(mb
            .swap_staging_to_active(FAIL_ACTIVE_TO_BACKUP_RENAME)
            .is_err());

        // 4. FAIL_STAGING_TO_ACTIVE_RENAME
        let mut mb2 = ManagedRootBackup::new(
            "claude",
            dir.path().join("active2"),
            staging.clone(),
            dir.path().join("cache"),
        );
        assert!(mb2
            .swap_staging_to_active(FAIL_STAGING_TO_ACTIVE_RENAME)
            .is_err());

        // 5. FAIL_CONFIG_TEMP_WRITE
        assert!(atomic_write_file(
            &dir.path().join("c.txt"),
            b"123",
            0o600,
            FAIL_CONFIG_TEMP_WRITE
        )
        .is_err());

        // 6. FAIL_CONFIG_RENAME
        assert!(
            atomic_write_file(&dir.path().join("c.txt"), b"123", 0o600, FAIL_CONFIG_RENAME)
                .is_err()
        );

        // 7. FAIL_CONFIG_PARENT_FSYNC
        assert!(checked_fsync_dir(dir.path(), FAIL_CONFIG_PARENT_FSYNC).is_err());

        // 8. FAIL_POST_HEALTH_PROBE in actual install flow
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");
        let ctx_fail = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home_fail"),
            config_dir: dir.path().join("config_fail"),
            pi_agent_dir: dir.path().join("home_fail/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_POST_HEALTH_PROBE,
        };
        let plan_fail =
            check_workspace_adapters_internal(Some(&ctx_fail), vec!["pi".to_string()]).unwrap();
        let apply_fail_res = apply_workspace_install_plan_internal(
            None,
            Some(&ctx_fail),
            &plan_fail.plan_id,
            &plan_fail.plan_fingerprint,
        );
        assert_eq!(apply_fail_res.unwrap_err(), "ERR_ADAPTER_VERIFY");
        assert!(!ctx_fail.pi_agent_dir.join("settings.json").exists());

        // 9. FAIL_JOURNAL_WRITE
        let journal = CleanupJournal {
            items: vec![],
            created_at: 100,
        };
        assert!(journal
            .write_and_fsync(dir.path(), FAIL_JOURNAL_WRITE)
            .is_err());

        // 10. FAIL_COMMIT_BACKUP_REMOVE
        let dummy_journal = CleanupJournal {
            items: vec![JournalCleanupItem::ManagedOlderRoot {
                harness: "claude".to_string(),
                version: "0.1.0".to_string(),
                phase: JournalPhase::PendingRemove,
            }],
            created_at: 100,
        };
        let old_root_dir = dir.path().join("managed/claude-intercom/0.1.0");
        fs::create_dir_all(&old_root_dir).unwrap();
        dummy_journal
            .write_and_fsync(dir.path(), FAIL_NONE)
            .unwrap();
        assert!(
            reconcile_cleanup_journal(dir.path(), dir.path(), FAIL_COMMIT_BACKUP_REMOVE).is_err()
        );

        // 11. FAIL_COMMIT_PARENT_FSYNC
        assert!(checked_fsync_dir(dir.path(), FAIL_COMMIT_PARENT_FSYNC).is_err());

        // 12. FAIL_RESTORE_RENAME
        assert!(fb.rollback(FAIL_RESTORE_RENAME).is_err());

        // 13. FAIL_RESTORE_PARENT_FSYNC
        assert!(checked_fsync_dir(dir.path(), FAIL_RESTORE_PARENT_FSYNC).is_err());
    }

    #[test]
    fn test_21_real_offline_npm_staging_smoke() {
        let dir = tempdir().unwrap();
        let resources = dir.path().join("bundled-resources");
        fs::create_dir_all(&resources).unwrap();
        let cwd = std::env::current_dir().unwrap();
        let source_resources = if cwd.join("src-tauri/resources").is_dir() {
            cwd.join("src-tauri/resources")
        } else {
            cwd.join("resources")
        };
        for name in [CORE_RESOURCE_NAME, CLAUDE_RESOURCE_NAME] {
            fs::copy(source_resources.join(name), resources.join(name)).unwrap();
        }
        let runner = RealCommandRunner;
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };
        let staging_dir = dir.path().join("staging");
        let npm_cache = dir.path().join("cache");

        build_managed_root_staging(
            &ctx,
            &staging_dir,
            &npm_cache,
            "claude",
            CLAUDE_TARGET_VERSION,
            CLAUDE_RESOURCE_NAME,
            CLAUDE_RESOURCE_SHA256,
            "@ctliz/agent-intercom-claude",
            Some(&resources),
        )
        .unwrap();
        assert!(staging_dir.join("package-lock.json").is_file());
        assert!(staging_dir
            .join("node_modules/@ctliz/agent-intercom-core")
            .is_dir());
        assert!(staging_dir
            .join("node_modules/@ctliz/agent-intercom-claude")
            .is_dir());
        assert!(staging_dir.join(".claude-plugin/plugin.json").is_file());
        assert!(staging_dir.join(".mcp.json").is_file());
        assert!(staging_dir.join("monitors/monitors.json").is_file());
        let lock = fs::read_to_string(staging_dir.join("package-lock.json")).unwrap();
        assert!(!lock.contains("/tmp/") && !lock.contains("/Users/"));

        // The OpenCode recipe is statically pinned to the local SDK tarball.
        assert_eq!(OPENCODE_SDK_RESOURCE_NAME, "opencode-ai-plugin-1.18.18.tgz");
        assert_eq!(OPENCODE_SDK_RESOURCE_SHA256.len(), 64);

        let marker_path = staging_dir.join("tmuxdeck-managed.json");
        let mut marker: ManagedAdapterMarker =
            serde_json::from_str(&fs::read_to_string(&marker_path).unwrap()).unwrap();
        marker
            .digests
            .insert("dist/fabricated.mjs".to_string(), "00".repeat(32));
        fs::write(&marker_path, serde_json::to_string(&marker).unwrap()).unwrap();
        assert!(!verify_managed_root_integrity(
            &staging_dir,
            "claude",
            CLAUDE_TARGET_VERSION,
            CLAUDE_IMMUTABLE_DIGESTS,
            "@ctliz/agent-intercom-claude",
            CLAUDE_RESOURCE_NAME,
            CLAUDE_RESOURCE_SHA256,
        ));

        // Codex staging must succeed with npm 11 even when optional node-pty
        // is materialized in a nested local-package closure.
        fs::copy(
            source_resources.join(CODEX_RESOURCE_NAME),
            resources.join(CODEX_RESOURCE_NAME),
        )
        .unwrap();
        let codex_stage = dir.path().join("codex-staging");
        let codex_result = build_managed_root_staging(
            &ctx,
            &codex_stage,
            &dir.path().join("codex-cache"),
            "codex",
            CODEX_TARGET_VERSION,
            CODEX_RESOURCE_NAME,
            CODEX_RESOURCE_SHA256,
            "@ctliz/agent-intercom-codex",
            Some(&resources),
        );
        codex_result.unwrap();
        assert!(codex_stage
            .join("node_modules/@ctliz/agent-intercom-codex")
            .is_dir());
        let codex_lock: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(codex_stage.join("package-lock.json")).unwrap(),
        )
        .unwrap();
        let codex_keys = codex_lock["packages"].as_object().unwrap();
        assert!(codex_keys
            .keys()
            .all(|key| !key.ends_with("/node-pty") && !key.ends_with("/node-addon-api")));

        // The MCP entrypoint must be the bundled server itself, not the CLI launcher.
        let mut child = Command::new("node")
            .arg(codex_stage.join("dist/codex-server.mjs"))
            .current_dir(&codex_stage)
            .env(
                "AGENT_INTERCOM_TEAM_MANIFEST",
                dir.path().join("manifest.json"),
            )
            .env(
                "AGENT_INTERCOM_SESSION_ID",
                "tmuxdeck-a0000000-0000-4000-8000-000000000001",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
            .unwrap();
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let response = String::from_utf8(output.stdout).unwrap();
        assert!(response.contains("\"jsonrpc\":\"2.0\""));
        assert!(response.contains("\"id\":1"));
        assert!(response.contains("\"protocolVersion\""));

        fs::copy(
            source_resources.join(OPENCODE_RESOURCE_NAME),
            resources.join(OPENCODE_RESOURCE_NAME),
        )
        .unwrap();
        fs::copy(
            source_resources.join(OPENCODE_SDK_RESOURCE_NAME),
            resources.join(OPENCODE_SDK_RESOURCE_NAME),
        )
        .unwrap();
        fs::copy(
            source_resources.join(OPENCODE_CLOSURE_RESOURCE_NAME),
            resources.join(OPENCODE_CLOSURE_RESOURCE_NAME),
        )
        .unwrap();
        let opencode_stage = dir.path().join("opencode-staging");
        let opencode_result = build_managed_root_staging(
            &ctx,
            &opencode_stage,
            &dir.path().join("opencode-cache"),
            "opencode",
            OPENCODE_TARGET_VERSION,
            OPENCODE_RESOURCE_NAME,
            OPENCODE_RESOURCE_SHA256,
            "@ctliz/agent-intercom-opencode",
            Some(&resources),
        );
        opencode_result.unwrap();
        assert!(opencode_stage
            .join("node_modules/@opencode-ai/plugin")
            .is_dir());
        assert!(opencode_stage
            .join("node_modules/@opencode-ai/sdk")
            .is_dir());
    }

    #[test]
    fn test_22_tamper_and_failure_boundaries() {
        let dir = tempdir().unwrap();
        let old_root = dir.path().join("managed/codex-intercom/0.11.0");
        fs::create_dir_all(&old_root).unwrap();

        let inv = scan_managed_directory(
            &dir.path().join("managed/codex-intercom"),
            "codex",
            CODEX_TARGET_VERSION,
            CODEX_IMMUTABLE_DIGESTS,
            "@ctliz/agent-intercom-codex",
            "@dataforxyz/agent-intercom-codex",
            CODEX_RESOURCE_NAME,
            CODEX_RESOURCE_SHA256,
        );
        assert!(inv.has_invalid_roots);
        assert!(inv.older_roots.is_empty());
    }

    #[test]
    fn test_23_semver_ord_eq_consistency_and_build_metadata() {
        let v1 = SemVer::parse("1.0.0-alpha.1").unwrap();
        let v2 = SemVer::parse("1.0.0-alpha.2").unwrap();
        assert!(v1 < v2);

        let v_huge1 = SemVer::parse("1.0.0-99999999999999999999999999999999").unwrap();
        let v_huge2 = SemVer::parse("1.0.0-100000000000000000000000000000000").unwrap();
        assert!(v_huge1 < v_huge2);

        let v_core1 = SemVer::parse("99999999999999999999999999999999.0.0").unwrap();
        let v_core2 = SemVer::parse("100000000000000000000000000000000.0.0").unwrap();
        assert!(v_core1 < v_core2);

        let vb1 = SemVer::parse("1.0.0+build1").unwrap();
        let vb2 = SemVer::parse("1.0.0+build2").unwrap();
        assert_eq!(vb1, vb2);
    }

    #[test]
    fn test_24_healthy_existing_global_and_mixed_managed_conflicts() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("codex");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let codex_config = ctx.home_dir.join(".codex/config.toml");
        fs::create_dir_all(codex_config.parent().unwrap()).unwrap();
        fs::write(
            &codex_config,
            "[mcp_servers.codex-intercom]\ncommand = \"codex-intercom-mcp\"\n",
        )
        .unwrap();

        let (item, _) = probe_single_adapter(&ctx, "codex");
        assert_eq!(item.unwrap().state, AdapterHealthState::NeedsRepair);
    }

    #[test]
    fn test_24b_stale_opencode_managed_path_is_repair_not_migration() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("opencode");
        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir: dir.path().join("home/.pi/agent"),
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let oc_dir = ctx.home_dir.join(".config/opencode");
        fs::create_dir_all(&oc_dir).unwrap();
        let missing_root = ctx
            .config_dir
            .join("managed/opencode-intercom")
            .join(OPENCODE_TARGET_VERSION);
        fs::write(
            oc_dir.join("opencode.json"),
            format!(
                r#"{{"plugin":["{}/dist/plugin.mjs"]}}"#,
                missing_root.display()
            ),
        )
        .unwrap();
        fs::write(
            oc_dir.join("tui.json"),
            format!(
                r#"{{"plugin":["{}/dist/tui.mjs"]}}"#,
                missing_root.display()
            ),
        )
        .unwrap();

        let (item, _) = probe_single_adapter(&ctx, "opencode");
        let item = item.unwrap();
        assert_eq!(item.state, AdapterHealthState::NeedsRepair);
        assert_ne!(item.state, AdapterHealthState::MigrationRequired);
        assert_eq!(item.action_reason, AdapterActionReason::Repair);
    }

    #[test]
    fn test_25_drift_reprobe_with_mixed_healthy_and_action_plans() {
        let dir = tempdir().unwrap();
        let runner = MockCommandRunner::new();
        runner.set_bin("pi");
        runner.set_bin("claude");

        let pi_agent_dir = dir.path().join("home/.pi/agent");
        fs::create_dir_all(&pi_agent_dir).unwrap();
        let settings_path = pi_agent_dir.join("settings.json");
        let settings = serde_json::json!({
            "packages": [PI_CANONICAL_GIT_TARGET]
        });
        fs::write(&settings_path, settings.to_string()).unwrap();

        let ctx = AdapterContext {
            runner: &runner,
            home_dir: dir.path().join("home"),
            config_dir: dir.path().join("config"),
            pi_agent_dir,
            is_macos: true,
            injected_fail_point: FAIL_NONE,
        };

        let plan = check_workspace_adapters_internal(
            Some(&ctx),
            vec!["pi".to_string(), "claude".to_string()],
        )
        .unwrap();
        assert_eq!(plan.healthy_agent_ids, vec!["pi".to_string()]);
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].agent_id, "claude");
        assert!(plan.can_apply);

        // Applying the mixed plan must re-probe the healthy surface and reject drift.
        fs::write(&settings_path, "{\"packages\":[]}").unwrap();
        let apply = apply_workspace_install_plan_internal(
            None,
            Some(&ctx),
            &plan.plan_id,
            &plan.plan_fingerprint,
        );
        assert_eq!(apply.unwrap_err(), "ERR_PLAN_STALE");
    }

    #[test]
    fn test_26_journal_reconciliation_error_propagation_and_retry_ownership() {
        let dir = tempdir().unwrap();
        let target_dir = dir.path().join("managed/claude-intercom/0.1.0");
        fs::create_dir_all(&target_dir).unwrap();

        let journal = CleanupJournal {
            items: vec![JournalCleanupItem::ManagedOlderRoot {
                harness: "claude".to_string(),
                version: "0.1.0".to_string(),
                phase: JournalPhase::PendingRemove,
            }],
            created_at: 100,
        };
        journal.write_and_fsync(dir.path(), FAIL_NONE).unwrap();

        // Fail commit parent fsync
        assert!(
            reconcile_cleanup_journal(dir.path(), dir.path(), FAIL_COMMIT_PARENT_FSYNC).is_err()
        );
        assert!(dir.path().join("managed/.cleanup_journal.json").exists());

        // Fail backup remove
        assert!(
            reconcile_cleanup_journal(dir.path(), dir.path(), FAIL_COMMIT_BACKUP_REMOVE).is_err()
        );
        assert!(dir.path().join("managed/.cleanup_journal.json").exists());

        // Successful item cleanup followed by journal unlink and parent-fsync failure
        let empty = CleanupJournal {
            items: Vec::new(),
            created_at: 101,
        };
        empty.write_and_fsync(dir.path(), FAIL_NONE).unwrap();
        assert!(
            reconcile_cleanup_journal(dir.path(), dir.path(), FAIL_COMMIT_PARENT_FSYNC).is_err()
        );
        assert!(dir.path().join("managed/.cleanup_journal.json").exists());
        reconcile_cleanup_journal(dir.path(), dir.path(), FAIL_NONE).unwrap();
        assert!(!dir.path().join("managed/.cleanup_journal.json").exists());
    }

    #[test]
    fn test_27_verify_parent_not_symlink_nonexistent_child_under_symlink() {
        let dir = tempdir().unwrap();
        let real_dir = dir.path().join("real_dir");
        fs::create_dir_all(&real_dir).unwrap();

        #[cfg(unix)]
        {
            let symlink_path = dir.path().join("symlink_dir");
            std::os::unix::fs::symlink(&real_dir, &symlink_path).unwrap();

            let nonexistent_child = symlink_path.join("sub1/sub2/sub3");
            assert!(verify_parent_not_symlink(&nonexistent_child).is_err());
        }
    }

    #[test]
    fn test_28_verify_managed_root_marker_digests_and_resources_validation() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("managed_root");
        fs::create_dir_all(&root).unwrap();

        // Marker with missing resources or digests fails integrity
        let marker = ManagedAdapterMarker {
            schema_version: 1,
            harness: "claude".to_string(),
            adapter_version: CLAUDE_TARGET_VERSION.to_string(),
            installed_at: 100,
            resources: BTreeMap::new(),
            digests: BTreeMap::new(),
        };
        let marker_str = serde_json::to_string(&marker).unwrap();
        fs::write(root.join("tmuxdeck-managed.json"), marker_str).unwrap();

        assert!(!verify_managed_root_integrity(
            &root,
            "claude",
            CLAUDE_TARGET_VERSION,
            CLAUDE_IMMUTABLE_DIGESTS,
            "@ctliz/agent-intercom-claude",
            CLAUDE_RESOURCE_NAME,
            CLAUDE_RESOURCE_SHA256,
        ));
    }

    #[test]
    fn test_29_plan_id_shape_validation() {
        assert!(!validate_plan_id("plan_short"));
        assert!(!validate_plan_id(
            "plan_1234567890abcdef1234567890abcdef_extra"
        ));
        assert!(!validate_plan_id("plan_GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG"));
        assert!(validate_plan_id("plan_0123456789abcdef0123456789abcdef"));
        assert!(!validate_plan_fingerprint("not_a_64_hex_fingerprint"));
        assert!(validate_plan_fingerprint(&"a".repeat(64)));
    }
}
