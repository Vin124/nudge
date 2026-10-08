//! Click-to-focus terminal (lane L4, D7). Best effort, window-level.

mod pick;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use nudge_proto::ProcInfo;

/// Bring the terminal window hosting this session to the foreground.
/// `ancestors` = the hook process's ancestors, nearest parent first; `cwd` picks
/// the right window when the host process owns several.
pub fn focus_session(ancestors: &[ProcInfo], cwd: &str) -> Result<(), String> {
    let candidates = pick::candidates(ancestors);
    platform_focus(&candidates, cwd)
}

#[cfg(windows)]
fn platform_focus(candidates: &[u32], cwd: &str) -> Result<(), String> {
    // The Windows path has a Windows Terminal fallback, so it runs even with no candidates.
    windows::focus(candidates, cwd)
}

#[cfg(target_os = "macos")]
fn platform_focus(candidates: &[u32], _cwd: &str) -> Result<(), String> {
    if candidates.is_empty() {
        return Err("no terminal app in the session's process chain".into());
    }
    macos::focus(candidates)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_focus(_candidates: &[u32], _cwd: &str) -> Result<(), String> {
    Err("focus is not supported on this platform".into())
}
