//! Resolve each account's log root, stream the JSONL, dedupe, and aggregate.

use super::model::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// A log root to scan: an account label + its `<config_dir>/projects` dir.
struct Account {
    label: String,
    projects_dir: PathBuf,
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
            out.push(Account { label, projects_dir: projects });
        }
    };

    for p in crate::profile::store::load().profiles {
        push(&mut out, &mut seen, p.label, PathBuf::from(&p.config_dir));
    }
    if let Some(home) = dirs::home_dir() {
        push(&mut out, &mut seen, "Default (~/.claude)".to_string(), home.join(".claude"));
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
                acc.ingest_line(&line, &account.label, &project, range);
            }
        }
    }
    acc.finish(range.clone())
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
}

impl Accumulator {
    /// Parse one JSONL line; fold in its usage if it's a deduped, in-range assistant message.
    pub fn ingest_line(&mut self, line: &str, account: &str, project: &str, range: &Option<Range>) {
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

        let date = local_date(v.get("timestamp").and_then(Value::as_str).unwrap_or(""));
        if !in_range(&date, range) {
            return;
        }

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
        self.per_model_day.entry((date, model)).or_default().add(&tokens);
    }

    pub fn finish(self, range: Option<Range>) -> Analytics {
        let mut per_account: Vec<AccountRow> = self
            .per_account
            .into_iter()
            .map(|(account, (tokens, messages))| AccountRow { account, tokens, messages })
            .collect();
        per_account.sort_by(|a, b| b.tokens.total().cmp(&a.tokens.total()));

        let mut per_model: Vec<ModelRow> = self
            .per_model
            .into_iter()
            .map(|(model, (tokens, messages))| ModelRow { model, tokens, messages })
            .collect();
        per_model.sort_by(|a, b| b.tokens.total().cmp(&a.tokens.total()));

        let mut per_day: Vec<DayRow> = self
            .per_day
            .into_iter()
            .map(|(date, tokens)| DayRow { date, tokens })
            .collect();
        per_day.sort_by(|a, b| a.date.cmp(&b.date));

        let mut per_project: Vec<ProjectRow> = self
            .per_project
            .into_iter()
            .map(|(project, tokens)| ProjectRow { project, tokens })
            .collect();
        per_project.sort_by(|a, b| b.tokens.total().cmp(&a.tokens.total()));

        let mut per_model_per_day: Vec<ModelDayRow> = self
            .per_model_day
            .into_iter()
            .map(|((date, model), tokens)| ModelDayRow { date, model, tokens })
            .collect();
        per_model_per_day.sort_by(|a, b| a.date.cmp(&b.date).then(a.model.cmp(&b.model)));

        Analytics {
            totals: self.totals,
            message_count: self.messages,
            per_account,
            per_model,
            per_day,
            per_project,
            per_model_per_day,
            range,
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
        acc.ingest_line(&line("r1", "claude-opus-4-8", "2026-07-02T07:00:00.000Z", 100, 10, 50, 20), "Main", "proj-a", &None);
        acc.ingest_line(&line("r1", "claude-opus-4-8", "2026-07-02T07:00:00.000Z", 100, 10, 50, 20), "Main", "proj-a", &None); // dup requestId
        acc.ingest_line(&line("r2", "claude-fable-5", "2026-07-03T07:00:00.000Z", 200, 20, 0, 0), "Work", "proj-b", &None);
        let a = acc.finish(None);

        assert_eq!(a.message_count, 2, "duplicate requestId counted once");
        assert_eq!(a.totals.input, 300);
        assert_eq!(a.totals.output, 30);
        assert_eq!(a.totals.cache_write, 50);
        assert_eq!(a.totals.cache_read, 20);
        assert_eq!(a.per_account.len(), 2);
        assert_eq!(a.per_model.len(), 2);
        // per_account is sorted desc by total; Work (220) > Main (180)
        assert_eq!(a.per_account[0].account, "Work");
    }

    #[test]
    fn skips_non_assistant_and_malformed_and_missing_usage() {
        let mut acc = Accumulator::default();
        acc.ingest_line(r#"{"type":"user","message":{"content":"hi"}}"#, "Main", "p", &None);
        acc.ingest_line("not json at all", "Main", "p", &None);
        acc.ingest_line(r#"{"type":"assistant","message":{"model":"x"}}"#, "Main", "p", &None); // no usage
        acc.ingest_line("", "Main", "p", &None);
        let a = acc.finish(None);
        assert_eq!(a.message_count, 0);
        assert_eq!(a.totals.total(), 0);
    }

    #[test]
    fn respects_date_range_and_tolerates_extra_fields() {
        let mut acc = Accumulator::default();
        // extra/unknown fields in usage must be ignored, not error
        let l = r#"{"type":"assistant","requestId":"r9","timestamp":"2026-07-10T00:00:00.000Z","message":{"model":"m","usage":{"input_tokens":5,"output_tokens":1,"service_tier":"standard","iterations":[1,2]}}}"#;
        let range = Some(Range { from: Some("2026-07-01".into()), to: Some("2026-07-05".into()) });
        acc.ingest_line(l, "Main", "p", &range); // 07-10 is outside 01..05
        assert_eq!(acc.finish(range).message_count, 0);

        let mut acc2 = Accumulator::default();
        let range2 = Some(Range { from: Some("2026-07-01".into()), to: Some("2026-07-31".into()) });
        acc2.ingest_line(l, "Main", "p", &range2);
        assert_eq!(acc2.finish(range2).totals.input, 5);
    }
}
