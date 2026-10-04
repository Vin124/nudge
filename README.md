# Nudge — a notch for Claude Code

A small black notch on the edge of your screen that watches every Claude Code
session on your machine. It shows your 5-hour and weekly usage, and pulls you
back (edge glow, a sound or voice, a little mascot) the moment a session
finishes or needs your permission. So you can stop doom-scrolling while you wait.

- **Live sessions.** One dot per session: grey idle, white running, green done, orange needs you.
- **Usage wheels.** 5-hour and weekly limits from Claude Code's own statusline data (the weekly wheel is optional).
- **Alerts.** Screen-edge glow, synthesized chimes or alarms, a spoken "api-refactor is done", and a mascot popup. If you ignore an alert, it repeats after 2 and 5 minutes. Has a Do Not Disturb mode.
- **Click to return.** Hover the notch to see the list, then click a session to bring its terminal to the front.
- **Drag it anywhere** along any screen edge.

Windows and macOS. MIT licensed. Not affiliated with Anthropic.

## Install

Ask Claude Code to do it, or run these two commands inside Claude Code yourself:

```
/plugin marketplace add nudge-app/nudge
/plugin install nudge
```

The next time a session starts, the plugin downloads the app for your platform
from this repo's GitHub Releases and verifies its SHA-256. It installs to
`~/.nudge/bin`, points your statusline at Nudge (your existing statusline keeps
working, wrapped unchanged), and launches the app.

## What it touches

| Path | Why |
|---|---|
| `~/.nudge/` | the app, config, a statusline backup, and `runtime.json` (local port + per-launch token) |
| `~/.claude/settings.json` → `statusLine` | wrapped to feed usage numbers; backed up first |
| Claude Code hooks (from the plugin) | report session state to the app on `127.0.0.1` only |

Nudge never reads your Claude login token, **unless** you turn on
*Settings → Usage → Live usage when no session is running*. With it on, the
token is read on your machine and sent only to `api.anthropic.com`.

## Uninstall

```
/nudge:uninstall
/plugin uninstall nudge
```

This restores your original statusline. Then delete `~/.nudge`.

## Develop

Requires Rust (stable), Node 22, and on Windows the MSVC C++ Build Tools.

```
npm install && npm --prefix ui install
npm run dev          # app + UI with hot reload
npm test             # cargo test --workspace && vitest
```

Layout: `crates/nudge-proto` (wire contract), `crates/nudge-hook` and
`crates/nudge-status` (sidecars Claude Code runs), `src-tauri` (app core:
session state machine, local server, alerts, focus, tray), `ui` (notch,
alerts, settings pages), `plugin` (Claude Code plugin). Design decisions are in
[`SPEC.md`](SPEC.md).
