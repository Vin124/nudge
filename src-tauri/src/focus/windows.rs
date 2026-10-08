//! Windows: find the host's top-level window and force it to the foreground.

use super::pick;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, KEYBD_EVENT_FLAGS, VK_MENU};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, SetForegroundWindow, ShowWindow, GWL_EXSTYLE, GW_OWNER, SW_RESTORE, WS_EX_TOOLWINDOW,
};

/// Visible, unowned, non-tool top-level windows in z-order (topmost first), with owner pid.
fn app_windows() -> Vec<(HWND, u32)> {
    unsafe extern "system" fn cb(hwnd: HWND, lp: LPARAM) -> BOOL {
        // SAFETY: lp is the &mut Vec passed to EnumWindows below, alive for the whole call.
        let out = unsafe { &mut *(lp.0 as *mut Vec<(HWND, u32)>) };
        unsafe {
            let unowned = GetWindow(hwnd, GW_OWNER).map(|o| o.is_invalid()).unwrap_or(true);
            let tool = (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32) & WS_EX_TOOLWINDOW.0 != 0;
            if IsWindowVisible(hwnd).as_bool() && unowned && !tool {
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                out.push((hwnd, pid));
            }
        }
        BOOL(1)
    }
    let mut out: Vec<(HWND, u32)> = Vec::new();
    // SAFETY: the callback only touches `out` through the pointer we pass.
    let _ = unsafe { EnumWindows(Some(cb), LPARAM(&mut out as *mut _ as isize)) };
    out
}

fn windows_terminal_pids() -> Vec<u32> {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System};
    let sys = System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()));
    sys.processes()
        .iter()
        .filter(|(_, p)| pick::norm(&p.name().to_string_lossy()) == "windowsterminal")
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

fn title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    // SAFETY: buf outlives the call and its length bounds the write.
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// The best-ranked candidate pid that owns a window; among that pid's windows,
/// the one titled with the session's folder, else the frontmost.
fn window_for(wins: &[(HWND, u32)], pids: &[u32], cwd: &str) -> Option<HWND> {
    pids.iter().find_map(|pid| {
        let mine: Vec<HWND> = wins.iter().filter(|(_, p)| p == pid).map(|(h, _)| *h).collect();
        if mine.is_empty() {
            return None;
        }
        let titles: Vec<String> = mine.iter().map(|h| title(*h)).collect();
        Some(mine[pick::best_window(&titles, cwd)])
    })
}

pub fn focus(candidates: &[u32], cwd: &str) -> Result<(), String> {
    let wins = app_windows();
    let hwnd = match window_for(&wins, candidates, cwd) {
        Some(h) => h,
        None => {
            // WT caveat: the chain may stop at OpenConsole / a re-parented shell.
            // z-order puts the most recently active WT window first.
            let h = window_for(&wins, &windows_terminal_pids(), cwd).ok_or("no terminal window found")?;
            eprintln!("nudge: focus: no ancestor window, fell back to the frontmost Windows Terminal window");
            h
        }
    };
    bring_to_front(hwnd)
}

fn is_front(hwnd: HWND) -> bool {
    unsafe { GetForegroundWindow() == hwnd }
}

fn bring_to_front(hwnd: HWND) -> Result<(), String> {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        // 1) AttachThreadInput to the foreground thread: lets a non-foreground
        //    process (our non-focusable notch) take the foreground lock.
        let fg_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let me = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
        let _ = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);
        if attached {
            let _ = AttachThreadInput(me, fg_thread, false);
        }
        if is_front(hwnd) {
            eprintln!("nudge: focus: ok via AttachThreadInput");
            return Ok(());
        }
        // 2) Synthetic ALT press: counts as recent input, which unlocks SetForegroundWindow.
        keybd_event(VK_MENU.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        let _ = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);
        if is_front(hwnd) {
            eprintln!("nudge: focus: ok via ALT-key fallback");
            return Ok(());
        }
    }
    Err("terminal window found but Windows refused to bring it to the front".into())
}
