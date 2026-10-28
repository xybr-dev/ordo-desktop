---
id: 279
title: Library views beside Songs share its master-detail
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/library-responses-prayers.html, docs/mockups/library-general-presentations.html, docs/mockups/library-choirs.html]
---

# Library views beside Songs share its master-detail

## Context

Decision [257](0257-make-library-a-multi-view-destination-with-revision-bound.md) gives the Library
views for Responses & Prayers and General Presentations and binds Mass plans to their revisions, and
proposed Decision [278](0278-manage-choirs-as-a-library-view.md) adds Choirs. Decisions
[87](0087-simplified-celebration-metadata-and-choir-management.md) and
[221](0221-optional-choir-setup-with-inline-lightweight-creation.md) say what a Choir record holds. No
decision says how these views are laid out or edited.

## Decision

- Each view uses the Songs master-detail layout (Decision 132): a list with search, filters and
  **Show archived** on the left, and the selected entry on the right.
- **Responses & Prayers** filters by language and liturgical role. An entry has a liturgical role, one
  language, a response header (Mass plan default, Hide or Override; Decision 84), and ordered
  exchanges of an optional operator-only prompt and a required projected response. Its generated
  response Slides show underneath.
- **General Presentations** entries have an optional purpose, a default auto-play time (eight seconds;
  Decision 258), and ordered Text or Image Slides. Each Slide can override the time. Private delivery
  context stays on the Mass item (Decision 257).
- Entry content saves as it's typed, with Undo for structural changes, as in the Arrangement editor.
  Each change is a new revision. Plans keep the revision they use and show **Update available** until
  someone reviews it (Decision 257). The entry's detail pane says how many plans and templates link it.
- An entry's name is edited in place in its detail header (as in proposed Decision 275). Enter or
  leaving the field saves it, Esc cancels, and an empty name isn't saved.
- **New** creates an entry in the language and role being filtered, with one empty exchange or text
  Slide. The entry is marked Incomplete until a response or some Slide text is written.
- **Choirs:** **Add choir** asks only for a name, compared ignoring case and spacing. A clash offers the
  existing choir, archived or not. The detail holds the short label, preferred languages (from Ordo's
  list, proposed Decision 277), private notes, and the repertoire: Songs whose active Arrangements
  name the choir, each with **Open**. Renaming a choir renames it on every Arrangement.
- Archived entries and choirs are inspect-only with **Restore**. Responses and Presentations offer
  **Duplicate**; Choirs don't.

## Alternatives considered

- **A staged details slide-over per entry, like Song details.** These entries are content, not
  identity, so they edit like an Arrangement's lyrics rather than like a Song's title.
- **One combined list of all reusable content.** Decision 257 keeps the specialized views.

## Consequences

The three views reuse the Songs layout and its components. Response roles, History for these entries,
and image storage are still open; their pages list them.
