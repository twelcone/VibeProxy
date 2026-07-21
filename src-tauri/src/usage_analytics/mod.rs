//! Usage analytics: parse Claude Code's per-account JSONL logs into token aggregates.

mod cost;
mod export;
pub mod model;
pub(crate) mod scan;

use std::sync::Mutex;

pub use export::to_csv;
pub use model::{Analytics, Range};

/// Last computed aggregate, reused while the underlying files are untouched.
struct Cached {
    fingerprint: u64,
    range: Option<Range>,
    analytics: Analytics,
}

static CACHE: Mutex<Option<Cached>> = Mutex::new(None);

/// Aggregate for `range`, reusing the previous result when no log file has changed.
///
/// Fingerprinting stats every `*.jsonl` (path + mtime + length), which costs milliseconds; parsing
/// them costs orders of magnitude more. So the check is worth it on every call, and re-opening the
/// Usage window or flipping a filter back is effectively free.
///
/// The whole result is cached rather than doing per-file incremental merging — a session append
/// invalidates everything anyway, and partial-merge bookkeeping is a correctness risk for no
/// meaningful gain at this data size.
pub fn scan(range: &Option<Range>) -> Analytics {
    let fingerprint = scan::fingerprint();

    if let Ok(guard) = CACHE.lock() {
        if let Some(c) = guard.as_ref() {
            if c.fingerprint == fingerprint && &c.range == range {
                return c.analytics.clone();
            }
        }
    }

    let analytics = scan::scan(range);
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(Cached { fingerprint, range: range.clone(), analytics: analytics.clone() });
    }
    analytics
}

/// Drop the cached aggregate, forcing a full re-parse on the next scan.
pub fn clear_cache() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    /// The cache must be a pure speedup: same inputs, byte-identical aggregate. Ignored by default
    /// because it reads this machine's logs.
    /// Run with `cargo test cache_ -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads this machine's real Claude Code logs"]
    fn cache_returns_identical_results_and_is_faster() {
        use std::time::Instant;

        super::clear_cache();
        let before = super::scan::fingerprint();
        let t0 = Instant::now();
        let cold = super::scan(&None);
        let cold_ms = t0.elapsed().as_millis();

        let t1 = Instant::now();
        let warm = super::scan(&None);
        let warm_ms = t1.elapsed().as_millis();
        let after = super::scan::fingerprint();

        eprintln!("cold {cold_ms}ms → warm {warm_ms}ms");

        // Claude Code may be appending to these logs as the test runs. A changed fingerprint means
        // the cache *correctly* missed, so the equality/speed assertions below wouldn't apply.
        if before != after {
            eprintln!("logs changed mid-test — cache correctly invalidated; skipping equality check");
            return;
        }

        assert_eq!(cold.totals, warm.totals, "cached totals must match");
        assert_eq!(cold.message_count, warm.message_count);
        assert_eq!(cold.per_model.len(), warm.per_model.len());
        assert_eq!(cold.per_day.len(), warm.per_day.len());
        assert!(
            (cold.total_value - warm.total_value).abs() < 1e-9,
            "cached value must match"
        );
        assert!(warm_ms * 4 < cold_ms.max(4), "warm scan should be far faster: {warm_ms} vs {cold_ms}");

        // A different range must not hand back the previous range's answer.
        let narrow = Some(super::Range { from: Some("2099-01-01".into()), to: None });
        assert_eq!(super::scan(&narrow).message_count, 0, "range is part of the cache key");
    }
}
