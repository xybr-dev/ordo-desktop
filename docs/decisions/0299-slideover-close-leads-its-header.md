---
id: 299
title: "A slideover's close button leads its header"
status: accepted
date: 2026-11-03
recorded: 2026-11-03
tags: [design, interaction, accessibility]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [DESIGN.md, docs/mockups/ordo-kit.js, docs/mockups/ordo.css]
---

# A slideover's close button leads its header

## Context

The mockups closed every slideover with an edge tab: a 28×56px chevron handle fixed halfway down the
panel's inner edge. Nothing else in Ordo is placed there. The tab covered content beside the panel, sat
far from the toggle that opened the panel, and looked like a resize handle rather than a dismiss
control. No decision or `DESIGN.md` rule had set that placement.

## Decision

- Every slideover, whether it is a constrained side pane (Decision 236) or an overlay slideover, closes
  from a quiet icon button with a ✕ at the start of its own header, before its title. This follows
  Notion's side peek.
- The header is the panel's top bar. No separate bar is added. The panel's own actions stay at the
  header's trailing end, and actions for the pane beneath do not move into it.
- A side pane that has no header when fixed gets a minimal one while it is a slideover: the close
  button and the pane's name.
- The button is named "Close" plus the panel's name, or by the action it cancels. Its tooltip names
  Escape. It is the first focus stop, and focus lands on it when the panel opens. Escape, the scrim and
  the toggle that opened the panel still close it.
- When a side pane is fixed, its header has no close button. Only that leading button appears and
  disappears between the two states.

## Alternatives considered

- **Close at the header's trailing end, the usual Windows placement.** It competes with the panel's
  actions for the trailing end, and on the list slideover it sits far from the toggle that opened it.
- **Only the toggle, Escape and the scrim close it.** A keyboard user inside the modal then has no
  visible close control.
- **Keep the edge tab but pin it level with the header.** It still covers content and still reads as
  a handle.

## Consequences

- Operators who expect close at the top right may look there first. A ✕ that leads a clearly framed
  header is still easy to find.
- Opening and closing the list slideover happen at the same spot, under the toggle.
- The GPUI slideover primitive owns the header slot and the close button, so a workspace cannot place
  its close control anywhere else.
