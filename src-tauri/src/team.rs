use serde::{Deserialize, Serialize};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

#[cfg(unix)]
extern "C" {
    fn getuid() -> u32;
}

use crate::config::get_config_dir;

pub const TEAM_MANIFEST_VERSION: &str = "tmuxdeck.team.v1";
pub const TEAM_BACKEND: &str = "tmuxdeck";

pub const OPTION_LEAD_ID: &str = "@tmuxdeck-lead-id";
pub const OPTION_TEAM_RUN_ID: &str = "@tmuxdeck-team-run-id";
pub const OPTION_INTERCOM_ID: &str = "@tmuxdeck-intercom-id";
pub const OPTION_ROLE: &str = "@tmuxdeck-role";
pub const OPTION_MANAGER_TARGET: &str = "@tmuxdeck-manager-target";

pub const ROLE_LEAD: &str = "lead";
pub const ROLE_WORKER: &str = "worker";
pub const ENV_ROLE_MANAGER: &str = "manager";
pub const ENV_ROLE_WORKER: &str = "worker";

pub const ERR_TEAM_UNAVAILABLE: &str = "ERR_TEAM_UNAVAILABLE";
pub const ERR_TEAM_CONFLICT: &str = "ERR_TEAM_CONFLICT";
pub const ERR_TEAM_CAPACITY: &str = "ERR_TEAM_CAPACITY";
pub const ERR_TEAM_ROLLBACK: &str = "ERR_TEAM_ROLLBACK";
pub const MAX_TEAM_MEMBERS: usize = 64;

pub static TEAM_MUTATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeamMember {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub role: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeamManifest {
    pub version: String,
    pub backend: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "leadId")]
    pub lead_id: String,
    pub members: Vec<TeamMember>,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    pub capabilities: Vec<String>,
}

pub fn is_valid_uuid_v4(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    let b = s.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            if c != b'-' {
                return false;
            }
        } else if i == 14 {
            if c != b'4' {
                return false;
            }
        } else if i == 19 {
            if c != b'8' && c != b'9' && c != b'a' && c != b'b' {
                return false;
            }
        } else if !c.is_ascii_hexdigit() || c.is_ascii_uppercase() {
            return false;
        }
    }
    true
}

pub fn is_valid_team_run_id(s: &str) -> bool {
    if let Some(uuid_part) = s.strip_prefix("team_") {
        is_valid_uuid_v4(uuid_part)
    } else {
        false
    }
}

pub fn is_valid_session_id(s: &str) -> bool {
    if let Some(uuid_part) = s.strip_prefix("tmuxdeck-") {
        is_valid_uuid_v4(uuid_part)
    } else {
        false
    }
}

fn generate_uuid_v4() -> Result<String, String> {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).map_err(|e| format!("ERR_RANDOM_ID|{}", e))?;
    b[6] = (b[6] & 0x0f) | 0x40; // RFC 4122 version 4
    b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant 1 (10xx_xxxx)
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    ))
}

pub fn generate_team_run_id() -> Result<String, String> {
    let uuid = generate_uuid_v4()?;
    Ok(format!("team_{}", uuid))
}

pub fn generate_session_id() -> Result<String, String> {
    let uuid = generate_uuid_v4()?;
    Ok(format!("tmuxdeck-{}", uuid))
}

pub fn now_ms() -> Result<u64, String> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("ERR_SYSTEM_TIME|{}", e))?
        .as_millis() as u64;
    if ms == 0 {
        return Err("ERR_SYSTEM_TIME|zero".to_string());
    }
    Ok(ms)
}

pub fn validate_team_manifest(manifest: &TeamManifest) -> Result<(), String> {
    if manifest.version != TEAM_MANIFEST_VERSION {
        return Err(format!("ERR_MANIFEST_INVALID|version|{}", manifest.version));
    }
    if manifest.backend != TEAM_BACKEND {
        return Err(format!("ERR_MANIFEST_INVALID|backend|{}", manifest.backend));
    }
    if !is_valid_team_run_id(&manifest.run_id) {
        return Err(format!("ERR_MANIFEST_INVALID|run_id|{}", manifest.run_id));
    }
    if !is_valid_session_id(&manifest.lead_id) {
        return Err(format!("ERR_MANIFEST_INVALID|lead_id|{}", manifest.lead_id));
    }
    if manifest.members.is_empty() || manifest.members.len() > MAX_TEAM_MEMBERS {
        return Err(format!(
            "ERR_MANIFEST_INVALID|members_count|{}",
            manifest.members.len()
        ));
    }
    if manifest.created_at == 0 {
        return Err("ERR_MANIFEST_INVALID|created_at_zero".to_string());
    }
    if !manifest.capabilities.is_empty() {
        return Err("ERR_MANIFEST_INVALID|capabilities_not_empty".to_string());
    }

    let mut lead_count = 0;
    let mut seen_ids = std::collections::HashSet::new();
    for member in &manifest.members {
        if !is_valid_session_id(&member.session_id) {
            return Err(format!(
                "ERR_MANIFEST_INVALID|member_session_id|{}",
                member.session_id
            ));
        }
        if !seen_ids.insert(&member.session_id) {
            return Err(format!(
                "ERR_MANIFEST_INVALID|duplicate_member|{}",
                member.session_id
            ));
        }
        match member.role.as_str() {
            ROLE_LEAD => {
                lead_count += 1;
                if member.session_id != manifest.lead_id {
                    return Err("ERR_MANIFEST_INVALID|lead_session_id_mismatch".to_string());
                }
            }
            ROLE_WORKER => {}
            other => {
                return Err(format!("ERR_MANIFEST_INVALID|unknown_role|{}", other));
            }
        }
    }
    if lead_count != 1 {
        return Err(format!("ERR_MANIFEST_INVALID|lead_count|{}", lead_count));
    }
    Ok(())
}

pub fn is_team_supported() -> bool {
    cfg!(target_os = "macos")
}

pub fn create_team_manifest(
    run_id: String,
    lead_id: String,
    members: Vec<TeamMember>,
) -> Result<TeamManifest, String> {
    if !is_team_supported() {
        return Err(ERR_TEAM_UNAVAILABLE.to_string());
    }
    let manifest = TeamManifest {
        version: TEAM_MANIFEST_VERSION.to_string(),
        backend: TEAM_BACKEND.to_string(),
        run_id,
        lead_id,
        members,
        created_at: now_ms()?,
        capabilities: Vec::new(),
    };
    validate_team_manifest(&manifest)?;
    Ok(manifest)
}

pub fn get_teams_dir() -> PathBuf {
    get_config_dir().join("teams")
}

pub fn team_manifest_path(run_id: &str) -> Result<PathBuf, String> {
    if !is_team_supported() {
        return Err(ERR_TEAM_UNAVAILABLE.to_string());
    }
    if !is_valid_team_run_id(run_id) {
        return Err(format!("ERR_MANIFEST_INVALID|run_id|{}", run_id));
    }
    Ok(get_teams_dir().join(format!("{}.json", run_id)))
}

pub fn verify_teams_dir_security(dir: &Path) -> Result<(), String> {
    if !is_team_supported() {
        return Err(ERR_TEAM_UNAVAILABLE.to_string());
    }
    if !dir.is_absolute() {
        return Err("ERR_MANIFEST_PATH|not_absolute".to_string());
    }
    let meta = fs::symlink_metadata(dir).map_err(|e| format!("ERR_MANIFEST_DIR|lstat|{}", e))?;
    if !meta.file_type().is_dir() {
        return Err("ERR_MANIFEST_DIR|not_a_directory".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        use std::os::unix::fs::PermissionsExt;
        let current_uid = unsafe { getuid() };
        if meta.uid() != current_uid {
            return Err("ERR_MANIFEST_DIR|owner_mismatch".to_string());
        }
        let mode = meta.permissions().mode() & 0o777;
        if mode != 0o700 {
            return Err(format!("ERR_MANIFEST_DIR|insecure_permissions|{:o}", mode));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveTeamMemberInfo {
    pub session_id: String,
    pub role: String,
    pub manager_target: String,
}

pub fn validate_live_team_members(
    manifest: &TeamManifest,
    live_members: &[LiveTeamMemberInfo],
) -> Result<(), String> {
    let mut seen_ids = std::collections::HashSet::new();
    for member_info in live_members {
        if member_info.session_id.is_empty() || member_info.role.is_empty() {
            return Err(ERR_TEAM_CONFLICT.to_string());
        }
        if !is_valid_session_id(&member_info.session_id) {
            return Err(ERR_TEAM_CONFLICT.to_string());
        }
        if !seen_ids.insert(member_info.session_id.clone()) {
            return Err(ERR_TEAM_CONFLICT.to_string());
        }
        let Some(member) = manifest
            .members
            .iter()
            .find(|m| m.session_id == member_info.session_id)
        else {
            return Err(ERR_TEAM_CONFLICT.to_string());
        };
        if member.role != member_info.role {
            return Err(ERR_TEAM_CONFLICT.to_string());
        }
        if member_info.role == ROLE_LEAD {
            if member_info.session_id != manifest.lead_id {
                return Err(ERR_TEAM_CONFLICT.to_string());
            }
            if !member_info.manager_target.is_empty() {
                return Err(ERR_TEAM_CONFLICT.to_string());
            }
        } else if member_info.role == ROLE_WORKER {
            if member_info.session_id == manifest.lead_id {
                return Err(ERR_TEAM_CONFLICT.to_string());
            }
            if member_info.manager_target != manifest.lead_id {
                return Err(ERR_TEAM_CONFLICT.to_string());
            }
        } else {
            return Err(ERR_TEAM_CONFLICT.to_string());
        }
    }
    Ok(())
}

pub fn write_team_manifest_in_dir(dir: &Path, manifest: &TeamManifest) -> Result<PathBuf, String> {
    if !is_team_supported() {
        return Err(ERR_TEAM_UNAVAILABLE.to_string());
    }
    if !dir.is_absolute() {
        return Err("ERR_MANIFEST_PATH|not_absolute".to_string());
    }
    validate_team_manifest(manifest)?;

    if dir.exists() {
        let meta =
            fs::symlink_metadata(dir).map_err(|e| format!("ERR_MANIFEST_DIR|lstat|{}", e))?;
        if !meta.file_type().is_dir() {
            return Err("ERR_MANIFEST_DIR|not_a_directory".to_string());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            use std::os::unix::fs::PermissionsExt;
            let current_uid = unsafe { getuid() };
            if meta.uid() != current_uid {
                return Err("ERR_MANIFEST_DIR|owner_mismatch".to_string());
            }
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("ERR_MANIFEST_WRITE|dir_permissions|{}", e))?;
        }
    } else {
        let mut dir_builder = DirBuilder::new();
        dir_builder.recursive(true);
        #[cfg(unix)]
        dir_builder.mode(0o700);
        dir_builder
            .create(dir)
            .map_err(|e| format!("ERR_MANIFEST_WRITE|dir|{}", e))?;
    }

    verify_teams_dir_security(dir)?;

    let final_path = dir.join(format!("{}.json", manifest.run_id));
    let mut nonce_bytes = [0u8; 8];
    getrandom::getrandom(&mut nonce_bytes).map_err(|e| format!("ERR_RANDOM_NONCE|{}", e))?;
    let nonce: String = nonce_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let temp_file_name = format!("tmp.{}.{}.json", std::process::id(), nonce);
    let temp_path = dir.join(temp_file_name);

    let json_bytes =
        serde_json::to_vec_pretty(manifest).map_err(|e| format!("ERR_MANIFEST_JSON|{}", e))?;

    let mut open_options = OpenOptions::new();
    open_options.write(true).create_new(true);
    #[cfg(unix)]
    open_options.mode(0o600);

    let mut file = match open_options.open(&temp_path) {
        Ok(f) => f,
        Err(e) => return Err(format!("ERR_MANIFEST_WRITE|open|{}", e)),
    };

    #[cfg(unix)]
    {
        if let Err(e) = file.set_permissions(fs::Permissions::from_mode(0o600)) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("ERR_MANIFEST_WRITE|permissions|{}", e));
        }
    }

    if let Err(e) = file.write_all(&json_bytes) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("ERR_MANIFEST_WRITE|write|{}", e));
    }

    if let Err(e) = file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("ERR_MANIFEST_WRITE|sync|{}", e));
    }

    drop(file);

    if let Err(e) = fs::rename(&temp_path, &final_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("ERR_MANIFEST_WRITE|rename|{}", e));
    }

    #[cfg(unix)]
    {
        let dir_file =
            fs::File::open(dir).map_err(|e| format!("ERR_MANIFEST_WRITE|dir_open|{}", e))?;
        dir_file
            .sync_all()
            .map_err(|e| format!("ERR_MANIFEST_WRITE|dir_sync|{}", e))?;
    }

    Ok(final_path)
}

pub fn write_team_manifest(manifest: &TeamManifest) -> Result<PathBuf, String> {
    write_team_manifest_in_dir(&get_teams_dir(), manifest)
}

pub fn read_team_manifest_in_dir(dir: &Path, run_id: &str) -> Result<TeamManifest, String> {
    if !is_team_supported() {
        return Err(ERR_TEAM_UNAVAILABLE.to_string());
    }
    if !dir.is_absolute() {
        return Err("ERR_MANIFEST_PATH|not_absolute".to_string());
    }
    if !is_valid_team_run_id(run_id) {
        return Err(format!("ERR_MANIFEST_INVALID|run_id|{}", run_id));
    }
    verify_teams_dir_security(dir)?;
    let path = dir.join(format!("{}.json", run_id));
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("ERR_MANIFEST_READ|lstat|{}", e))?;
    if !meta.file_type().is_file() {
        return Err("ERR_MANIFEST_READ|not_a_regular_file".to_string());
    }
    if meta.len() > 65536 {
        return Err("ERR_MANIFEST_READ|file_too_large".to_string());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        use std::os::unix::fs::PermissionsExt;
        let current_uid = unsafe { getuid() };
        if meta.uid() != current_uid {
            return Err("ERR_MANIFEST_READ|owner_mismatch".to_string());
        }
        let mode = meta.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(format!("ERR_MANIFEST_READ|insecure_permissions|{:o}", mode));
        }
    }

    let content = fs::read_to_string(&path).map_err(|e| format!("ERR_MANIFEST_READ|{}", e))?;
    let manifest: TeamManifest =
        serde_json::from_str(&content).map_err(|e| format!("ERR_MANIFEST_PARSE|{}", e))?;
    validate_team_manifest(&manifest)?;
    if manifest.run_id != run_id {
        return Err("ERR_MANIFEST_INVALID|run_id_content_mismatch".to_string());
    }
    Ok(manifest)
}

pub fn read_team_manifest(run_id: &str) -> Result<TeamManifest, String> {
    read_team_manifest_in_dir(&get_teams_dir(), run_id)
}

pub fn append_team_members_in_dir(
    dir: &Path,
    run_id: &str,
    new_members: &[TeamMember],
) -> Result<TeamManifest, String> {
    let mut manifest = read_team_manifest_in_dir(dir, run_id)?;
    if manifest.members.len() + new_members.len() > MAX_TEAM_MEMBERS {
        return Err(format!(
            "ERR_TEAM_CAPACITY|{}",
            manifest.members.len() + new_members.len()
        ));
    }
    for member in new_members {
        if manifest
            .members
            .iter()
            .any(|m| m.session_id == member.session_id)
        {
            return Err(format!(
                "ERR_MANIFEST_INVALID|duplicate_member|{}",
                member.session_id
            ));
        }
        manifest.members.push(member.clone());
    }
    validate_team_manifest(&manifest)?;
    write_team_manifest_in_dir(dir, &manifest)?;
    Ok(manifest)
}

pub fn append_team_members(
    run_id: &str,
    new_members: &[TeamMember],
) -> Result<TeamManifest, String> {
    append_team_members_in_dir(&get_teams_dir(), run_id, new_members)
}

pub fn remove_team_member_in_dir(
    dir: &Path,
    run_id: &str,
    session_id: &str,
) -> Result<TeamManifest, String> {
    let mut manifest = read_team_manifest_in_dir(dir, run_id)?;
    if manifest.lead_id == session_id {
        return Err("ERR_KILL_LEAD_NOT_ALLOWED".to_string());
    }
    let initial_len = manifest.members.len();
    manifest.members.retain(|m| m.session_id != session_id);
    if manifest.members.len() == initial_len {
        return Ok(manifest);
    }
    validate_team_manifest(&manifest)?;
    write_team_manifest_in_dir(dir, &manifest)?;
    Ok(manifest)
}

pub fn remove_team_member(run_id: &str, session_id: &str) -> Result<TeamManifest, String> {
    remove_team_member_in_dir(&get_teams_dir(), run_id, session_id)
}

pub fn delete_team_manifest_in_dir(dir: &Path, run_id: &str) -> Result<(), String> {
    if !is_team_supported() {
        return Ok(());
    }
    if !dir.is_absolute() {
        return Err("ERR_MANIFEST_PATH|not_absolute".to_string());
    }
    if !is_valid_team_run_id(run_id) {
        return Err(format!("ERR_MANIFEST_INVALID|run_id|{}", run_id));
    }
    if !dir.exists() {
        return Ok(());
    }
    verify_teams_dir_security(dir)?;
    let path = dir.join(format!("{}.json", run_id));
    let meta = match fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("ERR_MANIFEST_DELETE|lstat|{}", e)),
    };
    if !meta.file_type().is_file() {
        return Err("ERR_MANIFEST_DELETE|not_a_regular_file".to_string());
    }
    fs::remove_file(&path).map_err(|e| format!("ERR_MANIFEST_DELETE|{}", e))?;
    Ok(())
}

pub fn delete_team_manifest(run_id: &str) -> Result<(), String> {
    delete_team_manifest_in_dir(&get_teams_dir(), run_id)
}

pub fn reconcile_orphan_manifests_in_dir(
    dir: &Path,
    active_run_ids: &std::collections::HashSet<String>,
) -> Result<usize, String> {
    if !is_team_supported() {
        return Ok(0);
    }
    if !dir.is_absolute() {
        return Err("ERR_MANIFEST_PATH|not_absolute".to_string());
    }
    if !dir.exists() {
        return Ok(0);
    }
    verify_teams_dir_security(dir)?;
    let entries = fs::read_dir(dir).map_err(|e| format!("ERR_RECONCILE|{}", e))?;
    let mut pruned = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if file_name.starts_with("tmp.") {
                if fs::remove_file(&path).is_ok() {
                    pruned += 1;
                }
                continue;
            }
            if let Some(stem) = file_name.strip_suffix(".json") {
                if is_valid_team_run_id(stem) && !active_run_ids.contains(stem) {
                    if fs::remove_file(&path).is_ok() {
                        pruned += 1;
                    }
                }
            }
        }
    }
    Ok(pruned)
}

pub fn reconcile_orphan_manifests() -> Result<usize, String> {
    let teams_dir = get_teams_dir();
    if !teams_dir.exists() {
        return Ok(0);
    }
    let output = crate::tmux::run_tmux(&["list-sessions", "-F", "#{@tmuxdeck-team-run-id}"]);
    let mut active_run_ids = std::collections::HashSet::new();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let run_id = line.trim();
                if is_valid_team_run_id(run_id) {
                    active_run_ids.insert(run_id.to_string());
                }
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if crate::tmux::is_no_server_err(&stderr) {
                // Tmux server not running -> 0 active sessions
            } else {
                return Err(format!("ERR_RECONCILE|tmux error: {}", stderr.trim()));
            }
        }
        Err(e) => {
            return Err(format!("ERR_RECONCILE|tmux spawn error: {}", e));
        }
    }
    reconcile_orphan_manifests_in_dir(&teams_dir, &active_run_ids)
}

pub struct PaneTeamEnvOpts<'a> {
    pub workspace_name: &'a str,
    pub pane_index: usize,
    pub agent_id: &'a str,
    pub scope_id: &'a str,
    pub team_manifest_path: &'a str,
    pub session_id: &'a str,
    pub role: &'a str,
    pub lead_session_id: &'a str,
}

pub fn build_pane_team_env(opts: &PaneTeamEnvOpts) -> Vec<(String, String)> {
    let mut envs = Vec::new();
    envs.push((
        "AGENT_INTERCOM_SCOPE_ID".to_string(),
        opts.scope_id.to_string(),
    ));
    envs.push((
        "AGENT_INTERCOM_TEAM_MANIFEST".to_string(),
        opts.team_manifest_path.to_string(),
    ));

    let is_lead = opts.role == ROLE_LEAD;
    let role_val = if is_lead {
        ENV_ROLE_MANAGER
    } else {
        ENV_ROLE_WORKER
    };
    envs.push(("AGENT_INTERCOM_ROLE".to_string(), role_val.to_string()));
    envs.push((
        "AGENT_INTERCOM_SESSION_ID".to_string(),
        opts.session_id.to_string(),
    ));

    let agent_name = match opts.agent_id {
        "claude" => "Claude",
        "pi" => "Pi",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        "grok" => "Grok Build",
        "agy" => "AGY",
        "shell" => "Shell",
        other => other,
    };
    let session_name = format!(
        "{} · {} {:02}",
        opts.workspace_name, agent_name, opts.pane_index
    );
    envs.push((
        "AGENT_INTERCOM_SESSION_NAME".to_string(),
        session_name.clone(),
    ));

    if !is_lead {
        envs.push((
            "AGENT_INTERCOM_MANAGER_TARGET".to_string(),
            opts.lead_session_id.to_string(),
        ));
        envs.push((
            "AGENT_INTERCOM_MANAGER_SESSION_ID".to_string(),
            opts.lead_session_id.to_string(),
        ));
    }

    match opts.agent_id {
        "pi" => {
            envs.push((
                "PI_INTERCOM_SESSION_ID".to_string(),
                opts.session_id.to_string(),
            ));
        }
        "claude" | "grok" | "agy" => {
            envs.push((
                "CLAUDE_INTERCOM_SESSION_ID".to_string(),
                opts.session_id.to_string(),
            ));
            envs.push(("CLAUDE_INTERCOM_NAME".to_string(), session_name));
        }
        "codex" => {
            envs.push((
                "CODEX_INTERCOM_SESSION_ID".to_string(),
                opts.session_id.to_string(),
            ));
            envs.push(("CODEX_INTERCOM_NAME".to_string(), session_name));
        }
        "opencode" => {
            envs.push((
                "OPENCODE_INTERCOM_SESSION_ID".to_string(),
                opts.session_id.to_string(),
            ));
            envs.push(("OPENCODE_INTERCOM_NAME".to_string(), session_name));
        }
        _ => {}
    }

    envs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_temp_dir() -> PathBuf {
        let nonce: u64 = rand_nonce();
        let dir = std::env::temp_dir().join(format!(
            "tmuxdeck-test-teams-{}-{}",
            std::process::id(),
            nonce
        ));
        let mut dir_builder = DirBuilder::new();
        dir_builder.recursive(true);
        #[cfg(unix)]
        dir_builder.mode(0o700);
        let _ = dir_builder.create(&dir);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
        }
        dir
    }

    fn rand_nonce() -> u64 {
        let mut b = [0u8; 8];
        let _ = getrandom::getrandom(&mut b);
        u64::from_le_bytes(b)
    }

    #[test]
    fn test_uuidv4_rfc4122_version_and_variant() {
        for _ in 0..50 {
            let run_id = generate_team_run_id().unwrap();
            let session_id = generate_session_id().unwrap();

            assert!(
                is_valid_team_run_id(&run_id),
                "run_id should be valid: {}",
                run_id
            );
            assert!(
                is_valid_session_id(&session_id),
                "session_id should be valid: {}",
                session_id
            );

            // Test run_id format: "team_" (5 chars) + uuid (36 chars) = 41 chars
            assert_eq!(run_id.len(), 41);
            let run_uuid = &run_id[5..];
            assert_eq!(&run_uuid[14..15], "4", "UUID version must be 4");
            let var_char = run_uuid.chars().nth(19).unwrap();
            assert!(
                ['8', '9', 'a', 'b'].contains(&var_char),
                "UUID variant must be 8, 9, a, or b, got {}",
                var_char
            );

            // Test session_id format: "tmuxdeck-" (9 chars) + uuid (36 chars) = 45 chars
            assert_eq!(session_id.len(), 45);
            let sess_uuid = &session_id[9..];
            assert_eq!(&sess_uuid[14..15], "4", "UUID version must be 4");
            let sess_var = sess_uuid.chars().nth(19).unwrap();
            assert!(
                ['8', '9', 'a', 'b'].contains(&sess_var),
                "UUID variant must be 8, 9, a, or b, got {}",
                sess_var
            );
        }
    }

    #[test]
    fn test_manifest_serialization_and_write_in_temp_dir() {
        let temp_dir = create_test_temp_dir();
        let run_id = generate_team_run_id().unwrap();
        let lead_id = generate_session_id().unwrap();
        let worker_id = generate_session_id().unwrap();

        let members = vec![
            TeamMember {
                session_id: lead_id.clone(),
                role: ROLE_LEAD.to_string(),
            },
            TeamMember {
                session_id: worker_id.clone(),
                role: ROLE_WORKER.to_string(),
            },
        ];

        let manifest =
            create_team_manifest(run_id.clone(), lead_id.clone(), members.clone()).unwrap();
        assert_eq!(manifest.version, "tmuxdeck.team.v1");
        assert_eq!(manifest.backend, "tmuxdeck");
        assert_eq!(manifest.lead_id, lead_id);
        assert_eq!(manifest.members.len(), 2);
        assert_eq!(manifest.capabilities, Vec::<String>::new());

        let written_path = write_team_manifest_in_dir(&temp_dir, &manifest).unwrap();
        assert!(written_path.exists());

        let read_back = read_team_manifest_in_dir(&temp_dir, &run_id).unwrap();
        assert_eq!(read_back, manifest);

        let new_worker_id = generate_session_id().unwrap();
        let updated = append_team_members_in_dir(
            &temp_dir,
            &run_id,
            &[TeamMember {
                session_id: new_worker_id.clone(),
                role: ROLE_WORKER.to_string(),
            }],
        )
        .unwrap();
        assert_eq!(updated.members.len(), 3);
        assert_eq!(updated.members[2].session_id, new_worker_id);

        let removed = remove_team_member_in_dir(&temp_dir, &run_id, &worker_id).unwrap();
        assert_eq!(removed.members.len(), 2);
        assert!(!removed.members.iter().any(|m| m.session_id == worker_id));

        // Removing lead should fail
        assert_eq!(
            remove_team_member_in_dir(&temp_dir, &run_id, &lead_id),
            Err("ERR_KILL_LEAD_NOT_ALLOWED".to_string())
        );

        delete_team_manifest_in_dir(&temp_dir, &run_id).unwrap();
        assert!(!written_path.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_manifest_exact_json_schema_and_validation() {
        let manifest = TeamManifest {
            version: "tmuxdeck.team.v1".to_string(),
            backend: "tmuxdeck".to_string(),
            run_id: "team_11223344-5566-4778-8899-aabbccddeeff".to_string(),
            lead_id: "tmuxdeck-a0000000-0000-4000-8000-000000000001".to_string(),
            members: vec![
                TeamMember {
                    session_id: "tmuxdeck-a0000000-0000-4000-8000-000000000001".to_string(),
                    role: "lead".to_string(),
                },
                TeamMember {
                    session_id: "tmuxdeck-b0000000-0000-4000-8000-000000000002".to_string(),
                    role: "worker".to_string(),
                },
            ],
            created_at: 1723680000000,
            capabilities: vec![],
        };

        let json_val = serde_json::to_value(&manifest).unwrap();
        let obj = json_val.as_object().unwrap();

        assert_eq!(obj.get("version").unwrap(), "tmuxdeck.team.v1");
        assert_eq!(obj.get("backend").unwrap(), "tmuxdeck");
        assert_eq!(
            obj.get("runId").unwrap(),
            "team_11223344-5566-4778-8899-aabbccddeeff"
        );
        assert_eq!(
            obj.get("leadId").unwrap(),
            "tmuxdeck-a0000000-0000-4000-8000-000000000001"
        );
        assert_eq!(obj.get("createdAt").unwrap(), 1723680000000u64);
        assert_eq!(
            obj.get("capabilities").unwrap().as_array().unwrap().len(),
            0
        );

        let members_arr = obj.get("members").unwrap().as_array().unwrap();
        assert_eq!(members_arr.len(), 2);
        assert_eq!(
            members_arr[0].get("sessionId").unwrap(),
            "tmuxdeck-a0000000-0000-4000-8000-000000000001"
        );
        assert_eq!(members_arr[0].get("role").unwrap(), "lead");
        assert_eq!(
            members_arr[1].get("sessionId").unwrap(),
            "tmuxdeck-b0000000-0000-4000-8000-000000000002"
        );
        assert_eq!(members_arr[1].get("role").unwrap(), "worker");

        // Strict validator checks
        assert!(validate_team_manifest(&manifest).is_ok());

        // Unknown fields rejection check
        let json_with_unknown = r#"{
            "version": "tmuxdeck.team.v1",
            "backend": "tmuxdeck",
            "runId": "team_11223344-5566-4778-8899-aabbccddeeff",
            "leadId": "tmuxdeck-a0000000-0000-4000-8000-000000000001",
            "members": [
                { "sessionId": "tmuxdeck-a0000000-0000-4000-8000-000000000001", "role": "lead" }
            ],
            "createdAt": 1723680000000,
            "capabilities": [],
            "unknownField": "bad"
        }"#;
        assert!(serde_json::from_str::<TeamManifest>(json_with_unknown).is_err());

        // Duplicate members rejection check
        let mut dup_manifest = manifest.clone();
        dup_manifest.members.push(TeamMember {
            session_id: "tmuxdeck-b0000000-0000-4000-8000-000000000002".to_string(),
            role: "worker".to_string(),
        });
        assert!(validate_team_manifest(&dup_manifest).is_err());

        // Missing lead rejection check
        let mut no_lead_manifest = manifest.clone();
        no_lead_manifest.members[0].role = "worker".to_string();
        assert!(validate_team_manifest(&no_lead_manifest).is_err());

        // Lead ID mismatch check
        let mut lead_mismatch = manifest.clone();
        lead_mismatch.lead_id = "tmuxdeck-b0000000-0000-4000-8000-000000000002".to_string();
        assert!(validate_team_manifest(&lead_mismatch).is_err());

        // Capabilities not empty check
        let mut cap_manifest = manifest.clone();
        cap_manifest.capabilities.push("admin".to_string());
        assert!(validate_team_manifest(&cap_manifest).is_err());
    }

    #[test]
    fn test_capacity_and_file_security_in_temp_dir() {
        let temp_dir = create_test_temp_dir();
        let run_id = generate_team_run_id().unwrap();
        let lead_id = generate_session_id().unwrap();

        // 64 members should pass validation
        let mut members = vec![TeamMember {
            session_id: lead_id.clone(),
            role: ROLE_LEAD.to_string(),
        }];
        for _ in 1..64 {
            members.push(TeamMember {
                session_id: generate_session_id().unwrap(),
                role: ROLE_WORKER.to_string(),
            });
        }
        let manifest_64 =
            create_team_manifest(run_id.clone(), lead_id.clone(), members.clone()).unwrap();
        assert_eq!(manifest_64.members.len(), 64);
        assert!(validate_team_manifest(&manifest_64).is_ok());

        // 65 members should fail validation
        let mut members_65 = members.clone();
        members_65.push(TeamMember {
            session_id: generate_session_id().unwrap(),
            role: ROLE_WORKER.to_string(),
        });
        let manifest_65 = TeamManifest {
            version: TEAM_MANIFEST_VERSION.to_string(),
            backend: TEAM_BACKEND.to_string(),
            run_id: run_id.clone(),
            lead_id: lead_id.clone(),
            members: members_65,
            created_at: 1723680000000,
            capabilities: vec![],
        };
        assert!(validate_team_manifest(&manifest_65).is_err());

        // Test appending past 64 fails with ERR_TEAM_CAPACITY
        write_team_manifest_in_dir(&temp_dir, &manifest_64).unwrap();
        let append_overflow = append_team_members_in_dir(
            &temp_dir,
            &run_id,
            &[TeamMember {
                session_id: generate_session_id().unwrap(),
                role: ROLE_WORKER.to_string(),
            }],
        );
        assert!(matches!(append_overflow, Err(ref s) if s.starts_with("ERR_TEAM_CAPACITY")));

        // Test symlink rejection
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let target_file = temp_dir.join(format!("{}.json", run_id));
            let link_run_id = generate_team_run_id().unwrap();
            let link_file = temp_dir.join(format!("{}.json", link_run_id));
            let _ = symlink(&target_file, &link_file);
            assert!(matches!(
                read_team_manifest_in_dir(&temp_dir, &link_run_id),
                Err(ref s) if s.contains("not_a_regular_file")
            ));
        }

        // Test file too large rejection (>64KiB)
        let large_run_id = generate_team_run_id().unwrap();
        let large_path = temp_dir.join(format!("{}.json", large_run_id));
        let large_data = vec![b' '; 65537];
        fs::write(&large_path, large_data).unwrap();
        assert!(matches!(
            read_team_manifest_in_dir(&temp_dir, &large_run_id),
            Err(ref s) if s.contains("file_too_large")
        ));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_reconcile_orphans_in_temp_dir() {
        let temp_dir = create_test_temp_dir();
        let run_id_active = generate_team_run_id().unwrap();
        let run_id_orphan = generate_team_run_id().unwrap();
        let lead_id = generate_session_id().unwrap();

        let manifest_active = create_team_manifest(
            run_id_active.clone(),
            lead_id.clone(),
            vec![TeamMember {
                session_id: lead_id.clone(),
                role: ROLE_LEAD.to_string(),
            }],
        )
        .unwrap();

        let manifest_orphan = create_team_manifest(
            run_id_orphan.clone(),
            lead_id.clone(),
            vec![TeamMember {
                session_id: lead_id.clone(),
                role: ROLE_LEAD.to_string(),
            }],
        )
        .unwrap();

        write_team_manifest_in_dir(&temp_dir, &manifest_active).unwrap();
        write_team_manifest_in_dir(&temp_dir, &manifest_orphan).unwrap();

        let tmp_file = temp_dir.join("tmp.1234.abcd.json");
        let _ = fs::write(&tmp_file, "{}");

        let mut active_ids = std::collections::HashSet::new();
        active_ids.insert(run_id_active.clone());

        let pruned = reconcile_orphan_manifests_in_dir(&temp_dir, &active_ids).unwrap();
        assert_eq!(pruned, 2); // 1 orphan + 1 tmp file

        assert!(read_team_manifest_in_dir(&temp_dir, &run_id_active).is_ok());
        assert!(read_team_manifest_in_dir(&temp_dir, &run_id_orphan).is_err());
        assert!(!tmp_file.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_build_pane_team_env() {
        let opts = PaneTeamEnvOpts {
            workspace_name: "test_ws",
            pane_index: 1,
            agent_id: "claude",
            scope_id: "3a9f0e1b2c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d",
            team_manifest_path: "/tmp/test_manifest.json",
            session_id: "tmuxdeck-a0000000-0000-4000-8000-000000000001",
            role: ROLE_LEAD,
            lead_session_id: "tmuxdeck-a0000000-0000-4000-8000-000000000001",
        };
        let envs = build_pane_team_env(&opts);
        let map: std::collections::HashMap<_, _> = envs.into_iter().collect();

        assert_eq!(
            map.get("AGENT_INTERCOM_SCOPE_ID").unwrap(),
            "3a9f0e1b2c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d"
        );
        assert_eq!(map.get("AGENT_INTERCOM_ROLE").unwrap(), "manager");
        assert_eq!(
            map.get("AGENT_INTERCOM_SESSION_ID").unwrap(),
            "tmuxdeck-a0000000-0000-4000-8000-000000000001"
        );
        assert_eq!(
            map.get("AGENT_INTERCOM_SESSION_NAME").unwrap(),
            "test_ws · Claude 01"
        );
        assert_eq!(
            map.get("CLAUDE_INTERCOM_SESSION_ID").unwrap(),
            "tmuxdeck-a0000000-0000-4000-8000-000000000001"
        );
        assert_eq!(
            map.get("CLAUDE_INTERCOM_NAME").unwrap(),
            "test_ws · Claude 01"
        );
        assert!(map.get("AGENT_INTERCOM_MANAGER_TARGET").is_none());

        let worker_opts = PaneTeamEnvOpts {
            workspace_name: "test_ws",
            pane_index: 2,
            agent_id: "pi",
            scope_id: "3a9f0e1b2c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d",
            team_manifest_path: "/tmp/test_manifest.json",
            session_id: "tmuxdeck-b0000000-0000-4000-8000-000000000002",
            role: ROLE_WORKER,
            lead_session_id: "tmuxdeck-a0000000-0000-4000-8000-000000000001",
        };
        let worker_envs = build_pane_team_env(&worker_opts);
        let worker_map: std::collections::HashMap<_, _> = worker_envs.into_iter().collect();

        assert_eq!(worker_map.get("AGENT_INTERCOM_ROLE").unwrap(), "worker");
        assert_eq!(
            worker_map.get("AGENT_INTERCOM_MANAGER_TARGET").unwrap(),
            "tmuxdeck-a0000000-0000-4000-8000-000000000001"
        );
        assert_eq!(
            worker_map.get("AGENT_INTERCOM_MANAGER_SESSION_ID").unwrap(),
            "tmuxdeck-a0000000-0000-4000-8000-000000000001"
        );
        assert_eq!(
            worker_map.get("PI_INTERCOM_SESSION_ID").unwrap(),
            "tmuxdeck-b0000000-0000-4000-8000-000000000002"
        );
        assert_eq!(
            worker_map.get("AGENT_INTERCOM_SESSION_NAME").unwrap(),
            "test_ws · Pi 02"
        );
    }

    #[test]
    fn test_all_harness_env_mappings() {
        for (agent, harness_id_key, harness_name_key) in [
            (
                "codex",
                "CODEX_INTERCOM_SESSION_ID",
                Some("CODEX_INTERCOM_NAME"),
            ),
            (
                "opencode",
                "OPENCODE_INTERCOM_SESSION_ID",
                Some("OPENCODE_INTERCOM_NAME"),
            ),
            (
                "claude",
                "CLAUDE_INTERCOM_SESSION_ID",
                Some("CLAUDE_INTERCOM_NAME"),
            ),
            (
                "grok",
                "CLAUDE_INTERCOM_SESSION_ID",
                Some("CLAUDE_INTERCOM_NAME"),
            ),
            (
                "agy",
                "CLAUDE_INTERCOM_SESSION_ID",
                Some("CLAUDE_INTERCOM_NAME"),
            ),
            ("pi", "PI_INTERCOM_SESSION_ID", None),
        ] {
            let opts = PaneTeamEnvOpts {
                workspace_name: "harness_ws",
                pane_index: 3,
                agent_id: agent,
                scope_id: "scope12345678901234567890123456789012",
                team_manifest_path: "/tmp/manifest.json",
                session_id: "tmuxdeck-c0000000-0000-4000-8000-000000000003",
                role: ROLE_WORKER,
                lead_session_id: "tmuxdeck-a0000000-0000-4000-8000-000000000001",
            };
            let envs = build_pane_team_env(&opts);
            let map: std::collections::HashMap<_, _> = envs.into_iter().collect();

            assert_eq!(
                map.get(harness_id_key).unwrap(),
                "tmuxdeck-c0000000-0000-4000-8000-000000000003"
            );
            if let Some(name_key) = harness_name_key {
                assert_eq!(
                    map.get(name_key).unwrap(),
                    &format!(
                        "harness_ws · {} 03",
                        match agent {
                            "codex" => "Codex",
                            "opencode" => "OpenCode",
                            "claude" => "Claude",
                            "grok" => "Grok Build",
                            "agy" => "AGY",
                            _ => "",
                        }
                    )
                );
            }
        }
    }

    #[test]
    fn test_manifest_path_must_be_absolute() {
        let relative_path = Path::new("relative/teams/dir");
        let run_id = generate_team_run_id().unwrap();
        let lead_id = generate_session_id().unwrap();
        let valid_manifest = create_team_manifest(
            run_id.clone(),
            lead_id.clone(),
            vec![TeamMember {
                session_id: lead_id,
                role: ROLE_LEAD.to_string(),
            }],
        )
        .unwrap();
        assert_eq!(
            write_team_manifest_in_dir(relative_path, &valid_manifest),
            Err("ERR_MANIFEST_PATH|not_absolute".to_string())
        );
        assert_eq!(
            read_team_manifest_in_dir(relative_path, &run_id),
            Err("ERR_MANIFEST_PATH|not_absolute".to_string())
        );
    }

    #[test]
    fn test_shared_mutation_transaction_and_rollback() {
        let temp_dir = create_test_temp_dir();
        let run_id = generate_team_run_id().unwrap();
        let lead_id = generate_session_id().unwrap();
        let worker1 = generate_session_id().unwrap();

        let initial_manifest = create_team_manifest(
            run_id.clone(),
            lead_id.clone(),
            vec![
                TeamMember {
                    session_id: lead_id.clone(),
                    role: ROLE_LEAD.to_string(),
                },
                TeamMember {
                    session_id: worker1.clone(),
                    role: ROLE_WORKER.to_string(),
                },
            ],
        )
        .unwrap();

        write_team_manifest_in_dir(&temp_dir, &initial_manifest).unwrap();

        // Simulate add transaction failure & restore
        let old_manifest = read_team_manifest_in_dir(&temp_dir, &run_id).unwrap();
        let worker2 = generate_session_id().unwrap();
        append_team_members_in_dir(
            &temp_dir,
            &run_id,
            &[TeamMember {
                session_id: worker2.clone(),
                role: ROLE_WORKER.to_string(),
            }],
        )
        .unwrap();

        // Simulate spawn failure -> rollback to old_manifest
        let rollback_res = write_team_manifest_in_dir(&temp_dir, &old_manifest);
        assert!(rollback_res.is_ok());

        let restored = read_team_manifest_in_dir(&temp_dir, &run_id).unwrap();
        assert_eq!(restored.members.len(), 2);
        assert!(!restored.members.iter().any(|m| m.session_id == worker2));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_team_manifest_path_validation() {
        assert!(team_manifest_path("team_11223344-5566-4778-8899-aabbccddeeff").is_ok());
        assert!(team_manifest_path("invalid_run_id").is_err());
        assert!(team_manifest_path("team_not-a-uuid").is_err());
        assert!(team_manifest_path("../../../etc/passwd").is_err());
    }

    #[test]
    fn test_dir_symlink_and_0755_rejection() {
        let temp_dir = create_test_temp_dir();
        let run_id = generate_team_run_id().unwrap();
        let lead_id = generate_session_id().unwrap();
        let manifest = create_team_manifest(
            run_id.clone(),
            lead_id.clone(),
            vec![TeamMember {
                session_id: lead_id.clone(),
                role: ROLE_LEAD.to_string(),
            }],
        )
        .unwrap();

        // Test normal write & read works
        assert!(write_team_manifest_in_dir(&temp_dir, &manifest).is_ok());
        assert!(read_team_manifest_in_dir(&temp_dir, &run_id).is_ok());

        #[cfg(unix)]
        {
            use std::os::unix::fs::{symlink, PermissionsExt};

            // Insecure dir permissions (0755) rejection on read
            fs::set_permissions(&temp_dir, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(matches!(
                read_team_manifest_in_dir(&temp_dir, &run_id),
                Err(ref s) if s.contains("insecure_permissions")
            ));

            // Restore 0700
            fs::set_permissions(&temp_dir, fs::Permissions::from_mode(0o700)).unwrap();

            // Directory symlink rejection on both read and write
            let symlink_dir = temp_dir
                .parent()
                .unwrap()
                .join(format!("symlink-teams-{}", rand_nonce()));
            let _ = symlink(&temp_dir, &symlink_dir);
            assert!(matches!(
                read_team_manifest_in_dir(&symlink_dir, &run_id),
                Err(ref s) if s.contains("not_a_directory")
            ));
            assert!(matches!(
                write_team_manifest_in_dir(&symlink_dir, &manifest),
                Err(ref s) if s.contains("not_a_directory")
            ));
            let _ = fs::remove_file(&symlink_dir);
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_symlink_chmod_target_unchanged() {
        let temp_dir = create_test_temp_dir();
        let target_dir = temp_dir.join("target_dir");
        let _ = fs::create_dir_all(&target_dir);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{symlink, PermissionsExt};
            fs::set_permissions(&target_dir, fs::Permissions::from_mode(0o755)).unwrap();

            let symlink_dir = temp_dir.join("symlink_dir");
            let _ = symlink(&target_dir, &symlink_dir);

            let run_id = generate_team_run_id().unwrap();
            let lead_id = generate_session_id().unwrap();
            let manifest = create_team_manifest(
                run_id.clone(),
                lead_id.clone(),
                vec![TeamMember {
                    session_id: lead_id.clone(),
                    role: ROLE_LEAD.to_string(),
                }],
            )
            .unwrap();

            // Attempting to write through symlink must fail
            let res = write_team_manifest_in_dir(&symlink_dir, &manifest);
            assert!(res.is_err());

            // Target directory permissions must NOT have been changed by writer!
            let target_meta = fs::symlink_metadata(&target_dir).unwrap();
            assert_eq!(target_meta.permissions().mode() & 0o777, 0o755);

            // Attempting to delete through symlink must also fail
            assert!(delete_team_manifest_in_dir(&symlink_dir, &run_id).is_err());
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_validate_live_team_members() {
        let run_id = generate_team_run_id().unwrap();
        let lead_id = generate_session_id().unwrap();
        let worker_id = generate_session_id().unwrap();
        let other_worker = generate_session_id().unwrap();

        let manifest = create_team_manifest(
            run_id,
            lead_id.clone(),
            vec![
                TeamMember {
                    session_id: lead_id.clone(),
                    role: ROLE_LEAD.to_string(),
                },
                TeamMember {
                    session_id: worker_id.clone(),
                    role: ROLE_WORKER.to_string(),
                },
            ],
        )
        .unwrap();

        // Valid live members
        assert!(validate_live_team_members(
            &manifest,
            &[
                LiveTeamMemberInfo {
                    session_id: lead_id.clone(),
                    role: ROLE_LEAD.to_string(),
                    manager_target: "".to_string(),
                },
                LiveTeamMemberInfo {
                    session_id: worker_id.clone(),
                    role: ROLE_WORKER.to_string(),
                    manager_target: lead_id.clone(),
                }
            ]
        )
        .is_ok());

        // Subset of live members (e.g. 1 offline worker) is OK
        assert!(validate_live_team_members(
            &manifest,
            &[LiveTeamMemberInfo {
                session_id: lead_id.clone(),
                role: ROLE_LEAD.to_string(),
                manager_target: "".to_string(),
            }]
        )
        .is_ok());

        // Worker with wrong manager target -> conflict
        assert!(validate_live_team_members(
            &manifest,
            &[
                LiveTeamMemberInfo {
                    session_id: lead_id.clone(),
                    role: ROLE_LEAD.to_string(),
                    manager_target: "".to_string(),
                },
                LiveTeamMemberInfo {
                    session_id: worker_id.clone(),
                    role: ROLE_WORKER.to_string(),
                    manager_target: other_worker.clone(),
                }
            ]
        )
        .is_err());

        // Lead with non-empty manager target -> conflict
        assert!(validate_live_team_members(
            &manifest,
            &[LiveTeamMemberInfo {
                session_id: lead_id.clone(),
                role: ROLE_LEAD.to_string(),
                manager_target: worker_id.clone(),
            }]
        )
        .is_err());

        // Empty session ID / role -> conflict (Item 2 test)
        assert!(validate_live_team_members(
            &manifest,
            &[LiveTeamMemberInfo {
                session_id: "".to_string(),
                role: "".to_string(),
                manager_target: "".to_string(),
            }]
        )
        .is_err());

        // Unknown session ID -> conflict
        assert!(validate_live_team_members(
            &manifest,
            &[
                LiveTeamMemberInfo {
                    session_id: lead_id.clone(),
                    role: ROLE_LEAD.to_string(),
                    manager_target: "".to_string(),
                },
                LiveTeamMemberInfo {
                    session_id: other_worker.clone(),
                    role: ROLE_WORKER.to_string(),
                    manager_target: lead_id.clone(),
                }
            ]
        )
        .is_err());

        // Role mismatch -> conflict
        assert!(validate_live_team_members(
            &manifest,
            &[
                LiveTeamMemberInfo {
                    session_id: lead_id.clone(),
                    role: ROLE_WORKER.to_string(),
                    manager_target: lead_id.clone(),
                },
                LiveTeamMemberInfo {
                    session_id: worker_id.clone(),
                    role: ROLE_LEAD.to_string(),
                    manager_target: "".to_string(),
                }
            ]
        )
        .is_err());

        // Duplicate session ID in live list -> conflict
        assert!(validate_live_team_members(
            &manifest,
            &[
                LiveTeamMemberInfo {
                    session_id: lead_id.clone(),
                    role: ROLE_LEAD.to_string(),
                    manager_target: "".to_string(),
                },
                LiveTeamMemberInfo {
                    session_id: lead_id.clone(),
                    role: ROLE_LEAD.to_string(),
                    manager_target: "".to_string(),
                }
            ]
        )
        .is_err());
    }

    #[test]
    fn test_platform_fail_closed_static_and_posix_guard() {
        // Gating helper must return true on macOS only and false on all non-macOS platforms
        if cfg!(target_os = "macos") {
            assert!(is_team_supported());
            assert!(team_manifest_path("team_11223344-5566-4778-8899-aabbccddeeff").is_ok());
        } else {
            assert!(!is_team_supported());
            assert_eq!(
                team_manifest_path("team_11223344-5566-4778-8899-aabbccddeeff"),
                Err(ERR_TEAM_UNAVAILABLE.to_string())
            );
        }
    }
}
