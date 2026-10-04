# Lane L2 — Notch UI

## 1. Identity
You are lane **L2** of a 3-lane parallel build (Wave 1) of **Nudge**, a Tauri 2
desktop "notch" for Claude Code sessions. Repo root:
`C:\Users\Vin\Documents\claude-noti`. Lanes L1 (sidecars/plugin) and L3 (alerts)
are editing other files at the same time. You build the notch window's UI and
behavior: the black pill, usage wheels, session dots, the expanded list, and
dragging with edge snapping.

## 2. HARD RULES
- Do NOT spawn subagents or use the Agent tool.
- Do NOT run any git commands. Do NOT commit.
- Do NOT edit any file outside §4. READ-ONLY (orchestrator-owned): `ui/src/shared/**`,
  `ui/vite.config.ts`, `ui/tsconfig.json`, `ui/package.json`, `src-tauri/**`, other
  `ui/*.html`, `ui/src/{settings,glow,avatar}/**`.
- You may NOT add npm dependencies. Vanilla TypeScript + DOM + SVG + CSS only,
  plus `@tauri-apps/api` (already installed). If you believe a dependency is
  required, REPORT it instead.
- If you need a backend capability that doesn't exist (command, permission),
  REPORT it; don't work around it.
- Cross-lane typecheck errors in files you don't own: REPORT, don't fix.

## 3. READ FIRST (in order)
1. `SPEC.md` — D7, D8, §Notch UI, AC #3, #7, #8, #11.
2. `ui/src/shared/contracts.ts` — the ONLY API you may use to talk to the core:
   `onSnapshot`, `api.focusSession`, `api.ackSession`, `api.setConfig`, types
   `Snapshot`, `Session`, `UsageSnapshot`, `Config`, `Edge`.
3. `src-tauri/tauri.conf.json` — the `notch` window definition (220×44,
   transparent, undecorated, always-on-top, no focus) and
   `src-tauri/capabilities/default.json` — the window permissions you have.
4. `src-tauri/src/session.rs` and `src-tauri/src/usage.rs` — semantics behind the
   snapshot fields.

## 4. FILE OWNERSHIP (exclusive)
- `ui/notch.html`
- `ui/src/notch/**` (all TS/CSS for the notch, plus `*.test.ts` files beside them)

## 5. BUILD
Structure: keep pure logic in testable modules with no Tauri imports:
- `ui/src/notch/geometry.ts` — edge snapping and window placement math.
- `ui/src/notch/format.ts` — countdown/elapsed/percent formatting.
- `ui/src/notch/view.ts` — DOM rendering from a `Snapshot` (takes a root element; no Tauri imports).
- `ui/src/notch/main.ts` — Tauri glue only (window APIs, `onSnapshot`, events).

Behavior (decisions inlined):
- **Collapsed** (default): a black pill (`#0b0b0c`, fully rounded, subtle 1px
  `#ffffff14` border) filling the window. It contains two ring wheels (5h and weekly;
  weekly hidden when `config.notch.showWeekly` is false) and one dot per session.
  Dot colors: idle `#6b6b6b`, running = white with a spinning arc, done `#3ddc84`,
  blocked `#ff8a00`. A dot with `alertPending` pulses (CSS animation).
  Horizontal pill on top/bottom edges (220×44). Vertical on left/right edges (44×220,
  wheels and dots stacked).
- **Wheels:** an SVG ring per window, filled to `usedPercentage`. Color ramps
  green < 60% ≤ amber < 85% ≤ red. When `usage.available === false`, show "n/a"
  rings with tooltip "Usage limits aren't available for API-key sessions —
  enable live usage in Settings". When `available === null`, show empty grey rings.
- **Expanded** (hover ≥ 300 ms or click on the pill; collapse 400 ms after the pointer
  leaves): the window grows to 320 px wide (on side edges it grows inward from the
  edge) and lists sessions: dot, project name (ellipsized), state label, elapsed
  since `sinceMs` ("2m", "1h 4m"). Above the list sits a usage row with each wheel's
  percent and reset countdown ("resets in 1h 12m"). Empty state: "No live Claude Code sessions".
  Height = header + rows (max 8 visible, then scroll).
- **Click a row** → `api.focusSession(id)`. If it rejects, show an inline 2-second
  row message "Couldn't focus that window" (focus is implemented in a later lane;
  today it always rejects, so this path must work).
- **Drag (tricky part):** dragging must not trigger expand/click. Use pointer
  events with a 4 px movement threshold. While dragging, move the window with
  `getCurrentWindow().setPosition(new PhysicalPosition(...))` following the pointer
  (`screenX/screenY` × `scaleFactor`). Do NOT use `startDragging()`, because it
  hands control to the OS and you get no reliable drop point. On release:
  snap to the nearest edge of the monitor containing the window center
  (`availableMonitors()`). Compute `offset` (0..1 along that edge, clamped so the
  pill stays fully on-screen), switch orientation, animate into place (≤ 150 ms),
  and persist via `api.setConfig({...snapshot.config, notch: {...notch, edge, offset, monitor: monitor.name}})`.
- **Startup placement:** from `config.notch`. If `monitor` names a monitor that no
  longer exists, use the primary monitor (AC #7). Account for scale factor (use
  physical units throughout, and make sure the geometry tests cover scale 1.0 and 1.5).
- Re-render on every snapshot. Keep DOM updates cheap: keyed by session id, no
  full innerHTML rebuild per snapshot. Idle cost matters (AC #4): no
  `requestAnimationFrame` loops. Use CSS animations, and run a 1 s `setInterval`
  for elapsed/countdown text only while expanded.
- Transparent window: `html, body { background: transparent; margin: 0; overflow: hidden }`.
  No text selection, `cursor: default`, system UI font.
- Escape HTML: session/project names come from disk paths. Use `textContent`, never `innerHTML` with data.

## 6. TESTS + VERIFY
Vitest (`*.test.ts` next to the code, jsdom environment is configured):
- geometry: snap to each of the 4 edges; corner case picks the nearest edge; offset
  clamped at both ends; window size swaps for vertical edges; multi-monitor (window
  center on the second monitor → snaps there); scale factors 1.0 and 1.5; a missing
  monitor name falls back to the primary.
- format: elapsed (0s → "now", 59s, 61s, 2h 5m); countdown from `resetsAt` in the
  past → "resetting"; percent rounding.
- view: renders N dots for N sessions; states map to the right classes; the
  `alertPending` class; weekly hidden by config; n/a state; a project name with
  `<script>` renders as text; a re-render with one session changed touches only that row
  (assert node identity is preserved for the others); empty state text.

VERIFY:
```
npm --prefix ui run typecheck
npm --prefix ui test -- src/notch
```
(If typecheck fails only in files owned by other lanes, report it and stop.)

## 7. FINAL MESSAGE (raw data)
1. Files created/changed.
2. Exported functions of geometry.ts / format.ts / view.ts with signatures.
3. Test names + pass counts (paste vitest summary).
4. Anything you needed from the backend/capabilities but didn't have.
5. Cross-lane errors seen.
6. **Deviations from this brief, each with why.**
