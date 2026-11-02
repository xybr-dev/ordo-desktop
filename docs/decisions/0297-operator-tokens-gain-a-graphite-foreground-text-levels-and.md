---
id: 297
title: "Operator tokens gain a graphite foreground, text levels and a compact row"
status: accepted
date: 2026-11-02
recorded: 2026-11-02
tags: [design]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Operator tokens gain a graphite foreground, text levels and a compact row

## Context

An audit of `docs/mockups` found tokens that failed where used. `primary` measured 2.04:1 as a foreground
on graphite. Preparation muted text was 4.21 to 4.24:1 and warning 4.32 to 4.35:1 on tinted fills, and
`preparation-border-strong` was 2.93:1 on `surface-muted`. The kit also used text sizes and a row height
with no token: half of its operator text was `13px`, which neither `DESIGN.md` nor `ui::tokens` declared.

## Decision

- Add `primary-foreground-presenter` `#8FBFA3`, 5.98:1 or better on every graphite surface.
- `preparation-text-muted` becomes `#6B655C`, `preparation-border-strong` `#8A8072`, and
  `freeze-preparation` and `warning-preparation` `#945710`.
- Add `operator-heading` 18/600, `operator-meta` 13/400 and `operator-caption` 12/600. Other in-between
  sizes round to the scale.
- Add `compact-row` `32px` for the Mass outline and Presenter Ordo rail. Other rows stay `36px`.

## Alternatives considered

- **Use full text colour in selected rows.** It fixes one surface and leaves `surface-muted` failing.
- **Raise every row to `36px`.** A whole Mass outline stops fitting at `1366x768`.

## Consequences

`13px` still needs checking in a dim church before the level is final. `ui::tokens` pins the new tokens to
the frontmatter, and the matching `DESIGN.md` changes land with this record.
