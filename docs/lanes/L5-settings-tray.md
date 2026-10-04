# Lane L5 — Settings window + tray menu

## 1. Identity
You are lane **L5** of a 2-lane parallel build (Wave 2) of **Nudge**, a Tauri 2
desktop "notch" for Claude Code sessions. Repo root:
`C:\Users\Vin\Documents\claude-noti`. Lane L4 (focus + usage poller) is editing other
files at the same time. You build the Settings window and the system-tray menu.

## 2. HARD RULES
- Do NOT spawn subagents or use the Agent tool.
- Do NOT run any git commands. Do NOT commit.
- Do NOT edit any file outside §4. READ-ONLY: everything else, in particular
  `src-tauri/src/{lib,hub,session,config,usage,server,commands}.rs`, `src-tauri/Cargo.toml`,
  `src-tauri/tauri.conf.json`, `ui/src/shared/**`, `ui/src/{notch,glow,avatar,alerts-shared}/**`.
- Keep the signature `tray::init(&AppHandle, Arc<Core>) -> tauri::Result<()>`.
- No new npm deps or crates. If one is truly needed, REPORT it.
- Cross-lane errors: REPORT, don't fix.

## 3. READ FIRST (in order)
1. `SPEC.md` — §Notch UI (tray line), §Alerts, §Config, D3, D6, D8, D16, AC #5, #12.
2. `ui/src/shared/contracts.ts` — `api.setConfig`, `api.previewAlert`, `api.listAvatarPacks`, `onSnapshot`, `Config`.
3. `src-tauri/src/config.rs` — field semantics and `sanitized()` clamps (escalation 1–120 min, ≤ 5 entries; pulses 1–20; volume 0–1).
4. `src-tauri/src/hub.rs` — `Core::config()`, `Core::set_config()`.
5. `src-tauri/tauri.conf.json` — the `settings` window (hidden at start, 520×640).
6. The notch UI under `ui/src/notch/` for visual language (black, rounded, system font). Match it.

## 4. FILE OWNERSHIP (exclusive)
- `src-tauri/src/tray.rs` (may become `tray/` with submodules)
- `ui/settings.html`, `ui/src/settings/**`

## 5. BUILD
### 5a. Tray (`tray.rs`)
- Tray icon = `app.default_window_icon()`. Tooltip "Nudge".
- Menu: **Settings…** (show + focus the `settings` window), **Do Not Disturb** (check item
  bound to `config.dnd`), **Pause alerts ▸ 30 minutes / 1 hour / Until I resume**,
  separator, **Quit Nudge**.
- Pause = set `dnd: true` now and remember (in tray state, not in config) a deadline;
  a timer turns `dnd` back off at the deadline **only if** it's still the pause that set it
  (the user may have toggled DND manually since, so don't clobber that). Show the remaining
  time in the menu item text while paused ("Paused — 23 min left").
- Keep the DND check item in sync when config changes elsewhere (Settings window).
  Poll `core.config().dnd` on the pause timer tick (every 30 s) and on menu open if the API
  supports it. Don't add a new event to the core.
- Settings window: closing it must **hide** it, not destroy it (`on_window_event` →
  `CloseRequested` → `api.prevent_close()` + `hide()`). Register this in `tray::init`.
- Left-click on the tray icon opens Settings (Windows); the menu stays on right-click.

### 5b. Settings window (`ui/settings.html`, `ui/src/settings/**`)
- Sections:
  1. **Notch:** show weekly wheel (toggle). Note: "Drag the notch to move it."
  2. **When a session finishes** / **When a session needs you:** for each, toggles for
     Glow, Sound (select: Chime, Ding, Alarm, Bell, None), Voice, Avatar, and a color picker.
     Each section has a **Test** button → `api.previewAlert('done'|'blocked')`.
  3. **Alerts:** volume slider; glow pulses (1–20); "Remind me again after" as an editable
     list of minutes (chips, max 5, 1–120); voice picker filled from
     `speechSynthesis.getVoices()` (it loads async: listen to `voiceschanged`), "System default" = null.
  4. **Avatar:** "Built-in mascot" + `api.listAvatarPacks()` names; a hint showing the
     folder path `~/.nudge/avatars/<name>/done.png|gif|svg, blocked.*`.
  5. **Usage:** opt-in toggle "Live usage when no session is running" with this
     disclosure text verbatim: "Reads your Claude Code login token from this computer to
     ask Anthropic for your usage. The token never leaves your machine except to
     api.anthropic.com, and is never stored by Nudge." Off by default.
  6. **Do Not Disturb** toggle.
- Load via `onSnapshot` (config is in the snapshot). Saves are **debounced 300 ms** and
  send the whole config via `api.setConfig`. Re-render from the next snapshot, so the
  server-side `sanitized()` result is what's shown. Don't let a snapshot that arrives
  mid-edit clobber the field being typed in (tricky part: track the focused field, and
  skip overwriting it until blur).
- Pure logic (`model.ts`): config ↔ form mapping, minutes-list validation, and debounce.
  Testable without Tauri. DOM in `view.ts`. Tauri glue in `main.ts`.
- Accessibility: every control has a label, keyboard operable, visible focus ring.
- Use `textContent` for any data-derived text (pack names come from folder names).

## 6. TESTS + VERIFY
Vitest:
- model: config→form→config roundtrip is lossless; minutes validation (dedupe, sort, clamp 1–120,
  max 5, rejects non-numbers); null voice ↔ "System default"; null sound ↔ "None".
- debounce: 3 edits within 300 ms → 1 save carrying the last value.
- view: renders all sections; Test buttons call previewAlert with the right kind (mock api);
  a snapshot arriving while a field is focused doesn't overwrite it; a pack name with `<img onerror>` renders as text.
Rust (`tray.rs`): extract the pause/DND decision into a pure function and test it:
pause sets dnd; deadline restores it; a manual DND change during the pause blocks the restore;
"until I resume" has no deadline.
VERIFY:
```
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p nudge tray
cargo clippy -p nudge --all-targets -- -D warnings
npm --prefix ui run typecheck
npm --prefix ui test -- src/settings
```
Manual: `npm run dev`. Open Settings from the tray, change values, confirm
`~/.nudge/config.json` updates, close and reopen Settings (hidden, not destroyed), and
check that pause 30 min shows the countdown text. Report results.

## 7. FINAL MESSAGE (raw data)
1. Files created/changed.
2. Test names + pass counts.
3. Manual run results.
4. Cross-lane errors seen.
5. **Deviations from this brief, each with why.**
