//! Resolve each account's log root, stream the JSONL, dedupe, and aggregate.

use super::cost::Prices;
use super::model::*;
use crate::switch::journal::{self, Timeline};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// A log root to scan: an account label + its `<config_dir>/projects` dir.
struct Account {
    /// Who owns the directory. Still the answer for every dir that has never been hot-swapped.
    label: String,
    projects_dir: PathBuf,
    config_dir: PathBuf,
    /// Resolves which account owned this dir at a given instant.
    timeline: Timeline,
}

/// Resolve accounts from VibeProxy's profiles + the default `~/.claude`, deduped by resolved path.
/// Profiles come first so the profile's own label (e.g. "Main") wins over a generic "Default".
fn resolve_accounts() -> Vec<Account> {
    let mut out: Vec<Account> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    let push = |out: &mut Vec<Account>, seen: &mut HashSet<PathBuf>, label: String, config_dir: PathBuf| {
        let projects = config_dir.join("projects");
        let key = std::fs::canonicalize(&projects).unwrap_or_else(|_| projects.clone());
        if seen.insert(key) {
            out.push(Account {
                timeline: Timeline::owned_by(label.clone()),
                label,
                projects_dir: projects,
                config_dir,
            });
        }
    };

    for p in crate::profile::store::load().profiles {
        push(&mut out, &mut seen, p.label, PathBuf::from(&p.config_dir));
    }
    if let Some(home) = dirs::home_dir() {
        push(&mut out, &mut seen, "Default (~/.claude)".to_string(), home.join(".claude"));
    }

    // Overlay hot-swap boundaries. Dirs with no recorded swap keep a static timeline, which
    // resolves to the owner without a lookup.
    let owners: HashMap<String, String> = out
        .iter()
        .map(|a| (a.config_dir.to_string_lossy().to_string(), a.label.clone()))
        .collect();
    let timelines = journal::load_timelines(&owners);
    for a in &mut out {
        if let Some(t) = timelines.get(&a.config_dir.to_string_lossy().to_string()) {
            a.timeline = t.clone();
        }
    }
    out
}

/// Scan all accounts' logs and aggregate, optionally filtered to a local-date range.
pub fn scan(range: &Option<Range>) -> Analytics {
    let mut acc = Accumulator::default();
    for account in resolve_accounts() {
        for file in jsonl_files(&account.projects_dir) {
            let project = project_slug(&file, &account.projects_dir);
            let Ok(f) = std::fs::File::open(&file) else { continue };
            for line in BufReader::new(f).lines().map_while(Result::ok) {
                acc.ingest_line(&line, &account.timeline, &project, range);
            }
        }
    }
    acc.finish(range.clone(), Some(&Prices::load()))
}

/// Cheap change-detector over every account's logs: path + mtime + length of each `*.jsonl`.
/// Statting is milliseconds where parsing is seconds, so this gates the expensive work.
pub(super) fn fingerprint() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for account in resolve_accounts() {
        let mut files = jsonl_files(&account.projects_dir);
        files.sort(); // read_dir order is not stable; the hash must be
        for f in files {
            f.hash(&mut h);
            if let Ok(m) = std::fs::metadata(&f) {
                m.len().hash(&mut h);
                if let Ok(t) = m.modified() {
                    if let Ok(d) = t.duration_since(std::time::UNIX_EPOCH) {
                        d.as_nanos().hash(&mut h);
                    }
                }
            }
        }
    }
    h.finish()
}

/// Recursively collect `*.jsonl` under a projects dir.
fn jsonl_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("jsonl") {
                out.push(p);
            }
        }
    }
    walk(root, &mut out);
    out
}

/// Project name = the immediate sub-dir of `projects/` (Claude Code's slugified cwd).
fn project_slug(file: &Path, projects_dir: &Path) -> String {
    file.strip_prefix(projects_dir)
        .ok()
        .and_then(|rel| rel.components().next())
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Convert an RFC3339/ISO-Z timestamp to a local `YYYY-MM-DD`. Falls back to the UTC date prefix.
fn local_date(ts: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| ts.chars().take(10).collect())
}

fn in_range(date: &str, range: &Option<Range>) -> bool {
    match range {
        None => true,
        Some(r) => {
            r.from.as_deref().map_or(true, |f| date >= f) && r.to.as_deref().map_or(true, |t| date <= t)
        }
    }
}

fn u(usage: &Value, key: &str) -> u64 {
    usage.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// Accumulates token counts across dimensions, deduping by request id. Kept separate from file I/O
/// so it's unit-testable on raw lines.
#[derive(Default)]
pub struct Accumulator {
    seen: HashSet<String>,
    totals: Tokens,
    messages: u64,
    per_account: HashMap<String, (Tokens, u64)>,
    per_model: HashMap<String, (Tokens, u64)>,
    per_day: HashMap<String, Tokens>,
    per_project: HashMap<String, Tokens>,
    per_model_day: HashMap<(String, String), Tokens>,
    // Model splits so per-account/-project value can be priced per model (rates are per model).
    per_account_model: HashMap<(String, String), Tokens>,
    per_project_model: HashMap<(String, String), Tokens>,
    // (date, account, model) — the model leg is what lets an account's daily *value* be priced.
    per_account_day_model: HashMap<(String, String, String), Tokens>,
}

impl Accumulator {
    /// Parse one JSONL line; fold in its usage if it's a deduped, in-range assistant message.
    pub fn ingest_line(&mut self, line: &str, timeline: &Timeline, project: &str, range: &Option<Range>) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        if v.get("type").and_then(Value::as_str) != Some("assistant") {
            return;
        }
        let Some(msg) = v.get("message") else { return };
        let Some(usage) = msg.get("usage") else { return };

        // Dedup by requestId, else message.id. A line with neither is always counted.
        let dedup_key = v
            .get("requestId")
            .and_then(Value::as_str)
            .or_else(|| msg.get("id").and_then(Value::as_str));
        if let Some(key) = dedup_key {
            if !self.seen.insert(key.to_string()) {
                return;
            }
        }

        // Parse the timestamp once: the local date drives bucketing and range filtering, the
        // instant drives account attribution. A dir that was never hot-swapped has a static
        // timeline, so `resolve` returns its owner without inspecting the instant at all.
        let ts = v.get("timestamp").and_then(Value::as_str).unwrap_or("");
        let parsed = chrono::DateTime::parse_from_rfc3339(ts).ok();
        let date = match parsed {
            Some(dt) => dt.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string(),
            None => local_date(ts),
        };
        if !in_range(&date, range) {
            return;
        }
        // An unparseable timestamp cannot be placed on the timeline; fall back to the dir's owner.
        let account = timeline.resolve(parsed.map(|d| d.timestamp_millis()).unwrap_or(i64::MIN));

        let tokens = Tokens {
            input: u(usage, "input_tokens"),
            output: u(usage, "output_tokens"),
            cache_write: u(usage, "cache_creation_input_tokens"),
            cache_read: u(usage, "cache_read_input_tokens"),
        };
        let model = msg.get("model").and_then(Value::as_str).unwrap_or("unknown").to_string();

        self.totals.add(&tokens);
        self.messages += 1;
        add(&mut self.per_account, account.to_string(), &tokens);
        add(&mut self.per_model, model.clone(), &tokens);
        self.per_day.entry(date.clone()).or_default().add(&tokens);
        self.per_project.entry(project.to_string()).or_default().add(&tokens);
        self.per_model_day.entry((date.clone(), model.clone())).or_default().add(&tokens);
        self.per_account_model.entry((account.to_string(), model.clone())).or_default().add(&tokens);
        self.per_account_day_model
            .entry((date, account.to_string(), model.clone()))
            .or_default()
            .add(&tokens);
        self.per_project_model.entry((project.to_string(), model)).or_default().add(&tokens);
    }

    pub fn finish(self, range: Option<Range>, prices: Option<&Prices>) -> Analytics {
        // Group each split map into dim -> [(model, tokens)] so a dimension's value is the sum of
        // its models priced at their own rates (an unpriced model contributes 0 to the value).
        let group = |src: &HashMap<(String, String), Tokens>| -> HashMap<String, Vec<(String, Tokens)>> {
            let mut out: HashMap<String, Vec<(String, Tokens)>> = HashMap::new();
            for ((dim, model), t) in src {
                out.entry(dim.clone()).or_default().push((model.clone(), t.clone()));
            }
            out
        };
        let acct_models = group(&self.per_account_model);
        let proj_models = group(&self.per_project_model);
        let day_models = group(&self.per_model_day);
        let vsum = |models: Option<&Vec<(String, Tokens)>>| -> Option<f64> {
            prices.map(|p| {
                models
                    .map(|v| v.iter().map(|(m, t)| p.value(m, t).unwrap_or(0.0)).sum())
                    .unwrap_or(0.0)
            })
        };

        let mut total_value = 0.0;
        let mut unpriced = std::collections::BTreeSet::new();
        // Claude Code logs locally-fabricated messages (API errors, interrupts) as `<synthetic>`
        // with no usage. Zero-token rows render as empty bars and dashed table rows, so drop them
        // from every breakdown. Totals are unaffected — the rows contribute nothing to sum.
        let has_tokens = |t: &Tokens| t.total() > 0;

        let mut per_model: Vec<ModelRow> = self
            .per_model
            .into_iter()
            .filter(|(_, (tokens, _))| has_tokens(tokens))
            .map(|(model, (tokens, messages))| {
                let value = prices.and_then(|p| p.value(&model, &tokens));
                match value {
                    Some(v) => total_value += v,
                    None if prices.is_some() && tokens.total() > 0 => {
                        unpriced.insert(model.clone());
                    }
                    None => {}
                }
                ModelRow { model, tokens, messages, value }
            })
            .collect();
        per_model.sort_by(|a, b| b.tokens.total().cmp(&a.tokens.total()));

        let mut per_account: Vec<AccountRow> = self
            .per_account
            .into_iter()
            .map(|(account, (tokens, messages))| {
                let value = vsum(acct_models.get(&account));
                AccountRow { account, tokens, messages, value }
            })
            .collect();
        per_account.sort_by(|a, b| b.tokens.total().cmp(&a.tokens.total()));

        let mut per_day: Vec<DayRow> = self
            .per_day
            .into_iter()
            .map(|(date, tokens)| {
                let value = vsum(day_models.get(&date));
                DayRow { date, tokens, value }
            })
            .collect();
        per_day.sort_by(|a, b| a.date.cmp(&b.date));

        let mut per_project: Vec<ProjectRow> = self
            .per_project
            .into_iter()
            .filter(|(_, tokens)| has_tokens(tokens))
            .map(|(project, tokens)| {
                let value = vsum(proj_models.get(&project));
                ProjectRow { project, tokens, value }
            })
            .collect();
        per_project.sort_by(|a, b| b.tokens.total().cmp(&a.tokens.total()));

        let mut per_model_per_day: Vec<ModelDayRow> = self
            .per_model_day
            .into_iter()
            .filter(|(_, tokens)| has_tokens(tokens))
            .map(|((date, model), tokens)| {
                let value = prices.and_then(|p| p.value(&model, &tokens));
                ModelDayRow { date, model, tokens, value }
            })
            .collect();
        per_model_per_day.sort_by(|a, b| a.date.cmp(&b.date).then(a.model.cmp(&b.model)));

        // Fold (date, account, model) into (date, account), pricing each model leg at its own rate.
        let mut acct_day: HashMap<(String, String), (Tokens, Vec<(String, Tokens)>)> = HashMap::new();
        for ((date, account, model), t) in self.per_account_day_model {
            let e = acct_day.entry((date, account)).or_default();
            e.0.add(&t);
            e.1.push((model, t));
        }
        let mut per_account_per_day: Vec<AccountDayRow> = acct_day
            .into_iter()
            .filter(|(_, (tokens, _))| has_tokens(tokens))
            .map(|((date, account), (tokens, models))| AccountDayRow {
                date,
                account,
                value: vsum(Some(&models)),
                tokens,
            })
            .collect();
        per_account_per_day.sort_by(|a, b| a.date.cmp(&b.date).then(a.account.cmp(&b.account)));

        Analytics {
            totals: self.totals,
            message_count: self.messages,
            per_account,
            per_model,
            per_day,
            per_project,
            per_model_per_day,
            per_account_per_day,
            range,
            total_value,
            priced_as_of: prices.map(|p| p.updated_at.clone()).unwrap_or_default(),
            unpriced_models: unpriced.into_iter().collect(),
        }
    }
}

fn add(map: &mut HashMap<String, (Tokens, u64)>, key: String, tokens: &Tokens) {
    let entry = map.entry(key).or_insert_with(|| (Tokens::default(), 0));
    entry.0.add(tokens);
    entry.1 += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(req: &str, model: &str, ts: &str, i: u64, o: u64, cw: u64, cr: u64) -> String {
        format!(
            r#"{{"type":"assistant","requestId":"{req}","timestamp":"{ts}","message":{{"id":"m_{req}","model":"{model}","usage":{{"input_tokens":{i},"output_tokens":{o},"cache_creation_input_tokens":{cw},"cache_read_input_tokens":{cr}}}}}}}"#
        )
    }

    #[test]
    fn aggregates_across_accounts_models_and_dedupes() {
        let mut acc = Accumulator::default();
        acc.ingest_line(&line("r1", "claude-opus-4-8", "2026-07-02T07:00:00.000Z", 100, 10, 50, 20), &Timeline::owned_by("Main"), "proj-a", &None);
        acc.ingest_line(&line("r1", "claude-opus-4-8", "2026-07-02T07:00:00.000Z", 100, 10, 50, 20), &Timeline::owned_by("Main"), "proj-a", &None); // dup requestId
        acc.ingest_line(&line("r2", "claude-fable-5", "2026-07-03T07:00:00.000Z", 200, 20, 0, 0), &Timeline::owned_by("Work"), "proj-b", &None);
        let prices = super::super::cost::Prices::load();
        let a = acc.finish(None, Some(&prices));

        assert_eq!(a.message_count, 2, "duplicate requestId counted once");
        assert!(a.total_value > 0.0, "priced models produce value");
        assert!(a.per_account.iter().all(|r| r.value.is_some()), "per-account value computed");
        assert!(a.unpriced_models.is_empty(), "both fixture models are priced");
        assert_eq!(a.totals.input, 300);
        assert_eq!(a.totals.output, 30);
        assert_eq!(a.totals.cache_write, 50);
        assert_eq!(a.totals.cache_read, 20);
        assert_eq!(a.per_account.len(), 2);
        assert_eq!(a.per_model.len(), 2);
        // per_account is sorted desc by total; Work (220) > Main (180)
        assert_eq!(a.per_account[0].account, "Work");
    }

    /// The whole point of Phase 1: two messages in the SAME directory, either side of a swap,
    /// must be credited to different accounts.
    #[test]
    fn a_swap_boundary_splits_one_dir_across_two_accounts() {
        let timeline = crate::switch::journal::Timeline::for_test(
            "Work",
            &[("2026-07-15T00:00:00Z", "Personal")],
        );
        let mut acc = Accumulator::default();
        // before the swap → Work
        acc.ingest_line(&line("a", "claude-opus-4-8", "2026-07-14T10:00:00.000Z", 100, 0, 0, 0), &timeline, "p", &None);
        // after the swap → Personal, despite living in the same directory
        acc.ingest_line(&line("b", "claude-opus-4-8", "2026-07-16T10:00:00.000Z", 300, 0, 0, 0), &timeline, "p", &None);
        let a = acc.finish(None, None);

        assert_eq!(a.per_account.len(), 2, "one dir, two accounts after a swap");
        let by = |name: &str| {
            a.per_account.iter().find(|r| r.account == name).map(|r| r.tokens.input).unwrap_or(0)
        };
        assert_eq!(by("Work"), 100);
        assert_eq!(by("Personal"), 300);
        assert_eq!(a.totals.input, 400, "totals unaffected by attribution");
    }

    /// Attribution must not perturb anything when no swap has ever happened.
    #[test]
    fn static_timeline_matches_plain_owner_attribution() {
        let mut acc = Accumulator::default();
        acc.ingest_line(&line("a", "claude-opus-4-8", "2026-07-14T10:00:00.000Z", 100, 0, 0, 0), &Timeline::owned_by("Work"), "p", &None);
        acc.ingest_line(&line("b", "claude-opus-4-8", "2026-07-16T10:00:00.000Z", 300, 0, 0, 0), &Timeline::owned_by("Work"), "p", &None);
        let a = acc.finish(None, None);
        assert_eq!(a.per_account.len(), 1);
        assert_eq!(a.per_account[0].account, "Work");
        assert_eq!(a.per_account[0].tokens.input, 400);
    }

    #[test]
    fn skips_non_assistant_and_malformed_and_missing_usage() {
        let mut acc = Accumulator::default();
        acc.ingest_line(r#"{"type":"user","message":{"content":"hi"}}"#, &Timeline::owned_by("Main"), "p", &None);
        acc.ingest_line("not json at all", &Timeline::owned_by("Main"), "p", &None);
        acc.ingest_line(r#"{"type":"assistant","message":{"model":"x"}}"#, &Timeline::owned_by("Main"), "p", &None); // no usage
        acc.ingest_line("", &Timeline::owned_by("Main"), "p", &None);
        let a = acc.finish(None, None);
        assert_eq!(a.message_count, 0);
        assert_eq!(a.totals.total(), 0);
    }

    /// End-to-end against this machine's real Claude Code logs. Ignored by default: it depends on
    /// local state, so it can't run in CI. Run with
    /// `cargo test real_logs -- --ignored --nocapture`.
    ///
    /// Asserts only *internal consistency* — every breakdown must re-sum to the same totals — which
    /// is checkable without knowing the expected numbers in advance. Prints aggregates (never log
    /// content) so the figures can be eyeballed against `ccusage`.
    #[test]
    #[ignore = "reads this machine's real Claude Code logs"]
    fn real_logs_aggregate_consistently() {
        let a = scan(&None);
        if a.message_count == 0 {
            eprintln!("no local logs found — nothing to verify");
            return;
        }

        let total = a.totals.total();
        let sum_of = |v: u64| v;
        let acct: u64 = a.per_account.iter().map(|r| r.tokens.total()).sum();
        let model: u64 = a.per_model.iter().map(|r| r.tokens.total()).sum();
        let day: u64 = a.per_day.iter().map(|r| r.tokens.total()).sum();
        let proj: u64 = a.per_project.iter().map(|r| r.tokens.total()).sum();
        let md: u64 = a.per_model_per_day.iter().map(|r| r.tokens.total()).sum();
        let ad: u64 = a.per_account_per_day.iter().map(|r| r.tokens.total()).sum();

        eprintln!("--- real-log aggregates ---");
        eprintln!("messages       {}", a.message_count);
        eprintln!("total tokens   {total}");
        eprintln!("  input        {}", a.totals.input);
        eprintln!("  output       {}", a.totals.output);
        eprintln!("  cache write  {}", a.totals.cache_write);
        eprintln!("  cache read   {}", a.totals.cache_read);
        eprintln!("API value      ${:.2}  (priced as of {})", a.total_value, a.priced_as_of);
        eprintln!("accounts {}  models {}  days {}  projects {}",
            a.per_account.len(), a.per_model.len(), a.per_day.len(), a.per_project.len());
        for r in &a.per_account {
            eprintln!("  account  {:<28} {:>14} tok", r.account, r.tokens.total());
        }
        for r in &a.per_model {
            eprintln!("  model    {:<28} {:>14} tok  {:?}", r.model, r.tokens.total(), r.value);
        }
        if !a.unpriced_models.is_empty() {
            eprintln!("UNPRICED: {:?}", a.unpriced_models);
        }

        assert_eq!(sum_of(acct), total, "per_account must re-sum to totals");
        assert_eq!(sum_of(model), total, "per_model must re-sum to totals");
        assert_eq!(sum_of(day), total, "per_day must re-sum to totals");
        assert_eq!(sum_of(proj), total, "per_project must re-sum to totals");
        assert_eq!(sum_of(md), total, "per_model_per_day must re-sum to totals");
        assert_eq!(sum_of(ad), total, "per_account_per_day must re-sum to totals");
        assert!(a.per_day.windows(2).all(|w| w[0].date <= w[1].date), "per_day sorted by date");
        assert!(
            a.per_account.windows(2).all(|w| w[0].tokens.total() >= w[1].tokens.total()),
            "per_account sorted desc"
        );
    }

    #[test]
    fn respects_date_range_and_tolerates_extra_fields() {
        let mut acc = Accumulator::default();
        // extra/unknown fields in usage must be ignored, not error
        let l = r#"{"type":"assistant","requestId":"r9","timestamp":"2026-07-10T00:00:00.000Z","message":{"model":"m","usage":{"input_tokens":5,"output_tokens":1,"service_tier":"standard","iterations":[1,2]}}}"#;
        let range = Some(Range { from: Some("2026-07-01".into()), to: Some("2026-07-05".into()) });
        acc.ingest_line(l, &Timeline::owned_by("Main"), "p", &range); // 07-10 is outside 01..05
        assert_eq!(acc.finish(range, None).message_count, 0);

        let mut acc2 = Accumulator::default();
        let range2 = Some(Range { from: Some("2026-07-01".into()), to: Some("2026-07-31".into()) });
        acc2.ingest_line(l, &Timeline::owned_by("Main"), "p", &range2);
        assert_eq!(acc2.finish(range2, None).totals.input, 5);
    }
}
