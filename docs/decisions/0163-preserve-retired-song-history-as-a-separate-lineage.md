---
id: 163
title: Preserve retired Song history as a separate lineage
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Preserve retired Song history as a separate lineage

- Add one dedicated **Songs merged** event to the surviving Song's History rather than interleaving every pre-merge Edit revision from the retired Song into the survivor's timeline.
- Keep the retired Song's prior Edit revisions as a sealed lineage associated with its Merged Song redirect and the merge-history event. Let the operator inspect that lineage from the event without exposing the retired Song as an independently editable library entry.
- Keep each Arrangement's History attached to its unchanged Arrangement UUID while the Arrangement is rehomed beneath the survivor.
- When a merge is reversed, restore the retired Song under its original UUID with its original revision lineage intact, then append the reversal as later history rather than rewriting or deleting the merge record.
