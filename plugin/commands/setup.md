---
description: Route your Claude Code statusline through Nudge so the notch can show usage (keeps your existing statusline).
allowed-tools: Bash(~/.nudge/bin/nudge-status:*), Bash(~/.nudge/bin/nudge-status.exe:*)
---

Set up Nudge's statusline integration.

1. Run this with the Bash tool (use `nudge-status.exe` instead if only that file exists in `~/.nudge/bin/`):

   ```
   ~/.nudge/bin/nudge-status setup
   ```

2. If the binary is missing, tell the user Nudge is still being installed in the background (it downloads on the first session start; progress is logged in `~/.nudge/install.log`) and to retry `/nudge:setup` in a minute. Do not try to download or install anything yourself.
3. If it prints "already set up", say so. If it fails because `settings.json` is not valid JSON, show the error and ask the user to fix the file; do not edit it yourself.
4. On success, tell the user their previous statusline (if any) is preserved, its output is shown unchanged, and the original is backed up at `~/.nudge/statusline.backup.json`. The change takes effect on the next statusline refresh.
