---
id: 157
title: Targeted Song-merge reversal
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Targeted Song-merge reversal

- Record enough field-level and structural provenance in the merge-history event to reverse the merge as a targeted inverse transaction rather than restoring the entire library to an earlier snapshot.
- Let the immediate success toast's **Undo merge** run the inverse directly while no intervening edit has affected the merge. Expose **Reverse merge** from the dedicated History event for later recovery through an impact review.
- Restore the retired Song under its original UUID, remove its redirect, and move only the Arrangements recorded as transferred by that merge back beneath it.
- Preserve those Arrangements' current content, Edit revisions, Choir associations, and lifecycle state when moving them back; do not rewind Arrangement edits or archival changes made after the merge.
- Reverse only Song metadata changes attributable to the merge. Leave unrelated later edits and Arrangements created under the survivor after the merge untouched.
- If later edits overlap metadata that the inverse must change, require explicit conflict review before reversal and never overwrite those later values silently.
- Execute reversal atomically. If its dependencies cannot be reconciled, make no changes and explain the blocking impact rather than leaving a partially restored identity graph.
