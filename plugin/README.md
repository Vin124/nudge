# Nudge — Claude Code plugin

A notch for Claude Code: live session status, usage and context rings, and alerts.
Not an official Anthropic product.

## One folder, two plugins

This folder is both the Claude Code plugin and the Codex plugin (D25):

| | Claude Code | Codex |
|---|---|---|
| Manifest | `.claude-plugin/plugin.json` | `plugin.json` (portable manifest) |
| Hooks | `hooks/hooks.json` → `scripts/hook.sh` | `hooks/codex-hooks.json`, named by `extensions.com.openai.hooks` |
| Windows | Git Bash runs `hook.sh` | PowerShell: `scripts/hook.ps1` on SessionStart, `nudge-hook.exe --agent codex` inline otherwise |
| Installer | `scripts/install.sh` | `scripts/install.ps1` (Windows), `install.sh codex` (macOS); neither touches Claude's statusline |
| Marketplace | `/.claude-plugin/marketplace.json` | `/.agents/plugins/marketplace.json` |

Naming the Codex hooks file explicitly keeps Codex from loading Claude's
`hooks/hooks.json`. Changing `codex-hooks.json` makes Codex ask users to trust
the hooks again (D32), so keep it stable. Installers honour `NUDGE_BASE_URL` to
download from somewhere other than the latest GitHub release (tests).

## Install

```
/plugin marketplace add Vin124/nudge
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
