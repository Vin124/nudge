# Nudge — a notch for Claude Code

A small black notch on the edge of your screen that watches every Claude Code
session on your machine. It shows your 5-hour and weekly usage, each session's
context window, and pulls you back (edge glow, a sound or voice, a little pixel
fox) the moment a session finishes or needs your permission. So you can stop
doom-scrolling while you wait.

**[Website and live demo →](https://trynudge.lol)**

- **Live sessions.** One ring per session: white running, orange needs you, green done, grey idle. Each ring fills with that session's context-window use.
- **Usage at a glance.** 5-hour and weekly limits from Claude Code's own statusline data.
- **A fox that taps you on the shoulder.** It peeks out from behind the notch (or pops in at screen center) when a session is done or needs you.
- **Alerts your way.** Quiet, Normal or Loud, or tune glow, sound, voice and the fox per event under Advanced. Reminders repeat until you look. Do Not Disturb included.
- **Click to return.** Hover the notch to see every session, then click one to bring its terminal to the front.
- **Drag it anywhere** along any screen edge. Settings live right inside the notch.

Windows and macOS. MIT licensed. Not affiliated with Anthropic.

## Install

Ask Claude Code to do it, or run these two commands inside Claude Code yourself:

```
/plugin marketplace add Vin124/nudge
/plugin install nudge@nudge
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
