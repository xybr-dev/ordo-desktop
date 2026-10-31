---
id: 289
title: Liturgy import review copies into a Celebration's Mass plans
status: accepted
date: 2026-10-28
recorded: 2026-10-28
tags: [import]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/liturgy-import.html]
---

# Liturgy import review copies into a Celebration's Mass plans

## Context

Decision [27](0027-optional-content-source-extensions.md) has helpers outside Ordo emit a
source-neutral Liturgy Import Document, which Ordo validates and previews before copying into a Mass
plan. Decision [193](0193-universal-ordo-exchange-container.md) carries it as the `liturgy-import`
kind of `.ordo` file. Decision [223](0223-strict-invalid-versus-needs-decision-import-states.md)
separates Invalid from Needs decision. Decision [188](0188-imported-content-is-ordinary-local-content.md)
makes imported content ordinary local content. None of them says where the review happens, which
plans receive the content, or what happens to text already in a plan.

## Decision

- **Entry points.** The review opens from a Celebration's `…` menu (**Import liturgy…**), from
  **Import** in Transfer & Backup when the file's kind is `liturgy-import`, and from a Reading item in
  the Mass Planner.
- **Workspace.** It is a full review workspace in the Masses destination, with three panes: the
  file's Celebration match and entries, the selected entry with its preview and its outcome in each
  plan, and **Copy into** with counts.
- **Celebration match.** The file is matched to a Celebration by date. **Change Celebration** can
  override it; choosing one on another date warns that the readings may not suit it.
- **Target plans.** These are the Celebration's plans in the file's language, all ticked by default.
  Plans in other languages are listed but not offered (Decision
  [80](0080-independent-language-specific-liturgical-content.md)).
- **Outcome per entry and plan:**
  - **New** fills an empty item.
  - **Replaces reused text** replaces text that Reuse copied and marked Needs review (Decision
    [36](0036-reusing-a-past-mass-plan.md)), and clears that mark.
  - **Unchanged** means the plan already has the same text.
  - **Needs decision** when the plan's text was changed by hand: both versions show side by side with
    differences marked, and the operator chooses **Keep this plan's** or **Use the file's**.
  - **Needs decision** when the plan has no item for the entry: **Add it to Liturgy of the Word** or
    **Skip for this Mass**.
- **Copy.** **Copy into N Mass plans** is unavailable until no decision is left. Nothing changes
  before Copy. Afterwards each plan shows its readiness and **Open in Mass Planner**. Undo is through
  each plan's History, which records the source file.
- **Failures.** An Invalid file shows its reason, **Choose another file** and **View technical
  details**, and imports nothing. A valid file in a newer format says which Ordo can read it. A file
  whose language has no plan in the Celebration offers to add a Mass first and keeps nothing.

## Alternatives considered

- **A slide-over in the Mass Planner.** One file feeds several plans, and the side-by-side comparison
  needs width; Decision 181 chose a workspace for package review for the same reasons.
- **Overwrite hand-edited text silently.** It loses parish edits. Keeping the plan's text silently
  hides newer readings.

## Consequences

Reuse and liturgy import fit together: a reused plan's readings come in Needs review and a liturgy
file replaces them without questions. The Liturgy Import Document's schema still has to be written;
the entry kinds in the mockup are a guess at its contents.
