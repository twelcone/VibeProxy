//! Load/save `config.json` and the profile/settings data model.

use super::paths;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write};

/// Bump when the on-disk schema changes in a breaking way.
const SCHEMA_VERSION: u32 = 1;

/// One switchable Claude Code account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Stable random id; also the dir name for VibeProxy-created profiles.
    pub id: String,
    /// User-facing name.
    pub label: String,
    /// Absolute path to this profile's `CLAUDE_CONFIG_DIR`. May be the default `~/.claude`,
    /// an adopted dir (e.g. `~/vp-spike`), or `~/.vibeproxy/profiles/<id>`. The macOS Keychain
    /// service that holds the token is derived from THIS path, so it must never change once set.
    pub config_dir: String,
    /// Account identity/tier, filled from `claude auth status --json` (Phase 2/3).
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub org_id: Option<String>,
    #[serde(default)]
    pub subscription_type: Option<String>,
    /// Auto-switch preference order (lower = preferred).
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub created_at: String,
}

/// User-tunable behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub auto_switch_enabled: bool,
    /// Switch when the active profile crosses this utilization percent.
    pub threshold_pct: u8,
    /// How often to poll the active profile's usage (seconds).
    pub poll_interval_secs: u64,
    /// Anti-flap window after an auto-switch (seconds).
    pub cooldown_secs: u64,
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_switch_enabled: true,
            threshold_pct: 90,
            poll_interval_secs: 120,
            cooldown_secs: 300,
            launch_at_login: false,
        }
    }
}

/// Root persisted document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub schema_version: u32,
    #[serde(default)]
    pub active_profile_id: Option<String>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub settings: Settings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            active_profile_id: None,
            profiles: Vec::new(),
            settings: Settings::default(),
        }
    }
}

/// Load config, returning defaults if the file is missing or unreadable.
/// Tolerant by design — a corrupt file should not brick the menubar.
pub fn load() -> Config {
    match fs::read_to_string(paths::config_path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

/// Ensure `~/.vibeproxy` (+ `profiles/`) exists and `config.json` is present.
pub fn ensure_initialized() -> std::io::Result<()> {
    fs::create_dir_all(paths::vibeproxy_dir())?;
    fs::create_dir_all(paths::profiles_dir())?;
    if !paths::config_path().exists() {
        save(&Config::default())?;
    }
    Ok(())
}

/// Atomically persist config (temp file + rename, so a crash never truncates config.json).
pub fn save(cfg: &Config) -> std::io::Result<()> {
    fs::create_dir_all(paths::vibeproxy_dir())?;
    let path = paths::config_path();
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrips_through_json() {
        let cfg = Config::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back.schema_version, SCHEMA_VERSION);
        assert!(back.profiles.is_empty());
        assert_eq!(back.settings.threshold_pct, 90);
    }

    #[test]
    fn camel_case_keys_are_used() {
        let json = serde_json::to_string(&Config::default()).unwrap();
        assert!(json.contains("schemaVersion"));
        assert!(json.contains("activeProfileId"));
    }
}
