# Spike results (build step #1) — 2026-10-04, Claude Code 2.1.288

Method: string search of the shipped `claude.exe` binary. Treat field names as
verified; behavior around them as inferred until exercised end-to-end.

| Unknown | Finding | Status | Decision |
|---|---|---|---|
| Statusline payload has usage | `rate_limits.{five_hour,seven_day,spend_limit}.{used_percentage,resets_at}`; `rate_limits_available`; `subscription_type`; `refreshInterval` setting | verified (schema strings) | D3 default path stands |
| Notification hook distinguishes blocked | Payload carries `notification_type`, enum includes `permission_prompt`, `idle_prompt`, `auth_success`, `elicitation_dialog`, `agent_needs_input`, `agent_completed`, `elicitation_url_dialog`, `worker_permission_prompt`, `push_notification` | verified (enum strings) | **D13**: Blocked = `permission_prompt`, `elicitation_dialog`, `elicitation_url_dialog`, `agent_needs_input`, `worker_permission_prompt`. `idle_prompt` / others = no state change (escalation already covers idle). |
| Plugin can set `statusLine` | Not determinable from strings | unresolved | **D14**: assume no; `/nudge:setup` edits `~/.claude/settings.json` with a backup (spec §Rollback). |
| Usage endpoint for D3 opt-in | `GET .../api/oauth/usage` present in binary | verified (path only) | Base URL, auth header and token location are inferred (`https://api.anthropic.com`, `Authorization: Bearer`, `anthropic-beta: oauth-2025-04-20`; token in `~/.claude/.credentials.json` → `claudeAiOauth.accessToken` on Windows/Linux, Keychain item `Claude Code-credentials` on macOS). Lane L4 must verify against a live account before enabling. |
