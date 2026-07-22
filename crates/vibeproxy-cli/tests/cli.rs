//! Hermetic CLI integration test: drives the real binary against an isolated `VIBEPROXY_DIR` with
//! pre-seeded profiles, so it exercises list / switch / remove / shell-init without any network or
//! `claude` dependency. `adopt`, `status`, `usage`, and `auto` need a real login or logs and are
//! covered by manual smoke-testing, not here.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

const BIN: &str = env!("CARGO_BIN_EXE_vibeproxy");

fn temp_dir() -> PathBuf {
    // Unique per test: process::id() alone is shared across a binary's parallel tests, which had
    // them clobbering one config.json.
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vp-cli-test-{}-{}", std::process::id(), n));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    // Two accounts, "Work" active. Non-default config dirs so activate writes a real path.
    fs::write(
        d.join("config.json"),
        r#"{"schemaVersion":1,"activeProfileId":"a","profiles":[
            {"id":"a","label":"Work","configDir":"/tmp/vp-a","email":"work@example.com"},
            {"id":"b","label":"Personal","configDir":"/tmp/vp-b","email":"me@example.com"}
        ]}"#,
    )
    .unwrap();
    d
}

fn run(dir: &PathBuf, args: &[&str]) -> (String, String, bool) {
    let out = Command::new(BIN)
        .args(args)
        .env("VIBEPROXY_DIR", dir)
        .output()
        .expect("run vibeproxy");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

#[test]
fn list_marks_the_active_account() {
    let d = temp_dir();
    let (out, _, ok) = run(&d, &["list"]);
    assert!(ok);
    assert!(out.contains("Work") && out.contains("Personal"));
    // Active marker on Work, not Personal.
    let work_line = out.lines().find(|l| l.contains("Work")).unwrap();
    assert!(work_line.trim_start().starts_with('*'), "active account is marked: {work_line:?}");
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn list_json_is_parseable() {
    let d = temp_dir();
    let (out, _, ok) = run(&d, &["list", "--json"]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
    assert_eq!(v.as_array().unwrap().len(), 2);
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn switch_by_label_writes_the_active_path() {
    let d = temp_dir();
    let (_, _, ok) = run(&d, &["switch", "Personal"]);
    assert!(ok);
    // active-path now points at Personal's non-default dir.
    let active = fs::read_to_string(d.join("active-path")).unwrap();
    assert_eq!(active.trim(), "/tmp/vp-b");
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn switch_unknown_account_fails_cleanly() {
    let d = temp_dir();
    let (_, err, ok) = run(&d, &["switch", "Nonexistent"]);
    assert!(!ok, "unknown account is a non-zero exit");
    assert!(err.contains("no account matching"));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn remove_drops_the_account() {
    let d = temp_dir();
    let (_, _, ok) = run(&d, &["remove", "Personal"]);
    assert!(ok);
    let (out, _, _) = run(&d, &["list"]);
    assert!(!out.contains("Personal") && out.contains("Work"));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn shell_init_prints_the_integration_line_to_stdout() {
    let d = temp_dir();
    let (out, _, ok) = run(&d, &["shell-init"]);
    assert!(ok);
    // The line must reference the active-path file so `eval "$(vibeproxy shell-init)"` works.
    assert!(out.contains("vibeproxy/active-path"), "snippet: {out}");
    let _ = fs::remove_dir_all(&d);
}
