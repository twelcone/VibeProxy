//! Account-occupancy journal.
//!
//! Hot-swapping puts one account's credentials into another account's config dir, so from that
//! moment the dir's transcripts belong to a different account than the one that owns the directory.
//! Attributing usage by directory would then be silently wrong — and silently-wrong numbers are
//! worse than missing ones.
//!
//! This records *when* each dir changed hands. The scanner resolves a message's account from its
//! timestamp instead of from its location.

use crate::profile::paths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

/// One boundary: from `at` onward, `config_dir` belongs to `account_label`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Boundary {
    /// RFC3339, always UTC. Compared as an instant — never as a local date string.
    pub at: String,
    pub config_dir: String,
    pub account_id: String,
    pub account_label: String,
}

/// Resolves which account owned a directory at a given instant.
///
/// The common case — a dir that has never been swapped — has no boundaries at all, so `resolve`
/// returns the owner without touching the vector. That keeps the scanner's hot loop unchanged for
/// every user who never enables hot-swap.
#[derive(Debug, Clone)]
pub struct Timeline {
    owner: String,
    /// (instant in epoch millis, account label), sorted ascending.
    boundaries: Vec<(i64, String)>,
}

impl Timeline {
    pub fn owned_by(owner: impl Into<String>) -> Self {
        Timeline { owner: owner.into(), boundaries: Vec::new() }
    }

    /// Build a timeline directly, for tests that must not depend on a journal file on disk.
    #[cfg(test)]
    pub fn for_test(owner: &str, boundaries: &[(&str, &str)]) -> Self {
        let mut t = Timeline::owned_by(owner);
        t.boundaries = boundaries
            .iter()
            .filter_map(|(at, label)| parse_instant(at).map(|ms| (ms, label.to_string())))
            .collect();
        t.boundaries.sort_by_key(|(ms, _)| *ms);
        t
    }

    pub fn is_static(&self) -> bool {
        self.boundaries.is_empty()
    }

    /// Earliest boundary, used to warn when a scan reaches back before the journal begins.
    pub fn first_boundary_millis(&self) -> Option<i64> {
        self.boundaries.first().map(|(t, _)| *t)
    }

    /// Account owning this dir at `millis`. A boundary is inclusive: a message stamped exactly at
    /// the swap instant belongs to the incoming account.
    pub fn resolve(&self, millis: i64) -> &str {
        if self.boundaries.is_empty() {
            return &self.owner;
        }
        match self.boundaries.binary_search_by(|(t, _)| t.cmp(&millis)) {
            Ok(i) => &self.boundaries[i].1,
            // `Err(i)` is the insertion point; the owning boundary is the one before it.
            Err(0) => &self.owner, // predates every swap
            Err(i) => &self.boundaries[i - 1].1,
        }
    }
}

fn parse_instant(rfc3339: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(rfc3339).ok().map(|d| d.timestamp_millis())
}

/// Append a boundary, flushed to disk before returning — a swap that is not durably recorded would
/// misattribute every message that follows it.
pub fn append(entry: &Boundary) -> std::io::Result<()> {
    std::fs::create_dir_all(paths::vibeproxy_dir())?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths::swap_journal_file())?;
    let mut line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
    line.push('\n');
    f.write_all(line.as_bytes())?;
    f.sync_all()
}

/// Load boundaries grouped by config dir. Corrupt lines are skipped, not fatal: a half-written
/// trailing line must never take the whole analytics view down with it.
pub fn load_timelines(owners: &HashMap<String, String>) -> HashMap<String, Timeline> {
    let mut out: HashMap<String, Timeline> = owners
        .iter()
        .map(|(dir, owner)| (dir.clone(), Timeline::owned_by(owner.clone())))
        .collect();

    let Ok(f) = std::fs::File::open(paths::swap_journal_file()) else {
        return out; // no journal yet — every dir keeps its static owner
    };

    for line in BufReader::new(f).lines().map_while(Result::ok) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(b) = serde_json::from_str::<Boundary>(line) else { continue };
        let Some(millis) = parse_instant(&b.at) else { continue };
        out.entry(b.config_dir.clone())
            .or_insert_with(|| Timeline::owned_by(b.account_label.clone()))
            .boundaries
            .push((millis, b.account_label));
    }

    // The journal is append-only, but clock skew or a manual edit can leave it out of order.
    for t in out.values_mut() {
        t.boundaries.sort_by_key(|(ms, _)| *ms);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline(pairs: &[(&str, &str)], owner: &str) -> Timeline {
        let mut t = Timeline::owned_by(owner);
        t.boundaries = pairs
            .iter()
            .map(|(at, label)| (parse_instant(at).unwrap(), label.to_string()))
            .collect();
        t.boundaries.sort_by_key(|(ms, _)| *ms);
        t
    }

    #[test]
    fn static_timeline_always_returns_its_owner() {
        let t = Timeline::owned_by("Work");
        assert!(t.is_static());
        assert_eq!(t.resolve(0), "Work");
        assert_eq!(t.resolve(i64::MAX), "Work");
    }

    #[test]
    fn resolves_before_between_and_after_boundaries() {
        let t = timeline(
            &[("2026-07-10T00:00:00Z", "Personal"), ("2026-07-20T00:00:00Z", "Work")],
            "Work",
        );
        let at = |s: &str| parse_instant(s).unwrap();
        assert_eq!(t.resolve(at("2026-07-01T00:00:00Z")), "Work", "predates all swaps → owner");
        assert_eq!(t.resolve(at("2026-07-15T00:00:00Z")), "Personal", "between boundaries");
        assert_eq!(t.resolve(at("2026-07-25T00:00:00Z")), "Work", "after the last boundary");
    }

    #[test]
    fn boundary_instant_belongs_to_the_incoming_account() {
        let t = timeline(&[("2026-07-10T00:00:00Z", "Personal")], "Work");
        assert_eq!(t.resolve(parse_instant("2026-07-10T00:00:00Z").unwrap()), "Personal");
        assert_eq!(t.resolve(parse_instant("2026-07-09T23:59:59Z").unwrap()), "Work");
    }

    #[test]
    fn offset_timestamps_compare_as_instants_not_strings() {
        // Same instant, different textual offsets: must resolve identically.
        let t = timeline(&[("2026-07-10T00:00:00Z", "Personal")], "Work");
        let utc = parse_instant("2026-07-10T04:00:00Z").unwrap();
        let plus4 = parse_instant("2026-07-10T08:00:00+04:00").unwrap();
        assert_eq!(utc, plus4);
        assert_eq!(t.resolve(utc), t.resolve(plus4));
    }

    #[test]
    fn out_of_order_entries_are_sorted() {
        let t = timeline(
            &[("2026-07-20T00:00:00Z", "Work"), ("2026-07-10T00:00:00Z", "Personal")],
            "Work",
        );
        assert_eq!(t.resolve(parse_instant("2026-07-15T00:00:00Z").unwrap()), "Personal");
    }

    #[test]
    fn malformed_timestamp_is_rejected() {
        assert!(parse_instant("not a date").is_none());
        assert!(parse_instant("").is_none());
    }
}
