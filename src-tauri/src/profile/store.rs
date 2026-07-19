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
    /// Stable random id for this profile record (distinct from the config dir's folder name).
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

/// Serializes all read-modify-write mutations so concurrent writers (a tray-thread switch racing a
/// command) can't clobber each other. Readers use `load()` directly — `save()` is atomic, so a
/// reader always sees a whole old-or-new file.
static CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Load → mutate → save, under the config lock.
fn with_config<R>(f: impl FnOnce(&mut Config) -> R) -> std::io::Result<R> {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut c = load();
    let r = f(&mut c);
    save(&c)?;
    Ok(r)
}

/// Generate a stable, unique profile id.
pub fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("p_{nanos:x}")
}

/// Look up a profile by id.
pub fn find(id: &str) -> Option<Profile> {
    load().profiles.into_iter().find(|p| p.id == id)
}

/// Append a profile and persist.
pub fn add_profile(p: Profile) -> std::io::Result<()> {
    with_config(|c| c.profiles.push(p))
}

/// Replace a profile in place by id (preserves order and the active selection). No-op if absent.
pub fn update_profile(updated: Profile) -> std::io::Result<()> {
    with_config(|c| {
        if let Some(slot) = c.profiles.iter_mut().find(|p| p.id == updated.id) {
            *slot = updated;
        }
    })
}

/// Remove a profile; clears `active` if it pointed at that profile.
pub fn remove_profile(id: &str) -> std::io::Result<()> {
    with_config(|c| {
        c.profiles.retain(|p| p.id != id);
        if c.active_profile_id.as_deref() == Some(id) {
            c.active_profile_id = None;
        }
    })
}

/// Set the active profile id and persist.
pub fn set_active_profile_id(id: &str) -> std::io::Result<()> {
    with_config(|c| c.active_profile_id = Some(id.to_string()))
}

/// Reorder profiles to match `order` (ids); unknown/missing ids fall to the end. Renumbers priority.
pub fn reorder(order: &[String]) -> std::io::Result<()> {
    with_config(|c| {
        c.profiles
            .sort_by_key(|p| order.iter().position(|id| id == &p.id).unwrap_or(usize::MAX));
        for (i, p) in c.profiles.iter_mut().enumerate() {
            p.priority = i as i32;
        }
    })
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

/// State-transition tests over a real (temp-dir) config, guarding the delete/refresh/reorder paths
/// that previously had bugs. Serialized via a mutex since they share the `VIBEPROXY_DIR` env + file.
#[cfg(test)]
mod io_tests {
    use super::*;
    use std::sync::Mutex;

    static SERIAL: Mutex<()> = Mutex::new(());

    fn with_temp(f: impl FnOnce()) {
        let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = std::env::temp_dir().join(new_id());
        std::env::set_var("VIBEPROXY_DIR", &tmp);
        ensure_initialized().unwrap();
        f();
        std::env::remove_var("VIBEPROXY_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    fn prof(id: &str) -> Profile {
        Profile {
            id: id.into(),
            label: id.into(),
            config_dir: format!("/tmp/{id}"),
            email: None,
            org_id: None,
            subscription_type: None,
            priority: 0,
            created_at: String::new(),
        }
    }

    #[test]
    fn remove_clears_active_when_removing_active() {
        with_temp(|| {
            add_profile(prof("a")).unwrap();
            add_profile(prof("b")).unwrap();
            set_active_profile_id("a").unwrap();
            remove_profile("a").unwrap();
            let c = load();
            assert_eq!(c.active_profile_id, None);
            assert_eq!(c.profiles.len(), 1);
            assert_eq!(c.profiles[0].id, "b");
        });
    }

    #[test]
    fn update_preserves_active_and_position() {
        with_temp(|| {
            add_profile(prof("a")).unwrap();
            add_profile(prof("b")).unwrap();
            set_active_profile_id("a").unwrap();
            let mut a = find("a").unwrap();
            a.email = Some("x@y.z".into());
            update_profile(a).unwrap();
            let c = load();
            assert_eq!(c.active_profile_id.as_deref(), Some("a")); // not deactivated
            assert_eq!(c.profiles[0].id, "a"); // still first, not moved to end
            assert_eq!(c.profiles[0].email.as_deref(), Some("x@y.z"));
        });
    }

    #[test]
    fn reorder_renumbers_priority() {
        with_temp(|| {
            add_profile(prof("a")).unwrap();
            add_profile(prof("b")).unwrap();
            reorder(&["b".into(), "a".into()]).unwrap();
            let c = load();
            assert_eq!(c.profiles[0].id, "b");
            assert_eq!(c.profiles[0].priority, 0);
            assert_eq!(c.profiles[1].priority, 1);
        });
    }
}
