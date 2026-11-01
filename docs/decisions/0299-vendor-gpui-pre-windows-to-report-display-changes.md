---
id: 299
title: Vendor gpui-pre-windows to report display changes
status: accepted
date: 2026-11-01
recorded: 2026-11-01
tags: [display, qualification, gpui]
supersedes: [298]
superseded_by: null
amends: []
amended_by: []
sources: [vendor/README.md, vendor/gpui-pre-windows/src/events.rs, crates/ordo/src/output.rs, xtask/src/lib.rs]
---

# Vendor gpui-pre-windows to report display changes

## Context

Decision [298](0298-watch-the-display-topology-while-output-is-enabled.md) made enabled output compare the display topology
every 500 ms. That cost runs on the GPUI thread and no performance gate measures it, because
`presenter-lab` never enables output. The owner asked for an event-driven trigger.

Windows sends `WM_DISPLAYCHANGE` to each top-level window. `gpui-pre-windows` 0.3.8 receives it but
only updates its own display state, and exposes no callback. A separate hidden listener window
would widen Decision 269's `unsafe` exception and add a thread that could die silently, on a
safety path.

## Decision

Vendor `gpui-pre-windows` 0.3.8 in `vendor/` through `[patch.crates-io]`, changed in one place:
`handle_display_change_msg` calls the window's `moved` callback, including when the window is left
on no monitor. GPUI's bounds observers then run, so `crates/ordo/src/output.rs` refreshes through
its existing observer. The 500 ms watch is removed and the module is not polled again.

`cargo xtask verify` fails if the lockfile stops resolving the crate to `vendor/` or the marked
change disappears, because Cargo only warns about an unused `[patch]`.

## Alternatives considered

- **Keep the 500 ms watch.** Simple, but it does unmeasured work on the GPUI thread.
- **Hidden listener window.** More `unsafe`, a new thread, and a new silent failure.
- **Fork GPUI.** It means owning a 22-crate snapshot to change one function, and it reverses T0.4a's
  move to exact crates.io releases. Revisit if patches reach about three or touch `gpui-pre` itself.

## Consequences

Each GPUI upgrade must reapply the change and retest display loss on real hardware, which CI cannot
do. The change should be offered to Zed so the vendored crate can go away. Non-Windows platforms
get no display-change event from this; they are outside Milestone 0 sign-off.
