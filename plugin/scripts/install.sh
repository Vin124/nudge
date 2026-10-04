#!/bin/sh
# Nudge installer (D4, spec §Components 1, §Security). Started detached by
# hook.sh on the first SessionStart. Downloads the release for this platform,
# verifies its SHA-256 against the release's SHA256SUMS, extracts it into
# ~/.nudge/bin, runs `nudge-status setup` (D14) and launches the app.
#
# Logs to ~/.nudge/install.log. On any failure: log and exit 0.

NUDGE_HOME="${NUDGE_HOME:-$HOME/.nudge}"
BIN="$NUDGE_HOME/bin"
LOG="$NUDGE_HOME/install.log"
LOCK="$NUDGE_HOME/install.lock"
BASE_URL="https://github.com/${NUDGE_REPO:-nudge-app/nudge}/releases/latest/download"
TMP=""

mkdir -p "$NUDGE_HOME" 2>/dev/null || exit 0

log() {
    printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null)" "$*" >>"$LOG" 2>/dev/null
}

cleanup() {
    [ -n "$TMP" ] && rm -rf "$TMP"
    rm -rf "$LOCK"
}

fail() {
    log "ERROR: $*"
    cleanup
    exit 0
}

# ---- single installer at a time (mkdir is atomic) ---------------------------
if ! mkdir "$LOCK" 2>/dev/null; then
    # A lock older than 10 minutes belongs to a crashed install.
    if [ -n "$(find "$LOCK" -maxdepth 0 -mmin +10 2>/dev/null)" ]; then
        rm -rf "$LOCK"
        mkdir "$LOCK" 2>/dev/null || exit 0
    else
        log "another install is running; skipping"
        exit 0
    fi
fi
trap cleanup EXIT
trap 'cleanup; exit 0' HUP INT TERM

# Another session's install may have finished while we waited.
if [ -x "$BIN/nudge-hook.exe" ] || [ -x "$BIN/nudge-hook" ]; then
    log "already installed; skipping"
    exit 0
fi

# ---- platform ---------------------------------------------------------------
os=$(uname -s 2>/dev/null)
arch=$(uname -m 2>/dev/null)
case "$os" in
    MINGW* | MSYS* | CYGWIN*) os=windows ;;
    Darwin) os=macos ;;
    *) fail "unsupported OS: $os" ;;
esac
case "$arch" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) fail "unsupported architecture: $arch" ;;
esac
TARGET="$os-$arch"
case "$TARGET" in
    windows-x86_64 | macos-aarch64 | macos-x86_64) ;;
    *) fail "no release for $TARGET" ;;
esac
ASSET="nudge-$TARGET.zip"
log "installing $ASSET from $BASE_URL"

# Windows-native path for tools that don't understand /c/... paths.
winpath() {
    if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

# ---- download ---------------------------------------------------------------
TMP=$(mktemp -d 2>/dev/null) || TMP="$NUDGE_HOME/tmp.$$"
mkdir -p "$TMP" || fail "cannot create temp dir"

download() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 2 --connect-timeout 15 --max-time 300 -o "$2" "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -T 60 -O "$2" "$1"
    else
        return 127
    fi
}

download "$BASE_URL/$ASSET" "$TMP/$ASSET" 2>>"$LOG" || fail "download of $ASSET failed"
download "$BASE_URL/SHA256SUMS" "$TMP/SHA256SUMS" 2>>"$LOG" || fail "download of SHA256SUMS failed"

# ---- verify -----------------------------------------------------------------
sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print tolower($1)}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print tolower($1)}'
    elif command -v certutil >/dev/null 2>&1; then
        certutil -hashfile "$(winpath "$1")" SHA256 | sed -n 2p | tr -d ' \r' | tr 'A-F' 'a-f'
    else
        return 1
    fi
}

expected=$(awk -v f="$ASSET" '{ n = $2; sub(/^\*/, "", n); if (n == f) { print tolower($1); exit } }' "$TMP/SHA256SUMS")
[ -n "$expected" ] || fail "$ASSET not listed in SHA256SUMS"
actual=$(sha256_of "$TMP/$ASSET") || fail "no sha256 tool available"
[ "$expected" = "$actual" ] || fail "checksum mismatch for $ASSET (expected $expected, got $actual)"
log "checksum ok"

# ---- extract ----------------------------------------------------------------
STAGE="$TMP/stage"
mkdir -p "$STAGE" || fail "cannot create staging dir"
if command -v unzip >/dev/null 2>&1; then
    unzip -qo "$TMP/$ASSET" -d "$STAGE" >>"$LOG" 2>&1 || fail "unzip failed"
elif [ "$os" = macos ] && command -v ditto >/dev/null 2>&1; then
    ditto -x -k "$TMP/$ASSET" "$STAGE" >>"$LOG" 2>&1 || fail "ditto failed"
elif [ "$os" = windows ] && command -v powershell.exe >/dev/null 2>&1; then
    powershell.exe -NoProfile -NonInteractive -Command \
        "Expand-Archive -LiteralPath '$(winpath "$TMP/$ASSET")' -DestinationPath '$(winpath "$STAGE")' -Force" \
        >>"$LOG" 2>&1 || fail "Expand-Archive failed"
else
    fail "no unzip tool available"
fi

mkdir -p "$BIN" || fail "cannot create $BIN"
cp -Rf "$STAGE"/. "$BIN"/ >>"$LOG" 2>&1 || fail "copy into $BIN failed"
if [ "$os" = macos ]; then
    chmod +x "$BIN"/nudge "$BIN"/nudge-hook "$BIN"/nudge-status 2>/dev/null
fi

STATUS=""
for f in "$BIN/nudge-status.exe" "$BIN/nudge-status"; do
    [ -f "$f" ] && { STATUS="$f"; break; }
done
[ -n "$STATUS" ] || fail "nudge-status missing from $ASSET"
log "extracted to $BIN"

# ---- statusline (D14) -------------------------------------------------------
"$STATUS" setup >>"$LOG" 2>&1 || log "WARN: nudge-status setup failed (run /nudge:setup)"

# ---- launch (detached; the app is single-instance) --------------------------
detach() {
    if command -v nohup >/dev/null 2>&1; then
        nohup "$@" </dev/null >/dev/null 2>&1 &
    else
        "$@" </dev/null >/dev/null 2>&1 &
    fi
}
if [ -d "$BIN/Nudge.app" ]; then
    detach open -g "$BIN/Nudge.app"
else
    for app in "$BIN/nudge.exe" "$BIN/nudge"; do
        if [ -f "$app" ]; then
            detach "$app"
            break
        fi
    done
fi
log "install complete"
exit 0
