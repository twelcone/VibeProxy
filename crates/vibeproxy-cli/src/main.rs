//! `vibeproxy` — the headless CLI. Works anywhere `claude` does: a plain terminal, WSL, SSH, a
//! container. Every handler is a thin call into `vibeproxy-core`; the CLI only parses, invokes, and
//! formats. No GUI, no Tauri.

use clap::{Parser, Subcommand};
use std::path::Path;
use std::process::ExitCode;
use vibeproxy_core::profile::store::{self, Config, Profile};
use vibeproxy_core::usage::poll_profile;
use vibeproxy_core::usage_analytics::{self, Range};
use vibeproxy_core::{profile, shell, switch};

#[derive(Parser)]
#[command(name = "vibeproxy", version, about = "Switch Claude Code accounts and read usage, headless.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List configured accounts, marking the active one.
    List {
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Show the active account and its current usage.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Make an account active (by id or label). New terminals pick it up via the shell integration.
    Switch {
        /// Profile id or label.
        target: String,
    },
    /// Token-usage analytics across every account, from Claude Code's local logs.
    Usage {
        /// Inclusive local-date range, e.g. 2026-07-01..2026-07-31.
        #[arg(long, value_name = "FROM..TO")]
        range: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Write the usage analytics to a CSV file.
    Export {
        /// Output path.
        path: String,
        #[arg(long, value_name = "FROM..TO")]
        range: Option<String>,
    },
    /// Adopt an existing Claude login at a config dir as a new account.
    Adopt {
        /// A label for the account.
        label: String,
        /// The config dir (e.g. ~/.claude or ~/.vibeproxy/profiles/work).
        dir: String,
    },
    /// Remove an account from VibeProxy (leaves its Claude login untouched).
    Remove {
        /// Profile id or label.
        target: String,
    },
    /// Evaluate the quota threshold once and switch if the active account is over it.
    Auto,
    /// Print the shell line that makes switching reach your terminals (`eval "$(vibeproxy shell-init)"`).
    ShellInit {
        /// Append it to your shell profile instead of printing it.
        #[arg(long)]
        install: bool,
    },
}

fn main() -> ExitCode {
    if let Err(e) = store::ensure_initialized() {
        eprintln!("vibeproxy: could not initialize state: {e}");
        return ExitCode::FAILURE;
    }
    match run(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("vibeproxy: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cmd: Command) -> Result<(), String> {
    match cmd {
        Command::List { json } => list(json),
        Command::Status { json } => status(json),
        Command::Switch { target } => cmd_switch(&target),
        Command::Usage { range, json } => usage(range, json),
        Command::Export { path, range } => export(&path, range),
        Command::Adopt { label, dir } => adopt(label, &dir),
        Command::Remove { target } => remove(&target),
        Command::Auto => auto(),
        Command::ShellInit { install } => shell_init(install),
    }
}

/// Resolve a profile by id or by label.
fn resolve<'a>(cfg: &'a Config, key: &str) -> Result<&'a Profile, String> {
    cfg.profiles
        .iter()
        .find(|p| p.id == key || p.label == key)
        .ok_or_else(|| format!("no account matching \"{key}\""))
}

fn list(json: bool) -> Result<(), String> {
    let cfg = store::load();
    if json {
        println!("{}", to_json(&cfg.profiles)?);
        return Ok(());
    }
    if cfg.profiles.is_empty() {
        println!("No accounts yet. Add one with: vibeproxy adopt <label> <dir>");
        return Ok(());
    }
    for p in &cfg.profiles {
        let mark = if cfg.active_profile_id.as_deref() == Some(&p.id) { "*" } else { " " };
        let who = p.email.as_deref().unwrap_or(&p.config_dir);
        println!("{mark} {:<16} {}", p.label, who);
    }
    Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn status(json: bool) -> Result<(), String> {
    let cfg = store::load();
    let Some(active_id) = cfg.active_profile_id.clone() else {
        return Err("no active account. Set one with: vibeproxy switch <id|label>".into());
    };
    let p = resolve(&cfg, &active_id)?.clone();
    let u = poll_profile(&p.id, Path::new(&p.config_dir), true).await;

    if json {
        println!("{}", to_json(&u)?);
        return Ok(());
    }
    println!("Active: {}  ({})", p.label, p.email.as_deref().unwrap_or("unknown account"));
    match u.status {
        vibeproxy_core::usage::UsageStatus::Ok => {
            println!("  5-hour: {}", pct(u.five_hour_pct));
            println!("  weekly: {}", pct(u.weekly_pct));
        }
        vibeproxy_core::usage::UsageStatus::NeedsReauth => println!("  needs re-login"),
        vibeproxy_core::usage::UsageStatus::Error => {
            println!("  usage unavailable{}", u.error.map(|e| format!(" ({e})")).unwrap_or_default())
        }
    }
    Ok(())
}

fn cmd_switch(target: &str) -> Result<(), String> {
    let cfg = store::load();
    let p = resolve(&cfg, target)?;
    let (id, label) = (p.id.clone(), p.label.clone());
    switch::activate_profile(&id)?;
    println!("Switched to {label}. New terminals will use it (existing ones keep their account).");
    Ok(())
}

fn usage(range: Option<String>, json: bool) -> Result<(), String> {
    let range = parse_range(range)?;
    let a = usage_analytics::scan(&range);
    if json {
        println!("{}", to_json(&a)?);
        return Ok(());
    }
    let t = &a.totals;
    let total = t.input + t.output + t.cache_write + t.cache_read;
    println!("Total: {} tokens  ·  {} API-equivalent value  ·  {} messages",
        tokens(total), usd(a.total_value), a.message_count);
    if !a.per_model.is_empty() {
        println!("By model:");
        for m in &a.per_model {
            let mt = m.tokens.input + m.tokens.output + m.tokens.cache_write + m.tokens.cache_read;
            println!("  {:<28} {:>10}  {}", m.model, tokens(mt), usd(m.value.unwrap_or(0.0)));
        }
    }
    if !a.unpriced_models.is_empty() {
        println!("(unpriced: {})", a.unpriced_models.join(", "));
    }
    println!("Estimates; priced as of {}.", a.priced_as_of);
    Ok(())
}

fn export(path: &str, range: Option<String>) -> Result<(), String> {
    let range = parse_range(range)?;
    let csv = usage_analytics::to_csv(&usage_analytics::scan(&range));
    std::fs::write(path, csv).map_err(|e| format!("could not write {path}: {e}"))?;
    println!("Wrote {path}");
    Ok(())
}

fn adopt(label: String, dir: &str) -> Result<(), String> {
    let p = profile::adopt(label, dir)?;
    println!("Added {} ({}).", p.label, p.email.as_deref().unwrap_or("unknown account"));
    Ok(())
}

fn remove(target: &str) -> Result<(), String> {
    let cfg = store::load();
    let p = resolve(&cfg, target)?;
    let (id, label) = (p.id.clone(), p.label.clone());
    let was_active = cfg.active_profile_id.as_deref() == Some(&id);
    store::remove_profile(&id).map_err(|e| e.to_string())?;
    // Re-point the active account so it never dangles at a removed profile.
    if was_active {
        match store::load().profiles.first().map(|p| p.id.clone()) {
            Some(next) => switch::activate_profile(&next)?,
            None => switch::set_active_config_dir("").map_err(|e| e.to_string())?,
        }
    }
    println!("Removed {label}. Its Claude login is untouched.");
    Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn auto() -> Result<(), String> {
    let cfg = store::load();
    // Build the usage map by polling every profile (a one-shot; the app keeps this warm on a timer).
    let mut usage = std::collections::HashMap::new();
    for p in &cfg.profiles {
        let is_active = cfg.active_profile_id.as_deref() == Some(&p.id);
        usage.insert(p.id.clone(), poll_profile(&p.id, Path::new(&p.config_dir), is_active).await);
    }
    match vibeproxy_core::autoswitch::decide(&cfg, &usage, false) {
        vibeproxy_core::autoswitch::Decision::None => {
            println!("Active account is under the threshold — nothing to do.");
        }
        vibeproxy_core::autoswitch::Decision::Switch { target_id, target_label, from_label, pct } => {
            switch::activate_profile(&target_id)?;
            println!("{from_label} at {pct}% — switched to {target_label}.");
        }
        vibeproxy_core::autoswitch::Decision::Blocked { from_label, pct } => {
            println!("{from_label} at {pct}% and no other account has headroom.");
        }
    }
    Ok(())
}

fn shell_init(install: bool) -> Result<(), String> {
    if install {
        let file = shell::install()?;
        eprintln!("Added the shell integration to {file}. Open a new terminal for it to take effect.");
    } else {
        // Printed to stdout so it composes: eval "$(vibeproxy shell-init)"
        println!("{}", shell::snippet());
    }
    Ok(())
}

// --- formatting helpers (human-readable; --json is the machine path) ---

fn parse_range(range: Option<String>) -> Result<Option<Range>, String> {
    let Some(r) = range else { return Ok(None) };
    let (from, to) = r
        .split_once("..")
        .ok_or_else(|| "range must look like FROM..TO (e.g. 2026-07-01..2026-07-31)".to_string())?;
    let opt = |s: &str| (!s.is_empty()).then(|| s.to_string());
    Ok(Some(Range { from: opt(from), to: opt(to) }))
}

fn to_json<T: serde::Serialize>(v: &T) -> Result<String, String> {
    serde_json::to_string_pretty(v).map_err(|e| format!("serialize: {e}"))
}

/// `1234567` → `1.2M`. Uppercase suffixes, locale-independent — matches the GUI.
fn tokens(n: u64) -> String {
    let n = n as f64;
    let (suffix, div) = if n >= 1e9 { ("B", 1e9) } else if n >= 1e6 { ("M", 1e6) } else if n >= 1e3 { ("K", 1e3) } else { ("", 1.0) };
    if div == 1.0 {
        return format!("{}", n as u64);
    }
    let scaled = n / div;
    if scaled < 100.0 { format!("{scaled:.1}{suffix}") } else { format!("{:.0}{suffix}", scaled) }
}

fn usd(v: f64) -> String {
    format!("${v:.2}")
}

fn pct(v: Option<f32>) -> String {
    match v {
        Some(p) => format!("{}%", p.round() as i32),
        None => "—".into(),
    }
}
