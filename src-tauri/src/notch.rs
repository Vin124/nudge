//! Notch window mechanics that must not round-trip through JS per frame (D20):
//! click-through outside the visible shape, cursor-driven drag, snap animation.
//!
//! The notch window is a fixed transparent envelope; the UI draws and animates
//! the shape inside it and reports the shape's rect here. The window ignores
//! the cursor everywhere except over that rect.
//!
//! Lock rule: never call a window getter while holding `inner`. Getters wait on
//! the main thread, and sync commands lock `inner` on the main thread.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow};

pub const LABEL: &str = "notch";
pub const EVENT_HOVER: &str = "nudge://notch-hover";
pub const EVENT_DROP: &str = "nudge://notch-drop";
const HOVER_EVERY: Duration = Duration::from_millis(33);
const FRAME: Duration = Duration::from_millis(8);

/// Window-relative rect in logical (CSS) px.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy)]
struct Drag {
    grab: (f64, f64),
    last: Pos,
}

#[derive(Default)]
struct Inner {
    hit: Option<Rect>,
    drag: Option<Drag>,
    inside: bool,
    ignoring: Option<bool>,
}

#[derive(Default)]
pub struct Notch {
    inner: Mutex<Inner>,
    /// Bumped to cancel a running snap animation.
    anim: AtomicU64,
}

/// Is the physical `cursor` inside `hit` for a window at physical `origin`?
pub fn contains(hit: Rect, scale: f64, origin: (f64, f64), cursor: (f64, f64)) -> bool {
    let x = (cursor.0 - origin.0) / scale;
    let y = (cursor.1 - origin.1) / scale;
    x >= hit.x && x < hit.x + hit.width && y >= hit.y && y < hit.y + hit.height
}

/// Window top-left that keeps the grab point under the cursor.
pub fn drag_pos(cursor: (f64, f64), grab: (f64, f64)) -> Pos {
    Pos { x: (cursor.0 - grab.0).round() as i32, y: (cursor.1 - grab.1).round() as i32 }
}

/// Ease-out cubic. Must match the UI's `cubic-bezier(0.215, 0.61, 0.355, 1)`.
pub fn ease_out_cubic(t: f64) -> f64 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

pub fn lerp_pos(a: Pos, b: Pos, t: f64) -> Pos {
    Pos {
        x: (a.x as f64 + (b.x - a.x) as f64 * t).round() as i32,
        y: (a.y as f64 + (b.y - a.y) as f64 * t).round() as i32,
    }
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// The visible shape's screen rect (x, y, w, h, physical), for placing popups next to it.
pub fn shape_screen_rect(app: &AppHandle) -> Option<(i32, i32, i32, i32)> {
    let w = window(app)?;
    let (o, s) = (w.outer_position().ok()?, w.scale_factor().ok()?);
    let hit = app.try_state::<Arc<Notch>>()?.inner.lock().unwrap().hit?;
    let px = |v: f64| (v * s).round() as i32;
    Some((o.x + px(hit.x), o.y + px(hit.y), px(hit.width), px(hit.height)))
}

#[cfg(windows)]
fn primary_button_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SWAPBUTTON};
    // SAFETY: plain Win32 queries with no pointers.
    unsafe {
        let vk = if GetSystemMetrics(SM_SWAPBUTTON) != 0 { VK_RBUTTON } else { VK_LBUTTON };
        (GetAsyncKeyState(vk.0 as i32) as u16) & 0x8000 != 0
    }
}

/// No portable poll; the UI's pointerup ends the drag instead.
#[cfg(not(windows))]
fn primary_button_down() -> bool {
    true
}

fn set_ignore(w: &WebviewWindow, ignore: bool) {
    if let Err(e) = w.set_ignore_cursor_events(ignore) {
        eprintln!("nudge: notch set_ignore_cursor_events failed: {e}");
    }
}

pub fn spawn_hover(app: AppHandle, notch: Arc<Notch>) {
    std::thread::Builder::new()
        .name("nudge-notch-hover".into())
        .spawn(move || loop {
            std::thread::sleep(HOVER_EVERY);
            let Some(w) = window(&app) else { continue };
            let (Ok(c), Ok(o), Ok(s)) = (w.cursor_position(), w.outer_position(), w.scale_factor()) else {
                continue;
            };
            let (ignore, hover) = {
                let mut g = notch.inner.lock().unwrap();
                if g.drag.is_some() {
                    continue;
                }
                let inside = g.hit.is_some_and(|r| contains(r, s, (o.x as f64, o.y as f64), (c.x, c.y)));
                let ignore = (g.ignoring != Some(!inside)).then_some(!inside);
                g.ignoring = Some(!inside);
                let hover = (g.inside != inside).then_some(inside);
                g.inside = inside;
                (ignore, hover)
            };
            if let Some(i) = ignore {
                set_ignore(&w, i);
            }
            if let Some(h) = hover {
                let _ = app.emit_to(LABEL, EVENT_HOVER, h);
            }
        })
        .expect("spawn notch hover");
}

/// End the drag once (UI pointerup or button poll, whichever comes first).
fn finish_drag(app: &AppHandle, notch: &Notch) {
    let done = notch.inner.lock().unwrap().drag.take();
    if let Some(d) = done {
        let _ = app.emit_to(LABEL, EVENT_DROP, d.last);
    }
}

#[tauri::command]
pub fn notch_set_hit(notch: State<'_, Arc<Notch>>, rect: Option<Rect>) {
    notch.inner.lock().unwrap().hit = rect;
}

#[tauri::command]
pub fn notch_drag_start(app: AppHandle, notch: State<'_, Arc<Notch>>) -> Result<(), String> {
    let w = window(&app).ok_or("no notch window")?;
    let c = w.cursor_position().map_err(|e| e.to_string())?;
    let o = w.outer_position().map_err(|e| e.to_string())?;
    notch.anim.fetch_add(1, Ordering::SeqCst);
    {
        let mut g = notch.inner.lock().unwrap();
        if g.drag.is_some() {
            return Ok(());
        }
        g.drag = Some(Drag { grab: (c.x - o.x as f64, c.y - o.y as f64), last: Pos { x: o.x, y: o.y } });
        g.ignoring = Some(false);
    }
    set_ignore(&w, false);
    let notch = notch.inner().clone();
    std::thread::Builder::new()
        .name("nudge-notch-drag".into())
        .spawn(move || loop {
            std::thread::sleep(FRAME);
            if !primary_button_down() {
                finish_drag(&app, &notch);
                return;
            }
            let Ok(c) = w.cursor_position() else { continue };
            let p = {
                let mut g = notch.inner.lock().unwrap();
                let Some(d) = g.drag.as_mut() else { return };
                let p = drag_pos((c.x, c.y), d.grab);
                if p == d.last {
                    continue;
                }
                d.last = p;
                p
            };
            let _ = w.set_position(PhysicalPosition::new(p.x, p.y));
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn notch_drag_end(app: AppHandle, notch: State<'_, Arc<Notch>>) {
    finish_drag(&app, &notch);
}

/// Slide the window to `(x, y)` (physical) over `ms`, ease-out cubic.
#[tauri::command]
pub fn notch_animate_to(app: AppHandle, notch: State<'_, Arc<Notch>>, x: i32, y: i32, ms: u64) -> Result<(), String> {
    let w = window(&app).ok_or("no notch window")?;
    let o = w.outer_position().map_err(|e| e.to_string())?;
    let gen = notch.anim.fetch_add(1, Ordering::SeqCst) + 1;
    let notch = notch.inner().clone();
    let (from, to) = (Pos { x: o.x, y: o.y }, Pos { x, y });
    std::thread::Builder::new()
        .name("nudge-notch-snap".into())
        .spawn(move || {
            let start = Instant::now();
            let total = Duration::from_millis(ms.max(1));
            loop {
                if notch.anim.load(Ordering::SeqCst) != gen {
                    return;
                }
                let t = start.elapsed().as_secs_f64() / total.as_secs_f64();
                let p = lerp_pos(from, to, ease_out_cubic(t));
                let _ = w.set_position(PhysicalPosition::new(p.x, p.y));
                if t >= 1.0 {
                    return;
                }
                std::thread::sleep(FRAME);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIT: Rect = Rect { x: 100.0, y: 0.0, width: 200.0, height: 46.0 };

    #[test]
    fn contains_converts_physical_to_logical() {
        // Window at (1000, 0), scale 1.5: logical (100, 0) is physical (1150, 0).
        assert!(contains(HIT, 1.5, (1000.0, 0.0), (1150.0, 0.0)));
        assert!(contains(HIT, 1.5, (1000.0, 0.0), (1449.0, 68.0)));
        assert!(!contains(HIT, 1.5, (1000.0, 0.0), (1149.0, 10.0)));
        assert!(!contains(HIT, 1.5, (1000.0, 0.0), (1450.0, 10.0)));
        assert!(!contains(HIT, 1.5, (1000.0, 0.0), (1200.0, 69.0)));
    }

    #[test]
    fn contains_handles_negative_monitor_origins() {
        assert!(contains(HIT, 1.0, (-1920.0, -200.0), (-1720.0, -190.0)));
    }

    #[test]
    fn drag_pos_keeps_grab_point_under_cursor() {
        assert_eq!(drag_pos((500.4, 300.6), (40.0, 20.0)), Pos { x: 460, y: 281 });
    }

    #[test]
    fn ease_and_lerp_hit_endpoints() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert_eq!(ease_out_cubic(2.0), 1.0);
        assert!(ease_out_cubic(0.5) > 0.5, "ease-out front-loads motion");
        let (a, b) = (Pos { x: 0, y: 100 }, Pos { x: 200, y: 0 });
        assert_eq!(lerp_pos(a, b, 0.0), a);
        assert_eq!(lerp_pos(a, b, 1.0), b);
        assert_eq!(lerp_pos(a, b, 0.5), Pos { x: 100, y: 50 });
    }
}
