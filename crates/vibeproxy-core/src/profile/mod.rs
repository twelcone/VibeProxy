//! Profile data model, on-disk store, and path resolution.

pub mod account_meta;
pub mod paths;
pub mod store;

pub use store::{Config, Profile, Settings};

/// Adopt an existing Claude login at `config_dir` as a new profile: read its identity, reject a
/// dir with no login, de-dupe by config-dir or org, and make it active if it is the first profile.
/// Returns the created profile. The desktop app calls this then refreshes the tray; the CLI calls it
/// directly. Contains no GUI concerns.
pub fn adopt(label: String, config_dir: &str) -> Result<Profile, String> {
    let config_dir = paths::expand_tilde(config_dir);
    let status = account_meta::fetch(std::path::Path::new(&config_dir))?;
    if !status.logged_in {
        return Err("no logged-in Claude account at that config dir".to_string());
    }
    let cfg = store::load();
    if let Some(existing) = cfg.profiles.iter().find(|p| {
        p.config_dir == config_dir || (p.org_id.is_some() && p.org_id == status.org_id)
    }) {
        return Err(format!("that account is already added as \"{}\"", existing.label));
    }
    let is_first = cfg.profiles.is_empty();
    let profile = Profile {
        id: store::new_id(),
        label,
        config_dir,
        email: status.email,
        org_id: status.org_id,
        subscription_type: status.subscription_type,
        priority: cfg.profiles.len() as i32,
        created_at: String::new(),
    };
    store::add_profile(profile.clone()).map_err(|e| e.to_string())?;
    if is_first {
        crate::switch::activate_profile(&profile.id)?;
    }
    Ok(profile)
}

/// Given a profile and a freshly-read auth status, return an updated profile **iff** the account
/// identity actually changed. Pure — no I/O, no persistence — so it is unit-testable. A logged-out
/// status returns `None`: blanking a profile's identity would lose the record of which account it is
/// supposed to be, which is exactly what a re-login restores.
pub fn apply_identity(p: &Profile, status: &account_meta::AuthStatus) -> Option<Profile> {
    if !status.logged_in {
        return None;
    }
    if status.email == p.email
        && status.org_id == p.org_id
        && status.subscription_type == p.subscription_type
    {
        return None;
    }
    let mut updated = p.clone();
    updated.email = status.email.clone();
    updated.org_id = status.org_id.clone();
    updated.subscription_type = status.subscription_type.clone();
    Some(updated)
}

/// Re-read a profile's identity from the official client and diff it. Blocking (spawns `claude`);
/// callers wrap it in `spawn_blocking`. Returns an updated profile only on a real change.
pub fn refresh_identity(p: &Profile) -> Option<Profile> {
    let status = account_meta::fetch(std::path::Path::new(&p.config_dir)).ok()?;
    apply_identity(p, &status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use account_meta::AuthStatus;

    fn profile() -> Profile {
        Profile {
            id: "p1".into(),
            label: "Work".into(),
            config_dir: "/tmp/p1".into(),
            email: Some("old@example.com".into()),
            org_id: Some("org-old".into()),
            subscription_type: Some("max".into()),
            priority: 0,
            created_at: String::new(),
        }
    }
    fn status(logged_in: bool, email: &str, org: &str) -> AuthStatus {
        AuthStatus {
            logged_in,
            email: Some(email.into()),
            org_id: Some(org.into()),
            subscription_type: Some("max".into()),
        }
    }

    #[test]
    fn unchanged_identity_returns_none() {
        assert!(apply_identity(&profile(), &status(true, "old@example.com", "org-old")).is_none());
    }

    #[test]
    fn changed_identity_returns_the_updated_profile() {
        let got = apply_identity(&profile(), &status(true, "new@example.com", "org-new")).unwrap();
        assert_eq!(got.email.as_deref(), Some("new@example.com"));
        assert_eq!(got.org_id.as_deref(), Some("org-new"));
        assert_eq!(got.id, "p1"); // identity of the record itself is untouched
    }

    #[test]
    fn logged_out_keeps_the_last_known_identity() {
        assert!(apply_identity(&profile(), &status(false, "whatever", "whatever")).is_none());
    }
}
