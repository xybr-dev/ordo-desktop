---
id: 298
title: "High Contrast keeps Ordo's palette for the MVP; following system colours is deferred"
status: accepted
date: 2026-11-02
recorded: 2026-11-02
tags: [design, accessibility, platform]
supersedes: []
superseded_by: null
amends: [211]
amended_by: []
sources: [docs/qualification/GPUI-BASE-T0.4a.md, crates/ui/src/primitives.rs]
---

# High Contrast keeps Ordo's palette for the MVP; following system colours is deferred

## Context

Decision [211](0211-dedicated-two-layer-visible-keyboard-focus.md), `DESIGN.md` and `MVP-PLAN.md` said the system highlight treatment takes
precedence under Windows High Contrast. A GPU-composited GPUI window gets no system repaint, and
`gpui-pre` has no high-contrast or forced-colors API (`docs/qualification/GPUI-BASE-T0.4a.md`). So
`ui::primitives` paints Ordo's own focus treatment in every mode, and the checked manual gate asks for
that treatment to be preserved. The documents and the code disagreed.

## Decision

- For the MVP, Ordo keeps its own palette under Windows High Contrast and does not detect the mode.
- Focus, selection, and every Live, Freeze, Blackout, warning and Unavailable state must stay clear
  there. They already pair color with a word and a shape. T6.6 tests this.
- Following the system's High Contrast colors is deferred until after the MVP.

## Alternatives considered

- **Defer High Contrast entirely.** It removes a gate that protects operators and that focus already meets.
- **Build the bridge for the MVP.** It adds a palette and a platform seam that no MVP journey needs.

## Consequences

An operator using High Contrast does not get their chosen colors in Ordo for the MVP.

The deferred work, as understood today and not yet prototyped:

1. **Detect.** In `crates/platform`, read `SystemParametersInfoW(SPI_GETHIGHCONTRAST)` and `GetSysColor`
   through `windows-sys`, which is already a dependency and needs more feature flags. These are
   synchronous reads with no retained pointers or callbacks.
2. **Expose.** Return plain safe data, such as `Option<ContrastColors>`, and `None` off Windows.
3. **React.** Extend the vendored GPUI `WM_SETTINGCHANGE` handler to report the change, or read the
   setting at startup as a first version. Whether toggling the mode fires `ImmersiveColorSet` is untested.
4. **Apply.** Add a third palette in `ui::tokens`. Canonical Slides and congregation output do not change.

The `windows-sys` calls need `unsafe`, so this work needs a new decision widening Decision
[269](0269-allow-one-audited-unsafe-module-for-process-memory-record.md) and an `xtask` allowlist change.
The rejected-for-now alternative is WinRT `AccessibilitySettings` and `UISettings`, which likely avoids
`unsafe` in Ordo at the cost of a larger direct dependency.

The larger cost is design: Windows themes offer few colors, so Live, Freeze, warning, danger, selection
and focus would share some and lean on their shape and word cues, checked on all four contrast themes.
