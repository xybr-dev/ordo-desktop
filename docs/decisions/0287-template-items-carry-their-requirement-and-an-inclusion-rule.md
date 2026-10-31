---
id: 287
title: Template items carry their requirement and an inclusion rule
status: accepted
date: 2026-10-28
recorded: 2026-10-28
tags: [mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/mass-templates.html]
---

# Template items carry their requirement and an inclusion rule

## Context

Decision [76](0076-typed-template-slots-and-multiple-templates-per-language.md) gives templates Song
and Reading slots that are required, optional or conditional. Decision
[75](0075-conditional-template-items.md) evaluates conditional items from Celebration metadata such as
season and tags, and leaves an item unresolved when the metadata is missing. Neither says how a
template author writes such a rule or sees its effect.

## Decision

- The Template Editor's centre pane edits the selected item.
  - Song, Reading and General Presentation items have **Required**, **Optional** or **Conditional**.
  - Response sequences and Holds have **Always** or **Conditional**.
- A **Song slot** names its usual moment, which filters the Song chooser in each plan. A
  **Reading slot** names its role (First Reading, Responsorial Psalm, Second Reading, Gospel
  Acclamation, Gospel). Neither holds content; the pane says each plan fills it.
- A conditional item has one rule, **Include only on** or **Omit on**, over a set of days (Sunday,
  Solemnity, Feast, Weekday) and liturgical seasons. Under the rule, the result for a few sample
  Celebrations shows with its reason, including **Unresolved** for a Celebration missing the season
  the rule needs.
- The outline marks a conditional item with a split icon, and an optional one with **Optional**. The
  template's details pane has **Check against a Celebration**: choosing a sample Celebration strikes
  through the items its rules would omit. Nothing is saved by checking.
- Template edits autosave. Adding, removing and moving items or sections offers Undo. A banner says
  plans already made from the template don't change (Decision 78).

## Alternatives considered

- **Free-form rule expressions.** More powerful, but a parish author can't check them by reading, and
  Decision 75 keeps rules advisory.
- **Tags in rules.** Decision 75 names explicit Celebration tags, but no decision defines a tag
  vocabulary yet; days and seasons cover the common cases until it does.

## Consequences

Rules use only Celebration date metadata that already exists. Tag conditions can be added to the same
rule later without changing its shape.
