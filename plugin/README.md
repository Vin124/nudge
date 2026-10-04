# Nudge — Claude Code plugin

A notch for Claude Code: live session status, usage wheels and alerts.
Not an official Anthropic product.

## Install

```
/plugin marketplace add nudge-app/nudge
/plugin install nudge
```

On the next session start the plugin downloads the Nudge app for your
platform (Windows x64, macOS Apple Silicon / Intel) from GitHub Releases,
verifies its SHA-256 against the release's `SHA256SUMS`, installs it, routes
your statusline through Nudge and launches it. Sessions that were already
running show up on their next hook event.

If the notch doesn't show usage, run `/nudge:setup`.

## What it touches on disk

| Path | What |
|---|---|
| `~/.nudge/bin/` | The app, `nudge-hook` and `nudge-status` binaries |
| `~/.nudge/install.log` | Installer log |
| `~/.nudge/statusline.backup.json` | Your original `statusLine` setting |
| `~/.nudge/runtime.json`, `~/.nudge/config.json` | App port/token and settings |
| `~/.claude/settings.json` | Only the `statusLine` key: it now runs `nudge-status --wrap-from …`, which runs your original statusline and prints its output unchanged |

Hooks never print anything into Claude's context and always exit 0 quickly,
so Claude Code behaves the same when the app isn't running.

## Uninstall

1. `/nudge:uninstall` — restores your original `statusLine`.
2. `/plugin uninstall nudge` — removes the hooks.
3. Optional: quit Nudge from the tray and delete `~/.nudge`.
