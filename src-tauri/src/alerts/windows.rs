//! Alert windows: one click-through glow overlay per monitor plus one avatar
//! popup. None of them may ever take keyboard focus from the user's app.

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const AVATAR_LABEL: &str = "avatar";
const AVATAR_LOGICAL: f64 = 240.0;
/// Gap between the notch and the avatar popup (logical px).
const AVATAR_GAP: f64 = 12.0;

#[cfg(windows)]
mod native {
    // tao shows windows with SW_SHOW, which activates them and steals keyboard
    // focus even with WS_EX_NOACTIVATE. Call user32 directly (no extra crate).
    #[link(name = "user32")]
    extern "system" {
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    }
    const SW_HIDE: i32 = 0;
    const SW_SHOWNOACTIVATE: i32 = 4;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;

    pub fn show(w: &tauri::WebviewWindow) {
        if let Ok(h) = w.hwnd() {
            let h = h.0 as isize;
            // tao renders `decorations(false)` by answering WM_NCCALCSIZE, but only
            // forces that recalculation inside its own show(). Bypassing show()
            // left the default caption frame visible, so force it here.
            // SAFETY: `h` is a live HWND owned by `w`; neither call has other preconditions.
            unsafe {
                SetWindowPos(h, 0, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
                ShowWindow(h, SW_SHOWNOACTIVATE);
            }
        }
    }
    pub fn hide(w: &tauri::WebviewWindow) {
        if let Ok(h) = w.hwnd() {
            // SAFETY: as above.
            unsafe { ShowWindow(h.0 as isize, SW_HIDE) };
        }
    }
}

/// Show without activating (never steals keyboard focus). Visibility of alert
/// windows is owned here, not by tao's show()/hide(), so the two never disagree.
pub fn show_no_focus(w: &WebviewWindow) {
    #[cfg(windows)]
    native::show(w);
    #[cfg(not(windows))]
    let _ = w.show();
}

pub fn hide_window(w: &WebviewWindow) {
    #[cfg(windows)]
    native::hide(w);
    #[cfg(not(windows))]
    let _ = w.hide();
}

pub fn glow_label(i: usize) -> String {
    format!("glow-{i}")
}

fn glow_labels(app: &AppHandle) -> Vec<String> {
    app.webview_windows().into_keys().filter(|l| l.starts_with("glow-")).collect()
}

fn build_glow(app: &AppHandle, label: &str) -> tauri::Result<WebviewWindow> {
    let w = WebviewWindowBuilder::new(app, label, WebviewUrl::App("glow.html".into()))
        .title("Nudge glow")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        // Never steal focus: not focused on creation and not activatable.
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()?;
    w.set_ignore_cursor_events(true)?;
    Ok(w)
}

/// Make the set of `glow-<n>` windows match the monitors that exist right now
/// and size each to its monitor's full bounds. Returns the labels in use.
pub fn sync_glow(app: &AppHandle) -> Vec<String> {
    let monitors = app.available_monitors().unwrap_or_default();
    let mut labels = Vec::new();
    for (i, m) in monitors.iter().enumerate() {
        let label = glow_label(i);
        let win = match app.get_webview_window(&label) {
            Some(w) => Some(w),
            None => build_glow(app, &label).map_err(|e| eprintln!("nudge: glow window {label}: {e}")).ok(),
        };
        let Some(win) = win else { continue };
        let _ = win.set_position(PhysicalPosition::new(m.position().x, m.position().y));
        let _ = win.set_size(PhysicalSize::new(m.size().width, m.size().height));
        labels.push(label);
    }
    for stale in glow_labels(app) {
        if !labels.contains(&stale) {
            if let Some(w) = app.get_webview_window(&stale) {
                let _ = w.close();
            }
        }
    }
    labels
}

pub fn build_avatar(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, AVATAR_LABEL, WebviewUrl::App("avatar.html".into()))
        .title("Nudge avatar")
        .inner_size(AVATAR_LOGICAL, AVATAR_LOGICAL)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()
}

/// Pure placement: put a `w`x`h` box `gap` px inward of `notch` (x, y, w, h),
/// toward the screen interior, then clamp inside `mon` (x, y, w, h).
pub fn avatar_position(
    notch: (i32, i32, i32, i32),
    mon: (i32, i32, i32, i32),
    (w, h): (i32, i32),
    gap: i32,
) -> (i32, i32) {
    let (nx, ny, nw, nh) = notch;
    let (mx, my, mw, mh) = mon;
    let to_left = nx - mx;
    let to_right = (mx + mw) - (nx + nw);
    let to_top = ny - my;
    let to_bottom = (my + mh) - (ny + nh);
    let nearest = to_left.min(to_right).min(to_top).min(to_bottom);
    let (x, y) = if nearest == to_left {
        (nx + nw + gap, ny + nh / 2 - h / 2)
    } else if nearest == to_right {
        (nx - w - gap, ny + nh / 2 - h / 2)
    } else if nearest == to_top {
        (nx + nw / 2 - w / 2, ny + nh + gap)
    } else {
        (nx + nw / 2 - w / 2, ny - h - gap)
    };
    (x.clamp(mx, (mx + mw - w).max(mx)), y.clamp(my, (my + mh - h).max(my)))
}

/// Move the avatar next to the notch (best effort; falls back to where it is).
pub fn place_avatar(app: &AppHandle, avatar: &WebviewWindow) {
    let Some(notch) = app.get_webview_window("notch") else { return };
    let (Ok(p), Ok(s), Ok(Some(m))) = (notch.outer_position(), notch.outer_size(), notch.current_monitor()) else {
        return;
    };
    let Ok(asz) = avatar.outer_size() else { return };
    let gap = (AVATAR_GAP * m.scale_factor()).round() as i32;
    let (x, y) = avatar_position(
        (p.x, p.y, s.width as i32, s.height as i32),
        (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32),
        (asz.width as i32, asz.height as i32),
        gap,
    );
    let _ = avatar.set_position(PhysicalPosition::new(x, y));
}

#[cfg(test)]
mod tests {
    use super::*;

    const MON: (i32, i32, i32, i32) = (0, 0, 1920, 1080);

    #[test]
    fn avatar_goes_left_of_a_right_edge_notch() {
        let (x, y) = avatar_position((1700, 400, 220, 44), MON, (240, 240), 12);
        assert_eq!(x, 1700 - 240 - 12);
        assert_eq!(y, 400 + 22 - 120);
    }

    #[test]
    fn avatar_goes_below_a_top_edge_notch() {
        let (x, y) = avatar_position((850, 0, 220, 44), MON, (240, 240), 12);
        assert_eq!(y, 44 + 12);
        assert_eq!(x, 850 + 110 - 120);
    }

    #[test]
    fn avatar_is_clamped_to_the_monitor() {
        let (x, y) = avatar_position((0, 10, 220, 44), MON, (240, 240), 12);
        assert_eq!(x, 232);
        assert_eq!(y, 0);
        let (_, y) = avatar_position((0, 1060, 220, 44), (0, 0, 1920, 1080), (240, 240), 12);
        assert!(y + 240 <= 1080);
    }
}
