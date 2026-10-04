//! macOS: activate the host app by pid. Compiles only on macOS; unverified at
//! runtime (built and tested on Windows only).

use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

pub fn focus(candidates: &[u32]) -> Result<(), String> {
    for &pid in candidates {
        let Ok(pid) = i32::try_from(pid) else { continue };
        #[allow(unused_unsafe)]
        let app = unsafe { NSRunningApplication::runningApplicationWithProcessIdentifier(pid) };
        let Some(app) = app else { continue };
        // ActivateIgnoringOtherApps is a no-op on macOS 14+, so AllWindows only.
        #[allow(unused_unsafe)]
        let ok = unsafe { app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows) };
        if ok {
            return Ok(());
        }
    }
    Err("no terminal app found".into())
}
