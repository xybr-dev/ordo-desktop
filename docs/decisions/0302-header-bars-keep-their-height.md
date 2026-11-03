---
id: 302
title: "Header bars keep their height across states"
status: accepted
date: 2026-11-03
recorded: 2026-11-03
tags: [design, layout, responsive]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [DESIGN.md, docs/mockups/ordo.css, docs/mockups/ordo-kit.js]
---

# Header bars keep their height across states

## Context

The workspace headers, editor headers and pane heads took their height from whatever the current
state put in them. Library's Songs tab has **Import .ordo** beside the title and the other tabs do
not, so changing tab moved everything below by about 3px. In a narrow workspace the tabs went to a
second row only when actions crowded them, so Songs was 114px tall and Choirs 54px. Masses,
Diagnostics, Themes, Transfer & Backup and the Liturgy import had the same kind of jump, from an
action, a status or a long name wrapping. Pane heads were 48px with no actions, 49px with icon
buttons and 53px with a full button.

## Decision

- **A header bar's height depends on the workspace width only**, never on the tab, the item open or
  which actions and statuses the current state shows.
- **Floors.** A workspace header row and an editor header are always tall enough for a full-height
  control, with or without one. A pane head is always one control plus its padding (`53px` at 100%).
- **Narrow workspace.** Under `1000px` logical width, a header's tabs always take their own second row,
  whatever is beside the title.
- **One line.** Editor and workspace header titles, breadcrumbs and secondary meta (such as Saved) take
  one line and end in an ellipsis, with the full text on hover and focus. Warnings and unsaved-change
  statuses never shrink. In a narrow workspace an editor header's secondary actions show their icon
  alone, keeping their label as the accessible name and tooltip, so the name keeps its room. The
  primary action and the back action keep their labels.
- **Not covered.** The Mass plan header keeps its two-line title and date, so its height follows the
  width there too. Overlays and confirmations are separate surfaces and size to their content.

## Alternatives considered

- **Fixed heights.** At larger text scales this cuts off text, which the floors avoid.
- **Letting titles wrap.** Each title or status that wraps changes the height, which is the jump this
  decision removes.

## Consequences

- GPUI header bars need a minimum height tied to the control height, and a width breakpoint, rather
  than one derived from their current children.
