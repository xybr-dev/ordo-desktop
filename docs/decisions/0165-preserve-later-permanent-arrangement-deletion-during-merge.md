---
id: 165
title: Preserve later permanent Arrangement deletion during merge reversal
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

# Preserve later permanent Arrangement deletion during merge reversal

- Do not treat the merge-history event alone as a durable content reference that indefinitely blocks permanent deletion of an Arrangement transferred by that merge. The Arrangement must still pass every ordinary child, Mass-plan, Presentation-snapshot, History, and other reference-safety rule before deletion.
- When reversing the Song merge later, preserve a transferred Arrangement's deliberate subsequent permanent deletion rather than resurrecting erased content or failing the entire reversal merely because that Arrangement no longer exists.
- Restore the retired Song and reverse all otherwise applicable merge effects atomically, but leave the permanently deleted Arrangement absent. Retain sufficient tombstone provenance in the merge event to distinguish this known later deletion from corruption or an unexplained missing dependency.
- Disclose the exception prominently in the reversal impact review, including the number and identity evidence of transferred Arrangements that were permanently deleted and cannot be restored.
- Treat this as preservation of a later non-undoable action, not as an unnoticed partial reversal. An unexplained missing Arrangement remains an integrity failure and must block reversal.
