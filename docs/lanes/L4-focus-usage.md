# Lane L4 — Click-to-focus terminal + opt-in live usage poller

## 1. Identity
You are lane **L4** of a 2-lane parallel build (Wave 2) of **Nudge**, a Tauri 2
desktop "notch" for Claude Code sessions. Repo root:
`C:\Users\Vin\Documents\claude-noti`. Lane L5 (settings UI + tray) is editing other
files at the same time. You build (a) "click a session → its terminal window comes
to the front" (D7) and (b) the D3 opt-in poller that reads the local Claude login
token to fetch usage when no statusline data is flowing.

## 2. HARD RULES
- Do NOT spawn subagents or use the Agent tool.
- Do NOT run any git commands. Do NOT commit.
- Do NOT edit any file outside §4. Everything else is READ-ONLY, in particular
  `src-tauri/src/{lib,hub,session,config,usage,server,commands,tray}.rs`,
  `src-tauri/src/alerts/**`, `crates/**`, `ui/**`.
- Keep the public signatures: `focus::focus_session(&[ProcInfo]) -> Result<(), String>`
  and `usage_poll::spawn(Arc<Core>)`.
- **Security floor (spec §Security):** the OAuth token is read only when
  `config.usage.live_when_idle` is true, only into memory, and only sent to
  `https://api.anthropic.com`. Never log, print, persist or include it in an error
  string. Don't print it in your final report either.
- Cross-lane errors: REPORT, don't fix.

## 3. READ FIRST (in order)
1. `SPEC.md` — D3, D7, §Security, AC #8, #11; `docs/SPIKE.md` row 4 (usage endpoint facts and what is only inferred).
2. `src-tauri/src/focus/mod.rs`, `src-tauri/src/usage_poll.rs` — the stubs you replace.
3. `src-tauri/src/hub.rs` — `Core::config()`, `Core::apply_poll(five, seven, ts_ms)`, `Core::snapshot().usage` (to see statusline freshness).
4. `src-tauri/src/usage.rs` — `UsageWindow`, newest-wins rule.
5. `crates/nudge-proto/src/lib.rs` — `ProcInfo`, `ancestors()` (nearest parent first; that's what `focus_session` receives).
6. `src-tauri/Cargo.toml` — see §4 for the one change you may make there.

## 4. FILE OWNERSHIP (exclusive)
- `src-tauri/src/focus/**` (replace `mod.rs`; add `windows.rs`, `macos.rs`, `pick.rs`, …)
- `src-tauri/src/usage_poll.rs` (you may turn it into `usage_poll/` with `mod.rs` + submodules; then delete the old file)
- `src-tauri/Cargo.toml`: **one precise change only**: add dependencies you need, under
  `[target.'cfg(windows)'.dependencies]`, `[target.'cfg(target_os = "macos")'.dependencies]`,
  or `[dependencies]` for the HTTP client. Do not touch anything else in that file.
  List every added crate + version + why in your report.

## 5. BUILD
### 5a. Focus (D7)
- `pick.rs` (pure, tested): from `ancestors` (nearest first), choose the **host app
  process**: the first ancestor that is not a shell, the Claude process, or a console helper.
  Skip names (case-insensitive, with or without `.exe`): `claude`, `node`, `bash`, `sh`,
  `zsh`, `fish`, `cmd`, `powershell`, `pwsh`, `conhost`, `openconsole`, `wsl`,
  `wslhost`, `login`, `tmux`, `screen`, `sudo`. Known hosts to prefer if present anywhere
  in the chain: `WindowsTerminal`, `Code`, `Code - Insiders`, `Cursor`, `idea64`,
  `Terminal`, `iTerm2`, `Warp`, `WezTerm`, `wezterm-gui`, `alacritty`, `kitty`, `ghostty`, `Hyper`.
  Return candidate pids in priority order.
- **Windows** (`windows.rs`, `windows` crate): for each candidate pid, `EnumWindows` → the
  first visible, non-tool, unowned top-level window whose `GetWindowThreadProcessId` matches.
  Restore if minimized (`ShowWindow(SW_RESTORE)`), then bring to front.
  **Tricky part:** `SetForegroundWindow` is refused when the caller isn't the
  foreground process (the notch window is non-focusable). Use the standard workaround:
  `AttachThreadInput` to the current foreground thread, `SetForegroundWindow` +
  `BringWindowToTop`, then detach. If that still fails, fall back to sending a
  synthetic ALT key press (`keybd_event(VK_MENU)` down/up) right before
  `SetForegroundWindow`. Verify which one works on this machine and report it.
- **Windows Terminal caveat:** sessions in WT run under `OpenConsole.exe` and may be
  re-parented, so the ancestor chain might not reach `WindowsTerminal.exe`. If no
  candidate has a window, fall back to the most recently active `WindowsTerminal.exe`
  window (by z-order) and report how often you hit this in testing.
- **macOS** (`macos.rs`): activate the candidate app by pid with
  `NSRunningApplication runningApplicationWithProcessIdentifier:` →
  `activateWithOptions:` (objc2 + objc2-app-kit, or a minimal `objc` binding). It must
  compile under `cfg(target_os = "macos")`. You can't run it here, so say exactly what's unverified.
- Return `Err("no terminal window found")` / a short reason on failure. Never panic.

### 5b. Usage poller (D3 opt-in)
- A thread that wakes every 60 s. It does nothing unless `core.config().usage.live_when_idle`.
  It also skips the fetch when `snapshot().usage.source == Statusline` and
  `updated_ms` is < 5 min old (the statusline is fresh, so no need to poll).
- Token source: Windows/Linux `~/.claude/.credentials.json` → `claudeAiOauth.accessToken`
  (respect `CLAUDE_CONFIG_DIR`). macOS: run
  `security find-generic-password -s "Claude Code-credentials" -w` and parse the same JSON.
  If `claudeAiOauth.expiresAt` (ms) is in the past, skip, because Claude Code refreshes it and you must not.
- Request: `GET https://api.anthropic.com/api/oauth/usage` with
  `Authorization: Bearer <token>`, `anthropic-beta: oauth-2025-04-20`,
  `User-Agent: nudge/<version>`, timeout 10 s. HTTP client: `ureq` (rustls) or
  similar small blocking client. Justify your choice.
- **Tricky part, unverified response shape:** the spike only proved the path exists.
  Make ONE real request on this machine (the user has a logged-in Claude Code) and
  record the response's **field names and value types only**: no values, no token.
  Parse `five_hour` / `seven_day` from it. Percent may be called `utilization`
  (0–100 or 0–1, so detect which), and `resets_at` may be an ISO string (convert
  it to unix seconds). Write a parser that tolerates missing fields, with tests built
  from the shape you observed.
- On 401/403: back off to 30 min. On other errors: back off exponentially (1, 2, 5, 15 min).
  On success: `core.apply_poll(...)`.

## 6. TESTS + VERIFY
- pick: WT chain, VS Code chain, npm-installed claude under node, all-shells chain → empty,
  case and `.exe` insensitivity, preference for a known host deeper in the chain.
- usage parse: the observed shape; fraction vs percent; ISO and numeric `resets_at`;
  missing seven_day; garbage → None.
- poller gating (with an injectable fetch fn + clock): disabled → no token read and no
  fetch (assert the token reader was never called); fresh statusline → skip; stale → fetch;
  expired token → skip; 401 → 30-min backoff.
VERIFY:
```
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p nudge focus usage_poll
cargo clippy -p nudge --all-targets -- -D warnings
```
Manual (Windows): `npm run dev` from the repo root. Open a Claude Code session in Windows
Terminal and one in the VS Code terminal so they register. Minimize/cover them, then call
`window.__TAURI_INTERNALS__.invoke('focus_session',{id:'<id>'})` from the notch devtools
(get ids from `invoke('get_snapshot')`). Report the result for each terminal.

## 7. FINAL MESSAGE (raw data)
1. Files created/changed; crates added (name, version, why).
2. Observed usage response shape (field names + types only).
3. Test names + pass counts.
4. Manual focus results per terminal; which foreground workaround was needed.
5. What is unverified (macOS).
6. Cross-lane errors seen.
7. **Deviations from this brief, each with why.**
