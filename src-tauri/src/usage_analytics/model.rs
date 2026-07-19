//! Data model for usage analytics — token aggregates serialized to the UI as camelCase.

use serde::{Deserialize, Serialize};

/// Token counts by class. Cache-write/read are billed at different rates than fresh input (Phase 2).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_write + self.cache_read
    }
    pub fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_write += o.cache_write;
        self.cache_read += o.cache_read;
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRow {
    pub account: String,
    pub tokens: Tokens,
    pub messages: u64,
    /// API-equivalent value (USD), or None if the model(s) are unpriced.
    #[serde(default)]
    pub value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    pub model: String,
    pub tokens: Tokens,
    pub messages: u64,
    /// API-equivalent value (USD), or None if the model(s) are unpriced.
    #[serde(default)]
    pub value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayRow {
    pub date: String, // YYYY-MM-DD (local)
    pub tokens: Tokens,
    #[serde(default)]
    pub value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub project: String,
    pub tokens: Tokens,
    #[serde(default)]
    pub value: Option<f64>,
}

/// One (day, model) bucket for the multi-series trend chart (Phase 4).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDayRow {
    pub date: String,
    pub model: String,
    pub tokens: Tokens,
    #[serde(default)]
    pub value: Option<f64>,
}

/// Inclusive local-date filter (YYYY-MM-DD). Both bounds optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub from: Option<String>,
    pub to: Option<String>,
}

/// The full aggregate the UI renders.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analytics {
    pub totals: Tokens,
    pub message_count: u64,
    pub per_account: Vec<AccountRow>,
    pub per_model: Vec<ModelRow>,
    pub per_day: Vec<DayRow>,
    pub per_project: Vec<ProjectRow>,
    pub per_model_per_day: Vec<ModelDayRow>,
    pub range: Option<Range>,
    /// Total API-equivalent value (USD) across priced models.
    #[serde(default)]
    pub total_value: f64,
    /// Date the bundled pricing was last updated (shown as "priced as of").
    #[serde(default)]
    pub priced_as_of: String,
    /// Model ids that had tokens but no price (UI disclaimer).
    #[serde(default)]
    pub unpriced_models: Vec<String>,
}
