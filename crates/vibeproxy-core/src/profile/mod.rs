//! Profile data model, on-disk store, and path resolution.

pub mod account_meta;
pub mod paths;
pub mod store;

pub use store::{Config, Profile, Settings};

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
