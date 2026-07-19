//! API-equivalent value: what the tokens would cost on pay-per-token pricing.
//! These are estimates from a bundled table — the accounts are flat-fee subscriptions, so this is a
//! value/leverage figure, NOT actual spend.

use super::model::Tokens;
use serde::Deserialize;
use std::collections::HashMap;

const PRICING_JSON: &str = include_str!("pricing.json");

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rates {
    input: f64,
    output: f64,
    cache_write: f64,
    cache_read: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Table {
    updated_at: String,
    per_million_tokens: HashMap<String, Rates>,
}

/// Loaded pricing table + the date it was last updated.
pub struct Prices {
    table: HashMap<String, Rates>,
    pub updated_at: String,
}

impl Prices {
    pub fn load() -> Self {
        let t: Table = serde_json::from_str(PRICING_JSON).expect("bundled pricing.json is valid");
        Prices { table: t.per_million_tokens, updated_at: t.updated_at }
    }

    /// Match a model id to rates: exact first, then a prefix match (strips date / `[1m]` suffixes,
    /// e.g. `claude-haiku-4-5-20251001` → `claude-haiku-4-5`).
    fn rates(&self, model: &str) -> Option<&Rates> {
        if let Some(r) = self.table.get(model) {
            return Some(r);
        }
        self.table
            .iter()
            .filter(|(k, _)| model.starts_with(k.as_str()))
            .max_by_key(|(k, _)| k.len()) // longest prefix wins
            .map(|(_, r)| r)
    }

    /// API-equivalent value in USD for a token bundle on a given model, or None if the model is unpriced.
    pub fn value(&self, model: &str, t: &Tokens) -> Option<f64> {
        let r = self.rates(model)?;
        Some(
            (t.input as f64 * r.input
                + t.output as f64 * r.output
                + t.cache_write as f64 * r.cache_write
                + t.cache_read as f64 * r.cache_read)
                / 1_000_000.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_a_known_model_by_class() {
        let p = Prices::load();
        // opus: 1M input @15 + 1M output @75 + 1M cacheWrite @18.75 + 1M cacheRead @1.5 = 110.25
        let t = Tokens { input: 1_000_000, output: 1_000_000, cache_write: 1_000_000, cache_read: 1_000_000 };
        let v = p.value("claude-opus-4-8", &t).unwrap();
        assert!((v - 110.25).abs() < 1e-6, "got {v}");
    }

    #[test]
    fn prefix_matches_dated_haiku_and_unknown_is_none() {
        let p = Prices::load();
        let t = Tokens { input: 1_000_000, ..Default::default() };
        assert!((p.value("claude-haiku-4-5-20251001", &t).unwrap() - 1.0).abs() < 1e-6);
        assert!(p.value("some-unknown-model", &t).is_none());
    }

    #[test]
    fn cache_read_is_cheaper_than_input() {
        let p = Prices::load();
        let inp = Tokens { input: 1_000_000, ..Default::default() };
        let cr = Tokens { cache_read: 1_000_000, ..Default::default() };
        assert!(p.value("claude-opus-4-8", &cr) < p.value("claude-opus-4-8", &inp));
    }
}
