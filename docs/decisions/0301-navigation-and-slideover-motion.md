---
id: 301
title: "Navigation and slideover motion"
status: accepted
date: 2026-11-03
recorded: 2026-11-03
tags: [design, motion, navigation]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [DESIGN.md, docs/mockups/ordo-kit.js, docs/mockups/ordo.css]
---

# Navigation and slideover motion

## Context

`DESIGN.md` set the motion durations: `120ms` and `160ms` for controls, `400ms` in and `200ms` out
for dialogs and slideovers (Decision [256](0256-refine-operator-appearance-overlays-and-bounded-output.md)).
It did not say how navigation moves. The mockups showed none of it: the current marker jumped, the
rail entry's label and icon (Decision [300](0300-current-navigation-destination-fills-its-icon.md))
switched instantly, and slideovers appeared and vanished without the specified motion.

## Decision

- **Rail.** One pill per rail travels to the new current destination in `160ms`. On the entry being
  left, the label fades back in and the icon slides up to make room. On the entry arriving, the label
  fades out and the icon slides to the middle. The label never leaves the layout, so it remains the
  entry's accessible name and no entry changes height.
- **Tabs.** One underline per tab strip slides to the new tab in `160ms`, changing width to match it.
- **Changing tab.** The new view slides about `24px` in from the side the operator moved towards, and
  fades in, over `160ms`. The old view disappears at once.
- **Changing rail destination.** The new workspace rises about `12px` and fades in over `160ms`.
- **What stays still.** The rail, the workspace header and the tab strip never move as part of a page
  change. Only the content beneath them does.
- **Slideovers and side panes.** Overlay slideovers and side-pane slideovers slide in from their own
  edge in `400ms` with fast-start/slow-finish motion, and leave towards it in `200ms` with smooth
  acceleration. An overlay's dim scrim fades with it.
- **Full-workspace views.** A view that replaces the whole workspace for a while, namely Review Song
  import (from **Import .ordo**) and a Theme opened for editing, rises from the bottom over the
  workspace it covers in `400ms`, and sinks back over it in `200ms` when cancelled or finished. The
  rail stays visible and still. The covered workspace is shown underneath while the view moves, so it
  reads as a sheet over the page rather than a different page.
- **Input is never delayed.** A destination or tab change takes effect, and accepts the next input,
  immediately; the motion only follows it. The mockups wait one `160ms` beat before loading the next
  page, because each destination is a separate HTML page. That delay is part of the mockup, not the
  product.
- **Presenter is excluded.** Presenter views and the congregation output never use page motion, so
  nothing on the operator surface can look like a slide change.
- **Reduced motion.** There is no translation. The pills jump to their new position, and a page,
  label or slideover change is at most a `100ms` opacity fade, as `DESIGN.md` already requires.

## Alternatives considered

- **No page motion.** Changing tab then looks the same as content reloading, and nothing shows which
  way the operator moved.
- **Animating the old view out as well.** It doubles the wait and adds nothing the incoming view does
  not already show.
- **Sliding whole pages horizontally for a destination change.** That would look like a tab change.
  The rail is vertical, so a short rise keeps the two kinds of navigation distinct.

## Consequences

- GPUI needs an animated position for the rail and tab markers, and an entrance animation on the
  workspace content area that does not cover the header or the tab strip.
- Closing a slideover has to keep its last frame on screen for `200ms` after its state has closed,
  without that frame taking focus or the pointer.
