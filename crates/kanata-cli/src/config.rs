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
#[allow(dead_code)]
pub fn save_config(config: &KanataConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_yaml::to_string(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}
