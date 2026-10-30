//! One BudMix at a time, where the copy opened last wins.
//!
//! Without this, opening a newly downloaded `BudMix.exe` while an older
//! copy runs in the tray only brought the old copy forward, so the update
//! seemed to do nothing.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// Passed to a copy that is taking over, so it waits for the old one to exit.
pub const REPLACE_ARG: &str = "--replace";
const REPLACE_WAIT: Duration = Duration::from_secs(2);

/// Called before anything else starts: a copy taking over gives the one it
/// replaces time to exit and release the single-instance lock.
pub fn wait_if_replacing() {
    if std::env::args().any(|arg| arg == REPLACE_ARG) {
        thread::sleep(REPLACE_WAIT);
    }
}

/// Runs in the copy that is already open when another one starts.
pub fn second_instance(app: &AppHandle, args: &[String], cwd: &str) {
    let other = args.first().map(|exe| resolve(exe, Path::new(cwd)));
    let this = std::env::current_exe().ok();
    match (other, this) {
        (Some(other), Some(this)) if !same_executable(&other, &this) => hand_over(app, &other),
        _ => crate::dock::show(app),
    }
}

fn hand_over(app: &AppHandle, other: &Path) {
    log::info!(
        "BudMix was opened from {}; handing over to it",
        other.display()
    );
    if let Err(err) = Command::new(other).arg(REPLACE_ARG).spawn() {
        log::warn!("could not start {}: {err}", other.display());
        crate::dock::show(app);
        return;
    }
    if let Some(state) = app.try_state::<AppState>() {
        state.prepare_quit();
    }
    app.exit(0);
}

/// `exe` as an absolute path, relative paths taken from the caller's `cwd`.
pub fn resolve(exe: &str, cwd: &Path) -> PathBuf {
    let path = Path::new(exe);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// Whether two paths name the same program file. Windows paths compare
/// without regard to case.
pub fn same_executable(a: &Path, b: &Path) -> bool {
    let canonical = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let (a, b) = (canonical(a), canonical(b));
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn resolves_relative_paths_against_the_callers_folder() {
        let cwd = std::env::temp_dir();
        assert_eq!(resolve("BudMix.exe", &cwd), cwd.join("BudMix.exe"));
        let absolute = cwd.join("bin").join("BudMix.exe");
        assert_eq!(
            resolve(absolute.to_str().unwrap(), Path::new("/elsewhere")),
            absolute
        );
    }

    #[test]
    fn same_file_through_different_spellings_is_the_same_program() {
        let dir = std::env::temp_dir().join(format!("budmix-instance-{}", std::process::id()));
        fs::create_dir_all(dir.join("sub")).unwrap();
        let exe = dir.join("BudMix.exe");
        fs::write(&exe, b"").unwrap();

        assert!(same_executable(
            &exe,
            &dir.join("sub").join("..").join("BudMix.exe")
        ));

        let other = dir.join("BudMix (1).exe");
        fs::write(&other, b"").unwrap();
        assert!(!same_executable(&exe, &other));

        fs::remove_dir_all(dir).unwrap();
    }
}
