---
id: 276
title: Edit Song identity in a staged Song details slide-over
status: accepted
date: 2026-10-28
recorded: 2026-10-28
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/song-library-master-detail-v2.html]
---

# Edit Song identity in a staged Song details slide-over

## Context

Decision [114](0114-arrangement-free-song-creation-and-nested-duplicate-preview.md) covers creating a
Song and Decision [143](0143-song-and-arrangement-revision-comparison-and-restoration.md) records and
restores Song identity edits, but no decision says where a Song's title, composer, aliases or usual
moments are edited. The regenerated Song Library mockup showed them read-only, so a Song could not be
edited at all.

## Decision

- The selected Song's detail header has **Edit details** beside History. It opens **Song details**, the
  same slide-over and fields as **New song**: primary title (required), composer or original author,
  aliases and usual moments.
- Changes are staged. **Save details** commits them as one Song revision and is disabled while the
  title is empty or nothing has changed; **Cancel** discards them. Saving shows a toast with Undo.
- When the Song has active Arrangements, a notice before saving says how many show its title and
  credits, that Mass plans using the Song show the change, and that past Presentation snapshots don't
  change.
- A changed title runs the same possible-duplicate check as New song, excluding the Song itself. The
  preview is read-only and says merging comes after the MVP (Decision 167).
- An archived Song has no Edit details; it is restored first (Decision 148).
- Arrangement-owned fields stay in Arrangement details (Decision 117).

## Alternatives considered

- **Edit each field inline in the detail pane.** Faster for a single field, but a title change has
  effects (Arrangements, Mass plans, duplicates) that are better shown before the change is saved.
- **A separate Song editor page.** Too heavy for four fields.

## Consequences

The Song detail header gains one button. Song History's "Edited details" sessions now have a visible
source. Shown in `song-library-master-detail-v2`, section I.
