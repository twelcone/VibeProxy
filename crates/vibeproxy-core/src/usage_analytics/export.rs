//! CSV export of an aggregate.
//!
//! Emitted in "tidy" long format — one row per (section, date, key) with a shared header — rather
//! than several tables glued together with blank lines. A single header row means the file opens
//! cleanly in any spreadsheet and every breakdown in the UI can be recovered by filtering `section`.

use super::model::{Analytics, Tokens};

const HEADER: &str = "section,date,key,input,output,cache_write,cache_read,total,api_value_usd";

/// Quote a field only when it could otherwise break the row. Model ids and project slugs are tame,
/// but account labels are user-supplied and can contain commas or quotes.
fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn value(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.6}")).unwrap_or_default()
}

fn row(section: &str, date: &str, key: &str, t: &Tokens, v: Option<f64>) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{}",
        section,
        date,
        field(key),
        t.input,
        t.output,
        t.cache_write,
        t.cache_read,
        t.total(),
        value(v),
    )
}

/// Render the whole aggregate as CSV text.
pub fn to_csv(a: &Analytics) -> String {
    let mut out = String::with_capacity(64 * (a.per_model_per_day.len() + a.per_project.len() + 16));
    out.push_str(HEADER);
    out.push('\n');

    out.push_str(&row("total", "", "all", &a.totals, Some(a.total_value)));
    out.push('\n');
    for r in &a.per_account {
        out.push_str(&row("account", "", &r.account, &r.tokens, r.value));
        out.push('\n');
    }
    for r in &a.per_model {
        out.push_str(&row("model", "", &r.model, &r.tokens, r.value));
        out.push('\n');
    }
    for r in &a.per_project {
        out.push_str(&row("project", "", &r.project, &r.tokens, r.value));
        out.push('\n');
    }
    for r in &a.per_day {
        out.push_str(&row("day", &r.date, "", &r.tokens, r.value));
        out.push('\n');
    }
    for r in &a.per_model_per_day {
        out.push_str(&row("model_day", &r.date, &r.model, &r.tokens, r.value));
        out.push('\n');
    }
    for r in &a.per_account_per_day {
        out.push_str(&row("account_day", &r.date, &r.account, &r.tokens, r.value));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage_analytics::model::{AccountRow, ModelRow};

    fn analytics() -> Analytics {
        Analytics {
            totals: Tokens { input: 1, output: 2, cache_write: 3, cache_read: 4 },
            message_count: 1,
            per_account: vec![AccountRow {
                account: "Work, inc \"main\"".into(),
                tokens: Tokens { input: 1, ..Default::default() },
                messages: 1,
                value: Some(0.5),
            }],
            per_model: vec![ModelRow {
                model: "claude-opus-4-8".into(),
                tokens: Tokens { output: 2, ..Default::default() },
                messages: 1,
                value: None,
            }],
            per_day: vec![],
            per_project: vec![],
            per_model_per_day: vec![],
            per_account_per_day: vec![],
            range: None,
            total_value: 0.5,
            priced_as_of: "2026-07-19".into(),
            unpriced_models: vec![],
        }
    }

    #[test]
    fn quotes_fields_containing_commas_and_quotes() {
        let csv = to_csv(&analytics());
        assert!(csv.contains(r#""Work, inc ""main""""#), "got: {csv}");
    }

    #[test]
    fn every_row_has_the_same_column_count_and_unpriced_is_blank() {
        let csv = to_csv(&analytics());
        let cols = HEADER.split(',').count();
        for line in csv.lines() {
            // Naive split is safe here only because the one quoted field has no bare commas left
            // after quoting; count via a tiny state machine instead.
            let mut n = 1;
            let mut in_q = false;
            for c in line.chars() {
                match c {
                    '"' => in_q = !in_q,
                    ',' if !in_q => n += 1,
                    _ => {}
                }
            }
            assert_eq!(n, cols, "column count mismatch in: {line}");
        }
        // The unpriced model row ends with an empty value field.
        assert!(csv.lines().any(|l| l.starts_with("model,") && l.ends_with(',')));
    }
}
