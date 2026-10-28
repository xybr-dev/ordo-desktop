---
id: 281
title: "Help holds keyboard shortcuts, offline guides and About Ordo"
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/help.html]
---

# Help holds keyboard shortcuts, offline guides and About Ordo

## Context

Decision [250](0250-use-a-hybrid-global-rail-and-local-tab-navigation-hierarchy.md) anchors **Help**
at the bottom of the rail, and Decision
[232](0232-discoverable-contextual-keyboard-shortcut-reference.md) puts a labelled **Keyboard
shortcuts** entry under it. Nothing says what else Help contains. The church computer is offline
(Decision [17](0017-local-storage-and-portable-transfer.md)), so help can't depend on a website.

## Decision

- Help uses the same page list as Settings. Its entries are **Keyboard shortcuts**, a few short
  **guides**, and **About Ordo**.
- **Keyboard shortcuts** shows the Shortcut Reference Module's list (Decision 232) on the page, with
  **Open as a panel** for the floating panel that `Ctrl+/` toggles everywhere.
- Guides ship inside Ordo and work offline. Each is a handful of numbered steps for one task the MVP
  plan names: preparing a Sunday Mass, carrying a Celebration to the church computer, backing up the
  parish, and calibrating the projector.
- **About Ordo** gives the version and build, the parish name, a statement that data stays on this
  computer and nothing is sent, the bundled font licences, and the open-source licence texts. It
  links to Diagnostics.

## Alternatives considered

- **Only the shortcut reference.** Leaves a rail destination with one entry and nothing for an
  operator who forgets how to move a Celebration between computers.
- **Links to an online manual.** Breaks on the air-gapped church computer.

## Consequences

Guides become part of what a release must keep accurate. Their wording follows `DESIGN.md` and
`CONTEXT.md` terms so they never teach a different vocabulary from the screens.
