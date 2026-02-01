use std::path::PathBuf;

use anyhow::Result;
use kanata_types::KanataConfig;

/// Return the path to the Kanata configuration file.
///
/// Typically `~/.config/kanata/config.yaml` on Linux,
/// `~/Library/Application Support/kanata/config.yaml` on macOS,
/// or `%APPDATA%\kanata\config.yaml` on Windows.
pub fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("kanata").join("config.yaml")
}

/// Check whether this is the first run (no config file exists).
pub fn is_first_run() -> bool {
    !config_path().exists()
}

/// Load configuration from disk, falling back to defaults if no file exists.
pub fn load_config() -> Result<KanataConfig> {
    let path = config_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let config: KanataConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    } else {
        Ok(KanataConfig::default())
    }
}

/// Save configuration to disk, creating parent directories if needed.
pub fn save_config(config: &KanataConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_yaml::to_string(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Validate configuration and return a warning message if there are issues.
pub fn validate_config(config: &KanataConfig) -> Option<String> {
    let mut warnings = Vec::new();

    // Check trust level range
    if config.trust_level > 4 {
        warnings.push(format!(
            "trust_level {} is out of range (expected 1-4), using as-is",
            config.trust_level
        ));
    }

    // Check for empty API keys
    if config.api_keys.is_empty() {
        warnings.push("No API keys configured. Set one via /config or config.yaml.".to_string());
    }

    for (provider, key) in &config.api_keys {
        if key.is_empty() {
            warnings.push(format!("API key for '{provider}' is empty."));
        }
    }

    if warnings.is_empty() { None } else { Some(warnings.join("\n")) }
}

/// Mask an API key for display: show first 4 and last 4 characters.
pub fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    let prefix = &key[..4];
    let suffix = &key[key.len() - 4..];
    format!("{prefix}...{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_is_not_empty() {
        let path = config_path();
        assert!(path.to_str().is_some_and(|s| !s.is_empty()));
    }

    #[test]
    fn test_load_config_defaults() {
        // Falls back to defaults when no file exists
        let cfg = load_config().expect("should load");
        assert_eq!(cfg.default_model, "claude-sonnet-4");
        assert_eq!(cfg.trust_level, 1);
    }

    #[test]
    fn test_validate_config_no_keys() {
        let cfg = KanataConfig::default();
        let warning = validate_config(&cfg);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("No API keys"));
    }

    #[test]
    fn test_validate_config_high_trust() {
        let mut cfg = KanataConfig { trust_level: 10, ..KanataConfig::default() };
        cfg.api_keys.insert("test".to_string(), "key".to_string());
        let warning = validate_config(&cfg);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("out of range"));
    }

    #[test]
    fn test_validate_config_ok() {
        let mut cfg = KanataConfig::default();
        cfg.api_keys.insert("anthropic".to_string(), "sk-test-key".to_string());
        let warning = validate_config(&cfg);
        assert!(warning.is_none());
    }

    #[test]
    fn test_mask_api_key() {
        assert_eq!(mask_api_key("sk-1234567890abcdef"), "sk-1...cdef");
        assert_eq!(mask_api_key("short"), "****");
        assert_eq!(mask_api_key(""), "****");
    }
}
