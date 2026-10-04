#!/bin/sh
# Nudge hook shim (D4, D15). Claude Code runs this for every registered hook
# event with the hook JSON on stdin; $1 is the event name (see hooks.json).
#
# Contract: never write to stdout (SessionStart/UserPromptSubmit stdout is
# injected into Claude's context) and always exit 0 (AC #10).

NUDGE_HOME="${NUDGE_HOME:-$HOME/.nudge}"
BIN="$NUDGE_HOME/bin"
HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" 2>/dev/null && pwd) || HERE=.

# Nothing started from here may reach Claude's context.
exec >/dev/null
[ "${NUDGE_DEBUG:-}" = 1 ] || exec 2>/dev/null

# Start a program fully detached: no inherited stdio, so Claude Code never
# waits on it.
detach() {
    if command -v nohup >/dev/null 2>&1; then
        nohup "$@" </dev/null >/dev/null 2>&1 &
    else
        "$@" </dev/null >/dev/null 2>&1 &
    fi
}

# The app is single-instance, so launching it on every SessionStart is safe.
launch_app() {
    if [ -d "$BIN/Nudge.app" ]; then
        detach open -g "$BIN/Nudge.app"
        return
    fi
    for app in "$BIN/nudge.exe" "$BIN/nudge"; do
        if [ -f "$app" ] && [ -x "$app" ]; then
            detach "$app"
            return
        fi
    done
}

hook=""
for f in "$BIN/nudge-hook.exe" "$BIN/nudge-hook"; do
    if [ -f "$f" ] && [ -x "$f" ]; then
        hook="$f"
        break
    fi
done

if [ "${1:-}" = "SessionStart" ]; then
    launch_app
    if [ -z "$hook" ]; then
        # First run: install in the background; this event is dropped.
        detach sh "$HERE/install.sh"
    fi
fi

if [ -n "$hook" ]; then
    exec "$hook"
fi
cat >/dev/null
exit 0
