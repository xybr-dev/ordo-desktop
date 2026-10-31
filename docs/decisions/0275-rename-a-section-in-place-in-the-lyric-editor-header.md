---
id: 275
title: Rename a section in place in the lyric editor header
status: accepted
date: 2026-10-28
recorded: 2026-10-28
tags: [songs]
supersedes: []
superseded_by: null
amends: [119]
amended_by: []
sources: [docs/mockups/arrangement-details-v2.html]
---

# Rename a section in place in the lyric editor header

## Context

Decision [119](0119-arrangement-section-creation-and-rename-surface.md) put **Rename section**
behind the selected section's compact action button, reusing the Add section popover. Reviewing
the regenerated Songs mockups, the owner found that a detour: the section name is already shown as
the heading above its lyrics, which is where people look when they notice a wrong name.

## Decision

- The selected section's name in the Lyrics task's lyric-editor header is an editable field. It looks
  like a heading, shows a pencil on hover, and takes focus on click.
- **Enter** or leaving the field renames the section; **Esc** restores the current name. A rename
  shows a toast with Undo.
- An empty name is not saved: the field returns to the current name and the toast says why.
- The section row no longer carries a rename action button; its move buttons stay. The Add section
  popover from Decision 119 is unchanged and is used only for adding.
- Renaming still keeps lyrics, slide boundaries, approval state and every Sequence occurrence, which
  show the new name at once (Decision 119).
- An archived Arrangement shows the name as plain text.

## Alternatives considered

- **Keep the rename popover (Decision 119).** It takes two clicks and opens a second surface for a
  one-field change.
- **Rename inline in the section list.** The list rows are narrow and already hold the slide count,
  use count and status, so an editable name there would crowd them.

## Consequences

This amends Decision 119's rename bullet; its creation popover stays authoritative.
Shown in `arrangement-details-v2`, section E.
