//! Advisory locks compatible with the ones Claude Code takes for its own OAuth refresh.
//!
//! Claude Code reads, refreshes over the network, and saves credentials while holding these. A swap
//! that writes underneath that sequence gets silently overwritten by the refresh that follows. The
//! lock is a directory because `mkdir` is atomic on every filesystem we care about.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Matches `proper-lockfile`'s default staleness window, which is what Claude Code uses.
const STALE_AFTER: Duration = Duration::from_secs(10);

/// Held lock, released on drop — including on panic, so a crash mid-swap cannot strand it.
pub struct LockGuard {
    dirs: Vec<PathBuf>,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        for d in &self.dirs {
            let _ = std::fs::remove_dir(d);
        }
    }
}

fn is_stale(dir: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(dir) else { return false };
    let Ok(modified) = meta.modified() else { return false };
    SystemTime::now().duration_since(modified).map(|age| age > STALE_AFTER).unwrap_or(false)
}

fn try_acquire_one(dir: &Path) -> bool {
    match std::fs::create_dir(dir) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Reclaim a lock whose holder died without releasing it.
            if is_stale(dir) {
                let _ = std::fs::remove_dir(dir);
                std::fs::create_dir(dir).is_ok()
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Take every lock or none. Returns `None` rather than blocking: a swap that cannot get exclusive
/// access must abort and report, never queue behind a long-running refresh.
pub fn acquire(paths: &[PathBuf], attempts: u32, gap: Duration) -> Option<LockGuard> {
    for _ in 0..attempts.max(1) {
        let mut held = Vec::new();
        let mut ok = true;
        for p in paths {
            if try_acquire_one(p) {
                held.push(p.clone());
            } else {
                ok = false;
                break;
            }
        }
        if ok {
            return Some(LockGuard { dirs: held });
        }
        drop(LockGuard { dirs: held }); // release partial set before retrying
        std::thread::sleep(gap);
    }
    None
}

/// The locks Claude Code itself takes, in a fixed order so two swappers cannot deadlock.
pub fn claude_lock_paths() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return Vec::new() };
    vec![home.join(".claude.lock"), home.join(".claude.json.lock")]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!("vp-lock-{}", std::process::id()))
    }

    #[test]
    fn second_acquire_fails_while_first_is_held() {
        let p = vec![tmp().with_extension("a")];
        let _ = std::fs::remove_dir(&p[0]);
        let held = acquire(&p, 1, Duration::from_millis(1)).expect("first acquire");
        assert!(acquire(&p, 1, Duration::from_millis(1)).is_none(), "must not double-acquire");
        drop(held);
        assert!(acquire(&p, 1, Duration::from_millis(1)).is_some(), "released on drop");
    }

    #[test]
    fn partial_acquisition_is_fully_released() {
        let a = tmp().with_extension("p1");
        let b = tmp().with_extension("p2");
        for d in [&a, &b] {
            let _ = std::fs::remove_dir(d);
        }
        let blocker = acquire(std::slice::from_ref(&b), 1, Duration::from_millis(1)).unwrap();
        // Wants a then b; b is taken, so it must give a back rather than stranding it.
        assert!(acquire(&[a.clone(), b.clone()], 1, Duration::from_millis(1)).is_none());
        assert!(!a.exists(), "partially held lock must be released on failure");
        drop(blocker);
    }
}
