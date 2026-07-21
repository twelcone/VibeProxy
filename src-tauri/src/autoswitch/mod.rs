//! Pre-emptive auto-switch: when the active account crosses the usage threshold, move to the
//! freshest eligible account before the user hits a failed turn. Called each poll cycle.
//!
//! No live 429 is visible without a proxy, so this is threshold-based and takes effect on the next
//! `claude` launch (the notification offers a relaunch). Numeric thresholds only — never `severity`
//! strings, whose values near 100% are unobserved.

use crate::profile::store::Config;
use crate::usage::{ProfileUsage, UsageStatus};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

/// A candidate must be this far below the threshold to be eligible (hysteresis against flip-flop).
const HYSTERESIS: f32 = 5.0;

/// The outcome of evaluating the current usage state.
#[derive(Debug, PartialEq)]
pub enum Decision {
    /// Active profile is fine (or auto-switch disabled / cooling down) — do nothing.
    None,
    /// Active is over threshold and a fresher account exists — switch to it.
    Switch { target_id: String, target_label: String, from_label: String, pct: i32 },
    /// Active is over threshold but nothing else has headroom — warn the user.
    Blocked { from_label: String, pct: i32 },
}

/// Pure decision: what should happen given config + usage. `cooling` = within the anti-flap window.
pub fn decide(cfg: &Config, usage: &HashMap<String, ProfileUsage>, cooling: bool) -> Decision {
    if !cfg.settings.auto_switch_enabled || cooling {
        return Decision::None;
    }
    let Some(active_id) = cfg.active_profile_id.as_deref() else {
        return Decision::None;
    };
    let Some(active) = usage.get(active_id) else {
        return Decision::None;
    };
    if active.status != UsageStatus::Ok {
        return Decision::None;
    }

    let thr = cfg.settings.threshold_pct as f32;
    let over = active.five_hour_pct.is_some_and(|p| p >= thr) || active.weekly_pct.is_some_and(|p| p >= thr);
    if !over {
        return Decision::None;
    }

    // Eligible = a different profile with fresh Ok usage, 5-hour below (threshold - hysteresis),
    // and weekly not itself at/over threshold. Prefer lowest priority, then lowest 5-hour usage.
    // A profile on the SAME account as the active one is not a fallback — switching to it changes
    // nothing and the quota it reports is the quota we are already out of. Identity drifts when a
    // user logs out and back in outside VibeProxy, so two profiles can silently converge on one
    // account; `adopt_profile` only de-dupes at add time, which is too early to catch that.
    let active_org = cfg
        .profiles
        .iter()
        .find(|p| p.id == active_id)
        .and_then(|p| p.org_id.clone());

    let mut candidates: Vec<_> = cfg
        .profiles
        .iter()
        .filter(|p| p.id != active_id)
        .filter(|p| match (&active_org, &p.org_id) {
            (Some(a), Some(b)) => a != b,
            _ => true, // unknown identity on either side — fall back to usage-based judgement
        })
        .filter(|p| match usage.get(&p.id) {
            Some(u) if u.status == UsageStatus::Ok => {
                u.five_hour_pct.is_some_and(|v| v < thr - HYSTERESIS)
                    && u.weekly_pct.map_or(true, |v| v < thr)
            }
            _ => false,
        })
        .collect();
    candidates.sort_by(|a, b| {
        a.priority.cmp(&b.priority).then_with(|| {
            let av = usage.get(&a.id).and_then(|u| u.five_hour_pct).unwrap_or(100.0);
            let bv = usage.get(&b.id).and_then(|u| u.five_hour_pct).unwrap_or(100.0);
            av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    let from_label = label_of(cfg, active_id);
    let pct = active.five_hour_pct.unwrap_or(0.0).round() as i32;
    match candidates.first() {
        Some(t) => Decision::Switch {
            target_id: t.id.clone(),
            target_label: t.label.clone(),
            from_label,
            pct,
        },
        None => Decision::Blocked { from_label, pct },
    }
}

/// Evaluate and act: switch + notify, or warn. `cooldown_until` guards against rapid re-switching.
pub fn maybe_switch(
    app: &AppHandle,
    cfg: &Config,
    usage: &HashMap<String, ProfileUsage>,
    cooldown_until: &mut Option<Instant>,
) {
    let cooling = matches!(cooldown_until, Some(t) if Instant::now() < *t);
    match decide(cfg, usage, cooling) {
        Decision::None => {}
        Decision::Switch { target_id, target_label, from_label, pct } => {
            if crate::activate(app, &target_id).is_ok() {
                *cooldown_until = Some(Instant::now() + Duration::from_secs(cfg.settings.cooldown_secs));

                // The path file only affects newly launched terminals, which is no help to the
                // session that just hit the limit. When enabled, also move the credentials into
                // the dir that session is already using. Always write the path file too, so a new
                // terminal and a hot-swapped session never disagree about the active account.
                let hot = if cfg.settings.hot_swap_enabled {
                    hot_swap_active_session(cfg, &target_id, &target_label)
                } else {
                    None
                };
                let tail = match hot {
                    Some(true) => "Your running session switched too.",
                    Some(false) => "Couldn't switch the running session — relaunch Claude Code to use it.",
                    None => "Relaunch Claude Code to use it.",
                };
                notify(
                    app,
                    &format!("Switched to {target_label}"),
                    &format!("{from_label} reached {pct}% — moved to a fresher account. {tail}"),
                );
                let _ = app.emit(
                    "auto-switched",
                    serde_json::json!({ "from": from_label, "to": target_label, "pct": pct }),
                );
            }
        }
        Decision::Blocked { from_label, pct } => {
            notify(
                app,
                "All accounts near their limit",
                &format!("{from_label} is at {pct}% and no other account has headroom. Try again after a reset."),
            );
            *cooldown_until = Some(Instant::now() + Duration::from_secs(cfg.settings.cooldown_secs));
            let _ = app.emit("auto-switch-blocked", serde_json::json!({ "active": from_label, "pct": pct }));
        }
    }
}

/// Swap the target account's credentials into the dir the *previously active* profile uses, which
/// is the dir any running session is bound to. Returns None when there is nothing to swap.
fn hot_swap_active_session(cfg: &Config, target_id: &str, target_label: &str) -> Option<bool> {
    let previous = cfg.active_profile_id.as_deref()?;
    if previous == target_id {
        return None;
    }
    let dir_of = |id: &str| cfg.profiles.iter().find(|p| p.id == id).map(|p| p.config_dir.clone());
    let target_dir = dir_of(target_id)?;
    let session_dir = dir_of(previous)?;

    match crate::switch::hotswap::swap_into(
        std::path::Path::new(&session_dir),
        std::path::Path::new(&target_dir),
        target_id,
        target_label,
    ) {
        Ok(()) => Some(true),
        // Never surface the underlying error text: it can carry keychain detail.
        Err(_) => Some(false),
    }
}

fn label_of(cfg: &Config, id: &str) -> String {
    cfg.profiles
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.label.clone())
        .unwrap_or_default()
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{store::Settings, Profile};

    fn profile(id: &str, priority: i32) -> Profile {
        Profile {
            id: id.into(),
            label: id.to_uppercase(),
            config_dir: format!("/tmp/{id}"),
            email: None,
            org_id: None,
            subscription_type: None,
            priority,
            created_at: String::new(),
        }
    }

    fn cfg(active: &str, threshold: u8, enabled: bool) -> Config {
        Config {
            schema_version: 1,
            active_profile_id: Some(active.into()),
            profiles: vec![profile("a", 0), profile("b", 1)],
            settings: Settings { threshold_pct: threshold, auto_switch_enabled: enabled, ..Settings::default() },
        }
    }

    fn usage(id: &str, five: f32) -> ProfileUsage {
        ProfileUsage::ok(id, Some(five), None, Some(10.0), None)
    }

    #[test]
    fn switches_to_fresher_account_when_active_is_over() {
        let u = HashMap::from([("a".to_string(), usage("a", 95.0)), ("b".to_string(), usage("b", 10.0))]);
        match decide(&cfg("a", 90, true), &u, false) {
            Decision::Switch { target_id, .. } => assert_eq!(target_id, "b"),
            other => panic!("expected switch, got {other:?}"),
        }
    }

    #[test]
    fn blocked_when_no_candidate_has_headroom() {
        let u = HashMap::from([("a".to_string(), usage("a", 95.0)), ("b".to_string(), usage("b", 92.0))]);
        assert!(matches!(decide(&cfg("a", 90, true), &u, false), Decision::Blocked { .. }));
    }

    #[test]
    fn no_action_below_threshold_or_disabled_or_cooling() {
        let under = HashMap::from([("a".to_string(), usage("a", 40.0)), ("b".to_string(), usage("b", 10.0))]);
        assert_eq!(decide(&cfg("a", 90, true), &under, false), Decision::None);

        let over = HashMap::from([("a".to_string(), usage("a", 95.0)), ("b".to_string(), usage("b", 10.0))]);
        assert_eq!(decide(&cfg("a", 90, false), &over, false), Decision::None); // disabled
        assert_eq!(decide(&cfg("a", 90, true), &over, true), Decision::None); // cooling
    }

    /// Two profiles on one account look like a fallback pair but are not: switching to the twin
    /// leaves the user on the same exhausted quota while reporting success.
    #[test]
    fn a_profile_on_the_same_account_is_not_a_fallback() {
        let mut c = cfg("a", 90, true);
        c.profiles[0].org_id = Some("org-1".into());
        c.profiles[1].org_id = Some("org-1".into()); // same account, drifted after a re-login
        let u = HashMap::from([("a".to_string(), usage("a", 95.0)), ("b".to_string(), usage("b", 10.0))]);
        assert!(
            matches!(decide(&c, &u, false), Decision::Blocked { .. }),
            "must report blocked, not switch to the same account"
        );
    }

    #[test]
    fn distinct_accounts_still_switch() {
        let mut c = cfg("a", 90, true);
        c.profiles[0].org_id = Some("org-1".into());
        c.profiles[1].org_id = Some("org-2".into());
        let u = HashMap::from([("a".to_string(), usage("a", 95.0)), ("b".to_string(), usage("b", 10.0))]);
        assert!(matches!(decide(&c, &u, false), Decision::Switch { .. }));
    }

    /// Unknown identity must not silently disable failover.
    #[test]
    fn unknown_org_falls_back_to_usage_based_judgement() {
        let mut c = cfg("a", 90, true);
        c.profiles[0].org_id = None;
        c.profiles[1].org_id = None;
        let u = HashMap::from([("a".to_string(), usage("a", 95.0)), ("b".to_string(), usage("b", 10.0))]);
        assert!(matches!(decide(&c, &u, false), Decision::Switch { .. }));
    }

    #[test]
    fn hysteresis_excludes_a_candidate_just_under_threshold() {
        // threshold 90, hysteresis 5 → candidate must be < 85. b at 87 is NOT eligible → Blocked.
        let u = HashMap::from([("a".to_string(), usage("a", 95.0)), ("b".to_string(), usage("b", 87.0))]);
        assert!(matches!(decide(&cfg("a", 90, true), &u, false), Decision::Blocked { .. }));
    }
}
