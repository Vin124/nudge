//! Tray menu (lane L5): Settings, Do Not Disturb, timed pause, Quit.
//! Pause is tray-local state layered on `config.dnd`; the decision logic is
//! pure (`begin`, `tick`, `label`) so it can be tested without a window system.

use crate::hub::Core;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

const SETTINGS: &str = "settings";
const ID_SETTINGS: &str = "open-settings";
const ID_DND: &str = "dnd";
const ID_P30: &str = "pause-30";
const ID_P60: &str = "pause-60";
const ID_PFOREVER: &str = "pause-forever";
const ID_QUIT: &str = "quit";
const TICK: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseLen {
    Minutes(u64),
    Forever,
}

/// A pause that this tray started. `deadline_ms == None` means "until I resume".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pause {
    pub active: bool,
    pub deadline_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickAction {
    Nothing,
    /// Deadline reached and the pause still owns DND: turn DND off.
    RestoreDnd,
}

/// Starting a pause: the caller must also set `dnd = true`.
pub fn begin(len: PauseLen, now_ms: u64) -> Pause {
    Pause {
        active: true,
        deadline_ms: match len {
            PauseLen::Minutes(m) => Some(now_ms + m * 60_000),
            PauseLen::Forever => None,
        },
    }
}

/// One timer step. If DND is already off the user (or Settings) changed it
/// manually, so the pause is dropped and nothing is restored.
pub fn tick(p: Pause, dnd_now: bool, now_ms: u64) -> (Pause, TickAction) {
    if !p.active {
        return (p, TickAction::Nothing);
    }
    if !dnd_now {
        return (Pause::default(), TickAction::Nothing);
    }
    match p.deadline_ms {
        Some(d) if now_ms >= d => (Pause::default(), TickAction::RestoreDnd),
        _ => (p, TickAction::Nothing),
    }
}

pub fn label(p: Pause, now_ms: u64) -> String {
    match (p.active, p.deadline_ms) {
        (false, _) => "Pause alerts".to_string(),
        (true, None) => "Paused — until you resume".to_string(),
        (true, Some(d)) => {
            let mins = d.saturating_sub(now_ms).div_ceil(60_000);
            format!("Paused — {mins} min left")
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn set_dnd(core: &Core, on: bool) {
    let mut cfg = core.config();
    if cfg.dnd != on {
        cfg.dnd = on;
        if let Err(e) = core.set_config(cfg) {
            eprintln!("nudge: could not save DND change: {e}");
        }
    }
}

pub fn init(app: &AppHandle, core: Arc<Core>) -> tauri::Result<()> {
    // Closing Settings hides it so reopening is instant and keeps its state.
    if let Some(w) = app.get_webview_window(SETTINGS) {
        let w2 = w.clone();
        w.on_window_event(move |ev| {
            if let WindowEvent::CloseRequested { api, .. } = ev {
                api.prevent_close();
                let _ = w2.hide();
            }
        });
    }

    let open = MenuItem::with_id(app, ID_SETTINGS, "Settings…", true, None::<&str>)?;
    let dnd = CheckMenuItem::with_id(app, ID_DND, "Do Not Disturb", true, core.config().dnd, None::<&str>)?;
    let p30 = MenuItem::with_id(app, ID_P30, "30 minutes", true, None::<&str>)?;
    let p60 = MenuItem::with_id(app, ID_P60, "1 hour", true, None::<&str>)?;
    let pinf = MenuItem::with_id(app, ID_PFOREVER, "Until I resume", true, None::<&str>)?;
    let pause_menu = Submenu::with_items(app, "Pause alerts", true, &[&p30, &p60, &pinf])?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit Nudge", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &dnd, &pause_menu, &sep, &quit])?;

    let pause = Arc::new(Mutex::new(Pause::default()));

    // Keep the check item and pause label in sync with config changes made
    // elsewhere, and expire timed pauses.
    {
        let (core, pause, dnd, pause_menu) = (core.clone(), pause.clone(), dnd.clone(), pause_menu.clone());
        std::thread::Builder::new()
            .name("nudge-tray-tick".into())
            .spawn(move || loop {
                std::thread::sleep(TICK);
                let now = now_ms();
                let cur = core.config().dnd;
                let p = {
                    let mut g = pause.lock().unwrap_or_else(|e| e.into_inner());
                    let (next, action) = tick(*g, cur, now);
                    *g = next;
                    if action == TickAction::RestoreDnd {
                        set_dnd(&core, false);
                    }
                    *g
                };
                let _ = dnd.set_checked(core.config().dnd);
                let _ = pause_menu.set_text(label(p, now));
            })
            .map_err(tauri::Error::Io)?;
    }

    let c = core.clone();
    let pause_ev = pause.clone();
    let (dnd_ev, pm_ev) = (dnd.clone(), pause_menu.clone());
    let mut builder = TrayIconBuilder::new()
        .tooltip("Nudge")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, ev| {
            let start = |len: PauseLen| {
                let p = begin(len, now_ms());
                *pause_ev.lock().unwrap_or_else(|e| e.into_inner()) = p;
                set_dnd(&c, true);
                let _ = dnd_ev.set_checked(true);
                let _ = pm_ev.set_text(label(p, now_ms()));
            };
            match ev.id().as_ref() {
                ID_SETTINGS => show_settings(app),
                ID_DND => {
                    // A manual toggle takes ownership of DND away from any pause.
                    *pause_ev.lock().unwrap_or_else(|e| e.into_inner()) = Pause::default();
                    set_dnd(&c, !c.config().dnd);
                    let _ = dnd_ev.set_checked(c.config().dnd);
                    let _ = pm_ev.set_text(label(Pause::default(), now_ms()));
                }
                ID_P30 => start(PauseLen::Minutes(30)),
                ID_P60 => start(PauseLen::Minutes(60)),
                ID_PFOREVER => start(PauseLen::Forever),
                ID_QUIT => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                show_settings(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 1_000_000;

    #[test]
    fn pause_sets_deadline_and_label() {
        let p = begin(PauseLen::Minutes(30), T0);
        assert!(p.active);
        assert_eq!(p.deadline_ms, Some(T0 + 30 * 60_000));
        assert_eq!(label(p, T0 + 7 * 60_000), "Paused — 23 min left");
        assert_eq!(label(Pause::default(), T0), "Pause alerts");
    }

    #[test]
    fn deadline_restores_dnd() {
        let p = begin(PauseLen::Minutes(30), T0);
        assert_eq!(tick(p, true, T0 + 29 * 60_000), (p, TickAction::Nothing));
        assert_eq!(tick(p, true, T0 + 30 * 60_000), (Pause::default(), TickAction::RestoreDnd));
    }

    #[test]
    fn manual_dnd_change_blocks_restore() {
        let p = begin(PauseLen::Minutes(30), T0);
        // user switched DND off mid-pause; at the deadline we must not touch it
        let (p2, a) = tick(p, false, T0 + 10 * 60_000);
        assert_eq!((p2, a), (Pause::default(), TickAction::Nothing));
        assert_eq!(tick(p2, true, T0 + 31 * 60_000).1, TickAction::Nothing);
    }

    #[test]
    fn until_resume_has_no_deadline() {
        let p = begin(PauseLen::Forever, T0);
        assert_eq!(p.deadline_ms, None);
        assert_eq!(tick(p, true, T0 + 10_000 * 60_000), (p, TickAction::Nothing));
        assert_eq!(label(p, T0), "Paused — until you resume");
    }
}
