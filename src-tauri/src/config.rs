use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CustomAgent {
    pub name: String,
    pub command: String,
}

fn default_panel_bypass_permissions() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub default_terminal: String,
    pub default_agent: String,
    pub default_panes: u8,
    pub custom_agent: Option<CustomAgent>,
    pub recent_dirs: Vec<String>,
    #[serde(default)]
    pub use_standard_claude: bool,
    #[serde(default = "default_panel_bypass_permissions")]
    pub panel_bypass_permissions: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_terminal: String::new(),
            default_agent: "pi".to_string(),
            default_panes: 4,
            custom_agent: None,
            recent_dirs: Vec::new(),
            use_standard_claude: false,
            panel_bypass_permissions: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn missing_panel_bypass_field_defaults_to_enabled() {
        let config: Config = serde_json::from_str(
            r#"{"default_terminal":"ghostty","default_agent":"pi","default_panes":4,"custom_agent":null,"recent_dirs":[]}"#,
        )
        .unwrap();
        assert!(config.panel_bypass_permissions);
    }

    #[test]
    fn panel_bypass_field_round_trips_disabled() {
        let mut config = Config::default();
        config.panel_bypass_permissions = false;
        let encoded = serde_json::to_string(&config).unwrap();
        let decoded: Config = serde_json::from_str(&encoded).unwrap();
        assert!(!decoded.panel_bypass_permissions);
    }
}

pub fn get_config_dir() -> std::path::PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        config_dir.join("tmuxdeck")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home)
            .join(".config")
            .join("tmuxdeck")
    }
}

pub fn get_config_path() -> std::path::PathBuf {
    get_config_dir().join("config.json")
}

#[tauri::command]
pub fn load_config() -> Config {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(cfg) = serde_json::from_str::<Config>(&content) {
                return cfg;
            }
        }
    }
    Config::default()
}

#[tauri::command]
pub fn save_config(config: Config) -> Result<(), String> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json =
        serde_json::to_string_pretty(&config).map_err(|e| format!("ERR_CONFIG_SAVE|{}", e))?;
    std::fs::write(path, json).map_err(|e| format!("ERR_CONFIG_SAVE|{}", e))?;
    crate::registry::invalidate_environment_cache();
    Ok(())
}
