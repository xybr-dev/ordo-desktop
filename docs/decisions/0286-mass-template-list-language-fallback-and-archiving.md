---
id: 286
title: Mass template list, language fallback and archiving
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

# Mass template list, language fallback and archiving

## Context

Decision [77](0077-template-library-ownership-and-organization.md) puts templates under
**Masses → Templates**, with creation, duplication, language filtering, applicability, modification
and usage context. Decision [79](0079-template-recommendation-and-fallback-selection.md) allows one
fallback template per language. Decision [220](0220-empty-mass-templates-within-the-permanent-list-editor.md)
covers the empty list and what creation asks for. None of them says what a row shows, how the
fallback is chosen, or how an unwanted template leaves the list.

## Decision

- Each row shows the template's name, a **Fallback for {language}** chip when it is one, its language,
  slot count, the number of Mass plans made from it, when it was last edited, its purpose, and the
  days and seasons it's recommended for. **Edit** opens the Template Editor; a `…` menu holds
  **Duplicate**, **Use as {language} fallback** and **Archive**.
- A language filter sits above the list. Template names are unique, compared ignoring case and spacing.
- Making a template the fallback takes the role from whichever template held it, and the toast names
  that template. The first template created in a language becomes its fallback.
- **Archive** hides a template from New Celebration's choices. Plans made from it keep their copy,
  because the copy is independent (Decision [78](0078-template-instantiation-and-revision-boundary.md)).
  Archiving the fallback leaves that language without one until another is chosen. **Show archived**
  lists archived templates with **Restore**. Permanent deletion is left out of the MVP.
- **Duplicate** asks for a name and a language. A copy in another language keeps the structure, and
  its responses are translated afterwards.
- A template's language is fixed after creation.

## Alternatives considered

- **Delete instead of Archive.** Deleting would lose a template's provenance in plans that name it,
  with nothing gained, since plans don't depend on it.
- **Fallback set in Settings.** Templates are preparation content (Decision 77), so their roles stay
  with them.

## Consequences

New Celebration ranks only active templates. A language with no fallback and no matching template
gets the empty plan with its readiness warning (Decision 79).
