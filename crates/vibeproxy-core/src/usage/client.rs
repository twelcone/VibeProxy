//! The Anthropic OAuth usage endpoint client.
//!
//! `GET /api/oauth/usage` returns per-account 5-hour + weekly utilization. Read-only. The token is
//! the profile's own OAuth token from the Keychain — never relayed to any inference endpoint.

use serde::Deserialize;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// Distinguishes an expired/invalid token (→ NeedsReauth) from a transient failure.
pub enum FetchError {
    Unauthorized,
    Other(String),
}

/// Schema-tolerant view of the response — unknown fields ignored, missing ones become None so a
/// server-side shape change degrades gracefully rather than crashing the poller.
#[derive(Debug, Deserialize)]
pub struct UsageResponse {
    #[serde(default)]
    pub five_hour: Option<Window>,
    #[serde(default)]
    pub seven_day: Option<Window>,
}

#[derive(Debug, Deserialize)]
pub struct Window {
    #[serde(default)]
    pub utilization: Option<f32>,
    #[serde(default)]
    pub resets_at: Option<String>,
}

/// Fetch usage for one account using its OAuth access token.
pub async fn fetch(token: &str) -> Result<UsageResponse, FetchError> {
    let resp = reqwest::Client::new()
        .get(USAGE_URL)
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(|e| FetchError::Other(e.to_string()))?;

    if resp.status().as_u16() == 401 {
        return Err(FetchError::Unauthorized);
    }
    if !resp.status().is_success() {
        return Err(FetchError::Other(format!("usage endpoint returned {}", resp.status())));
    }
    resp.json::<UsageResponse>()
        .await
        .map_err(|e| FetchError::Other(format!("parse usage json: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_near_100_and_ignores_unknown_fields() {
        let json = r#"{
            "five_hour": {"utilization": 96.0, "resets_at": "2026-07-19T03:00:00Z"},
            "seven_day": {"utilization": 100.0, "resets_at": "2026-07-23T02:59:59Z"},
            "limits": [{"kind":"session","severity":"critical"}],
            "some_new_field": 42
        }"#;
        let r: UsageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.five_hour.unwrap().utilization, Some(96.0));
        assert_eq!(r.seven_day.unwrap().utilization, Some(100.0));
    }

    #[test]
    fn tolerates_missing_windows() {
        let r: UsageResponse = serde_json::from_str("{}").unwrap();
        assert!(r.five_hour.is_none() && r.seven_day.is_none());
    }
}
