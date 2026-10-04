# Nudge — a notch for Claude Code

Status: spec locked 2026-10-04 (decisions D1–D10 below). License: MIT.

## Context

Claude Code users fire off a prompt, pick up their phone, and come back 20 minutes
later to a session that finished (or got stuck on a permission prompt) 19 minutes
ago. Multiply by 3–4 parallel sessions. Nudge is a small black, draggable "notch"
on the screen edge that shows every live Claude Code session's state plus the
5-hour / weekly usage, and pulls you back with an edge glow, a sound/voice, and a
cute avatar the moment a session needs you.

Audience: any Claude Code user on Windows or macOS. Install path is "ask your
Claude Code to install it."

## Decisions

| ID  | Decision |
|-----|----------|
| D1  | v1 targets **Windows + macOS**; built with **Tauri 2** (Rust core + web UI). Linux deferred. |
| D2  | Alert on **Finished** (Stop) and **Blocked** (needs permission/input), with distinct styles: green = done, orange = needs you. |
| D3  | Usage source: **statusline payload by default** (no credential access). **Opt-in** "live usage when idle" reads the local OAuth token and calls the usage endpoint. |
| D4  | Distribution: **Claude Code plugin** (bundles hooks) that downloads the signed app from GitHub Releases on first run. |
| D5  | v1 ships **all three** alert modes: edge glow, sounds + TTS, avatar popup. |
| D6  | Avatar: **original SVG mascot**, CSS-animated; users may drop custom GIF/PNG packs into a folder. |
| D7  | Notch **expands on hover/click** to a session list; **clicking a session focuses its terminal window** (window-level, best-effort). |
| D8  | **Escalating nag until acknowledged** (default re-alerts at +2 min, +5 min, then stop); DND toggle. |
| D9  | Name **Nudge**, MIT. Tagline: "a notch for Claude Code". Never brand as official Anthropic. |
| D10 | Release gates = the 4 measurable criteria in Acceptance Criteria §A. |
| D11 | Toolchain: Rust (rustup) + MSVC Build Tools installed; stay on Tauri. |
| D12 | Build via wave-build: Wave 0 orchestrator, Wave 1 = L1/L2/L3, Wave 2 = L4/L5; max 3 concurrent agents. |
| D13 | Blocked = Notification `notification_type` ∈ {permission_prompt, elicitation_dialog, elicitation_url_dialog, agent_needs_input, worker_permission_prompt}. See docs/SPIKE.md. |
| D14 | A plugin can't be assumed to set `statusLine`; `nudge-status setup` edits `~/.claude/settings.json` with a backup. |
| D15 | `nudge-hook` forwards a **whitelisted subset** of hook stdin (tool payloads can exceed the 64KB cap) and never writes to stdout (stdout of SessionStart/UserPromptSubmit hooks is injected into Claude's context). |
| D16 | Alert sounds are **synthesized with WebAudio** (no audio asset files → no licensing); TTS via the webview's `speechSynthesis` (OS voices). |
| D17 | One real usage-endpoint request with the local token was approved to verify the D3 opt-in response shape (`five_hour`/`seven_day` → `utilization` 0–100, `resets_at` ISO-8601). |
| D18 | Cargo build output lives on `E:/nudge-target` on the dev machine (machine-local `.cargo/config.toml`, git-ignored). |
| D19 | Idle memory: the 80 MB gate was unreachable (WebView2's fixed base is ~308 MB). Settings is created on demand and destroyed on close; alert windows stay pre-loaded for latency. Gate #4 revised below. |

## Verified current state (this machine, 2026-10-04)

- Claude Code **2.1.288**, native binary at
  `%APPDATA%\npm\node_modules\@anthropic-ai\claude-code\bin\claude.exe`.
- The binary's statusline payload schema contains
  `rate_limits: { five_hour, seven_day, spend_limit }`, each with
  `used_percentage` and `resets_at` (epoch seconds), plus
  `rate_limits_available` (false for API-key / Bedrock / Vertex → `rate_limits` is
  null) and `subscription_type`. Statusline supports `refreshInterval`.
  → D3's default path is viable without touching credentials.
- **Unverified, must check in step 1 of the build:** (a) whether a plugin can
  register a `statusLine` (assumed **no** → handled by the `/nudge:setup`
  command), (b) exact `Notification` hook payload fields that distinguish
  permission prompts from idle prompts, (c) the usage endpoint and token file
  layout for the D3 opt-in path.

## Architecture

```
 Claude Code session(s)
   ├─ hooks (from plugin) ──► nudge-hook <event>  ──┐  HTTP POST, 300ms timeout,
   └─ statusLine ──────────► nudge-status --wrap ──┤  always exit 0, never blocks
                                                   ▼
                              Nudge app (Tauri, tray, single instance)
                              127.0.0.1:<port>  (port + auth token in ~/.nudge/runtime.json)
                              ├─ SessionStore (state machine, liveness sweeps)
                              ├─ UsageStore (statusline feed | opt-in poller)
                              ├─ AlertEngine (glow / sound+TTS / avatar, escalation)
                              └─ Windows: notch · glow (one per monitor) · avatar · settings
```

### Components

1. **Plugin** (`plugin/`): `.claude-plugin/plugin.json`, `hooks/hooks.json`,
   `commands/setup.md` (`/nudge:setup`), `commands/uninstall.md`.
   On `SessionStart`, the hook checks for the app and, if it's missing, downloads
   the platform release asset into `~/.nudge/bin/`, verifies its SHA-256 against
   the release manifest, and launches it.
2. **`nudge-hook`**: a tiny Rust sidecar binary. It reads hook JSON from stdin and
   POSTs `{event, session_id, cwd, transcript_path, ts, ppid_chain, payload}` with
   header `X-Nudge-Token`. If the app is down, it drops the event silently and
   exits 0 (a missed event recovers on the next one).
3. **`nudge-status --wrap "<original cmd>"`**: it forwards stdin to the user's
   original statusline command and prints that command's output unchanged. It
   also POSTs `rate_limits` + `session_id` to the app. With no original command,
   it prints a minimal line (`5h 42% · wk 18%`).
4. **App**: Tauri 2. Rust core owns state and the HTTP server. The web UI (Svelte
   or vanilla TS, no heavy framework) renders the windows.

### Session state machine

| Hook event | Transition |
|---|---|
| `SessionStart` | → `idle` (create record: id, cwd, project = basename(cwd), terminal pid) |
| `UserPromptSubmit` | → `running` (acknowledges any pending alert for this session) |
| `PreToolUse` / `PostToolUse` | `blocked` → `running` (permission was granted) |
| `Notification` (permission / input needed) | → `blocked` → **alert (orange)** |
| `Stop` | → `done` → **alert (green)** |
| `SessionEnd` | remove |
| Liveness sweep, every 5s | Claude pid gone → remove (covers crashes and closed terminals) |

Sessions that were already running when Nudge was installed appear on their next
hook event. That's acceptable, and the README says so.

### Notch UI

- Collapsed: a black pill (~180×36 px when horizontal, rotated when on a side
  edge) with two ring "wheels" (5h, weekly; weekly can be hidden in settings) and
  one status dot per session (grey idle, white spinner running, green done,
  orange blocked).
- Drag: free drag, snaps to the nearest edge (top/bottom/left/right) of any
  monitor. Position is persisted per monitor layout.
- Expanded (hover 300ms or click): a list with project name, state, elapsed time
  since the last transition, and a reset countdown for each usage wheel.
  Clicking a row focuses that session's terminal window and acknowledges the
  alert.
- Tray menu: Settings, DND, Pause alerts 30m/1h, Quit.

### Alerts

- **Glow**: one full-screen transparent, click-through, always-on-top window per
  monitor. It draws an animated gradient border. Configurable: color per state,
  intensity, pulse count (default 3), width.
- **Sound + TTS**: 4 bundled CC0 sounds (chime, ding, alarm, soft bell) plus
  optional OS TTS ("api-refactor is done" / "api-refactor needs you"). Voice
  picker lists the OS voices. Volume slider.
- **Avatar**: an SVG mascot pops in near the notch with a speech bubble ("Done!"
  / "Need you!"), auto-hides after 6s, click → focus terminal. Custom packs go in
  `~/.nudge/avatars/<name>/` with `done.(gif|png|svg)` and `blocked.*`.
- **Escalation**: re-fire at +2m, +5m (configurable list, empty = once).
  Acknowledged by: clicking the notch row/avatar, a `UserPromptSubmit` for that
  session, or the terminal window gaining focus (best-effort).
- **DND**: suppresses glow, sound, and avatar. The dots still update.
- Each mode can be toggled independently, per state (done vs blocked).

### Config — `~/.nudge/config.json`

```json
{
  "version": 1,
  "notch": { "edge": "right", "offset": 0.4, "showWeekly": true },
  "alerts": {
    "done":    { "glow": true, "sound": "chime", "tts": false, "avatar": true, "color": "#3ddc84" },
    "blocked": { "glow": true, "sound": "alarm", "tts": true,  "avatar": true, "color": "#ff8a00" },
    "escalateMinutes": [2, 5],
    "glowPulses": 3, "volume": 0.7, "ttsVoice": null
  },
  "usage": { "liveWhenIdle": false },
  "dnd": false
}
```

## Security

- The HTTP server binds to **127.0.0.1 only**. A random 32-byte token is
  regenerated each launch and written to `~/.nudge/runtime.json` with user-only
  permissions. Requests without the token are rejected. The body size cap is
  64KB.
- The OAuth token (D3 opt-in only) is read in memory, sent only to the Anthropic
  usage endpoint, and never logged or written to disk.
- Release binaries are signed (Windows Authenticode, macOS notarized). The plugin
  verifies the SHA-256 before executing.
- Hook payload strings (cwd, project name) are rendered as text, never as HTML.

## Acceptance criteria

**A. Release gates (D10)**
1. On a clean Win 11 and a clean macOS 14+ machine, going from `/plugin install nudge` to a visible notch takes under 2 minutes with no manual file edits (`/nudge:setup` is run by the install flow).
2. The alert fires within 1s of the `Stop` hook (measured: hook timestamp → glow first frame, p95 over 20 runs).
3. With 4 concurrent sessions, every session started after install appears within 1s. An exited or killed session disappears within 10s.
4. Idle cost of the whole process tree (`nudge.exe` + WebView2 children) is under 1% CPU and under 600 MB working set (5-minute average, 3 sessions idle, production build). *(Revised by D19 from "80 MB", which no WebView2 app can meet.)*

**B. Functional**
5. Done → green glow, the chosen sound, and the avatar. Blocked → orange, a distinct sound, TTS if enabled.
6. Escalation re-fires at +2m and +5m when unacknowledged, and stops immediately on acknowledgment.
7. The notch snaps to all 4 edges on each monitor, and its position survives a restart.
8. The usage wheels show `five_hour.used_percentage` / `seven_day.used_percentage` within 1 statusline refresh. For API-key users they show "n/a" and a hint to enable live usage.
9. `/nudge:setup` preserves an existing statusline: its output is byte-identical through the wrapper. `/nudge:uninstall` restores the original statusline and leaves no files outside `~/.nudge` and the plugin directory.
10. If the app isn't running, Claude Code behavior is unchanged: hooks exit 0 in under 300ms.
11. Clicking a session row focuses its terminal window in Windows Terminal, the VS Code terminal, macOS Terminal, and iTerm2.
12. DND suppresses all glow, sound, and avatar alerts.

## Testing plan

| Layer | What | Count |
|---|---|---|
| Unit (Rust) | State machine transitions incl. out-of-order events, escalation timer, config migration, token auth | +20 |
| Unit (TS) | Wheel math, edge-snap geometry, avatar pack loader | +10 |
| Integration | `nudge-hook`/`nudge-status` → app with replayed real hook payloads; app-down path exits 0 fast | +8 |
| E2E (manual checklist, both OSes) | Gates 1–4, multi-monitor, the terminal-focus matrix | 1 checklist |

## Build order

```
#1 Spike: verify unknowns (plugin statusLine, Notification payload, usage endpoint) ─┐
#2 App core: HTTP server + SessionStore + notch window (dots only)  ◄───────────────┘
#3 nudge-hook + plugin hooks ──► #4 Alerts: glow → sound/TTS → avatar → escalation
#5 nudge-status wrapper + /nudge:setup ──► usage wheels
#6 Click-to-focus terminal (per-OS) · #7 Settings UI · #8 Signing, release CI, plugin downloader
```
The spike comes first because three of its answers change the plugin layout.
Alerts come before usage because alerts are the core value.

## Effort (CC-assisted)

Spike 0.5d · core + notch 1.5d · hooks/plugin 0.5d · alerts 1.5d (incl. mascot) ·
statusline/usage 0.5d · terminal focus 1d · settings 0.5d · signing/release/CI 1d
→ about 7 working days. Signing certificates (Apple Developer $99/yr, Windows code
signing) are a cost and lead-time item to start now.

## Rollback

`/nudge:uninstall` restores the backed-up statusline (`~/.nudge/statusline.backup.json`)
and removes the plugin hooks. Hooks always exit 0, so a broken app never blocks
Claude Code.

## Out of scope (v1)

- Linux; Claude Desktop / claude.ai web sessions; remote/SSH sessions
- Sessions started before install (until their next hook event)
- Focusing the exact terminal *tab* (window-level only)
- Mobile push notifications, auto-update channel (v1.1), avatar marketplace
