pub const SCOPE_ENV_VAR: &str = "AGENT_INTERCOM_SCOPE_ID";

pub fn validate_scope_id(scope: &str) -> Result<&str, String> {
    let len = scope.len();
    if !(16..=128).contains(&len) {
        return Err("ERR_SCOPE_UNAVAILABLE".to_string());
    }
    if !scope
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("ERR_SCOPE_UNAVAILABLE".to_string());
    }
    Ok(scope)
}

pub fn generate_workspace_scope_id() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("ERR_SCOPE_GEN_FAILED|{}", e))?;
    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    validate_scope_id(&hex).map_err(|_| "ERR_SCOPE_GEN_FAILED".to_string())?;
    Ok(hex)
}

pub fn parse_environment_stdout(stdout: &str, var_name: &str) -> Result<Option<String>, String> {
    let unset_marker = format!("-{}", var_name);
    let prefix = format!("{}=", var_name);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed == unset_marker {
            return Ok(None);
        }
        if let Some(val) = trimmed.strip_prefix(&prefix) {
            let val = val.trim();
            if val.is_empty() {
                return Ok(None);
            }
            validate_scope_id(val)?;
            return Ok(Some(val.to_string()));
        }
    }
    Ok(None)
}

pub fn read_session_scope(target: &str) -> Result<Option<String>, String> {
    let output = crate::tmux::run_tmux(&["show-environment", "-t", target, SCOPE_ENV_VAR])
        .map_err(|e| format!("ERR_SCOPE_UNAVAILABLE|{}", e))?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_environment_stdout(&stdout, SCOPE_ENV_VAR)
}

pub fn read_targets_scope(targets: &[&str]) -> Result<String, String> {
    if targets.is_empty() {
        return Err("ERR_SCOPE_UNAVAILABLE".to_string());
    }
    let mut unified_scope: Option<String> = None;
    for target in targets {
        let scope =
            read_session_scope(target)?.ok_or_else(|| "ERR_SCOPE_UNAVAILABLE".to_string())?;
        match &unified_scope {
            None => unified_scope = Some(scope),
            Some(existing) if existing != &scope => {
                return Err("ERR_SCOPE_CONFLICT".to_string());
            }
            _ => {}
        }
    }
    unified_scope.ok_or_else(|| "ERR_SCOPE_UNAVAILABLE".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_workspace_scope_id() {
        let id1 = generate_workspace_scope_id().unwrap();
        let id2 = generate_workspace_scope_id().unwrap();
        assert_eq!(id1.len(), 48);
        assert_eq!(id2.len(), 48);
        assert_ne!(id1, id2);
        assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(validate_scope_id(&id1).is_ok());
    }

    #[test]
    fn test_validate_scope_id_bounds_and_charset() {
        // 16 chars (min valid)
        assert!(validate_scope_id("1234567890abcdef").is_ok());
        // 15 chars (too short)
        assert!(validate_scope_id("1234567890abcde").is_err());
        // 128 chars (max valid)
        let exact_128 = "a".repeat(128);
        assert!(validate_scope_id(&exact_128).is_ok());
        // 129 chars (too long)
        let exact_129 = "a".repeat(129);
        assert!(validate_scope_id(&exact_129).is_err());
        // Valid chars: alphanumeric, _, -
        assert!(validate_scope_id("scope_with-123_valid").is_ok());
        // Valid starting with -
        assert!(validate_scope_id("-leading_hyphen_scope123").is_ok());
        // Invalid chars: space, @, /, :, .
        assert!(validate_scope_id("scope with space123").is_err());
        assert!(validate_scope_id("scope@invalid!1234").is_err());
        assert!(validate_scope_id("scope/invalid/1234").is_err());
        assert!(validate_scope_id("scope:invalid:1234").is_err());
    }

    #[test]
    fn test_parse_environment_stdout() {
        // Standard matching variable
        let out1 = "AGENT_INTERCOM_SCOPE_ID=tdscope_0123456789abcdef\n";
        assert_eq!(
            parse_environment_stdout(out1, SCOPE_ENV_VAR).unwrap(),
            Some("tdscope_0123456789abcdef".to_string())
        );

        // Leading hyphen scope
        let out_hyphen = "AGENT_INTERCOM_SCOPE_ID=-tdscope_0123456789abcdef\n";
        assert_eq!(
            parse_environment_stdout(out_hyphen, SCOPE_ENV_VAR).unwrap(),
            Some("-tdscope_0123456789abcdef".to_string())
        );

        // Unset variable marker
        let out_unset = "-AGENT_INTERCOM_SCOPE_ID\n";
        assert_eq!(
            parse_environment_stdout(out_unset, SCOPE_ENV_VAR).unwrap(),
            None
        );

        // Empty value
        let out_empty = "AGENT_INTERCOM_SCOPE_ID=\n";
        assert_eq!(
            parse_environment_stdout(out_empty, SCOPE_ENV_VAR).unwrap(),
            None
        );

        // Target variable not present in output
        let out_other = "OTHER_VAR=value_1234567890123456\n";
        assert_eq!(
            parse_environment_stdout(out_other, SCOPE_ENV_VAR).unwrap(),
            None
        );

        // Empty string
        assert_eq!(parse_environment_stdout("", SCOPE_ENV_VAR).unwrap(), None);

        // Invalid scope ID value triggers Err
        let out_invalid = "AGENT_INTERCOM_SCOPE_ID=short\n";
        assert!(parse_environment_stdout(out_invalid, SCOPE_ENV_VAR).is_err());
    }
}
