//! The per-profile usage snapshot the UI and tray render.

use serde::Serialize;

/// Freshness/health of a profile's usage reading.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageStatus {
    /// Live data from the usage endpoint.
    Ok,
    /// Token expired / not authorized — needs a re-login (Phase 4 keep-fresh couldn't refresh it).
    NeedsReauth,
    /// Transient fetch error (network / endpoint). Keep showing last value as "unavailable".
    Error,
}

/// A profile's usage, serialized to the frontend as camelCase.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUsage {
    pub profile_id: String,
    pub five_hour_pct: Option<f32>,
    pub five_hour_resets_at: Option<String>,
    pub weekly_pct: Option<f32>,
    pub weekly_resets_at: Option<String>,
    pub status: UsageStatus,
    /// Non-fatal error detail (only set when status == Error).
    pub error: Option<String>,
}

impl ProfileUsage {
    pub fn ok(
        profile_id: &str,
        five_hour_pct: Option<f32>,
        five_hour_resets_at: Option<String>,
        weekly_pct: Option<f32>,
        weekly_resets_at: Option<String>,
    ) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            five_hour_pct: five_hour_pct.map(clamp_pct),
            five_hour_resets_at,
            weekly_pct: weekly_pct.map(clamp_pct),
            weekly_resets_at,
            status: UsageStatus::Ok,
            error: None,
        }
    }

    pub fn needs_reauth(profile_id: &str) -> Self {
        Self::blank(profile_id, UsageStatus::NeedsReauth, None)
    }

    pub fn error(profile_id: &str, detail: String) -> Self {
        Self::blank(profile_id, UsageStatus::Error, Some(detail))
    }

    fn blank(profile_id: &str, status: UsageStatus, error: Option<String>) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            five_hour_pct: None,
            five_hour_resets_at: None,
            weekly_pct: None,
            weekly_resets_at: None,
            status,
            error,
        }
    }
}

/// Utilization arrives 0–100 in the observed responses; clamp defensively.
fn clamp_pct(v: f32) -> f32 {
    v.clamp(0.0, 100.0)
}
