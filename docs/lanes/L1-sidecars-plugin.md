# Lane L1 — Sidecars + Claude Code plugin

## 1. Identity
You are lane **L1** of a 3-lane parallel build (Wave 1) of **Nudge**, a Tauri 2
desktop "notch" that tracks live Claude Code sessions. Repo root:
`C:\Users\Vin\Documents\claude-noti`. Lanes L2 (notch UI) and L3 (alerts) are
editing other files at the same time. You build the two sidecar binaries and the
Claude Code plugin that installs and wires them.

## 2. HARD RULES
- Do NOT spawn subagents or use the Agent tool.
- Do NOT run any git commands. Do NOT commit.
- Do NOT edit any file outside §4. READ-ONLY (orchestrator-owned):
  `crates/nudge-proto/**`, `src-tauri/**`, `ui/**`, `Cargo.toml` (workspace root), `SPEC.md`, `docs/SPIKE.md`.
- If you need something from `nudge-proto` that is missing, REPORT it; do not edit it.
- Cross-lane build errors in files you don't own: REPORT, don't fix.
- Match the existing idiom (see `crates/nudge-proto/src/lib.rs`): small std-first
  code, doc comments citing decision IDs (D3, D13, D14, D15…).
- Never print secrets; never read `~/.claude/.credentials.json`.

## 3. READ FIRST (in order)
1. `SPEC.md` — decisions table (D1–D16), §Architecture components 1–3, §Security, §Rollback, Acceptance criteria #1, #9, #10.
2. `docs/SPIKE.md` — hook payload facts.
3. `crates/nudge-proto/src/lib.rs` — the wire contract you MUST use (`HookEnvelope`, `StatusEnvelope::from_statusline`, `post`, `ancestors`, `nudge_dir`, `PATH_HOOK`, `PATH_STATUS`, `SIDECAR_TIMEOUT`, `MAX_BODY_BYTES`). Its tests are the house test style.
4. `src-tauri/src/session.rs` — how the app interprets the payload fields you forward.
5. `crates/nudge-hook/Cargo.toml`, `crates/nudge-status/Cargo.toml` (you own these; you may add deps).

## 4. FILE OWNERSHIP (exclusive)
- `crates/nudge-hook/**` (Cargo.toml, src/**, tests/**)
- `crates/nudge-status/**` (Cargo.toml, src/**, tests/**)
- `plugin/**` (everything: manifest, hooks, commands, scripts, README)

## 5. BUILD
### 5a. `nudge-hook` (binary)
- Usage: Claude Code runs it for every hook event with the hook JSON on stdin.
- Read stdin (bounded: stop after 4 MB), parse JSON. **D15 tricky part:**
  `PreToolUse`/`PostToolUse` payloads include `tool_input`/`tool_response`,
  which can be megabytes. Forward ONLY these keys if present:
  `session_id, cwd, hook_event_name, notification_type, transcript_path, source, reason`
  plus `message` truncated to 500 chars. The envelope must stay < `MAX_BODY_BYTES`.
- Build `HookEnvelope { v: PROTOCOL_VERSION, ts_ms: now_ms(), ancestors: ancestors(8), payload }` and `post(PATH_HOOK, …)`.
- **D15 tricky part #2:** write NOTHING to stdout, ever. Stdout from
  SessionStart/UserPromptSubmit hooks is injected into Claude's context. Errors → stderr only when `NUDGE_DEBUG=1`, otherwise silent.
- **Always exit 0**, including on bad JSON, app down, timeout, panic (use `std::panic::catch_unwind` or a panic hook that exits 0).
- Budget: whole run < 300 ms when the app is down (AC #10). Measure `ancestors(8)` cost on this Windows machine and report the number. If it exceeds 150 ms, compute ancestors only for `SessionStart` and `UserPromptSubmit` and send `[]` otherwise (the app keeps the last non-empty list), and say so in deviations.

### 5b. `nudge-status` (binary, subcommands)
- `nudge-status` (no args) and `nudge-status --wrap-from <backup.json>`: statusline mode.
  - Read all stdin. In a background thread, POST `StatusEnvelope::from_statusline(&json)` to `PATH_STATUS` (ignore errors).
  - Without `--wrap-from`: print a minimal line: `5h 42% · wk 18%` (omit missing parts; print nothing if no data).
  - With `--wrap-from <path>`: the file is the backup JSON written by `setup` (below). Run the ORIGINAL statusline command with the same stdin bytes, and print its stdout **byte-for-byte unchanged** (AC #9). Do not append anything. Run it the way Claude Code does: on Windows via `bash -c` if `bash` is on PATH, else `cmd /C`; on unix `sh -c`. Pass through the original's exit code. If the backup file is missing or invalid, behave like no-wrap mode.
  - The POST must not delay output more than `SIDECAR_TIMEOUT`.
- `nudge-status setup` (D14): edit `~/.claude/settings.json` (path overridable with env `CLAUDE_CONFIG_DIR` → `<dir>/settings.json`, which is how tests isolate):
  - If the file doesn't exist, treat as `{}`. If it exists but is not valid JSON → print an error to stderr, exit 1, write NOTHING.
  - **Idempotent:** if `statusLine.command` already contains `nudge-status`, do nothing and print "already set up".
  - Otherwise write `<nudge_dir>/statusline.backup.json` = `{"statusLine": <original value or null>}`, then set `statusLine` to `{"type":"command","command":"<abs path of this exe> --wrap-from <abs backup path>"}` and copy `padding` from the original if present. Paths with spaces must be quoted so `bash -c` parses them.
  - Preserve every other key and the key order (`serde_json` feature `preserve_order`). Write atomically (tmp + rename), 2-space pretty JSON.
- `nudge-status uninstall`: if backup exists, restore `statusLine` to the backed-up value (remove the key if it was null), delete the backup. If statusLine isn't ours, leave it alone. Same invalid-JSON rule.

### 5c. `plugin/` (Claude Code plugin, D4)
- `plugin/.claude-plugin/plugin.json`: name `nudge`, version `0.1.0`, description "A notch for Claude Code: live session status, usage wheels and alerts.", license MIT. No `author` email.
- `plugin/hooks/hooks.json`: register `SessionStart`, `UserPromptSubmit`, `PreToolUse` (matcher `*`), `PostToolUse` (matcher `*`), `Notification`, `Stop`, `SessionEnd`, each running `bash "${CLAUDE_PLUGIN_ROOT}/scripts/hook.sh"` with `timeout: 5`. Check the current Claude Code plugin hook schema in its docs (WebFetch `https://docs.claude.com/en/docs/claude-code/plugins-reference` and `/hooks`) and follow it exactly; report any schema difference as a deviation.
- `plugin/scripts/hook.sh`: POSIX sh. If `~/.nudge/bin/nudge-hook(.exe)` exists, `exec` it with stdin. Otherwise consume stdin, and if the event is SessionStart, start `install.sh` **detached in the background** (no waiting) and exit 0. Also on SessionStart, if the app binary exists, launch it detached (single-instance dedups). Never write stdout.
- `plugin/scripts/install.sh`: detect OS/arch (`windows-x86_64`, `macos-aarch64`, `macos-x86_64`); download `nudge-<target>.zip` and `SHA256SUMS` from `https://github.com/${NUDGE_REPO:-nudge-app/nudge}/releases/latest/download/`; verify sha256 (sha256sum or `shasum -a 256` or `certutil` on Windows); extract to `~/.nudge/bin/`; run `nudge-status setup`; launch the app detached. Log to `~/.nudge/install.log`. A lock file prevents two concurrent installs. On any failure: log and exit 0.
- `plugin/commands/setup.md` and `plugin/commands/uninstall.md`: slash commands `/nudge:setup` and `/nudge:uninstall` that instruct Claude to run `~/.nudge/bin/nudge-status setup|uninstall` (uninstall also tells the user to run `/plugin uninstall nudge`).
- `plugin/README.md`: install in two lines (`/plugin marketplace add …` + `/plugin install nudge`), what it touches on disk, how to uninstall.

## 6. TESTS + VERIFY
Required tests (Rust, in each crate):
- hook: whitelist strips `tool_input`/`tool_response` and keeps the 7 keys; `message` truncated to 500; 1 MB `tool_response` input → envelope < 64 KB; invalid JSON → no panic; app down → returns within 300 ms (set `NUDGE_HOME` to an empty temp dir); stdout is empty (run the built binary via `std::process::Command` with `env!("CARGO_BIN_EXE_nudge-hook")` and assert exit 0 + empty stdout).
- status: no-wrap formatting (both, only 5h, none); wrap passes stdout byte-identical including trailing newline and ANSI escapes (wrap an `echo`/`printf` command); missing backup → no-wrap mode; setup on missing file, on file with other keys (order preserved), on existing custom statusLine (backup written, padding copied), on already-set-up file (no change), on invalid JSON (exit 1, file untouched); uninstall restores original exactly; uninstall with null original removes key. Use `CLAUDE_CONFIG_DIR` + `NUDGE_HOME` temp dirs; tests must not touch the real `~/.claude`.
- plugin: `bash -n` syntax check of both scripts; a JSON parse of `hooks.json` and `plugin.json`.

VERIFY (all must pass before reporting):
```
cargo test -p nudge-hook -p nudge-status
cargo clippy -p nudge-hook -p nudge-status -- -D warnings
bash -n plugin/scripts/hook.sh && bash -n plugin/scripts/install.sh
```

## 7. FINAL MESSAGE (raw data, no pleasantries)
1. Files created/changed (full list).
2. CLI surface of each binary (exact usage strings).
3. Test names + pass counts (paste the `test result:` lines).
4. Measured `ancestors(8)` time and total hook run time with app down.
5. Hook schema as verified from the docs (URL + what you followed).
6. Cross-lane errors seen.
7. **Deviations from this brief, each with why.**
