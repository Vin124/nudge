---
description: Remove Nudge. In Claude Code, restores your original statusline; in Codex, explains how to remove the plugin.
allowed-tools: Bash(~/.nudge/bin/nudge-status:*), Bash(~/.nudge/bin/nudge-status.exe:*)
---

Undo Nudge's changes.

0. If you are OpenAI Codex rather than Claude Code, run nothing: a Codex install never changed any settings (D31). Tell the user to finish removing Nudge by:
   - running `codex plugin remove nudge@nudge` (removes the hooks), and
   - quitting Nudge from its tray menu and deleting the `~/.nudge` folder if they want the app and its config gone too. If they also use Nudge with Claude Code, they should run `/nudge:uninstall` there first and keep `~/.nudge` until then.

   Then stop.

1. Run this with the Bash tool (use `nudge-status.exe` instead if only that file exists in `~/.nudge/bin/`):

   ```
   ~/.nudge/bin/nudge-status uninstall
   ```

2. Report what it printed. If the binary is missing, there is nothing to restore; say so. If it fails because `settings.json` is not valid JSON, show the error and do not edit the file yourself.
3. Then tell the user to finish removing Nudge by:
   - running `/plugin uninstall nudge` (removes the hooks), and
   - quitting Nudge from its tray menu and deleting the `~/.nudge` folder if they want the app and its config gone too.
