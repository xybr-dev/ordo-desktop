---
id: 288
title: Add Mass and Reuse for another Celebration slide-overs
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/masses-index.html]
---

# Add Mass and Reuse for another Celebration slide-overs

## Context

Decision [84](0084-optional-two-stage-celebration-and-first-plan-creation.md) says creating a plan
from a template collects schedule, language and choir and shows the recommended template, and that
**+ Add another Mass** offers the same starting methods. Decision
[85](0085-celebration-date-scheduled-start-and-anticipated-masses.md) makes Anticipated an explicit
confirmation. Decision [89](0089-lightweight-presider-location-and-notes-input.md) makes Presider and
Location editable comboboxes that start from the parish defaults. Decisions
[36](0036-reusing-a-past-mass-plan.md), [74](0074-celebration-and-scheduled-mass-plan-relationship.md)
and [81](0081-language-aware-duplicate-for-another-mass.md) define Reuse and Duplicate. None of them
places these fields on one surface.

## Decision

- **Add Mass** on a Celebration, and **Next: add the Mass** at the end of New Celebration, open one
  slide-over for that Celebration. New Celebration itself keeps only the Celebration's details and the
  choice of how the first plan starts.
- The slide-over's **Start from** offers **From a template**, **Reuse an earlier plan**, and, once the
  Celebration has a plan, **Duplicate a Mass here**. For Reuse and Duplicate the source plan is chosen
  first.
- Then, in order: **Date** (the Celebration's day, or the evening before with an **Anticipated**
  checkbox to confirm), **Time** in 24-hour form, **Language**, **Choir**, **Presider** and
  **Location**. Presider and Location start as the parish defaults; a Reused or Duplicated plan keeps
  its source's Presider. A name that isn't a regular presider is marked as visiting and stays on that
  Mass only.
- From a template, the template list follows the language and preselects the recommendation, with
  **No template: start empty** as the last choice. A language with no template says so and offers the
  empty plan.
- For Reuse and Duplicate, a summary lists what is kept and what is left to finish. A Duplicate in
  another language follows Decision 81.
- **Reuse for another Celebration**, from a Celebration's menu or **Reuse** on a History row, opens its
  own slide-over:
  - the source's plans with checkboxes, each with a new time;
  - **Into** an upcoming Celebration or a new one, with date, title and season;
  - a summary of what is **Copied**, what **Needs review** (readings always, and seasonal items when
    the season differs), and what is **Not copied**.
  - Its button names the count, for example **Reuse 2 plans**. The copies land under the target with
    readings marked Needs review, and Undo is offered.

## Alternatives considered

- **A dialog instead of a slide-over.** The fields and the summary don't fit a dialog at 1366×768, and
  the Celebration's group should stay visible behind it.
- **Asking for schedule, language and choir in New Celebration.** That puts plan fields on a
  Celebration form and doesn't help with a second Mass, so both paths share the Add Mass slide-over.

## Consequences

A Celebration's plans can all be made from one surface. Reuse can fill several Masses of a new
Celebration in one step.
