---
id: 313
title: Hide the pointer over the congregation window
status: accepted
date: 2026-11-07
recorded: 2026-11-07
tags: [display, gpui, presenter]
supersedes: []
superseded_by: null
amends: [308]
amended_by: []
sources: [vendor/README.md, vendor/gpui-pre-windows/src/events.rs, vendor/gpui-pre-windows/src/window.rs, crates/platform/src/congregation_window.rs, xtask/src/lib.rs]
---

# Hide the pointer over the congregation window

## Context

Decision [56](0056-passive-congregation-window-and-cursor-recovery.md) hides the system pointer only
while it is over the congregation window, and never system-wide or trapped. T1.6 shipped only the
cursor locator. GPUI's `CursorStyle` has no hidden variant, so the pointer stayed visible on the
projector (issue #130).

`gpui-pre-windows` already chooses each window's cursor in its `WM_SETCURSOR` handler, which Windows
sends only to the window under the pointer. The crate is vendored for Decision
[308](0308-vendor-gpui-pre-windows-to-report-display-changes.md).

## Decision

- The vendored backend gains a second change, marked `ORDO PATCH`:
  - a window opened with `focus: false` records `hides_pointer`;
  - its `WM_SETCURSOR` handling sets no cursor.
- The congregation window is the only window Ordo opens without focus
  (`crates/platform/src/congregation_window.rs`), so it is the only window where the pointer is
  hidden. The pointer shows again as soon as it leaves, and the cursor locator still guides it back.
- `cargo xtask verify` checks each marked patch separately, so a GPUI upgrade that drops either one
  fails.
- No `unsafe` is added to Ordo's crates. `SetCursor` is already called inside the vendored crate.

## Alternatives considered

- **Confine the pointer to the operator display (`ClipCursor`).** It traps the pointer, which
  Decision 56 rules out, and it reaches beyond what an app should do to the desktop.
- **Hide it from Ordo's crates with `ShowCursor`/`SetCursor`.** That would need a third `unsafe`
  module (Decision [312](0312-allow-a-second-audited-unsafe-module-for-monitor-identity.md)), and it
  would race GPUI's own `WM_SETCURSOR` handling.
- **Keep only the locator.** The pointer stays visible to the congregation.

## Consequences

- A GPUI upgrade has to reapply two marked changes instead of one.
- The GPUI test platform has no OS cursor, so the hiding is checked by hand: issue #130 and T1.6's
  `[MANUAL]` row.
- Any future window opened with `focus: false` also hides the pointer. Opening one means checking
  this decision.
