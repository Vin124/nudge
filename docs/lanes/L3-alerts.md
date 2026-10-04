# Lane L3 — Alerts (glow, sound + voice, avatar, escalation)

## 1. Identity
You are lane **L3** of a 3-lane parallel build (Wave 1) of **Nudge**, a Tauri 2
desktop "notch" for Claude Code sessions. Repo root:
`C:\Users\Vin\Documents\claude-noti`. Lanes L1 (sidecars/plugin) and L2 (notch UI)
are editing other files at the same time. You build everything that happens when
a session finishes or gets blocked: the screen-edge glow, the synthesized sounds
and spoken voice, the mascot avatar popup, and the D8 escalation timers.

## 2. HARD RULES
- Do NOT spawn subagents or use the Agent tool.
- Do NOT run any git commands. Do NOT commit.
- Do NOT edit any file outside §4. READ-ONLY (orchestrator-owned):
  `src-tauri/src/{lib,hub,session,config,usage,server,commands}.rs`, `src-tauri/Cargo.toml`,
  `src-tauri/tauri.conf.json`, `src-tauri/capabilities/**`, `ui/src/shared/**`,
  `ui/vite.config.ts`, `ui/package.json`, `ui/tsconfig.json`, `ui/notch.html`,
  `ui/src/{notch,settings}/**`.
- **Keep the public signatures in `src-tauri/src/alerts/mod.rs` exactly as they are**
  (`AlertKind`, `AlertEngine::init`, `on_transitions`, `preview`). `lib.rs` and
  `commands.rs` call them. You replace the bodies and may add private items and
  submodules.
- No new npm dependencies, no audio/image asset files (D16: sounds are synthesized).
  New Rust crates: REPORT them instead of adding them (you don't own Cargo.toml).
  Use std + tauri + serde only.
- Cross-lane errors in files you don't own: REPORT, don't fix.

## 3. READ FIRST (in order)
1. `SPEC.md` — D2, D5, D6, D8, D16, §Alerts, AC #2, #5, #6, #12.
2. `src-tauri/src/alerts/mod.rs` — the stub whose signatures you must keep.
3. `src-tauri/src/session.rs` — `Transition` and `Transition::is_alert()`. This is your only input.
4. `src-tauri/src/config.rs` — `Config.alerts` (`done`/`blocked` `StateAlert`, `escalate_minutes`,
   `glow_pulses`, `volume`, `tts_voice`, `avatar_pack`) and `dnd`.
5. `src-tauri/src/lib.rs` — how the engine is created and called (after the state lock
   is released, from the ingest thread). `hub.rs` tests are the house Rust test style.
6. `src-tauri/tauri.conf.json` + `capabilities/default.json`: the window labels `glow-*`
   and `avatar` are already granted permissions. The asset protocol is scoped to
   `$HOME/.nudge/avatars/**`.
7. `ui/src/shared/contracts.ts` — `api.focusSession`, `AlertKind`.

## 4. FILE OWNERSHIP (exclusive)
- `src-tauri/src/alerts/**` (replace `mod.rs` bodies; add e.g. `schedule.rs`, `windows.rs`)
- `ui/glow.html`, `ui/src/glow/**`
- `ui/avatar.html`, `ui/src/avatar/**`
- `ui/src/alerts-shared/**` (the `AlertFire` TS type + shared helpers for glow/avatar)

## 5. BUILD
### 5a. Scheduler (pure, Tauri-free, unit-tested) — `alerts/schedule.rs`
- Input: `&[Transition]`, `&Config`, `now: Instant` (injectable clock). Output: a list of `Fire { session_id, project, kind, escalation: u32 }` to emit now, plus its internal timer table.
- Rules: `Transition::is_alert()` → fire escalation 0 now, then schedule escalation k at
  `first_fire + escalate_minutes[k-1]` minutes. **Any other transition for the same
  session id (Changed to non-alert, Acked, Removed) cancels that session's
  pending escalations** (AC #6). A new alert for the same session replaces its schedule.
  `tick(now)` returns due fires.
- DND is checked **at fire time** (`config.dnd` → drop the fire but keep the schedule
  consumed). `preview()` bypasses DND and schedules nothing.
- Kind: `to == Done` → `AlertKind::Done`, `to == Blocked` → `AlertKind::Blocked`.

### 5b. Engine + windows — `alerts/mod.rs` (+ `windows.rs`)
- `init`: create one `glow-<n>` window per monitor (from `app.available_monitors()`),
  sized to the monitor's full bounds: transparent, undecorated, always on top,
  skip taskbar, not resizable, **hidden**, and **click-through**
  (`set_ignore_cursor_events(true)` after build). Also create one `avatar` window
  (240×240, transparent, undecorated, always on top, skip taskbar, hidden).
  Pages: `glow.html`, `avatar.html` (`WebviewUrl::App`). Start a ticker thread
  (250 ms) that calls the scheduler's `tick` and emits due fires.
- **Tricky part, focus stealing:** alert windows must NEVER take keyboard focus
  from the app the user is typing in. Build them with `.focused(false)`. If the
  installed Tauri version exposes `.focusable(false)`, use that too. Show them with
  `show()` and never call `set_focus()`. Verify on Windows that typing in another app
  continues uninterrupted while a preview fires, and report the result. If Tauri can't
  prevent activation on Windows, report it (a later fix may use `WS_EX_NOACTIVATE`).
- **Tricky part, monitor changes:** if monitors are added or removed after startup,
  re-enumerate on each fire and create/close `glow-<n>` windows to match before emitting.
- Fire = emit event `nudge://alert` with payload `AlertFire` (camelCase) to the glow and
  avatar windows, and show the windows whose mode is enabled for that kind:
  `{ sessionId, project, kind, escalation, color, glow, pulses, sound, tts, ttsVoice, volume, avatar, avatarSrc }`
  `avatarSrc` = absolute path to `~/.nudge/avatars/<avatar_pack>/<kind>.(gif|png|svg)` if
  `avatar_pack` is set and that file exists (check in that order), else null → built-in mascot.
  Phrase for TTS: `"<project> is done"` / `"<project> needs you"`. On escalation ≥ 1:
  `"<project> is still waiting"`.
- Place the avatar window near the notch: read the `notch` window's outer position/size
  and put the avatar 12 px inward from the notch, clamped to that monitor.
- Thread-safety: `on_transitions` is called from the ingest thread; the ticker is
  another thread. Guard the scheduler with a `Mutex` and never hold it while calling
  Tauri window APIs.

### 5c. Glow UI — `ui/glow.html`, `ui/src/glow/**`
- Full-window transparent page. On `nudge://alert` with `glow: true`, draw an animated
  border around the entire screen edge in `color`: an inner box-shadow/gradient about
  18 px thick with a soft falloff, pulsing `pulses` times (each ~700 ms: fade in →
  peak → fade out). It should be noticeably stronger than the Claude-in-Chrome aura,
  but not seizure-inducing: max 2 pulses per second, and respect
  `prefers-reduced-motion` with a single steady 1.5 s glow instead. When done, hide the
  window (`getCurrentWindow().hide()`).
- CSS-only animation (GPU-composited `opacity`), no rAF loops.

### 5d. Avatar UI — `ui/avatar.html`, `ui/src/avatar/**`
- **Mascot (D6):** an original inline SVG character, a small rounded black "pebble" blob
  with two eyes and a tiny spark, matching the notch. It has two expressions: happy (done)
  and urgent (blocked, eyebrows + orange accent). Animations: pop-in (scale bounce),
  a little wave/jiggle, fade-out. If `avatarSrc` is set, show that image instead
  (`convertFileSrc(avatarSrc)`).
- Speech bubble with the phrase. Auto-hide after 6 s. Click on the avatar →
  `api.focusSession(sessionId)` (catch rejection silently), then hide.
- **Sound (D16):** synthesize with WebAudio. No files. Four sounds: `chime`
  (two-note bell, sine + soft decay), `ding` (single bright sine), `alarm` (3 short
  square-wave beeps, low-passed so it isn't harsh), `bell` (FM-ish bell). Gain = `volume`.
  `sound: null` → silent.
- **Voice:** if `tts`, use `speechSynthesis` with the voice whose `name === ttsVoice`,
  else the default. Speak after the sound ends.
- **Tricky part, autoplay:** webviews may block `AudioContext` until a user gesture.
  Create the AudioContext lazily and call `resume()`. If it stays `suspended`, report
  what happened on WebView2 in your deviations. Don't silently ship a mute alert.
- The avatar page handles sound/TTS even when `avatar: false` (then the window stays
  hidden). So exactly one page plays audio, never one per monitor.

## 6. TESTS + VERIFY
Rust (`cargo test -p nudge alerts`), scheduler only (no Tauri runtime in tests):
- alert transition → 1 immediate fire with escalation 0; ticks at +2m and +5m → escalations 1 and 2; nothing after.
- Acked / Changed-to-Running / Removed each cancel pending escalations.
- a second alert for the same session replaces the schedule (no doubled fires).
- two sessions are scheduled independently.
- `escalate_minutes = []` → exactly one fire.
- DND at fire time drops the fire. DND turned off before the next escalation → that one fires.
- Done vs Blocked map to the right kind; preview bypasses DND and schedules nothing.
- non-alert transitions alone produce no fires.
Vitest (`ui/src/glow`, `ui/src/avatar`, `ui/src/alerts-shared`):
- phrase builder (done/blocked/escalation); avatarSrc fallback logic if any lives in TS;
  the sound synth produces a node graph for each of the 4 ids with a mocked AudioContext,
  and null → no nodes; the pulse count drives the animation-iteration-count; reduced
  motion → a single steady glow; project names render as text, not HTML.

VERIFY:
```
cargo test -p nudge alerts
cargo clippy -p nudge -- -D warnings
npm --prefix ui run typecheck
npm --prefix ui test -- src/glow src/avatar src/alerts-shared
```
Then run the app (`npm run dev` from the repo root). Trigger `preview_alert` from the
devtools console of the notch window:
`window.__TAURI_INTERNALS__.invoke('preview_alert',{kind:'done'})`
and the same with `blocked`. Report what you saw and heard, and whether typing in
another app was interrupted.

## 7. FINAL MESSAGE (raw data)
1. Files created/changed.
2. `AlertFire` payload shape (Rust struct + TS type), and the event name.
3. Test names + pass counts.
4. Manual run results: glow visible on all monitors? sound audible? TTS? avatar placement? focus stolen (yes/no)?
5. Crates or permissions you needed but didn't have.
6. Cross-lane errors seen.
7. **Deviations from this brief, each with why.**
