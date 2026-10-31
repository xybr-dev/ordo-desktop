---
id: 277
title: Pick Arrangement languages from a built-in list
status: accepted
date: 2026-10-28
recorded: 2026-10-28
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/arrangement-details-v2.html]
---

# Pick Arrangement languages from a built-in list

## Context

Decision [117](0117-arrangement-metadata-boundary-and-staged-details-surface.md) requires at least one
language on every Arrangement, and languages are also used by Song Library filters (Decision
[133](0133-arrangement-level-song-library-filtering.md)), Mass plans and templates. No decision says
where the list of languages comes from or how a language not shown yet is added. Choirs already have
a rule (Decision [221](0221-optional-choir-setup-with-inline-lightweight-creation.md): find, or
create by name).

## Decision

- Ordo ships a fixed list of languages. Each entry has one display name and other names people search
  by, such as Bisaya and Visayan for Cebuano, or Ilonggo for Hiligaynon.
- Arrangement details shows the languages already used in the Library as chips. **+ Language** opens a
  search over the whole list; a match on another name says so ("also called Ilonggo"). Choosing one
  selects it and adds it to the chips.
- A language can't be typed in as a new value. When nothing matches, the picker says the language isn't
  in Ordo's list.
- The same list serves Mass-plan and template languages, so filters and template matching compare like
  with like.

## Alternatives considered

- **Free-form languages created inline, like Choirs.** Easy to start, but one language would end up
  under several spellings (Cebuano, Bisaya, Binisaya), which breaks filtering and template matching.
- **A parish-managed language list.** Adds a settings screen for something that rarely changes.

## Consequences

The list has to cover the languages parishes actually sing in; adding one means an Ordo release. Whether
a parish can add its own entry later is left open.
Shown in `arrangement-details-v2`, section H.
